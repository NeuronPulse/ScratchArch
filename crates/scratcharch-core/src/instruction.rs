use core::fmt;

pub type Label = String;
pub type FunctionName = String;

#[derive(Debug, Clone, PartialEq)]
pub enum Instruction {
    ConstI32(u32),
    ConstF64(f64),
    ConstI1(bool),
    Drop,
    Dup,
    I32Add,
    I32Sub,
    I32Mul,
    I32Div,
    I32Rem,
    And,
    Or,
    Xor,
    Shl,
    Shr,
    Eq,
    Lt,
    Gt,
    Load,
    Store,
    Alloc,
    Jump(Label),
    Branch(Label, Label),
    Call(FunctionName),
    Return,
    Pick(u32),
    LocalGet(u32),
    LocalSet(u32),
}

impl Instruction {
    pub fn name(&self) -> &'static str {
        match self {
            Instruction::ConstI32(_) => "const_i32",
            Instruction::ConstF64(_) => "const_f64",
            Instruction::ConstI1(_) => "const_i1",
            Instruction::Drop => "drop",
            Instruction::Dup => "dup",
            Instruction::I32Add => "i32.add",
            Instruction::I32Sub => "i32.sub",
            Instruction::I32Mul => "i32.mul",
            Instruction::I32Div => "i32.div",
            Instruction::I32Rem => "i32.rem",
            Instruction::And => "and",
            Instruction::Or => "or",
            Instruction::Xor => "xor",
            Instruction::Shl => "shl",
            Instruction::Shr => "shr",
            Instruction::Eq => "eq",
            Instruction::Lt => "lt",
            Instruction::Gt => "gt",
            Instruction::Load => "load",
            Instruction::Store => "store",
            Instruction::Alloc => "alloc",
            Instruction::Jump(_) => "jump",
            Instruction::Branch(_, _) => "branch",
            Instruction::Call(_) => "call",
            Instruction::Return => "return",
            Instruction::Pick(_) => "pick",
            Instruction::LocalGet(_) => "local.get",
            Instruction::LocalSet(_) => "local.set",
        }
    }
}

impl fmt::Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Instruction::ConstI32(v) => write!(f, "const_i32 {v}"),
            Instruction::ConstF64(v) => write!(f, "const_f64 {v}"),
            Instruction::ConstI1(v) => write!(f, "const_i1 {}", if *v { 1 } else { 0 }),
            Instruction::Drop => write!(f, "drop"),
            Instruction::Dup => write!(f, "dup"),
            Instruction::I32Add => write!(f, "i32.add"),
            Instruction::I32Sub => write!(f, "i32.sub"),
            Instruction::I32Mul => write!(f, "i32.mul"),
            Instruction::I32Div => write!(f, "i32.div"),
            Instruction::I32Rem => write!(f, "i32.rem"),
            Instruction::And => write!(f, "and"),
            Instruction::Or => write!(f, "or"),
            Instruction::Xor => write!(f, "xor"),
            Instruction::Shl => write!(f, "shl"),
            Instruction::Shr => write!(f, "shr"),
            Instruction::Eq => write!(f, "eq"),
            Instruction::Lt => write!(f, "lt"),
            Instruction::Gt => write!(f, "gt"),
            Instruction::Load => write!(f, "load"),
            Instruction::Store => write!(f, "store"),
            Instruction::Alloc => write!(f, "alloc"),
            Instruction::Jump(l) => write!(f, "jump {l}"),
            Instruction::Branch(t, f_) => write!(f, "branch {t} {f_}"),
            Instruction::Call(n) => write!(f, "call {n}"),
            Instruction::Return => write!(f, "return"),
            Instruction::Pick(n) => write!(f, "pick {n}"),
            Instruction::LocalGet(n) => write!(f, "local.get {n}"),
            Instruction::LocalSet(n) => write!(f, "local.set {n}"),
        }
    }
}
