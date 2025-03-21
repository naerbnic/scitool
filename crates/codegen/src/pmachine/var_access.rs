use super::Opcode;

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
        0x80 | (op_bits << 5) | (use_acc_bit << 4) | (other_type_bits << 3) | (var_type_bits << 1)
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
}
