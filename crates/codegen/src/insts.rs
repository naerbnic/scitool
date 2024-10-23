//! The definition of instruction traits, and types.

use crate::{args::ArgsWidth, opcode::Opcode, writer::BytecodeWriter};

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
    fn write_inst<W: std::io::Write>(&self, arg_width: ArgsWidth, buf: W) -> anyhow::Result<()>;
}

pub trait AsmInst<T>: InstBase {
    /// Writes the entire instruction to the buffer, including the opcode byte. This
    /// may also include relocation information.
    fn write_inst<Sym, W: BytecodeWriter<Sym, T>>(
        &self,
        arg_width: ArgsWidth,
        buf: W,
    ) -> anyhow::Result<()>;
}

pub struct SizedInst<K> {
    inst: K,
    args_width: ArgsWidth,
}

pub struct SizedAsmInst<K, T> {
    inst: K,
    args_width: ArgsWidth,
    _phantom: std::marker::PhantomData<T>,
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

    pub fn write_inst<W: std::io::Write>(&self, buf: W) -> anyhow::Result<()> {
        self.inst.write_inst(self.args_width, buf)
    }
}

impl<K, T> SizedAsmInst<K, T>
where
    K: AsmInst<T>,
{
    pub fn new(inst: K, args_width: ArgsWidth) -> Self {
        SizedAsmInst {
            inst,
            args_width,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn opcode(&self) -> <K as InstBase>::Opcode {
        self.inst.opcode()
    }

    pub fn write_inst<Sym, W: BytecodeWriter<Sym, T>>(&self, buf: W) -> anyhow::Result<()> {
        self.inst.write_inst(self.args_width, buf)
    }
}
