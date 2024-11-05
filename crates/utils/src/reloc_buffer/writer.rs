use super::{expr::Expr, RelocSize, RelocType};

pub trait SymbolGenerator<Sym> {
    /// Generates a new symbol.
    fn generate(&mut self) -> Sym;
}

pub trait RelocWriter<Ext, Sym> {
    /// Writes a single byte to the output.
    fn write_u8(&mut self, value: u8);
    /// Writes a 16-bit little-endian value to the output. No alignment is done
    fn write_u16_le(&mut self, value: u16);
    /// Aligns the current position to the given alignment. If padding is needed,
    /// it is filled with zeroes.
    fn align(&mut self, alignment: usize);
    /// Marks the current position with a symbol.
    fn mark_symbol(&mut self, symbol: Sym);
    /// Adds a relocation entry to the current position. It advances the current
    /// position by the size of the relocation.
    fn add_reloc(&mut self, reloc_type: RelocType, reloc_size: RelocSize, reloc: Expr<Ext, Sym>);
}
