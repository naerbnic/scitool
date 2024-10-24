use std::collections::{btree_map, BTreeMap};

use crate::{
    numbers::safe_signed_narrow,
    reloc::{RelocSize, RelocType},
    writer::BytecodeWriter,
};

#[derive(Clone, Copy, Debug)]
struct Relocation<RelocSymbol> {
    reloc_type: RelocType,
    offset: usize,
    size: RelocSize,
    symbol: RelocSymbol,
}

impl<RelocSymbol> Relocation<RelocSymbol> {
    // Either resolves this relocation in place, or returns a new relocation
    pub fn resolve<R: RelocResolver<RelocSymbol>>(
        &self,
        resolver: &R,
        data: &mut [u8],
    ) -> anyhow::Result<Option<Relocation<R::TargetSymbol>>> {
        let target_address: u16 = match resolver.resolve(&self.symbol)? {
            RelocResult::Success(target_address) => target_address,
            RelocResult::Modified(new_symbol) => {
                return Ok(Some(Relocation {
                    reloc_type: self.reloc_type,
                    offset: self.offset,
                    size: self.size,
                    symbol: new_symbol,
                }))
            }
        };

        match self.size {
            RelocSize::Byte => {
                let offset = data[self.offset] as i8 as i16 as u16;
                let modified_value = self.reloc_type.apply(offset, target_address);
                data[self.offset] = safe_signed_narrow(modified_value)?;
            }
            RelocSize::Word => {
                let data_slice: &mut [u8; 2] = (&mut data[self.offset..][..2]).try_into()?;
                let offset = u16::from_le_bytes(*data_slice);
                let modified_value = self.reloc_type.apply(offset, target_address);
                *data_slice = modified_value.to_le_bytes();
            }
        }
        Ok(None)
    }

    pub fn with_added_offset(self, offset: usize) -> Self {
        Self {
            offset: self.offset + offset,
            ..self
        }
    }
}

/// Represents an unlinked section of the object code.
#[derive(Clone, Debug)]
pub struct Section<TargetSymbol, RelocSymbol> {
    /// The data in this section.
    data: Vec<u8>,
    /// The list of symbols defined in this section. Keys are symbol names,
    /// and values are offsets in `self.data` that map to that symbol.
    symbols: BTreeMap<TargetSymbol, usize>,
    /// The relocations that have to happen in this section before being
    /// fully linked.
    relocations: Vec<Relocation<RelocSymbol>>,
    /// The overall byte alignment of this section.
    alignment: usize,
}

pub enum RelocResult<TargetSymbol> {
    /// The relocation was successful. Returns the target address.
    Success(u16),
    /// The relocation was bypassed, creating a target relocation.
    Modified(TargetSymbol),
}

pub trait RelocResolver<SourceSymbol> {
    /// The type of resolver that will be output, if a resolver is bypassed.
    ///
    /// This is useful when only a subset of relocations are supported, e.g.
    /// a resolver that resolves local branches, but not global symbols.
    type TargetSymbol;

    /// Resolve a single relocation entry.
    fn resolve(&self, symbol: &SourceSymbol) -> anyhow::Result<RelocResult<Self::TargetSymbol>>;
}

impl<SymbolT, RelocT> Section<SymbolT, RelocT>
where
    SymbolT: Ord + std::fmt::Debug,
{
    pub fn new(alignment: usize) -> Self {
        Self {
            data: Vec::new(),
            symbols: BTreeMap::new(),
            relocations: Vec::new(),
            alignment,
        }
    }

    pub fn resolve_into<ResolverT: RelocResolver<RelocT>>(
        mut self,
        resolver: &ResolverT,
    ) -> anyhow::Result<Section<SymbolT, ResolverT::TargetSymbol>> {
        let mut errors = Vec::new();
        let mut new_relocs = Vec::new();
        for reloc in &self.relocations {
            match reloc.resolve(resolver, &mut self.data) {
                Ok(Some(new_reloc)) => new_relocs.push(new_reloc),
                Ok(None) => {}
                Err(err) => errors.push(err),
            }
        }
        if !errors.is_empty() {
            anyhow::bail!("relocation errors: {:?}", errors);
        }
        Ok(Section {
            data: self.data,
            symbols: self.symbols,
            relocations: new_relocs,
            alignment: self.alignment,
        })
    }

    /// Merge two sections together, concatenating their data and
    /// ensuring all symbols and reloc entries are valid.
    pub fn merge(self, other: Self) -> Self {
        let mut data = self.data;
        let mut symbols = self.symbols;
        let mut relocations = self.relocations;
        let alignment = self.alignment.max(other.alignment);

        if self.alignment < other.alignment {
            let padding = other.alignment - (data.len() % other.alignment);
            data.extend(std::iter::repeat(0).take(padding));
        }

        // The self section is now aligned, with sufficient padding to
        // match the other section's alignment. The resulting sections will
        // simply append the data.

        let other_offset = data.len();
        data.extend(other.data);
        for (symbol, symbol_offset) in other.symbols {
            let new_offset = symbol_offset + other_offset;
            match symbols.entry(symbol) {
                btree_map::Entry::Vacant(entry) => {
                    entry.insert(new_offset);
                }
                btree_map::Entry::Occupied(entry) => {
                    panic!(
                        "symbol {:?} already defined at offset {} (new offset: {})",
                        entry.key(),
                        entry.get(),
                        new_offset
                    );
                }
            }
        }
        relocations.extend(
            other
                .relocations
                .into_iter()
                .map(|r| r.with_added_offset(other_offset)),
        );

        Self {
            data,
            symbols,
            relocations,
            alignment,
        }
    }
}

pub struct SectionBuilder<TargetSymbol, RelocSymbol> {
    section: Section<TargetSymbol, RelocSymbol>,
}

impl<TargetSymbol, RelocSymbol> SectionBuilder<TargetSymbol, RelocSymbol>
where
    TargetSymbol: Ord,
{
    pub fn new() -> Self {
        Self {
            section: Section {
                data: Vec::new(),
                symbols: BTreeMap::new(),
                relocations: Vec::new(),
                alignment: 1,
            },
        }
    }

    pub fn build(self) -> Section<TargetSymbol, RelocSymbol> {
        self.section
    }
}

impl<TargetSymbol, RelocSymbol> Default for SectionBuilder<TargetSymbol, RelocSymbol>
where
    TargetSymbol: Ord,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<TargetSymbol, RelocSymbol> BytecodeWriter<TargetSymbol, RelocSymbol>
    for SectionBuilder<TargetSymbol, RelocSymbol>
where
    TargetSymbol: Ord,
{
    fn write_u8(&mut self, value: u8) {
        self.section.data.push(value);
    }

    fn write_u16_le(&mut self, value: u16) {
        self.section.data.extend(&value.to_le_bytes());
    }

    fn align(&mut self, alignment: usize) {
        // Update the alignment of the section, to the minimum necessary.
        if self.section.alignment < alignment {
            self.section.alignment = alignment;
        }

        // We still align the section to the requested alignment, even if
        // the section is already aligned to a higher value.
        let padding = alignment - (self.section.data.len() % alignment);
        if padding != alignment {
            self.section.data.extend(std::iter::repeat(0).take(padding));
        }
    }

    fn mark_symbol(&mut self, symbol: TargetSymbol) {
        self.section.symbols.insert(symbol, self.section.data.len());
    }

    fn add_byte_reloc(&mut self, reloc_type: RelocType, offset: u8, reloc: RelocSymbol) {
        let pos = self.section.data.len();
        self.write_u8(offset);
        self.section.relocations.push(Relocation {
            reloc_type,
            offset: pos,
            size: RelocSize::Byte,
            symbol: reloc,
        });
    }

    fn add_word_reloc(&mut self, reloc_type: RelocType, offset: u16, reloc: RelocSymbol) {
        let pos = self.section.data.len();
        self.write_u16_le(offset);
        self.section.relocations.push(Relocation {
            reloc_type,
            offset: pos,
            size: RelocSize::Word,
            symbol: reloc,
        });
    }
}
