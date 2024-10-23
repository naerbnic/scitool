//! This module defines the instruction set for the SCI 1.1 VM.
//! It's derived from the original SCICompanion codebase.

use crate::{args::ArgsWidth, opcode::Opcode};

pub trait Inst {
    type Opcode: Opcode<Inst = Self>;
    fn opcode(&self) -> Self::Opcode;
    /// Writes the entire instruction to the buffer, including the opcode byte.
    fn write_inst<R: std::io::Write>(&self, buf: R) -> anyhow::Result<()>;
}

/// A macro that generates the basic implementation of opcodes.
macro_rules! define_insts {
    ($($name:ident($op_name: literal, $opcode:literal, ($($type:ident),*));)*) => {
        #[allow(clippy::upper_case_acronyms, non_camel_case_types, reason = "Keeping parity with the original code")]
        #[derive(Clone, Copy, Debug)]
        pub enum Inst {
            $($name($(op_arg_types::$type),*)),*
        }

        impl Inst {
            pub fn to_kind(&self) -> InstKind {
                match self {
                    $(Inst::$name(..) => InstKind::$name,)*
                }
            }
        }

        // The base Opcode enum
        #[allow(clippy::upper_case_acronyms, non_camel_case_types, reason = "Keeping parity with the original code")]
        pub enum InstKind {
            $($name,)*
        }

        impl InstKind {
            pub fn to_opcode(&self) -> u8 {
                match self {
                    $(InstKind::$name => $opcode,)*
                }
            }

            pub fn from_opcode(opcode: u8) -> Option<InstKind> {
                match opcode {
                    $($opcode => Some(InstKind::$name),)*
                    _ => None,
                }
            }

            pub fn name(&self) -> &'static str {
                match self {
                    $(InstKind::$name => $op_name,)*
                }
            }

            pub fn from_name(name: &str) -> Option<InstKind> {
                match name {
                    $($op_name => Some(InstKind::$name),)*
                    _ => None,
                }
            }
        }
    }
}

codegen_macros::define_insts! {
    BNOT("bnot", 0x00, ());  // acc = ~acc
    ADD("add", 0x01, ());  // acc += pop()
    SUB("sub", 0x02, ());  // acc -= pop()
    MUL("mul", 0x03, ());  // acc *= pop()
    DIV("div", 0x04, ());  // acc /= pop()
    MOD("mod", 0x05, ());  // acc %= pop()
    SHR("shr", 0x06, ());  // acc >>= pop()
    SHL("shl", 0x07, ());  // acc <<= pop()
    XOR("xor", 0x08, ());  // acc ^= pop()
    AND("and", 0x09, ());  // acc &= pop()
    OR("or", 0x0A, ());  // acc |= pop()
    NEG("neg", 0x0B, ()); // acc = -acc
    NOT("not", 0x0C, ()); // acc = !acc (boolean not)
    EQ("eq?", 0x0D, ());  // prev = acc; acc = (pop() == acc)
    NE("ne?", 0x0E, ());  // prev = acc; acc = (pop() != acc)
    GT("gt?", 0x0F, ());  // prev = acc; acc = (pop() > acc)
    GE("ge?", 0x10, ());  // prev = acc; acc = (pop() >= acc)
    LT("lt?", 0x11, ());  // prev = acc; acc = (pop() < acc)
    LE("le?", 0x12, ());  // prev = acc; acc = (pop() <= acc)
    UGT("ugt?", 0x13, ());  // prev = acc; acc = (pop() > acc) (unsigned)
    UGE("uge?", 0x14, ());  // prev = acc; acc = (pop() >= acc) (unsigned)
    ULT("ult?", 0x15, ());  // prev = acc; acc = (pop() < acc) (unsigned)
    ULE("ule?", 0x16, ());  // prev = acc; acc = (pop() <= acc) (unsigned)
    BT("bt", 0x17, (Label));      // if (acc) goto label
    BNT("bnt", 0x18, (Label));     // if (!acc) goto label
    JMP("jmp", 0x19, (Label));     // goto label
    LDI("ldi", 0x1A, (Label));     // acc = immediate (sign extended)
    PUSH("push", 0x1B, ());    // push(acc)
    PUSHI("pushi", 0x1C, (Label));   // push(immediate) (sign extended)
    TOSS("toss", 0x1D, ());  // pop() (discard top of stack)
    DUP("dup", 0x1E, ());   // push(peek())
    LINK("link", 0x1F, (UInt));    // add n arbitrary values to stack
    CALL("call", 0x20, (Label, UInt8));    // call local proc
    CALLK("callk", 0x21, (Kernel, UInt8));   // call kernel
    CALLB("callb", 0x22, (PubProc, UInt8));   // call public proc in main script
    CALLE("calle", 0x23, (UInt, PubProc, UInt8));   // call public proc in external script
    RET("ret", 0x24, ());  // Return from proc
    SEND("send", 0x25, (UInt8));  // Send selector sequence
    Class("class", 0x28, (Class));   // load address of class # to accumulator
    SELF("self", 0x2A, (UInt8));  // Send to self
    SUPER("super", 0x2B, (Class, UInt8));
    REST("rest", 0x2C, (PVar));
    LEA("lea", 0x2D, (UInt, UInt));     // load address of a variable into the acc
    SELFID("selfID", 0x2E, ());
    PPREV("pprev", 0x30, ());
    PTOA("pToa", 0x31, (Prop));    // property index to acc
    ATOP("aTop", 0x32, (Prop));    // acc to property index
    PTOS("pTos", 0x33, (Prop));    // property index to stack
    STOP("sTop", 0x34, (Prop));    // Stack to property index
    IPTOA("ipToa", 0x35, (Prop));   // Inc prop to acc
    DPTOA("dpToa", 0x36, (Prop));   // Dec prop to acc
    IPTOS("ipTos", 0x37, (Prop));   // Inc prop to stack
    DPTOS("dpTos", 0x38, (Prop));   // Dec prop to stack
    LOFSA("lofsa", 0x39, (Offs));   // Load offset (from pc) onto acc
    LOFSS("lofss", 0x3A, (Offs));   // Load offset (from pc) onto stack
    PUSH0("push0", 0x3B, ());
    PUSH1("push1", 0x3C, ());
    PUSH2("push2", 0x3D, ());
    PUSHSELF("pushSelf", 0x3E, ());
    LAG("lag", 0x40, (Var));     // load global to acc
    LAL("lal", 0x41, (Var));
    LAT("lat", 0x42, (Var));
    LAP("lap", 0x43, (Var));
    LSG("lsg", 0x44, (Var));     // load global to stack
    LSL("lsl", 0x45, (Var));
    LST("lst", 0x46, (Var));
    LSP("lsp", 0x47, (Var));
    LAGI("lagi", 0x48, (Var));    // index global and load to acc
    LALI("lali", 0x49, (Var));
    LATI("lati", 0x4A, (Var));
    LAPI("lapi", 0x4B, (Var));
    LSGI("lsgi", 0x4C, (Var));    // index global and load to stack
    LSLI("lsli", 0x4D, (Var));
    LSTI("lsti", 0x4E, (Var));
    LSPI("lspi", 0x4F, (Var));
    SAG("sag", 0x50, (Var));     // store acc in global
    SAL("sal", 0x51, (Var));
    SAT("sat", 0x52, (Var));
    SAP("sap", 0x53, (Var));
    SSG("ssg", 0x54, (Var));     // store stack in global
    SSL("ssl", 0x55, (Var));
    SST("sst", 0x56, (Var));
    SSP("ssp", 0x57, (Var));
    SAGI("sagi", 0x58, (Var));    // store acc in global and index?
    SALI("sali", 0x59, (Var));
    SATI("sati", 0x5A, (Var));
    SAPI("sapi", 0x5B, (Var));
    SSGI("ssgi", 0x5C, (Var));    // store stack in global and index?
    SSLI("ssli", 0x5D, (Var));
    SSTI("ssti", 0x5E, (Var));
    SSPI("sspi", 0x5F, (Var));
    pAG("+ag", 0x60, (Var));
    pAL("+al", 0x61, (Var));
    pAT("+at", 0x62, (Var));
    pAP("+ap", 0x63, (Var));
    pSG("+sg", 0x64, (Var));
    pSL("+sl", 0x65, (Var));
    pST("+st", 0x66, (Var));
    pSP("+sp", 0x67, (Var));
    pAGI("+agi", 0x68, (Var));
    pALI("+ali", 0x69, (Var));
    pATI("+ati", 0x6A, (Var));
    pAPI("+api", 0x6B, (Var));
    pSGI("+sgi", 0x6C, (Var));
    pSLI("+sli", 0x6D, (Var));
    pSTI("+sti", 0x6E, (Var));
    pSPI("+spi", 0x6F, (Var));
    nAG("-ag", 0x70, (Var));
    nAL("-al", 0x71, (Var));
    nAT("-at", 0x72, (Var));
    nAP("-ap", 0x73, (Var));
    nSG("-sg", 0x74, (Var));
    nSL("-sl", 0x75, (Var));
    nST("-st", 0x76, (Var));
    nSP("-sp", 0x77, (Var));
    nAGI("-agi", 0x78, (Var));
    nALI("-ali", 0x79, (Var));
    nATI("-ati", 0x7A, (Var));
    nAPI("-api", 0x7B, (Var));
    nSGI("-sgi", 0x7C, (Var));
    nSLI("-sli", 0x7D, (Var));
    nSTI("-sti", 0x7E, (Var));
    nSPI("-spi", 0x7F, (Var));
    // Opcodes have to be less than 0x80, as the opcode is stored
    // in 7 bits
    // Filename("_file_", 0x80, ());
    // LineNumber("_line_", 0x81, ());
}

#[derive(Clone, Copy, Debug)]
pub enum VarType {
    Local,
    Global,
    Temp,
    Param,
}

// Where to Load/Store the variable
#[derive(Clone, Copy, Debug)]
pub enum OtherType {
    Accumulator,
    Stack,
}

// What operation to perform on the variable
#[derive(Clone, Copy, Debug)]
pub enum Operation {
    Load,
    Store,
    IncLoad,
    DecLoad,
}

/// The PMachine has a family of opcodes that are used to read and write
/// to memory. This represents the opcodes in that family.
#[derive(Clone, Copy, Debug)]
pub struct VarAccessOp {
    var_type: VarType,
    other_type: OtherType,
    use_acc: bool,
    op: Operation,
}

impl Opcode for VarAccessOp {
    type Inst = VarAccessInst;

    fn from_opcode_byte(opcode: u8) -> anyhow::Result<Option<Self>> {
        if opcode & 0x80 == 0 {
            return Ok(None);
        }

        let var_type = match (opcode >> 1) & 0b11 {
            0b00 => VarType::Global,
            0b01 => VarType::Local,
            0b10 => VarType::Temp,
            0b11 => VarType::Param,
            _ => unreachable!(),
        };

        let other_type = match (opcode >> 3) & 0b1 {
            0b0 => OtherType::Accumulator,
            0b1 => OtherType::Stack,
            _ => unreachable!(),
        };

        let use_acc = (opcode >> 4) & 0b1 == 0b1;

        let op = match (opcode >> 5) & 0b11 {
            0b00 => Operation::Load,
            0b01 => Operation::Store,
            0b10 => Operation::IncLoad,
            0b11 => Operation::DecLoad,
            _ => unreachable!(),
        };

        Ok(Some(VarAccessOp {
            var_type,
            other_type,
            use_acc,
            op,
        }))
    }

    fn opcode_byte(&self) -> u8 {
        let var_type_bits: u8 = match self.var_type {
            VarType::Global => 0b00,
            VarType::Local => 0b01,
            VarType::Temp => 0b10,
            VarType::Param => 0b11,
        };

        let other_type_bits: u8 = match self.other_type {
            OtherType::Accumulator => 0b0,
            OtherType::Stack => 0b1,
        };

        let use_acc_bit: u8 = if self.use_acc { 0b1 } else { 0b0 };

        let op_bits: u8 = match self.op {
            Operation::Load => 0b00,
            Operation::Store => 0b01,
            Operation::IncLoad => 0b10,
            Operation::DecLoad => 0b11,
        };
        0x80 | (op_bits << 5) | (use_acc_bit << 4) | (other_type_bits << 3) | var_type_bits << 1
    }

    fn opcode_name(&self) -> std::borrow::Cow<str> {
        // This uses the scheme from the ScummVM specification.
        let var_type_str = match self.var_type {
            VarType::Global => "g",
            VarType::Local => "l",
            VarType::Temp => "t",
            VarType::Param => "p",
        };

        let other_type_str = match self.other_type {
            OtherType::Accumulator => "a",
            OtherType::Stack => "s",
        };

        let use_acc_str = if self.use_acc { "i" } else { "" };

        let op_str = match self.op {
            Operation::Load => "l",
            Operation::Store => "s",
            Operation::IncLoad => "+",
            Operation::DecLoad => "-",
        };

        format!(
            "{}{}{}{}",
            op_str, other_type_str, var_type_str, use_acc_str
        )
        .into()
    }

    fn parse_args<R: std::io::Read + std::io::Seek>(
        &self,
        args_width: ArgsWidth,
        buf: R,
    ) -> anyhow::Result<Self::Inst> {
        todo!()
    }
}

pub struct VarAccessInst {
    opcode: VarAccessOp,
    index: u16,
}

impl Inst for VarAccessInst {
    type Opcode = VarAccessOp;

    fn opcode(&self) -> Self::Opcode {
        todo!()
    }

    fn write_inst<R: std::io::Write>(&self, buf: R) -> anyhow::Result<()> {
        todo!()
    }
}
