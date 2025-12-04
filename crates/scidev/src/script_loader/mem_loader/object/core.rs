use crate::{
    script_loader::{
        mem_loader::read_null_terminated_string,
        selectors::{Selector, SelectorTable},
    },
    utils::mem_reader::MemReader,
};

use super::{
    error::{Error, ObjectError},
    object_data::ObjectData,
};

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
    pub(crate) fn from_block<'a, M>(
        selector_table: &SelectorTable,
        loaded_data: &M,
        obj_data: Vec<u16>,
    ) -> Result<Object, Error>
    where
        M: MemReader + 'a,
    {
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
        let name = if let Some(name_ptr) = object_data.get_property_at_index(NAME_INDEX) {
            if name_ptr == 0 {
                None
            } else {
                let string_data =
                    loaded_data.sub_reader_range("object name string data", name_ptr as usize..)?;
                Some(
                    read_null_terminated_string(string_data)
                        .map_err(|e| loaded_data.create_invalid_data_error(e))?
                        .clone(),
                )
            }
        } else {
            None
        };

        if script != 0xFFFF && object_data.get_num_properties() != object_data.get_num_fields() {
            return Err(ObjectError::PropertyMismatch {
                num_properties: object_data.get_num_properties(),
                num_fields: object_data.get_num_fields(),
            }
            .into());
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
            .field("size", &(self.data.get_num_properties() / 2))
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
