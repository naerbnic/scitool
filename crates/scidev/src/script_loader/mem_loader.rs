use crate::utils::{
    block::MemBlock,
    buffer::BufferExt,
    errors::{AnyInvalidDataError, NoError, OtherError, prelude::*},
    mem_reader::{self, BufferMemReader, MemReader, NoErrorResultExt},
};

use super::selectors::SelectorTable;
use bytes::BufMut;

mod object;

pub use object::Object;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error(transparent)]
    InvalidData(#[from] AnyInvalidDataError),

    #[error(transparent)]
    Object(object::ObjectError),

    #[error("Script data size must be be 2-byte-aligned. Found size: {size:x}")]
    ScriptSizeNotAligned { size: usize },
}

impl From<object::Error> for Error {
    fn from(err: object::Error) -> Self {
        match err {
            object::Error::InvalidData(invalid_data_err) => Self::InvalidData(invalid_data_err),
            object::Error::Object(err) => Self::Object(err),
        }
    }
}

impl From<mem_reader::Error<NoError>> for Error {
    fn from(err: mem_reader::Error<NoError>) -> Self {
        match err {
            mem_reader::Error::InvalidData(invalid_data_err) => Self::InvalidData(invalid_data_err),
            mem_reader::Error::BaseError(err) => err.absurd(),
        }
    }
}

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
) -> Result<(), Error>
where
    M: MemReader<Error = NoError> + 'a,
{
    let relocation_entries =
        relocations.read_length_delimited_records::<u16>("Relocation Table Contents")?;
    if !relocations.is_empty() {
        return Err(
            AnyInvalidDataError::from(relocations.create_invalid_data_error(OtherError::from_msg(
                "Relocation block size and length must match",
            )))
            .into(),
        );
    }

    for reloc_entry in relocation_entries {
        modify_u16_le_in_slice(buffer, reloc_entry as usize, |v| v.wrapping_add(offset));
    }
    Ok(())
}

fn read_null_terminated_string<M: MemReader>(mut buffer: M) -> Result<String, OtherError> {
    let string_data = buffer
        .read_until::<u8>("null terminated string", |b| *b == 0)
        .with_other_err()?;
    std::str::from_utf8(&string_data[..string_data.len() - 1])
        .map(ToString::to_string)
        .with_other_err()
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
    ) -> Result<Heap, Error>
    where
        M: MemReader<Error = NoError>,
    {
        let _ = resource_data.read_u16_le()?;
        let num_locals = resource_data.read_value::<u16>("Num locals")?;
        let locals = resource_data
            .read_to_subreader("Splitting locals.", (num_locals * 2).into())?
            .split_values::<u16>("Local variable IDs")?;

        let mut objects = Vec::new();
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

            if magic != 0x1234u16 {
                return Err(
                    AnyInvalidDataError::from(resource_data.create_invalid_data_error(
                        OtherError::from_msg("Invalid object magic number"),
                    ))
                    .into(),
                );
            }
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
            let mut string_data = resource_data.read_until::<u8>("string_obj", |b| *b == 0)?;
            string_data.pop(); // Remove the null terminator.
            let string = String::from_utf8(string_data)
                .map_err(|e| resource_data.create_invalid_data_error(e))
                .map_err(AnyInvalidDataError::from)?;
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

fn extract_relocation_block(data: &MemBlock) -> Result<(u16, MemBlock), Error> {
    let cloned_data = data.clone();
    let mut reader = BufferMemReader::from_ref(&cloned_data);
    let relocation_offset = reader.read_value::<u16>("Relocation offset")?;
    if relocation_offset as usize > cloned_data.size() {
        return Err(AnyInvalidDataError::from(
            reader
                .create_invalid_data_error(OtherError::from_msg("Relocation offset out of bounds")),
        )
        .into());
    }
    Ok((
        relocation_offset,
        cloned_data
            .sub_buffer(relocation_offset as usize..)
            .remove_no_error(),
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
    ) -> Result<LoadedScript, Error> {
        let heap_offset = script_data.size();
        if heap_offset % 2 != 0 {
            return Err(Error::ScriptSizeNotAligned { size: heap_offset });
        }

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
            &mut BufferMemReader::from_ref(&script_relocation_block),
            u16_heap_offset,
        )?;
        apply_relocations(
            heap_data_slice,
            &mut BufferMemReader::from_ref(&heap_relocation_block),
            u16_heap_offset,
        )?;

        let loaded_script = MemBlock::from_vec(loaded_script);
        let heap_data = loaded_script
            .clone()
            .sub_buffer(heap_offset..heap_offset + heap_relocation_offset as usize)
            .remove_no_error();
        let heap = Heap::from_block(
            selector_table,
            &BufferMemReader::from_ref(&loaded_script),
            &mut BufferMemReader::from_ref(&heap_data),
        )?;

        Ok(LoadedScript { heap })
    }

    pub fn objects(&self) -> impl Iterator<Item = &Object> {
        self.heap.objects.iter()
    }
}
