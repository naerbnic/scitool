//! This data represents the set of classes that are available in the game,
//! along with the script that they are contained in.

use sci_utils::buffer::{Buffer, FromFixedBytes};

struct RawClassSpeciesEntry {
    script_id: u16,
}

impl FromFixedBytes for RawClassSpeciesEntry {
    const SIZE: usize = 4;
    fn parse(bytes: &[u8]) -> anyhow::Result<Self> {
        let zero_value = u16::parse(&bytes[..2])?;
        anyhow::ensure!(zero_value == 0, "Expected zero value, got {}", zero_value);
        let script_id = u16::parse(&bytes[2..][..2])?;
        Ok(Self { script_id })
    }
}

#[derive(Clone, Copy, Debug)]
struct ClassSpeciesEntry {
    species_id: u16,
    script_id: u16,
}

#[derive(Clone, Debug)]
pub struct ClassSpeciesTable {
    species_entries: Vec<ClassSpeciesEntry>,
}

impl ClassSpeciesTable {
    pub fn load_from<'a, B: Buffer<'a, Idx = u16> + Clone>(data: B) -> anyhow::Result<Self> {
        let species_entries = data.split_values::<RawClassSpeciesEntry>()?;

        Ok(Self {
            species_entries: species_entries
                .into_iter()
                .enumerate()
                .map(|(id, raw_entry)| ClassSpeciesEntry {
                    species_id: id.try_into().unwrap(),
                    script_id: raw_entry.script_id,
                })
                .collect(),
        })
    }
}
