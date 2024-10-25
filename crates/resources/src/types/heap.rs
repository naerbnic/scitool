use sci_utils::{block::Block, buffer_ops::BufferOpsExt};

pub struct Heap {
    #[expect(dead_code)]
    resource_data: Block,
    #[expect(dead_code)]
    relocations: Block,
    #[expect(dead_code)]
    locals: Block,
    #[expect(dead_code)]
    objects: Vec<Object>,
    #[expect(dead_code)]
    strings: Vec<Block>,
}

impl Heap {
    pub fn from_block(resource_data: Block) -> anyhow::Result<Heap> {
        let relocations_offset = resource_data.read_u16_le_at(0);
        let (heap_data, relocations) = resource_data.split_at(relocations_offset as u64);
        let num_locals = heap_data.read_u16_le_at(2);
        let (locals, mut heap_data) = heap_data.subblock(4..).split_at((num_locals * 2) as u64);

        let mut objects = Vec::new();
        // Find all objects
        loop {
            let magic = heap_data.read_u16_le_at(0);
            if magic == 0 {
                heap_data = heap_data.subblock(2..);
                break;
            }

            eprintln!("Object: {:04X}", resource_data.offset_in(&heap_data));

            anyhow::ensure!(magic == 0x1234u16);
            let object_size = heap_data.read_u16_le_at(2);
            let (object_data, next_heap_data) = heap_data.split_at((object_size * 2) as u64);
            let new_obj = Object::from_block(object_data)?;
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
            relocations,
            locals,
            objects,
            strings,
        })
    }
}

pub struct Object {
    #[expect(dead_code)]
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
            var_selector_offfset,
            method_record_offset,
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
            .field(
                "var_entries",
                &((self.method_record_offset - self.var_selector_offfset) / 2),
            )
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
