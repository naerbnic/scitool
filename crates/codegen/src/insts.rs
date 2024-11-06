//! The definition of instruction traits, and types.

use sci_utils::reloc_buffer::writer::RelocWriter;

use crate::{args::ArgsWidth, opcode::Opcode};

pub trait InstBase {
    type Opcode: Opcode;
    fn opcode(&self) -> Self::Opcode;
    /// Get the size of this instruction in bytes for the given argument width.
    fn byte_size(&self, arg_width: ArgsWidth) -> usize;
}

/// Kinds for instructions that can be written directly to a data buffer, without
/// further resolution.
pub trait Inst: InstBase {
    /// Writes the entire instruction to the buffer, including the opcode byte.
    fn write_inst<W: std::io::Write>(
        &self,
        arg_width: ArgsWidth,
        buf: &mut W,
    ) -> anyhow::Result<()>;
}

pub trait AsmInst: InstBase {
    /// Writes the entire instruction to the buffer, including the opcode byte. This
    /// may also include relocation information.
    fn write_inst<W: RelocWriter>(&self, arg_width: ArgsWidth, buf: &mut W) -> anyhow::Result<()>;
}

pub struct SizedInst<K> {
    inst: K,
    args_width: ArgsWidth,
}

pub struct SizedAsmInst<K> {
    inst: K,
    args_width: ArgsWidth,
}

impl<K> SizedInst<K>
where
    K: Inst,
{
    pub fn new(inst: K, args_width: ArgsWidth) -> Self {
        SizedInst { inst, args_width }
    }

    pub fn opcode(&self) -> K::Opcode {
        self.inst.opcode()
    }

    pub fn write_inst<W: std::io::Write>(&self, buf: &mut W) -> anyhow::Result<()> {
        self.inst.write_inst(self.args_width, buf)
    }
}

impl<K> SizedAsmInst<K>
where
    K: AsmInst,
{
    pub fn new(inst: K, args_width: ArgsWidth) -> Self {
        SizedAsmInst { inst, args_width }
    }

    pub fn opcode(&self) -> <K as InstBase>::Opcode {
        self.inst.opcode()
    }

    pub fn write_inst<W: RelocWriter>(&self, buf: &mut W) -> anyhow::Result<()> {
        self.inst.write_inst(self.args_width, buf)
    }
}
