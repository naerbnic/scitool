use crate::{
    script_loader::{
        mem_loader::object::error::BadObjectPadding,
        selectors::{Selector, SelectorTable},
    },
    utils::{
        errors::OtherError,
        mem_reader::{FromFixedBytes, MemReader},
    },
};

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
    ) -> Result<Self, OtherError>
    where
        M: MemReader + 'a,
    {
        let var_selector_offfset = obj_data[2];
        let method_record_offset = obj_data[3];
        let padding = obj_data[4];
        if padding != 0 {
            return Err(OtherError::new(BadObjectPadding));
        }

        let mut var_selectors = loaded_data.sub_reader_range(
            "Var selector table",
            var_selector_offfset as usize..method_record_offset as usize,
        )?;

        let property_ids = var_selectors.split_values::<u16>("Property IDs")?;

        let mut method_record_remainder = loaded_data
            .sub_reader_range("Method record remainder", method_record_offset as usize..)?;

        let mut method_records =
            method_record_remainder.read_length_delimited_block("Method records", 4)?;
        let method_records = method_records.split_values::<MethodRecord>("Method records")?;

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
