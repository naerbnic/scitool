//! The definition of instruction traits, and types.

use sci_utils::reloc_buffer::writer::{RelocWriter, SymbolGenerator};

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
    fn write_inst<W: std::io::Write>(&self, arg_width: ArgsWidth, buf: &mut W) -> anyhow::Result<()>;
}

pub trait AsmInst<Ext, Sym>: InstBase {
    /// Writes the entire instruction to the buffer, including the opcode byte. This
    /// may also include relocation information.
    fn write_inst<G: SymbolGenerator<Sym>, W: RelocWriter<Ext, Sym>>(
        &self,
        sym_gen: &mut G,
        arg_width: ArgsWidth,
        buf: &mut W,
    ) -> anyhow::Result<()>
    where
        Sym: Clone;
}

pub struct SizedInst<K> {
    inst: K,
    args_width: ArgsWidth,
}

pub struct SizedAsmInst<K, Ext, Sym> {
    inst: K,
    args_width: ArgsWidth,
    _phantom: std::marker::PhantomData<(Ext, Sym)>,
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

impl<K, Ext, Sym> SizedAsmInst<K, Ext, Sym>
where
    K: AsmInst<Ext, Sym>,
    Sym: Clone,
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

    pub fn write_inst<G: SymbolGenerator<Sym>, W: RelocWriter<Ext, Sym>>(
        &self,
        sym_gen: &mut G,
        buf: &mut W,
    ) -> anyhow::Result<()> {
        self.inst.write_inst(sym_gen, self.args_width, buf)
    }
}
