use crate::reloc::RelocType;

pub trait BytecodeWriter<SymbolT, RelocT> {
    // Writes a single byte to the output.
    fn write_u8(&mut self, value: u8);
    // Writes a 16-bit little-endian value to the output. No alignment is done
    fn write_u16_le(&mut self, value: u16);
    // Aligns the current position to the given alignment. If padding is needed,
    // it is filled with zeroes.
    fn align(&mut self, alignment: usize);
    // Marks the current position with a symbol.
    fn mark_symbol(&mut self, symbol: SymbolT);
    // Adds a relocation entry to the current position. It advances the current
    // position by the size of the relocation.
    fn add_byte_reloc(&mut self, reloc_type: RelocType, offset: u8, reloc: RelocT);
    // Adds a relocation entry to the current position. It advances the current
    // position by the size of the relocation.
    fn add_word_reloc(&mut self, reloc_type: RelocType, offset: u16, reloc: RelocT);
}
