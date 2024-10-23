use crate::{
    numbers::{
        read_byte, read_word, safe_signed_narrow, safe_unsigned_narrow, signed_extend_byte,
        unsigned_extend_byte, write_byte, write_word,
    },
    reloc::RelocType,
    writer::BytecodeWriter,
};

#[derive(Clone, Copy, Debug)]
pub enum ArgsWidth {
    Byte,
    Word,
}

pub trait InstArg: Sized {
    fn byte_size(inst_args_width: ArgsWidth) -> usize;
    fn read_arg<R: std::io::Read + std::io::Seek>(
        inst_args_width: ArgsWidth,
        buf: R,
    ) -> anyhow::Result<Self>;

    fn write_arg<W: std::io::Write>(
        &self,
        inst_args_width: ArgsWidth,
        buf: W,
    ) -> anyhow::Result<()>;
}

/// An instruction argument created by the assembler, or during compilation.
/// This value may not have the final value, and can write a relocation action
/// to the output.
pub trait InstAsmArg<RelocSymbol>: Sized {
    /// The InstArg that this instruction argument will be converted to after
    /// relocation.
    ///
    /// This is used in the instruction set generation to determine the raw
    /// arg type for the instruction.
    type InstArg: InstArg;

    /// Writes the argument to the output. If needed, writes a relocation entry
    /// to the output.
    fn write_arg<SourceSymbol, W: BytecodeWriter<SourceSymbol, RelocSymbol>>(
        &self,
        inst_args_width: ArgsWidth,
        writer: &mut W,
    ) -> anyhow::Result<()>;
}

/// A variable length unsigned word. When extending from a byte, no sign extension is
/// done. When converted to a byte, the high byte must be 0.
#[derive(Clone, Copy, Debug)]
pub struct VarUWord(u16);

impl InstArg for VarUWord {
    fn byte_size(inst_args_width: ArgsWidth) -> usize {
        match inst_args_width {
            ArgsWidth::Byte => 1,
            ArgsWidth::Word => 2,
        }
    }

    fn read_arg<R: std::io::Read + std::io::Seek>(
        inst_args_width: ArgsWidth,
        buf: R,
    ) -> anyhow::Result<Self> {
        match inst_args_width {
            ArgsWidth::Byte => Ok(VarUWord(unsigned_extend_byte(read_byte(buf)?))),
            ArgsWidth::Word => Ok(VarUWord(read_word(buf)?)),
        }
    }

    fn write_arg<W: std::io::Write>(
        &self,
        inst_args_width: ArgsWidth,
        buf: W,
    ) -> anyhow::Result<()> {
        match inst_args_width {
            ArgsWidth::Byte => {
                write_byte(buf, safe_unsigned_narrow(self.0)?)?;
            }
            ArgsWidth::Word => {
                write_word(buf, self.0)?;
            }
        }
        Ok(())
    }
}

impl<T> InstAsmArg<T> for VarUWord {
    type InstArg = VarUWord;

    fn write_arg<SourceSymbol, W: BytecodeWriter<SourceSymbol, T>>(
        &self,
        inst_args_width: ArgsWidth,
        writer: &mut W,
    ) -> anyhow::Result<()> {
        match inst_args_width {
            ArgsWidth::Byte => writer.write_u8(safe_unsigned_narrow(self.0)?),
            ArgsWidth::Word => writer.write_u16_le(self.0),
        }
        Ok(())
    }
}

/// A variable length Word. When extending from a byte, sign extension is
/// done. When converted to a byte, all of the bits of the high byte must match
/// the sign bit of the lower byte.
#[derive(Clone, Copy, Debug)]
pub struct VarSWord(u16);

impl InstArg for VarSWord {
    fn byte_size(inst_args_width: ArgsWidth) -> usize {
        match inst_args_width {
            ArgsWidth::Byte => 1,
            ArgsWidth::Word => 2,
        }
    }

    fn read_arg<R: std::io::Read + std::io::Seek>(
        inst_args_width: ArgsWidth,
        buf: R,
    ) -> anyhow::Result<Self> {
        Ok(VarSWord(match inst_args_width {
            ArgsWidth::Byte => signed_extend_byte(read_byte(buf)?),
            ArgsWidth::Word => read_word(buf)?,
        }))
    }

    fn write_arg<W: std::io::Write>(
        &self,
        inst_args_width: ArgsWidth,
        buf: W,
    ) -> anyhow::Result<()> {
        match inst_args_width {
            ArgsWidth::Byte => {
                write_byte(buf, safe_signed_narrow(self.0)?)?;
            }
            ArgsWidth::Word => {
                write_word(buf, self.0)?;
            }
        }
        Ok(())
    }
}

impl<T> InstAsmArg<T> for VarSWord {
    type InstArg = VarSWord;

    fn write_arg<SourceSymbol, W: BytecodeWriter<SourceSymbol, T>>(
        &self,
        inst_args_width: ArgsWidth,
        writer: &mut W,
    ) -> anyhow::Result<()> {
        match inst_args_width {
            ArgsWidth::Byte => writer.write_u8(safe_signed_narrow(self.0)?),
            ArgsWidth::Word => writer.write_u16_le(self.0),
        }
        Ok(())
    }
}

/// A static length word. Signedness doesn't matter, as we don't do any sign
/// extension.
#[derive(Clone, Copy, Debug)]
pub struct Word(u16);

impl InstArg for Word {
    fn byte_size(_inst_args_width: ArgsWidth) -> usize {
        2
    }

    fn read_arg<R: std::io::Read + std::io::Seek>(
        _inst_args_width: ArgsWidth,
        buf: R,
    ) -> anyhow::Result<Self> {
        Ok(Word(read_word(buf)?))
    }

    fn write_arg<W: std::io::Write>(
        &self,
        _inst_args_width: ArgsWidth,
        buf: W,
    ) -> anyhow::Result<()> {
        write_word(buf, self.0)
    }
}

impl<T> InstAsmArg<T> for Word {
    type InstArg = VarSWord;

    fn write_arg<SourceSymbol, W: BytecodeWriter<SourceSymbol, T>>(
        &self,
        _inst_args_width: ArgsWidth,
        writer: &mut W,
    ) -> anyhow::Result<()> {
        writer.write_u16_le(self.0);
        Ok(())
    }
}

/// A static length byte.
#[derive(Clone, Copy, Debug)]
pub struct Byte(u8);

impl InstArg for Byte {
    fn byte_size(_inst_args_width: ArgsWidth) -> usize {
        1
    }

    fn read_arg<R: std::io::Read + std::io::Seek>(
        _inst_args_width: ArgsWidth,
        buf: R,
    ) -> anyhow::Result<Self> {
        Ok(Byte(read_byte(buf)?))
    }

    fn write_arg<W: std::io::Write>(
        &self,
        _inst_args_width: ArgsWidth,
        buf: W,
    ) -> anyhow::Result<()> {
        write_byte(buf, self.0)
    }
}

impl<T> InstAsmArg<T> for Byte {
    type InstArg = VarSWord;

    fn write_arg<SourceSymbol, W: BytecodeWriter<SourceSymbol, T>>(
        &self,
        _inst_args_width: ArgsWidth,
        writer: &mut W,
    ) -> anyhow::Result<()> {
        writer.write_u8(self.0);
        Ok(())
    }
}

/// A relocated symbol.
#[derive(Clone, Copy, Debug)]
pub struct Label<T> {
    label: T,
    /// The offset to add to the symbol. Note that this may be negative, but
    /// since wrapping addition is used, this is not a problem.
    offset: u16,
}

impl<T> InstAsmArg<T> for Label<T>
where
    T: Clone,
{
    type InstArg = VarSWord;

    fn write_arg<SourceSymbol, W: BytecodeWriter<SourceSymbol, T>>(
        &self,
        inst_args_width: ArgsWidth,
        writer: &mut W,
    ) -> anyhow::Result<()> {
        match inst_args_width {
            ArgsWidth::Byte => writer.add_byte_reloc(
                RelocType::Relative,
                safe_signed_narrow(self.offset)?,
                self.label.clone(),
            ),
            ArgsWidth::Word => {
                writer.add_word_reloc(RelocType::Relative, self.offset, self.label.clone())
            }
        }
        Ok(())
    }
}
