use crate::utils::{
    block::{BlockReader, MemBlock},
    buffer::{Buffer, BufferExt, BufferOpsExt},
    data_reader::DataReader,
};

use super::selectors::SelectorTable;
use bytes::{Buf, BufMut};

mod object;

pub use object::Object;

fn modify_u16_le_in_slice(
    slice: &mut [u8],
    at: usize,
    body: impl FnOnce(u16) -> anyhow::Result<u16>,
) -> anyhow::Result<()> {
    let slice: &mut [u8] = &mut slice[at..][..2];
    let slice: &mut [u8; 2] = slice.try_into()?;
    let value = u16::from_le_bytes(*slice);
    let new_value = body(value)?;
    slice.copy_from_slice(&new_value.to_le_bytes());
    Ok(())
}

fn apply_relocations<B>(buffer: &mut [u8], relocations: B, offset: u16) -> anyhow::Result<()>
where
    B: Buffer,
{
    let (relocation_entries, rest) = relocations.read_length_delimited_records::<u16>()?;
    anyhow::ensure!(rest.is_empty());

    for reloc_entry in relocation_entries {
        modify_u16_le_in_slice(buffer, reloc_entry as usize, |v| Ok(v.wrapping_add(offset)))?;
    }
    Ok(())
}

fn read_null_terminated_string_at(buffer: &[u8], offset: usize) -> anyhow::Result<&str> {
    let null_pos = buffer[offset..]
        .iter()
        .position(|&b| b == 0)
        .ok_or_else(|| anyhow::anyhow!("No null terminator found in string"))?;
    Ok(std::str::from_utf8(&buffer[offset..offset + null_pos])?)
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
    ) -> anyhow::Result<Heap> {
        let relocations_offset = resource_data.read_u16_le_at(0);
        let heap_data = resource_data
            .clone()
            .sub_buffer(..relocations_offset as usize);
        let num_locals = heap_data.read_u16_le_at(2);
        let (locals, mut heap_data) = heap_data.sub_buffer(4..).split_at((num_locals * 2).into());

        let mut objects = Vec::new();
        // Find all objects
        loop {
            let magic = heap_data.read_u16_le_at(0);
            if magic == 0 {
                // Indicates that we've gotten to the last object on the heap.
                // Break out of the loop.
                heap_data = heap_data.sub_buffer(2..);
                break;
            }

            anyhow::ensure!(magic == 0x1234u16);
            let object_size = heap_data.read_u16_le_at(2);
            let (object_data, next_heap_data) = heap_data.split_at((object_size * 2).into());
            let new_obj = Object::from_block(selector_table, loaded_script, object_data)?;
            objects.push(new_obj);
            heap_data = next_heap_data;
        }

        let mut strings = Vec::new();
        // Find all strings
        while !heap_data.is_empty() {
            let Some(null_pos) = heap_data.iter().position(|b| b == &0) else {
                anyhow::bail!("No null terminator found in string heap");
            };
            let (string_data, next_heap_data) =
                heap_data.split_at((null_pos + 1).try_into().unwrap());
            strings.push(string_data);
            heap_data = next_heap_data;
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

pub(crate) struct Script {
    #[expect(dead_code)]
    data: MemBlock,
    #[expect(dead_code)]
    relocations: MemBlock,
    #[expect(dead_code)]
    exports: Vec<u16>,
}

impl Script {
    pub(crate) fn from_block(data: MemBlock) -> anyhow::Result<Self> {
        let relocation_offset = {
            let mut reader = BlockReader::new(data.clone());
            reader.read_u16_le()?
        };
        let (script_data, relocations) = data.clone().split_at(relocation_offset.into());
        let (exports, _) = script_data
            .sub_buffer(2..)
            .read_length_delimited_records::<u16>()?;

        Ok(Self {
            data,
            relocations,
            exports,
        })
    }
}

fn extract_relocation_block<B>(data: B) -> B
where
    B: Buffer,
{
    let relocation_offset = data.lock().unwrap().get_u16_le();
    data.sub_buffer(relocation_offset..)
}

pub struct LoadedScript {
    #[expect(dead_code)]
    heap_offset: u16,
    #[expect(dead_code)]
    full_buffer: MemBlock,
    #[expect(dead_code)]
    script: Script,
    heap: Heap,
}

impl LoadedScript {
    pub fn load<B>(
        selector_table: &SelectorTable,
        script_data: &B,
        heap_data: &B,
    ) -> anyhow::Result<LoadedScript>
    where
        B: Buffer + Clone,
    {
        let heap_offset = script_data.size();
        anyhow::ensure!(heap_offset % 2 == 0);

        #[expect(clippy::cast_possible_truncation)]
        let u16_heap_offset = heap_offset as u16;

        // Concat the two blocks.
        //
        // It may be possible to get rid of the relocation block, but it's not clear.
        let mut loaded_script: Vec<u8> = script_data.to_vec()?;
        loaded_script.put(heap_data.lock()?);

        {
            let (script_data_slice, heap_data_slice) =
                loaded_script.split_at_mut(heap_offset.try_into().unwrap());
            let script_relocation_block = extract_relocation_block(script_data.clone());
            let heap_relocation_block = extract_relocation_block(heap_data.clone());

            apply_relocations(script_data_slice, script_relocation_block, u16_heap_offset)?;
            apply_relocations(heap_data_slice, heap_relocation_block, u16_heap_offset)?;
        }

        let loaded_script = MemBlock::from_vec(loaded_script);
        let (script_data, heap_data) = loaded_script.clone().split_at(heap_offset);
        let script = Script::from_block(script_data)?;
        let heap = Heap::from_block(selector_table, &loaded_script, heap_data)?;

        Ok(LoadedScript {
            heap_offset: u16_heap_offset,
            full_buffer: loaded_script,
            script,
            heap,
        })
    }

    pub fn objects(&self) -> impl Iterator<Item = &Object> {
        self.heap.objects.iter()
    }
}
