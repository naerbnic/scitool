use sci_utils::{
    block::{Block, BlockReader},
    buffer_ops::BufferOpsExt,
    data_reader::DataReader,
};

pub struct Heap {
    resource_data: Block,
    locals: Block,
    objects: Vec<Object>,
    strings: Vec<Block>,
}

impl Heap {
    pub fn from_block(resource_data: Block) -> anyhow::Result<Heap> {
        let relocations_offset = resource_data.read_u16_le_at(0);
        let (heap_data, relocations_block) = resource_data.split_at(relocations_offset as u64);
        let num_locals = heap_data.read_u16_le_at(0);
        let (locals, mut heap_data) = resource_data
            .subblock(2..)
            .split_at((num_locals * 2) as u64);

        let mut objects = Vec::new();
        // Find all objects
        loop {
            let magic = heap_data.read_u16_le_at(0);
            if magic == 0 {
                heap_data = heap_data.subblock(2..);
                break;
            }

            anyhow::ensure!(magic != 0x1234u16);
            let object_size = heap_data.read_u16_le_at(2);
            let (object_data, next_heap_data) = heap_data.split_at(object_size as u64);
            objects.push(Object::from_block(object_data)?);
            heap_data = next_heap_data;
        }

        let mut strings = Vec::new();
        // Find all strings
        while !heap_data.is_empty() {
            let Some(null_pos) = heap_data.iter().position(|b| b == &0) else {
                anyhow::bail!("No null terminator found in string heap");
            };
            let (string_data, next_heap_data) = heap_data.split_at((null_pos + 1) as u64);
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

pub struct Object {
    obj_data: Block,
    var_selector_offfset: u16,
    method_record_offset: u16,
    properties: Block,
}

impl Object {
    pub fn from_block(obj_data: Block) -> anyhow::Result<Object> {
        let var_selector_offfset = obj_data.read_u16_le_at(4);
        let method_record_offset = obj_data.read_u16_le_at(6);
        let padding = obj_data.read_u16_le_at(8);
        anyhow::ensure!(padding == 0);
        let properties = obj_data.subblock(10..);

        Ok(Self {
            obj_data,
            var_selector_offfset,
            method_record_offset,
            properties,
        })
    }
}
