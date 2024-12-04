//! Traits for writing to a relocatable buffer.
use crate::symbol::Symbol;

use super::{expr::Expr, RelocSize, RelocType};

/// A trait that allows a relocatable buffer to be written to.
pub trait RelocWriter {
    /// Writes a single byte to the output.
    fn write_u8(&mut self, value: u8);
    /// Writes a 16-bit little-endian value to the output. No alignment is done
    fn write_u16_le(&mut self, value: u16);
    /// Write bytes directly to the output. No alignment is done.
    fn write_bytes(&mut self, as_bytes: &[u8]);
    /// Aligns the current position to the given alignment. If padding is needed,
    /// it is filled with zeroes.
    fn align(&mut self, alignment: usize);
    /// Marks the current position with a symbol.
    fn mark_symbol(&mut self, symbol: Symbol);
    /// Adds a relocation entry to the current position. It advances the current
    /// position by the size of the relocation.
    fn add_reloc(&mut self, reloc_type: RelocType, reloc_size: RelocSize, reloc: Expr);
}
