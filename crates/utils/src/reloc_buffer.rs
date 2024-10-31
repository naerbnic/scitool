pub mod expr;
pub mod writer;

use std::collections::{btree_map, BTreeMap};

use expr::Expr;
use writer::RelocWriter;

use crate::{buffer::ToFixedBytes, numbers::bit_convert::NumConvert as _};

#[derive(Clone, Copy, Debug)]
pub enum RelocSize {
    I8,
    I16,
}

impl RelocSize {
    pub fn byte_size(&self) -> usize {
        match self {
            RelocSize::I8 => 1,
            RelocSize::I16 => 2,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum RelocType {
    /// The relocation should be written as an absolute address (independent
    /// of the address of the relocation).
    Absolute,
    /// The relocation should be written as a relative address (Subtracting
    /// the address of the relocation from the target address).
    Relative,
}

impl RelocType {
    pub fn apply(&self, offset: u16, target: u16) -> u16 {
        match self {
            RelocType::Absolute => target,
            RelocType::Relative => target.wrapping_sub(offset),
        }
    }
}

#[derive(Clone, Debug)]
struct Relocation<Ext, Sym> {
    expr: expr::Expr<Ext, Sym>,
    pos: usize,
    size: RelocSize,
    reloc_type: RelocType,
}

impl<Ext, Sym> Relocation<Ext, Sym>
where
    Ext: Clone,
    Sym: Clone,
{
    fn write_value_to_slice(&self, value: i64, data: &mut [u8]) -> anyhow::Result<()> {
        match (self.reloc_type, self.size) {
            (RelocType::Absolute, RelocSize::I8) => {
                let byte_value: u8 = value.convert_num_to()?;
                ToFixedBytes::to_bytes(&byte_value, &mut data[self.pos..][..1])?;
            }
            (RelocType::Absolute, RelocSize::I16) => {
                let word_value: u16 = value.convert_num_to()?;
                ToFixedBytes::to_bytes(&word_value, &mut data[self.pos..][..2])?;
            }
            (RelocType::Relative, RelocSize::I8) => {
                let byte_value: i8 = value.convert_num_to()?;
                ToFixedBytes::to_bytes(&byte_value, &mut data[self.pos..][..1])?;
            }
            (RelocType::Relative, RelocSize::I16) => {
                let word_value: i16 = value.convert_num_to()?;
                ToFixedBytes::to_bytes(&word_value, &mut data[self.pos..][..2])?;
            }
        }
        Ok(())
    }
    // Either resolves this relocation in place, or returns a new relocation
    pub fn partial_resolve<R: LocalResolver<Sym>>(
        &self,
        resolver: &R,
        data: &mut [u8],
    ) -> anyhow::Result<Option<Relocation<Ext, Sym>>> {
        if let Some(new_expr) = self.expr.partial_local_resolve(self.pos, resolver) {
            let Some(value) = new_expr.exact_value() else {
                // We can't fully simplify this expression yet. Return what we have.
                return Ok(Some(Relocation {
                    expr: new_expr,
                    ..self.clone()
                }));
            };
            self.write_value_to_slice(value, data)?;
        }
        Ok(None)
    }

    pub fn full_resolve<R>(&self, full_resolver: &R, data: &mut [u8]) -> anyhow::Result<()>
    where
        Sym: std::fmt::Debug,
        R: FullResolver<Ext, Sym>,
    {
        let value = self.expr.full_resolve(self.pos, full_resolver)?;
        self.write_value_to_slice(value, data)
    }

    pub fn with_added_offset(self, offset: usize) -> Self {
        Self {
            pos: self.pos + offset,
            ..self
        }
    }
}

pub enum RelocResult<TargetSymbol> {
    /// The relocation was successful. Returns the target address.
    Success(u16),
    /// The relocation was bypassed, creating a target relocation.
    Modified(TargetSymbol),
}

/// A symbol resolver for external symbols.
pub trait ExternalResolver<Ext> {
    /// Resolves an external symbol to an address. The address is expected to
    /// be a numeric value that can safely be converted to an `i64`.
    fn resolve(&self, expr: &Ext) -> anyhow::Result<i64>;
}

trait LocalResolver<Sym> {
    fn resolve_local_symbol(&self, symbol: &Sym) -> Option<i64>;
}

pub struct LocalOnlyResolver<'a, Sym> {
    symbols: &'a BTreeMap<Sym, usize>,
}

impl<Sym> LocalResolver<Sym> for LocalOnlyResolver<'_, Sym>
where
    Sym: Ord,
{
    fn resolve_local_symbol(&self, symbol: &Sym) -> Option<i64> {
        self.symbols
            .get(symbol)
            .copied()
            .map(|x| x.convert_num_to().unwrap())
    }
}

trait FullResolver<Ext, Sym>: ExternalResolver<Ext> + LocalResolver<Sym> {}

struct FullResolverImpl<'a, Ext, Sym, R> {
    external: &'a R,
    local: &'a BTreeMap<Sym, usize>,
    _phantom: std::marker::PhantomData<Ext>,
}

impl<'a, Ext, Sym, R> LocalResolver<Sym> for FullResolverImpl<'a, Ext, Sym, R>
where
    Sym: Ord,
    R: ExternalResolver<Ext>,
{
    fn resolve_local_symbol(&self, symbol: &Sym) -> Option<i64> {
        self.local
            .get(symbol)
            .copied()
            .map(|x| x.convert_num_to().unwrap())
    }
}

impl<'a, Ext, Sym, R> ExternalResolver<Ext> for FullResolverImpl<'a, Ext, Sym, R>
where
    Sym: Ord,
    R: ExternalResolver<Ext>,
{
    fn resolve(&self, expr: &Ext) -> anyhow::Result<i64> {
        self.external.resolve(expr)
    }
}

impl<'a, Ext, Sym, R> FullResolver<Ext, Sym> for FullResolverImpl<'a, Ext, Sym, R>
where
    Sym: Ord,
    R: ExternalResolver<Ext>,
{
}

/// Represents an unlinked section of the object code.
#[derive(Clone, Debug)]
pub struct RelocatableBuffer<Ext, Sym> {
    /// The data in this section.
    data: Vec<u8>,
    /// The list of symbols defined in this section. Keys are symbol names,
    /// and values are offsets in `self.data` that map to that symbol.
    symbols: BTreeMap<Sym, usize>,
    /// The relocations that have to happen in this section before being
    /// fully linked.
    relocations: Vec<Relocation<Ext, Sym>>,
    /// The overall byte alignment of this section.
    alignment: usize,
}

impl<Ext, Sym> RelocatableBuffer<Ext, Sym>
where
    Sym: Ord + Clone + std::fmt::Debug,
    Ext: Clone,
{
    fn new(alignment: usize) -> Self {
        Self {
            data: Vec::new(),
            symbols: BTreeMap::new(),
            relocations: Vec::new(),
            alignment,
        }
    }

    pub fn from_vec(data: Vec<u8>, alignment: usize) -> Self {
        Self {
            data,
            symbols: BTreeMap::new(),
            relocations: Vec::new(),
            alignment,
        }
    }

    pub fn local_resolve(mut self) -> anyhow::Result<RelocatableBuffer<Ext, Sym>> {
        let resolver = LocalOnlyResolver {
            symbols: &self.symbols,
        };
        let mut errors = Vec::new();
        let mut new_relocs = Vec::new();
        for reloc in &self.relocations {
            match reloc.partial_resolve(&resolver, &mut self.data) {
                Ok(Some(new_reloc)) => new_relocs.push(new_reloc),
                Ok(None) => {}
                Err(err) => errors.push(err),
            }
        }
        if !errors.is_empty() {
            anyhow::bail!("relocation errors: {:?}", errors);
        }
        Ok(RelocatableBuffer {
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

    pub fn resolve_all<R: ExternalResolver<Ext>>(
        mut self,
        resolver: &R,
    ) -> anyhow::Result<Vec<u8>> {
        let full_resolver = FullResolverImpl {
            external: resolver,
            local: &self.symbols,
            _phantom: std::marker::PhantomData,
        };
        for reloc in &self.relocations {
            reloc.full_resolve(&full_resolver, &mut self.data)?;
        }
        Ok(self.data)
    }
}

pub struct SectionBuilder<Ext, Sym> {
    section: RelocatableBuffer<Ext, Sym>,
}

impl<Ext, Sym> SectionBuilder<Ext, Sym>
where
    Ext: Clone,
    Sym: Ord + Clone + std::fmt::Debug,
{
    pub fn new() -> Self {
        Self {
            section: RelocatableBuffer::new(1),
        }
    }

    pub fn build(self) -> RelocatableBuffer<Ext, Sym> {
        self.section
    }
}

impl<Ext, Sym> Default for SectionBuilder<Ext, Sym>
where
    Ext: Clone,
    Sym: Ord + Clone + std::fmt::Debug,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<Ext, Sym> RelocWriter<Ext, Sym> for SectionBuilder<Ext, Sym>
where
    Ext: Clone,
    Sym: Ord + Clone,
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

    fn mark_symbol(&mut self, symbol: Sym) {
        self.section.symbols.insert(symbol, self.section.data.len());
    }

    fn add_reloc(&mut self, reloc_type: RelocType, size: RelocSize, expr: Expr<Ext, Sym>) {
        let pos = self.section.data.len();
        match size {
            RelocSize::I8 => self.write_u8(0),
            RelocSize::I16 => self.write_u16_le(0),
        }
        self.section.relocations.push(Relocation {
            expr,
            pos,
            size,
            reloc_type,
        });
    }
}

#[cfg(test)]
mod tests {
    use crate::reloc_buffer::expr::Expr;
    use crate::reloc_buffer::RelocSize;
    use crate::reloc_buffer::RelocType;

    use super::writer::RelocWriter;
    use super::RelocatableBuffer;
    use super::SectionBuilder;

    struct NullExternalResolver;

    impl super::ExternalResolver<()> for NullExternalResolver {
        fn resolve(&self, _: &()) -> anyhow::Result<i64> {
            anyhow::bail!("no external symbols should be resolved")
        }
    }

    #[test]
    fn can_build_empty_buffer() -> anyhow::Result<()> {
        let buffer: RelocatableBuffer<(), ()> = SectionBuilder::new().build();
        buffer.resolve_all(&NullExternalResolver)?;
        Ok(())
    }

    #[test]
    fn can_build_no_symbol_buffer() -> anyhow::Result<()> {
        let mut writer = SectionBuilder::new();
        writer.write_u8(0);
        writer.write_u16_le(0x1234);

        let buffer: RelocatableBuffer<(), ()> = writer.build();
        let data = buffer.resolve_all(&NullExternalResolver)?;
        assert_eq!(data, vec![0x00, 0x34, 0x12]);
        Ok(())
    }

    #[test]
    fn can_build_simple_symbol() -> anyhow::Result<()> {
        let mut writer = SectionBuilder::new();
        writer.add_reloc(RelocType::Absolute, RelocSize::I16, Expr::new_local("abc"));
        writer.mark_symbol("abc");

        let buffer: RelocatableBuffer<(), &'static str> = writer.build();
        let data = buffer.resolve_all(&NullExternalResolver)?;
        assert_eq!(data, vec![0x02, 0x00]);
        Ok(())
    }

    #[test]
    fn merge_advances_addresses() -> anyhow::Result<()> {
        let buffer1 = RelocatableBuffer::from_vec(vec![0x01, 0x02], 1);

        let mut writer = SectionBuilder::new();
        writer.add_reloc(RelocType::Absolute, RelocSize::I16, Expr::new_local("abc"));
        writer.mark_symbol("abc");
        let buffer2: RelocatableBuffer<(), &'static str> = writer.build();

        let buffer = buffer1.merge(buffer2);
        let data = buffer.resolve_all(&NullExternalResolver)?;
        assert_eq!(data, vec![0x01, 0x02, 0x04, 0x00]);
        Ok(())
    }

    #[test]
    fn relative_address_is_resolved_partially() -> anyhow::Result<()> {
        let mut writer = SectionBuilder::new();
        writer.mark_symbol("a");
        writer.add_reloc(
            RelocType::Relative,
            RelocSize::I16,
            Expr::new_subtract(Expr::new_local("b"), Expr::new_local("a")),
        );
        writer.mark_symbol("b");

        let buffer: RelocatableBuffer<(), &'static str> = writer.build();
        let buffer = buffer.local_resolve()?;
        assert!(buffer.relocations.is_empty());
        assert_eq!(buffer.data, vec![0x02, 0x00]);
        Ok(())
    }

    #[test]
    fn negative_relative_addresses_resolve_partially() -> anyhow::Result<()> {
        let mut writer = SectionBuilder::new();
        writer.mark_symbol("a");
        writer.add_reloc(
            RelocType::Relative,
            RelocSize::I16,
            Expr::new_subtract(Expr::new_local("a"), Expr::new_local("b")),
        );
        writer.mark_symbol("b");

        let buffer: RelocatableBuffer<(), &'static str> = writer.build();
        let buffer = buffer.local_resolve()?;
        assert!(buffer.relocations.is_empty());
        assert_eq!(buffer.data, vec![0xFE, 0xFF]);
        Ok(())
    }
}
