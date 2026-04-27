use crate::utils::{
    block::MemBlock,
    buffer::{Buffer, Splittable as _},
    mem_reader::{BufferMemReader, MemReader},
};

use super::selectors::SelectorTable;
use bytes::BufMut;
use scidev_errors::{AnyDiag, define_error, diag, ensure, in_err_context, prelude::*};

mod object;

pub use object::Object;

fn modify_u16_le_in_slice(slice: &mut [u8], at: usize, body: impl FnOnce(u16) -> u16) {
    let slice: &mut [u8] = &mut slice[at..][..2];
    let slice: &mut [u8; 2] = slice.try_into().unwrap();
    let value = u16::from_le_bytes(*slice);
    let new_value = body(value);
    slice.copy_from_slice(&new_value.to_le_bytes());
}

fn apply_relocations<'a, M>(
    buffer: &mut [u8],
    relocations: &mut M,
    offset: u16,
) -> Result<(), AnyDiag>
where
    M: MemReader + 'a,
{
    let relocation_entries = relocations
        .read_length_delimited_records::<u16>("Relocation Table Contents")
        .map_raise_err(diag!(|e| "Failed to read relocation entries"))?;
    ensure!(
        relocations.is_empty(),
        "Relocation block size and length must match. Found {num_entries} entries, had {bytes} bytes left.",
        num_entries = relocation_entries.len(),
        bytes = relocations.data_size(),
    );

    for reloc_entry in relocation_entries {
        modify_u16_le_in_slice(buffer, reloc_entry as usize, |v| v.wrapping_add(offset));
    }
    Ok(())
}

fn read_null_terminated_string<M: MemReader>(mut buffer: M) -> Result<String, AnyDiag> {
    let string_data = buffer
        .read_until::<u8>("null terminated string", |b| *b == 0)
        .map_raise_err(diag!(|e| "Failed to read null terminated string"))?;
    std::str::from_utf8(&string_data[..string_data.len() - 1])
        .map(ToString::to_string)
        .map_raise(diag!(|err| "Invalid UTF-8 in null terminated string {err}"))
}

pub(crate) struct Heap {
    #[expect(dead_code)]
    locals: Vec<u16>,
    objects: Vec<Object>,
    #[expect(dead_code)]
    strings: Vec<String>,
}

impl Heap {
    pub(crate) fn from_block<M>(
        selector_table: &SelectorTable,
        loaded_script: &M,
        resource_data: &mut M,
    ) -> Result<Heap, AnyDiag>
    where
        M: MemReader,
    {
        in_err_context(|| {
            let mut objects = Vec::new();
            let _ = resource_data.read_u16_le()?;
            let num_locals = resource_data.read_value::<u16>("Num locals")?;
            let locals = resource_data
                .read_to_subreader("Splitting locals.", (num_locals * 2).into())?
                .split_values::<u16>("Local variable IDs")?;
            // Find all objects
            loop {
                let obj_start = resource_data.tell();
                let magic = resource_data.read_value::<u16>("Object Magic Number")?;
                if magic == 0 {
                    // Indicates that we've gotten to the last object on the heap.
                    // Break out of the loop.
                    resource_data.seek_to(obj_start).unwrap(); // Rewind so the 0 can be read as part of the string table.
                    break;
                }

                ensure!(magic == 0x1234u16, "Invalid object magic number");

                let num_object_fields = resource_data.read_value::<u16>("Num Object Fields")?;

                resource_data.seek_to(obj_start)?;

                let object_data =
                    resource_data.read_values("Object fields", num_object_fields.into())?;
                // The size is based from the very start of the object, so we reuse the curr_heap_data.
                let new_obj = Object::from_block(selector_table, loaded_script, object_data)?;
                objects.push(new_obj);
            }

            let mut strings = Vec::new();
            // Find all strings
            while !resource_data.is_empty() {
                let mut string_data = resource_data
                    .read_until::<u8>("string_obj", |b| *b == 0)
                    .raise_err_with(diag!(|| "Failed to read null terminated string"))?;
                string_data.pop(); // Remove the null terminator.
                let string = String::from_utf8(string_data)
                    .map_raise(diag!(|err| "Non-UTF8 string data: {err}"))?;
                strings.push(string);
            }
            Ok(Self {
                locals,
                objects,
                strings,
            })
        })
        .reraise()
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

fn extract_relocation_block(data: &MemBlock) -> Result<(u16, MemBlock), AnyDiag> {
    let cloned_data = data.clone();
    let mut reader = BufferMemReader::new(cloned_data.as_fallible());
    let relocation_offset = reader
        .read_value::<u16>("Relocation offset")
        .raise_err_with(diag!(|| "Could not read relocation offset"))?;
    ensure!(
        relocation_offset as usize <= cloned_data.size(),
        "Relocation offset out of bounds"
    );
    Ok((
        relocation_offset,
        cloned_data.sub_buffer(relocation_offset as usize..),
    ))
}

pub struct LoadedScript {
    heap: Heap,
}

define_error! {
    pub struct LoadedScriptError;
}

impl LoadedScript {
    pub fn load(
        selector_table: &SelectorTable,
        script_data: &MemBlock,
        heap_data: &MemBlock,
    ) -> Result<LoadedScript, LoadedScriptError> {
        let heap_offset = script_data.size();
        ensure!(
            heap_offset.is_multiple_of(2),
            "Heap offset is not aligned. Offset: {heap_offset}"
        );

        let u16_heap_offset: u16 = heap_offset.try_into().unwrap();

        // Concat the two blocks.
        //
        // It may be possible to get rid of the relocation block, but it's not clear.
        let mut loaded_script: Vec<u8> = script_data.as_ref().to_vec();
        loaded_script.put(heap_data.as_ref());

        let (script_data_slice, heap_data_slice) = loaded_script.split_at_mut(heap_offset);
        let (_, script_relocation_block) = extract_relocation_block(script_data)?;
        let (heap_relocation_offset, heap_relocation_block) = extract_relocation_block(heap_data)?;

        apply_relocations(
            script_data_slice,
            &mut BufferMemReader::new(script_relocation_block.as_fallible()),
            u16_heap_offset,
        )?;
        apply_relocations(
            heap_data_slice,
            &mut BufferMemReader::new(heap_relocation_block.as_fallible()),
            u16_heap_offset,
        )?;

        let loaded_script = MemBlock::from_vec(loaded_script);
        let heap_data = loaded_script
            .clone()
            .sub_buffer(heap_offset..heap_offset + heap_relocation_offset as usize);
        let heap = Heap::from_block(
            selector_table,
            &BufferMemReader::new(loaded_script.as_fallible()),
            &mut BufferMemReader::new(heap_data.as_fallible()),
        )?;

        Ok(LoadedScript { heap })
    }

    pub fn objects(&self) -> impl Iterator<Item = &Object> {
        self.heap.objects.iter()
    }
}
