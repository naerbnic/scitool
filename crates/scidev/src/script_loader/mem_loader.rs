use crate::{
    script_loader::errors::MalformedDataError,
    utils::{
        block::MemBlock,
        buffer::{Buffer, BufferExt, BufferOpsExt},
        errors::{OtherError, ensure_other, prelude::*},
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

fn apply_relocations<B>(
    buffer: &mut [u8],
    relocations: B,
    offset: u16,
) -> Result<(), MalformedDataError>
where
    B: Buffer,
{
    let (relocation_entries, rest) = relocations
        .read_length_delimited_records::<u16>()
        .map_err(MalformedDataError::map_from("Relocation Table Contents"))?;
    ensure_other!(
        rest.is_empty(),
        "Relocation block size and length must match"
    );

    for reloc_entry in relocation_entries {
        modify_u16_le_in_slice(buffer, reloc_entry as usize, |v| v.wrapping_add(offset));
    }
    Ok(())
}

fn read_null_terminated_string_at(buffer: &[u8], offset: usize) -> Result<&str, OtherError> {
    let null_pos = buffer[offset..]
        .iter()
        .position(|&b| b == 0)
        .ok_or_else_other(|| "No null terminator found in string")?;
    std::str::from_utf8(&buffer[offset..offset + null_pos]).with_other_err()
}

pub(crate) struct Heap {
    #[expect(dead_code)]
    resource_data: MemBlock,
    #[expect(dead_code)]
    locals: MemBlock,
    objects: Vec<Object>,
    #[expect(dead_code)]
    strings: Vec<MemBlock>,
}

impl Heap {
    pub(crate) fn from_block(
        selector_table: &SelectorTable,
        loaded_script: &MemBlock,
        resource_data: MemBlock,
    ) -> Result<Heap, FromBlockError> {
        let relocations_offset = resource_data
            .read_u16_le_at(0)
            .map_err(MalformedDataError::map_from("Relocation offset"))?;
        let heap_data = resource_data
            .clone()
            .sub_buffer(2..relocations_offset as usize)
            .map_err(MalformedDataError::map_from("Separate heap data"))?;
        let (num_locals, heap_data) = heap_data
            .read_value::<u16>()
            .map_err(MalformedDataError::map_from("Num locals"))?;
        let (locals, mut heap_data) = heap_data
            .split_at((num_locals * 2).into())
            .map_err(MalformedDataError::map_from("Splitting locals."))?;

        let mut objects = Vec::new();
        // Find all objects
        loop {
            let (magic, curr_heap_data) = heap_data
                .clone()
                .read_value::<u16>()
                .map_err(MalformedDataError::map_from("Object Magic"))?;
            if magic == 0 {
                // Indicates that we've gotten to the last object on the heap.
                // Break out of the loop.
                heap_data = curr_heap_data;
                break;
            }

            ensure_other!(magic == 0x1234u16, "Invalid object magic number");
            let (num_object_fields, _) = curr_heap_data
                .read_value::<u16>()
                .map_err(MalformedDataError::map_from("Num Object Fields"))?;

            let (object_data, heap_data_after_object) = heap_data
                .read_values(num_object_fields.into())
                .map_err(MalformedDataError::map_from("Object fields"))?;

            // The size is based from the very start of the object, so we reuse the curr_heap_data.
            let new_obj =
                Object::from_block(selector_table, loaded_script, object_data).with_other_err()?;
            objects.push(new_obj);
            heap_data = heap_data_after_object;
        }

        let mut strings = Vec::new();
        // Find all strings
        while !heap_data.is_empty() {
            let null_pos = heap_data
                .iter()
                .position(|b| b == &0)
                .ok_or_else_other(|| "No null terminator found in string heap")?;
            let (string_data, curr_heap_data) = heap_data
                .split_at(null_pos + 1)
                .map_err(MalformedDataError::map_from("String entry"))?;
            strings.push(string_data);
            heap_data = curr_heap_data;
        }

        Ok(Self {
            resource_data,
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

fn extract_relocation_block<B>(data: B) -> Result<B, MalformedDataError>
where
    B: Buffer + Clone,
{
    let (relocation_offset, _) = data
        .clone()
        .read_value::<u16>()
        .map_err(MalformedDataError::map_from("Relocation offset"))?;
    data.sub_buffer(relocation_offset..)
        .map_err(MalformedDataError::map_from("Relocation block"))
}

pub struct LoadedScript {
    heap: Heap,
}

impl LoadedScript {
    pub fn load<B>(
        selector_table: &SelectorTable,
        script_data: &B,
        heap_data: &B,
    ) -> Result<LoadedScript, MalformedDataError>
    where
        B: Buffer + Clone,
    {
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

        {
            let (script_data_slice, heap_data_slice) = loaded_script.split_at_mut(heap_offset);
            let script_relocation_block = extract_relocation_block(script_data.clone())?;
            let heap_relocation_block = extract_relocation_block(heap_data.clone())?;

            apply_relocations(script_data_slice, script_relocation_block, u16_heap_offset)?;
            apply_relocations(heap_data_slice, heap_relocation_block, u16_heap_offset)?;
        }

        let loaded_script = MemBlock::from_vec(loaded_script);
        let heap_data = loaded_script
            .clone()
            .sub_buffer(heap_offset..)
            .expect("Constructed to have the correct offset.");
        let heap = Heap::from_block(selector_table, &loaded_script, heap_data).with_other_err()?;

        Ok(LoadedScript { heap })
    }

    pub fn objects(&self) -> impl Iterator<Item = &Object> {
        self.heap.objects.iter()
    }
}
