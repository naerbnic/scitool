use sci_utils::{
    block::MemBlock,
    buffer::{Buffer, BufferExt, BufferOpsExt, FromFixedBytes},
};

use crate::selectors::{Selector, SelectorTable};

struct MethodRecord {
    selector_id: u16,
    #[expect(dead_code)]
    method_offset: u16,
}

impl FromFixedBytes for MethodRecord {
    const SIZE: usize = 4;
    fn parse<B: bytes::Buf>(mut bytes: B) -> anyhow::Result<Self> {
        Ok(Self {
            selector_id: bytes.get_u16_le(),
            method_offset: bytes.get_u16_le(),
        })
    }
}

pub(crate) struct ObjectData {
    selector_table: SelectorTable,
    obj_data: MemBlock,
    var_selectors: MemBlock,
    method_records: MemBlock,
}

impl ObjectData {
    pub(crate) fn from_block(
        selector_table: &SelectorTable,
        loaded_data: &MemBlock,
        obj_data: MemBlock,
    ) -> anyhow::Result<Self> {
        let var_selector_offfset = obj_data.read_u16_le_at(4);
        let method_record_offset = obj_data.read_u16_le_at(6);
        let padding = obj_data.read_u16_le_at(8);
        anyhow::ensure!(padding == 0);

        let var_selectors = loaded_data
            .clone()
            .sub_buffer(var_selector_offfset as usize..method_record_offset as usize);

        let (method_records, _) = loaded_data
            .clone()
            .sub_buffer(method_record_offset as usize..)
            .read_length_delimited_block(4)?;

        Ok(Self {
            selector_table: selector_table.clone(),
            obj_data,
            var_selectors,
            method_records,
        })
    }

    pub(crate) fn get_num_properties(&self) -> usize {
        self.var_selectors.len() / 2
    }

    pub(crate) fn get_num_fields(&self) -> usize {
        self.obj_data.len() / 2
    }

    pub(crate) fn get_property_by_name(&self, name: &str) -> Option<u16> {
        let selector_id = self.selector_table.get_selector_by_name(name)?;
        self.get_property_by_id(selector_id.id())
    }

    pub(crate) fn get_property_by_id(&self, id: u16) -> Option<u16> {
        let index = self
            .var_selectors
            .clone()
            .split_values::<u16>()
            .ok()?
            .iter()
            .position(|found_id| *found_id == id)?;
        Some(self.obj_data.clone().split_values::<u16>().ok()?[index])
    }

    pub(crate) fn get_property_at_index(&self, index: usize) -> Option<u16> {
        let properties = self.obj_data.clone().split_values::<u16>().ok()?;
        properties.get(index).copied()
    }

    pub(crate) fn get_method_selectors(&self) -> impl Iterator<Item = &Selector> {
        self.method_records
            .clone()
            .split_values::<MethodRecord>()
            .unwrap()
            .into_iter()
            .map(|record| {
                self.selector_table
                    .get_selector_by_id(record.selector_id)
                    .unwrap()
            })
    }

    pub(crate) fn properties(&self) -> impl Iterator<Item = (&Selector, u16)> {
        let var_selector_ids = self.var_selectors.clone().split_values::<u16>().unwrap();
        let fields = self.obj_data.clone().split_values::<u16>().unwrap();
        assert_eq!(var_selector_ids.len(), fields.len());
        var_selector_ids
            .into_iter()
            .map(|selector_id| self.selector_table.get_selector_by_id(selector_id).unwrap())
            .zip(fields)
    }
}

pub struct Object {
    data: ObjectData,

    // Standard property values
    class_script: u16,
    script: u16,
    super_class: u16,
    info: u16,
    name: Option<String>,
}

impl Object {
    pub fn from_block(
        selector_table: &SelectorTable,
        loaded_data: &MemBlock,
        obj_data: MemBlock,
    ) -> anyhow::Result<Object> {
        // Read the standard properties.
        //
        // We can ony do this with selectors that are officially built in, which
        // unfortunately does not include "name". Selectors that are built-in
        // start and end with "-".
        //
        // 0: "-objID-"
        // 1: "-size-"
        // 2: "-propDict-"
        // 3: "-methDict-"
        // 4: "-classScript-"
        const CLASS_SCRIPT_INDEX: usize = 4;
        const SCRIPT_INDEX: usize = 5;
        const SUPER_INDEX: usize = 6;
        const INFO_INDEX: usize = 7;
        // Name is _usually_ found at index 8, but it's not guaranteed. Use it as a hack for now.
        const NAME_INDEX: usize = 8;

        let object_data = ObjectData::from_block(selector_table, loaded_data, obj_data)?;

        let class_script = object_data
            .get_property_at_index(CLASS_SCRIPT_INDEX)
            .unwrap();
        let script = object_data.get_property_at_index(SCRIPT_INDEX).unwrap();
        let super_class = object_data.get_property_at_index(SUPER_INDEX).unwrap();
        let info = object_data.get_property_at_index(INFO_INDEX).unwrap();
        let name = object_data
            .get_property_at_index(NAME_INDEX)
            .and_then(|name_ptr| {
                if name_ptr == 0 {
                    None
                } else {
                    Some(
                        super::read_null_terminated_string_at(loaded_data, name_ptr as usize)
                            .unwrap()
                            .to_string(),
                    )
                }
            });

        if script != 0xFFFF {
            assert_eq!(
                object_data.get_num_properties(),
                object_data.get_num_fields()
            );
        }

        Ok(Self {
            data: object_data,
            class_script,
            script,
            super_class,
            info,
            name,
        })
    }

    #[must_use]
    pub fn get_property_by_name(&self, name: &str) -> Option<u16> {
        self.data.get_property_by_name(name)
    }

    #[must_use]
    pub fn get_property_by_id(&self, id: u16) -> Option<u16> {
        self.data.get_property_by_id(id)
    }

    #[must_use]
    pub fn get_property_at_index(&self, index: usize) -> Option<u16> {
        self.data.get_property_at_index(index)
    }

    pub fn methods(&self) -> impl Iterator<Item = &Selector> {
        self.data.get_method_selectors()
    }

    pub fn properties(&self) -> impl Iterator<Item = (&Selector, u16)> {
        assert!(self.is_class());
        self.data.properties()
    }

    #[must_use]
    pub fn is_class(&self) -> bool {
        self.info & 0x8000 != 0
    }

    #[must_use]
    pub fn species(&self) -> u16 {
        self.script
    }

    #[must_use]
    pub fn super_class(&self) -> u16 {
        self.super_class
    }

    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
}

impl std::fmt::Debug for Object {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Object")
            .field("size", &(self.data.obj_data.len() / 2))
            .field("class_script", &self.class_script)
            .field("script", &self.script)
            .field("super_class", &self.super_class)
            .field("info", &self.info)
            .field("name", &self.name)
            .field("is_class", &self.is_class())
            .field(
                "methods",
                &self.data.get_method_selectors().collect::<Vec<_>>(),
            )
            .finish()
    }
}
