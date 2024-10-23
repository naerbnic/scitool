pub mod var_access;

use std::borrow::Cow;

use crate::insts::Inst;

// All literal byte opcodes on the PMachine consist of seven bits of opcode
// kind, and one bit of arg width. The arg width determines if some of the
// instruction arguments are 8 or 16 bits wide.

pub trait Opcode: Sized {
    /// Returns the opcode for this instruction. This ignores the low bit of the
    /// opcode byte, which is the arg width.
    fn from_opcode_byte(opcode: u8) -> anyhow::Result<Option<Self>>;
    /// Returns the opcode byte for this instruction. The low bit is not set.
    fn opcode_byte(&self) -> u8;
    // Returns the name of the opcode.
    fn opcode_name(&self) -> Cow<str>;
}

/// An instruction set is a collection of opcodes and their corresponding
/// instructions.
pub trait InstSet {
    type Opcode: Opcode;
    type Inst: Inst<Opcode = Self::Opcode>;

    fn parse_inst<R: std::io::Read + std::io::Seek>(&self, buf: R) -> anyhow::Result<Self::Inst>;
}
