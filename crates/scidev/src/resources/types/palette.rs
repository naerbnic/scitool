use std::collections::BTreeMap;

use scidev_macros_internal::other_fn;

use crate::utils::{
    buffer::{Buffer, BufferExt as _, SplittableBuffer},
    errors::{OpaqueError, bail_other},
    mem_reader::{BufferMemReader, MemReader as _},
};

#[derive(Clone, Copy, Debug)]
enum PaletteFormat {
    Variable,
    Constant,
}

#[derive(Debug)]
pub struct PaletteEntry {
    r: u8,
    g: u8,
    b: u8,
}

impl PaletteEntry {
    #[must_use]
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    #[must_use]
    pub fn red(&self) -> u8 {
        self.r
    }

    #[must_use]
    pub fn green(&self) -> u8 {
        self.g
    }

    #[must_use]
    pub fn blue(&self) -> u8 {
        self.b
    }
}

#[derive(Debug)]
pub struct Palette {
    mapping: BTreeMap<u8, PaletteEntry>,
    first_color: u8,
    last_color: u8,
}

impl Palette {
    #[other_fn]
    pub fn from_data<B>(data: B) -> Result<Self, OpaqueError>
    where
        B: SplittableBuffer,
    {
        if data.size() < 37 {
            bail_other!("Palette data is too small");
        }
        let data0 = data.read_at::<u8>(0);
        let data1 = data.read_at::<u8>(1);
        let data29 = data.read_at::<u16>(29);
        let block;
        let format;
        let color_start;
        let color_count;
        if (data0 == 0 && data1 == 1) || (data0 == 0 && data1 == 0 && data29 != 0) {
            block = data.sub_buffer(260usize..);
            format = PaletteFormat::Variable;
            color_start = 0;
            color_count = 256;
        } else {
            block = data.sub_buffer(37usize..);
            format = match data.read_at::<u8>(32) {
                0 => PaletteFormat::Variable,
                1 => PaletteFormat::Constant,
                _ => bail_other!("Invalid palette format"),
            };
            color_start = data.read_at::<u8>(25);
            color_count = data.read_at::<u16>(29);
        }

        Ok(Self::from_params(&block, format, color_start, color_count)?)
    }

    #[other_fn]
    fn from_params<B>(
        pal_data: &B,
        format: PaletteFormat,
        color_start: u8,
        color_count: u16,
    ) -> Result<Self, OpaqueError>
    where
        B: Buffer,
    {
        let mut reader = BufferMemReader::new(pal_data.as_fallible());
        let mut mapping = BTreeMap::new();
        for i in 0..color_count {
            let used = if let PaletteFormat::Variable = format {
                reader.read_u8()? != 0
            } else {
                true
            };
            let r = reader.read_u8()?;
            let g = reader.read_u8()?;
            let b = reader.read_u8()?;

            if !used {
                continue;
            }

            mapping.insert(
                u8::try_from(i).unwrap() + color_start,
                PaletteEntry { r, g, b },
            );
        }
        Ok(Self {
            mapping,
            first_color: color_start,
            last_color: color_start + u8::try_from(color_count - 1).unwrap(),
        })
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.mapping.is_empty()
    }

    #[must_use]
    pub fn range(&self) -> std::ops::RangeInclusive<u8> {
        self.first_color..=self.last_color
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.range().len()
    }

    #[must_use]
    pub fn get(&self, index: u8) -> Option<&PaletteEntry> {
        self.mapping.get(&index)
    }

    #[must_use]
    pub fn first_color(&self) -> u8 {
        self.first_color
    }

    #[must_use]
    pub fn last_color(&self) -> u8 {
        self.last_color
    }
}
