

#[derive(Clone, Copy, Debug)]
pub enum RelocSize {
    Byte,
    Word,
}

impl RelocSize {
    pub fn byte_size(&self) -> usize {
        match self {
            RelocSize::Byte => 1,
            RelocSize::Word => 2,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RelocPosition {
    offset: usize,
    size: RelocSize,
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