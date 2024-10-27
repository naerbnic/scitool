use sci_utils::{
    block::{Block, BlockReader},
    buffer::{Buffer, BufferOpsExt, FromFixedBytes, ToFixedBytes},
    data_reader::DataReader,
    numbers::{modify_u16_le_in_slice, read_u16_le_from_slice, write_u16_le_to_slice},
};

fn read_length_delimited_records<'a, B: Buffer<'a>, T: FromFixedBytes>(
    data: B,
) -> anyhow::Result<(Vec<T>, B)> {
    let (num_records, next) = data.read_value::<u16>()?;
    let (values, next) = next.read_values::<T>(num_records as usize)?;
    Ok((values, next))
}

fn apply_relocations(buffer: &mut [u8], relocations: &Block, offset: u16) -> anyhow::Result<()> {
    let (relocation_entries, rest) = read_length_delimited_records::<_, u16>(relocations.clone())?;
    anyhow::ensure!(rest.is_empty());

    for reloc_entry in relocation_entries {
        modify_u16_le_in_slice(buffer, reloc_entry as usize, |v| Ok(v.wrapping_add(offset)))?;
    }
    Ok(())
}

pub struct Heap {
    #[expect(dead_code)]
    resource_data: Block,
    #[expect(dead_code)]
    locals: Block,
    #[expect(dead_code)]
    objects: Vec<Object>,
    #[expect(dead_code)]
    strings: Vec<Block>,
}

impl Heap {
    pub fn from_block(loaded_script: &Block, resource_data: Block) -> anyhow::Result<Heap> {
        let relocations_offset = resource_data.read_u16_le_at(0);
        let heap_data = resource_data
            .clone()
            .sub_buffer(..relocations_offset as u64);
        let num_locals = heap_data.read_u16_le_at(2);
        let (locals, mut heap_data) = heap_data.sub_buffer(4..).split_at((num_locals * 2) as u64);

        let mut objects = Vec::new();
        // Find all objects
        loop {
            let magic = heap_data.read_u16_le_at(0);
            if magic == 0 {
                heap_data = heap_data.sub_buffer(2..);
                break;
            }

            eprintln!("Object: {:04X}", resource_data.offset_in(&heap_data));

            anyhow::ensure!(magic == 0x1234u16);
            let object_size = heap_data.read_u16_le_at(2);
            let (object_data, next_heap_data) = heap_data.split_at((object_size * 2) as u64);
            let new_obj = Object::from_block(loaded_script, object_data)?;
            println!("Object: {:?}", new_obj);
            objects.push(new_obj);
            heap_data = next_heap_data;
        }

        eprintln!(
            "Strings offset: {:04X}",
            resource_data.offset_in(&heap_data)
        );

        let mut strings = Vec::new();
        // Find all strings
        while !heap_data.is_empty() {
            let Some(null_pos) = heap_data.iter().position(|b| b == &0) else {
                anyhow::bail!("No null terminator found in string heap");
            };
            let (string_data, next_heap_data) = heap_data.split_at((null_pos + 1) as u64);
            eprintln!(
                "String @{:04X}: {:?}",
                resource_data.offset_in(&string_data),
                std::str::from_utf8(&string_data[..string_data.len() - 1]).unwrap()
            );
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

struct MethodRecord {
    field1: u16,
    field2: u16,
}

impl ToFixedBytes for MethodRecord {
    const SIZE: usize = 4;

    fn to_bytes(&self, dest: &mut [u8]) -> anyhow::Result<()> {
        write_u16_le_to_slice(dest, 0, self.field1);
        write_u16_le_to_slice(dest, 2, self.field2);
        Ok(())
    }
}

impl FromFixedBytes for MethodRecord {
    fn parse(bytes: &[u8]) -> anyhow::Result<Self> {
        Ok(Self {
            field1: read_u16_le_from_slice(bytes, 0),
            field2: read_u16_le_from_slice(bytes, 2),
        })
    }
}

pub struct Object {
    #[expect(dead_code)]
    obj_data: Block,
    var_selectors: Block,
    #[expect(dead_code)]
    method_records: Vec<MethodRecord>,
    properties: Block,
}

impl Object {
    pub fn from_block(loaded_data: &Block, obj_data: Block) -> anyhow::Result<Object> {
        let var_selector_offfset = obj_data.read_u16_le_at(4);
        let method_record_offset = obj_data.read_u16_le_at(6);
        let padding = obj_data.read_u16_le_at(8);
        anyhow::ensure!(padding == 0);

        let var_selectors = loaded_data
            .clone()
            .sub_buffer(var_selector_offfset as u64..method_record_offset as u64);
        let (method_records, _) = read_length_delimited_records::<_, MethodRecord>(
            loaded_data
                .clone()
                .sub_buffer(method_record_offset as u64..),
        )?;

        let properties = obj_data.clone().sub_buffer(10..);

        let is_class = properties.read_u16_le_at(4) & 0x8000 != 0;

        if is_class {
            assert!(
                var_selector_offfset < method_record_offset,
                "Offsets: {:04X} {:04X}",
                var_selector_offfset,
                method_record_offset
            );
            // assert_eq!(
            //     method_record_offset - var_selector_offfset,
            //     properties.len() as u16
            // );
        } else {
            assert_eq!(var_selector_offfset, method_record_offset);
        }

        Ok(Self {
            obj_data,
            var_selectors,
            method_records,
            properties,
        })
    }

    pub fn is_class(&self) -> bool {
        self.properties.read_u16_le_at(4) & 0x8000 != 0
    }
}

impl std::fmt::Debug for Object {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Object")
            .field("is_class", &self.is_class())
            .field("num_properties", &(self.properties.len() / 2))
            .field("var_entries", &(self.var_selectors.size() / 2))
            .field(
                "species",
                &format!("{:04X}", self.properties.read_u16_le_at(0)),
            )
            .field(
                "parent",
                &format!("{:04X}", self.properties.read_u16_le_at(2)),
            )
            .finish()
    }
}

struct Relocations {
    num_relocations: usize,
    #[expect(dead_code)]
    reloc_block: Block,
}

impl Relocations {
    #[expect(dead_code)]
    pub fn num_relocations(&self) -> usize {
        self.num_relocations
    }
}

pub struct Script {
    #[expect(dead_code)]
    data: Block,
    #[expect(dead_code)]
    relocations: Block,
    #[expect(dead_code)]
    exports: Vec<u16>,
}

impl Script {
    pub fn from_block(data: Block) -> anyhow::Result<Self> {
        let relocation_offset = {
            let mut reader = BlockReader::new(data.clone());
            reader.read_u16_le()?
        };
        let (script_data, relocations) = data.clone().split_at(relocation_offset as u64);
        let (exports, _) = read_length_delimited_records::<_, u16>(script_data.sub_buffer(2..))?;

        Ok(Self {
            data,
            relocations,
            exports,
        })
    }
}

fn extract_relocation_block(data: &Block) -> Block {
    let relocation_offset = data.read_u16_le_at(0) as u64;
    data.clone().sub_buffer(relocation_offset..)
}

pub struct LoadedScript {
    #[expect(dead_code)]
    heap_offset: u16,
    #[expect(dead_code)]
    full_buffer: Block,
    #[expect(dead_code)]
    script: Script,
    #[expect(dead_code)]
    heap: Heap,
}

pub fn load_script(script_data: &Block, heap_data: &Block) -> anyhow::Result<LoadedScript> {
    let heap_offset = script_data.size();
    anyhow::ensure!(heap_offset % 2 == 0);
    // Concat the two blocks.
    //
    // It may be possible to get rid of the relocation block, but it's not clear.
    let mut loaded_script: Vec<u8> = script_data.iter().copied().collect();
    loaded_script.extend_from_slice(heap_data);

    {
        let (script_data_slice, heap_data_slice) = loaded_script.split_at_mut(heap_offset as usize);
        let script_relocation_block = extract_relocation_block(script_data);
        let heap_relocation_block = extract_relocation_block(heap_data);

        apply_relocations(
            script_data_slice,
            &script_relocation_block,
            heap_offset as u16,
        )?;
        apply_relocations(heap_data_slice, &heap_relocation_block, heap_offset as u16)?;
    }

    let loaded_script = Block::from_vec(loaded_script);
    let (script_data, heap_data) = loaded_script.clone().split_at(heap_offset);
    let script = Script::from_block(script_data)?;
    let heap = Heap::from_block(&loaded_script, heap_data)?;

    Ok(LoadedScript {
        heap_offset: heap_offset as u16,
        full_buffer: loaded_script,
        script,
        heap,
    })
}
