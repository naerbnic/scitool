use std::sync::Arc;

use sci_utils::{
    numbers::{
        read_byte, read_word, safe_signed_narrow, safe_unsigned_narrow, signed_extend_byte,
        unsigned_extend_byte, write_byte, write_word,
    },
    reloc_buffer::{RelocSize, RelocType, expr::Expr, writer::RelocWriter},
    symbol::Symbol,
};

#[derive(Clone, Copy, Debug)]
pub enum ArgsWidth {
    Byte,
    Word,
}

pub trait ArgValueObject: std::fmt::Debug {
    /// Resolves the value of this argument to an expression that should
    /// be written to the output.
    fn make_value_expr(&self, end_of_inst_pos: &Symbol) -> Expr;
}

#[derive(Debug)]
pub struct ArgValue(Arc<dyn ArgValueObject>);

impl Clone for ArgValue {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl ArgValue {
    pub fn new<T>(value: T) -> Self
    where
        T: ArgValueObject + 'static,
    {
        Self(Arc::new(value))
    }

    pub fn make_value_expr(&self, end_of_inst_pos: &Symbol) -> Expr {
        self.0.make_value_expr(end_of_inst_pos)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Signedness {
    Signed,
    Unsigned,
}

#[derive(Clone, Copy, Debug)]
pub enum ArgType {
    Byte,
    Word,
    VarWord(Signedness),
}

impl ArgType {
    pub fn byte_size(self, inst_args_width: ArgsWidth) -> usize {
        match self {
            ArgType::Byte => 1,
            ArgType::Word => 2,
            ArgType::VarWord(_) => match inst_args_width {
                ArgsWidth::Byte => 1,
                ArgsWidth::Word => 2,
            },
        }
    }

    pub fn read_widened_value<R: std::io::Read>(
        self,
        inst_args_width: ArgsWidth,
        reader: R,
    ) -> anyhow::Result<u16> {
        Ok(match self {
            ArgType::Byte => unsigned_extend_byte(read_byte(reader)?),
            ArgType::Word => read_word(reader)?,
            ArgType::VarWord(signedness) => match inst_args_width {
                ArgsWidth::Word => read_word(reader)?,
                ArgsWidth::Byte => {
                    let byte = read_byte(reader)?;
                    match signedness {
                        Signedness::Signed => signed_extend_byte(byte),
                        Signedness::Unsigned => unsigned_extend_byte(byte),
                    }
                }
            },
        })
    }

    pub fn write_narrowed_value<W: std::io::Write>(
        self,
        inst_args_width: ArgsWidth,
        value: u16,
        writer: W,
    ) -> anyhow::Result<()> {
        match self {
            ArgType::Byte => write_byte(writer, safe_unsigned_narrow(value)?)?,
            ArgType::Word => write_word(writer, value)?,
            ArgType::VarWord(signedness) => match inst_args_width {
                ArgsWidth::Word => write_word(writer, value)?,
                ArgsWidth::Byte => {
                    let byte = match signedness {
                        Signedness::Signed => safe_signed_narrow(value)?,
                        Signedness::Unsigned => safe_unsigned_narrow(value)?,
                    };
                    write_byte(writer, byte)?;
                }
            },
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Arg {
    arg_type: ArgType,
    value: u16,
}

impl Arg {
    pub fn read_arg<R: std::io::Read>(
        arg_type: ArgType,
        inst_args_width: ArgsWidth,
        buf: R,
    ) -> anyhow::Result<Self> {
        let value = arg_type.read_widened_value(inst_args_width, buf)?;

        Ok(Self { arg_type, value })
    }

    pub fn write_arg<W: std::io::Write>(
        &self,
        inst_args_width: ArgsWidth,
        buf: W,
    ) -> anyhow::Result<()> {
        self.arg_type
            .write_narrowed_value(inst_args_width, self.value, buf)
    }
}

#[derive(Debug)]
pub struct AsmArg {
    arg_type: ArgType,
    value: ArgValue,
}

impl AsmArg {
    pub fn write_asm_arg<W>(
        &self,
        inst_args_width: ArgsWidth,
        inst_end: &Symbol,
        writer: &mut W,
    ) -> anyhow::Result<()>
    where
        W: RelocWriter,
    {
        let value_expr = self.value.make_value_expr(inst_end);
        let (reloc_width, reloc_type) = match self.arg_type {
            ArgType::Byte => (RelocSize::I8, RelocType::Absolute),
            ArgType::Word => (RelocSize::I16, RelocType::Absolute),
            ArgType::VarWord(signedness) => (
                match inst_args_width {
                    ArgsWidth::Byte => RelocSize::I8,
                    ArgsWidth::Word => RelocSize::I16,
                },
                match signedness {
                    Signedness::Signed => RelocType::Relative,
                    Signedness::Unsigned => RelocType::Absolute,
                },
            ),
        };
        writer.add_reloc(reloc_type, reloc_width, value_expr);
        Ok(())
    }
}

impl Clone for AsmArg {
    fn clone(&self) -> Self {
        Self {
            arg_type: self.arg_type,
            value: self.value.clone(),
        }
    }
}
