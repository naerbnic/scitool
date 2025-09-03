use crate::{
    script_loader::errors::MalformedDataError,
    utils::{
        block::MemBlock,
        buffer::{Buffer, BufferExt},
        errors::{OtherError, ensure_other, prelude::*},
        mem_reader::{BufferMemReader, MemReader},
    },
};

use super::selectors::SelectorTable;
use bytes::BufMut;

mod object;

pub use object::Object;

fn modify_u16_le_in_slice(slice: &mut [u8], at: usize, body: impl FnOnce(u16) -> u16) {
    let slice: &mut [u8] = &mut slice[at..][..2];
    let slice: &mut [u8; 2] = slice.try_into().unwrap();
    let value = u16::from_le_bytes(*slice);
    let new_value = body(value);
    slice.copy_from_slice(&new_value.to_le_bytes());
}

fn apply_relocations<'a, M: MemReader<'a>>(
    buffer: &mut [u8],
    relocations: &mut M,
    offset: u16,
) -> Result<(), MalformedDataError> {
    let relocation_entries = relocations
        .read_length_delimited_records::<u16>("Relocation Table Contents")
        .map_err(MalformedDataError::map_from("Relocation Table Contents"))?;
    ensure_other!(
        relocations.is_empty(),
        "Relocation block size and length must match"
    );

    for reloc_entry in relocation_entries {
        modify_u16_le_in_slice(buffer, reloc_entry as usize, |v| v.wrapping_add(offset));
    }
    Ok(())
}

fn read_null_terminated_string(buffer: &[u8]) -> Result<&str, OtherError> {
    let null_pos = buffer
        .iter()
        .position(|&b| b == 0)
        .ok_or_else_other(|| "No null terminator found in string")?;
    std::str::from_utf8(&buffer[..null_pos]).with_other_err()
}

pub(crate) struct Heap {
    #[expect(dead_code)]
    locals: Vec<u16>,
    objects: Vec<Object>,
    #[expect(dead_code)]
    strings: Vec<String>,
}

impl Heap {
    pub(crate) fn from_block<'a, M: MemReader<'a>>(
        selector_table: &SelectorTable,
        loaded_script: &M,
        resource_data: &mut M,
    ) -> Result<Heap, FromBlockError> {
        let _ = resource_data
            .read_u16_le()
            .map_err(MalformedDataError::map_from("Relocation offset"))?;
        let num_locals = resource_data
            .read_value::<u16>("Num locals")
            .map_err(MalformedDataError::map_from("Num locals"))?;
        let locals = resource_data
            .read_to_subreader("Splitting locals.", (num_locals * 2).into())
            .map_err(MalformedDataError::map_from("Splitting locals."))?
            .split_values::<u16>("Local variable IDs")
            .map_err(MalformedDataError::new)?;

        let mut objects = Vec::new();
        // Find all objects
        loop {
            let obj_start = resource_data.tell();
            let magic = resource_data
                .read_value::<u16>("Object Magic Number")
                .map_err(MalformedDataError::map_from("Object Magic"))?;
            if magic == 0 {
                // Indicates that we've gotten to the last object on the heap.
                // Break out of the loop.
                resource_data.seek_to(obj_start).unwrap(); // Rewind so the 0 can be read as part of the string table.
                break;
            }

            ensure_other!(magic == 0x1234u16, "Invalid object magic number");
            let num_object_fields = resource_data
                .read_value::<u16>("Num Object Fields")
                .map_err(MalformedDataError::map_from("Num Object Fields"))?;

            resource_data.seek_to(obj_start).unwrap();

            let object_data = resource_data
                .read_values("Object fields", num_object_fields.into())
                .map_err(MalformedDataError::map_from("Object fields"))?;

            // The size is based from the very start of the object, so we reuse the curr_heap_data.
            let new_obj =
                Object::from_block(selector_table, loaded_script, object_data).with_other_err()?;
            objects.push(new_obj);
        }

        let mut strings = Vec::new();
        // Find all strings
        while !resource_data.is_empty() {
            let mut string_data = resource_data
                .read_until::<u8>("string_obj", |b| *b == 0)
                .map_err(MalformedDataError::new)?;
            string_data.pop(); // Remove the null terminator.
            let string = String::from_utf8(string_data).map_err(MalformedDataError::new)?;
            strings.push(string);
        }

        Ok(Self {
            locals,
            objects,
            strings,
        })
    }
}

struct Relocations {
    num_relocations: usize,
    #[expect(dead_code)]
    reloc_block: MemBlock,
}

impl Relocations {
    #[expect(dead_code)]
    pub(crate) fn num_relocations(&self) -> usize {
        self.num_relocations
    }
}

#[derive(thiserror::Error, Debug)]
pub(crate) enum FromBlockError {
    #[error(transparent)]
    MalformedData(#[from] MalformedDataError),
    #[doc(hidden)]
    #[error(transparent)]
    Other(#[from] OtherError),
}

fn extract_relocation_block(data: &MemBlock) -> Result<(u16, MemBlock), MalformedDataError> {
    let cloned_data = data.clone();
    let mut reader = BufferMemReader::new(&cloned_data);
    let relocation_offset = reader
        .read_value::<u16>("Relocation offset")
        .map_err(MalformedDataError::map_from("Relocation offset"))?;
    Ok((
        relocation_offset,
        cloned_data
            .sub_buffer(relocation_offset as usize..)
            .map_err(MalformedDataError::map_from("Relocation block"))?,
    ))
}

pub struct LoadedScript {
    heap: Heap,
}

impl LoadedScript {
    pub fn load(
        selector_table: &SelectorTable,
        script_data: &MemBlock,
        heap_data: &MemBlock,
    ) -> Result<LoadedScript, MalformedDataError> {
        let heap_offset = script_data.size();
        ensure_other!(
            heap_offset % 2 == 0,
            "Heap offset must be be 2-byte-aligned"
        );

        #[expect(clippy::cast_possible_truncation)]
        let u16_heap_offset = heap_offset as u16;

        // Concat the two blocks.
        //
        // It may be possible to get rid of the relocation block, but it's not clear.
        let mut loaded_script: Vec<u8> = script_data.as_slice().to_vec();
        loaded_script.put(heap_data.as_slice());

        let (script_data_slice, heap_data_slice) = loaded_script.split_at_mut(heap_offset);
        let (_, script_relocation_block) = extract_relocation_block(script_data)?;
        let (heap_relocation_offset, heap_relocation_block) = extract_relocation_block(heap_data)?;

        apply_relocations(
            script_data_slice,
            &mut BufferMemReader::new(&script_relocation_block),
            u16_heap_offset,
        )?;
        apply_relocations(
            heap_data_slice,
            &mut BufferMemReader::new(&heap_relocation_block),
            u16_heap_offset,
        )?;

        let loaded_script = MemBlock::from_vec(loaded_script);
        let heap_data = loaded_script
            .clone()
            .sub_buffer(heap_offset..heap_offset + heap_relocation_offset as usize)
            .expect("Constructed to have the correct offset.");
        let heap = Heap::from_block(
            selector_table,
            &BufferMemReader::new(&loaded_script),
            &mut BufferMemReader::new(&heap_data),
        )
        .with_other_err()?;

        Ok(LoadedScript { heap })
    }

    pub fn objects(&self) -> impl Iterator<Item = &Object> {
        self.heap.objects.iter()
    }
}
