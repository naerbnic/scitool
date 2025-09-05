use crate::{
    script_loader::selectors::{Selector, SelectorTable},
    utils::{
        errors::{AnyInvalidDataError, NoError},
        mem_reader::{FromFixedBytes, MemReader, NoErrorResultExt as _},
    },
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    InvalidData(#[from] AnyInvalidDataError),
    #[error("Object data has unexpected padding bytes")]
    BadObjectPadding,
    #[error(
        "Class has script but number of properties does not equal number of fields: {num_properties} properties, {num_fields} fields"
    )]
    PropertyMismatch {
        num_properties: usize,
        num_fields: usize,
    },
}

struct MethodRecord {
    selector_id: u16,
    #[expect(dead_code)]
    method_offset: u16,
}

impl FromFixedBytes for MethodRecord {
    const SIZE: usize = 4;
    fn parse<B: bytes::Buf>(mut bytes: B) -> Self {
        Self {
            selector_id: bytes.get_u16_le(),
            method_offset: bytes.get_u16_le(),
        }
    }
}

pub(crate) struct ObjectData {
    selector_table: SelectorTable,
    obj_data: Vec<u16>,
    property_ids: Vec<u16>,
    method_records: Vec<MethodRecord>,
}

impl ObjectData {
    pub(crate) fn from_block<'a, M>(
        selector_table: &SelectorTable,
        loaded_data: &M,
        obj_data: Vec<u16>,
    ) -> Result<Self, Error>
    where
        M: MemReader<Error = NoError> + 'a,
    {
        let var_selector_offfset = obj_data[2];
        let method_record_offset = obj_data[3];
        let padding = obj_data[4];
        if padding != 0 {
            return Err(Error::BadObjectPadding);
        }

        let mut var_selectors = loaded_data
            .sub_reader_range(
                "Var selector table",
                var_selector_offfset as usize..method_record_offset as usize,
            )
            .remove_no_error()?;

        let property_ids = var_selectors
            .split_values::<u16>("Property IDs")
            .remove_no_error()?;

        let mut method_record_remainder = loaded_data
            .sub_reader_range("Method record remainder", method_record_offset as usize..)
            .remove_no_error()?;

        let mut method_records = method_record_remainder
            .read_length_delimited_block("Method records", 4)
            .remove_no_error()?;

        let method_records = method_records
            .split_values::<MethodRecord>("Method records")
            .remove_no_error()?;

        Ok(Self {
            selector_table: selector_table.clone(),
            obj_data,
            property_ids,
            method_records,
        })
    }

    pub(crate) fn get_num_properties(&self) -> usize {
        self.property_ids.len()
    }

    pub(crate) fn get_num_fields(&self) -> usize {
        self.obj_data.len()
    }

    pub(crate) fn get_property_by_name(&self, name: &str) -> Option<u16> {
        let selector_id = self.selector_table.get_selector_by_name(name)?;
        self.get_property_by_id(selector_id.id())
    }

    pub(crate) fn get_property_by_id(&self, id: u16) -> Option<u16> {
        let index = self
            .property_ids
            .iter()
            .position(|found_id| *found_id == id)?;
        Some(self.obj_data[index])
    }

    pub(crate) fn get_property_at_index(&self, index: usize) -> Option<u16> {
        self.obj_data.get(index).copied()
    }

    pub(crate) fn get_method_selectors(&self) -> impl Iterator<Item = &Selector> {
        self.method_records.iter().map(|record| {
            self.selector_table
                .get_selector_by_id(record.selector_id)
                .unwrap()
        })
    }

    pub(crate) fn properties(&self) -> impl Iterator<Item = (&Selector, u16)> {
        assert_eq!(self.property_ids.len(), self.obj_data.len());
        self.property_ids
            .iter()
            .copied()
            .map(|selector_id| self.selector_table.get_selector_by_id(selector_id).unwrap())
            .zip(self.obj_data.iter().copied())
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
    pub(crate) fn from_block<'a, M>(
        selector_table: &SelectorTable,
        loaded_data: &M,
        obj_data: Vec<u16>,
    ) -> Result<Object, Error>
    where
        M: MemReader<Error = NoError> + 'a,
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
                let string_data = loaded_data
                    .sub_reader_range("object name string data", name_ptr as usize..)
                    .remove_no_error()?;
                Some(
                    super::read_null_terminated_string(string_data)
                        .map_err(|e| loaded_data.create_invalid_data_error(e))
                        .map_err(AnyInvalidDataError::from)?
                        .to_string(),
                )
            }
        } else {
            None
        };

        if script != 0xFFFF && object_data.get_num_properties() != object_data.get_num_fields() {
            return Err(Error::PropertyMismatch {
                num_properties: object_data.get_num_properties(),
                num_fields: object_data.get_num_fields(),
            });
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
