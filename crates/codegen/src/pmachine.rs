//! This module defines the PMachine instruction set.

pub mod var_access;

use std::borrow::Cow;

use crate::insts::{AsmInst, Inst, InstBase};
use crate::{
    args::{ArgsWidth, Byte, InstArg, InstArgBase, InstAsmArg, Label, VarSWord, VarUWord},
    numbers::write_byte,
    opcode::Opcode,
    writer::BytecodeWriter,
};
use var_access::VarAccessOp;

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
    LINK("link", 0x1F, (VarUWord));    // add n arbitrary values to stack
    CALL("call", 0x20, (Label, Byte));    // call local proc
    CALLK("callk", 0x21, (VarUWord, Byte));   // call kernel
    CALLB("callb", 0x22, (VarUWord, Byte));   // call public proc in main script
    CALLE("calle", 0x23, (VarUWord, VarUWord, Byte));   // call public proc in external script
    RET("ret", 0x24, ());  // Return from proc
    SEND("send", 0x25, (Byte));  // Send selector sequence
    CLASS("class", 0x28, (VarUWord));   // load address of class # to accumulator (What are the semantics of this precisely?)
    SELF("self", 0x2A, (Byte));  // Send to self
    SUPER("super", 0x2B, (VarUWord, Byte));
    REST("rest", 0x2C, (VarUWord));
    LEA("lea", 0x2D, (VarUWord, VarUWord));     // load address of a variable into the acc
    SELFID("selfID", 0x2E, ());
    PPREV("pprev", 0x30, ());
    PTOA("pToa", 0x31, (VarUWord));    // property index to acc
    ATOP("aTop", 0x32, (VarUWord));    // acc to property index
    PTOS("pTos", 0x33, (VarUWord));    // property index to stack
    STOP("sTop", 0x34, (VarUWord));    // Stack to property index
    IPTOA("ipToa", 0x35, (VarUWord));   // Inc prop to acc
    DPTOA("dpToa", 0x36, (VarUWord));   // Dec prop to acc
    IPTOS("ipTos", 0x37, (VarUWord));   // Inc prop to stack
    DPTOS("dpTos", 0x38, (VarUWord));   // Dec prop to stack
    LOFSA("lofsa", 0x39, (VarUWord));   // Load offset (from pc) onto acc
    LOFSS("lofss", 0x3A, (VarUWord));   // Load offset (from pc) onto stack
    PUSH0("push0", 0x3B, ());
    PUSH1("push1", 0x3C, ());
    PUSH2("push2", 0x3D, ());
    PUSHSELF("pushSelf", 0x3E, ());
    VARACCESS(VarAccessOp, (VarUWord));
    // Add the VarAccessOp family of opcodes
    // TODO: Implement this
}
