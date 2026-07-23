use crate::errors::LlvmError;

#[derive(Debug, Clone, PartialEq)]
pub enum LlvmType {
    I1,
    I8,
    I16,
    I32,
    Ptr,
    Void,
    Array { inner: Box<LlvmType>, count: u32 },
    Struct(String),
    UnnamedStruct { fields: Vec<LlvmType> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct LlvmConst {
    pub value: i64,
    pub ty: LlvmType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LlvmValue {
    Const(LlvmConst),
    Local(String),
    Global(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum IcmpPred {
    Eq,
    Ne,
    Slt,
    Sgt,
    Sle,
    Sge,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LlvmInstr {
    BinOp {
        dest: Option<String>,
        op: String,
        ty: LlvmType,
        lhs: LlvmValue,
        rhs: LlvmValue,
    },
    Icmp {
        dest: String,
        pred: IcmpPred,
        ty: LlvmType,
        lhs: LlvmValue,
        rhs: LlvmValue,
    },
    Ret {
        value: Option<LlvmValue>,
    },
    Br {
        target: String,
    },
    CondBr {
        cond: LlvmValue,
        true_target: String,
        false_target: String,
    },
    Alloca {
        dest: String,
        ty: LlvmType,
        count: u32,
    },
    Load {
        dest: String,
        ty: LlvmType,
        addr: LlvmValue,
    },
    Store {
        ty: LlvmType,
        value: LlvmValue,
        addr: LlvmValue,
    },
    Call {
        dest: Option<String>,
        return_ty: LlvmType,
        callee: String,
        args: Vec<LlvmValue>,
    },
    Gep {
        dest: String,
        elem_ty: LlvmType,
        base: LlvmValue,
        indices: Vec<LlvmValue>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct LlvmBlock {
    pub label: String,
    pub instructions: Vec<LlvmInstr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LlvmFunction {
    pub name: String,
    pub return_ty: LlvmType,
    pub params: Vec<(LlvmType, String)>,
    pub blocks: Vec<LlvmBlock>,
    pub is_declaration: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LlvmProgram {
    pub functions: Vec<LlvmFunction>,
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Define,
    Declare,
    I1,
    I8,
    I16,
    I32,
    Ptr,
    Void,
    True,
    False,
    Label(String),
    GlobalId(String),
    LocalId(String),
    Number(i64),
    OpenBrace,
    CloseBrace,
    OpenParen,
    CloseParen,
    OpenBracket,
    CloseBracket,
    Comma,
    Equals,
    Ident(String),
    EqPred,
    NePred,
    SltPred,
    SgtPred,
    SlePred,
    SgePred,
    Eof,
}

struct Lexer {
    chars: Vec<char>,
    pos: usize,
}

impl Lexer {
    fn new(input: &str) -> Self {
        Lexer {
            chars: input.chars().collect(),
            pos: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn advance(&mut self) {
        self.pos += 1;
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            if c == ';' {
                while let Some(c) = self.peek() {
                    if c == '\n' {
                        break;
                    }
                    self.advance();
                }
            }
            if c.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn read_number(&mut self) -> Token {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                self.advance();
            } else {
                break;
            }
        }
        let s: String = self.chars[start..self.pos].iter().collect();
        Token::Number(s.parse::<i64>().unwrap())
    }

    fn next_token(&mut self) -> Token {
        self.skip_whitespace();
        let c = match self.peek() {
            None => return Token::Eof,
            Some(c) => c,
        };

        if c.is_alphabetic() || c == '_' || c == '.' {
            let start = self.pos;
            while let Some(c) = self.peek() {
                if c.is_alphanumeric() || c == '_' || c == '-' || c == '.' {
                    self.advance();
                } else {
                    break;
                }
            }
            let word: String = self.chars[start..self.pos].iter().collect();

            if self.peek() == Some(':') {
                self.advance();
                return Token::Label(word);
            }

            match word.as_str() {
                "define" => return Token::Define,
                "declare" => return Token::Declare,
                "i1" => return Token::I1,
                "i8" => return Token::I8,
                "i16" => return Token::I16,
                "i32" => return Token::I32,
                "ptr" => return Token::Ptr,
                "void" => return Token::Void,
                "true" => return Token::True,
                "false" => return Token::False,
                "eq" => return Token::EqPred,
                "ne" => return Token::NePred,
                "slt" => return Token::SltPred,
                "sgt" => return Token::SgtPred,
                "sle" => return Token::SlePred,
                "sge" => return Token::SgePred,
                _ => return Token::Ident(word),
            }
        }

        if c == '@' {
            self.advance();
            let start = self.pos;
            while let Some(c) = self.peek() {
                if c.is_alphanumeric() || c == '_' || c == '-' || c == '.' {
                    self.advance();
                } else {
                    break;
                }
            }
            let name: String = self.chars[start..self.pos].iter().collect();
            return Token::GlobalId(name);
        }

        if c == '%' {
            self.advance();
            let start = self.pos;
            while let Some(c) = self.peek() {
                if c.is_alphanumeric() || c == '_' || c == '-' || c == '.' {
                    self.advance();
                } else {
                    break;
                }
            }
            let name: String = self.chars[start..self.pos].iter().collect();
            return Token::LocalId(name);
        }

        if c.is_ascii_digit() {
            return self.read_number();
        }

        self.advance();
        match c {
            '{' => Token::OpenBrace,
            '}' => Token::CloseBrace,
            '(' => Token::OpenParen,
            ')' => Token::CloseParen,
            '[' => Token::OpenBracket,
            ']' => Token::CloseBracket,
            ',' => Token::Comma,
            '=' => Token::Equals,
            '-' => {
                if self.peek().is_some_and(|c| c.is_ascii_digit()) {
                    let start = self.pos;
                    while let Some(c) = self.peek() {
                        if c.is_ascii_digit() {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    let s: String = self.chars[start..self.pos].iter().collect();
                    let val: i64 = s.parse::<i64>().unwrap();
                    Token::Number(-val)
                } else {
                    Token::Ident("-".to_string())
                }
            }
            _ => Token::Ident(c.to_string()),
        }
    }
}

pub struct Parser {
    lexer: Lexer,
    current: Token,
}

impl Parser {
    pub fn new(input: &str) -> Self {
        let mut lexer = Lexer::new(input);
        let current = lexer.next_token();
        Parser { lexer, current }
    }

    fn advance(&mut self) {
        self.current = self.lexer.next_token();
    }

    fn expect(&mut self, expected: &Token) -> Result<(), LlvmError> {
        if self.current == *expected {
            self.advance();
            Ok(())
        } else {
            Err(LlvmError::Parse(format!(
                "expected {:?}, got {:?}", expected, self.current
            )))
        }
    }

    fn parse_type(&mut self) -> Result<LlvmType, LlvmError> {
        match &self.current {
            Token::OpenBracket => {
                self.advance();
                let count = match &self.current {
                    Token::Number(n) => {
                        let val = *n as u32;
                        self.advance();
                        val
                    }
                    _ => {
                        return Err(LlvmError::Parse(format!(
                            "expected array count, got {:?}", self.current
                        )))
                    }
                };
                self.expect(&Token::Ident("x".into()))?;
                let inner = self.parse_type()?;
                self.expect(&Token::CloseBracket)?;
                Ok(LlvmType::Array {
                    inner: Box::new(inner),
                    count,
                })
            }
            Token::OpenBrace => {
                self.advance();
                let mut fields = Vec::new();
                while self.current != Token::CloseBrace {
                    let field_ty = self.parse_type()?;
                    fields.push(field_ty);
                    if self.current == Token::Comma {
                        self.advance();
                    }
                }
                self.expect(&Token::CloseBrace)?;
                Ok(LlvmType::UnnamedStruct { fields })
            }
            Token::LocalId(name) => {
                let n = name.clone();
                self.advance();
                Ok(LlvmType::Struct(n))
            }
            _ => {
                let tok = self.current.clone();
                self.advance();
                match tok {
                    Token::I1 => Ok(LlvmType::I1),
                    Token::I8 => Ok(LlvmType::I8),
                    Token::I16 => Ok(LlvmType::I16),
                    Token::I32 => Ok(LlvmType::I32),
                    Token::Ptr => Ok(LlvmType::Ptr),
                    Token::Void => Ok(LlvmType::Void),
                    _ => Err(LlvmError::UnsupportedType(format!("{:?}", tok))),
                }
            }
        }
    }

    fn parse_value(&mut self) -> Result<LlvmValue, LlvmError> {
        self.parse_value_of_type_opt(None)
    }

    /// Parse a value, optionally with a known expected type.
    ///
    /// This is used by `store` (where the value's type is the store type) and by
    /// the general `parse_value` entry point.
    fn parse_value_of_type_opt(
        &mut self,
        expected_ty: Option<&LlvmType>,
    ) -> Result<LlvmValue, LlvmError> {
        match &self.current {
            Token::Number(n) => {
                let val = *n;
                self.advance();
                let ty = expected_ty.cloned().unwrap_or(LlvmType::I32);
                Ok(LlvmValue::Const(LlvmConst { value: val, ty }))
            }
            Token::True => {
                self.advance();
                let ty = expected_ty.cloned().unwrap_or(LlvmType::I1);
                Ok(LlvmValue::Const(LlvmConst { value: 1, ty }))
            }
            Token::False => {
                self.advance();
                let ty = expected_ty.cloned().unwrap_or(LlvmType::I1);
                Ok(LlvmValue::Const(LlvmConst { value: 0, ty }))
            }
            Token::LocalId(name) => {
                let n = name.clone();
                self.advance();
                Ok(LlvmValue::Local(n))
            }
            Token::GlobalId(name) => {
                let n = name.clone();
                self.advance();
                Ok(LlvmValue::Global(n))
            }
            _ => {
                let ty = self.parse_type()?;
                match &self.current {
                    Token::Number(n) => {
                        let val = *n;
                        self.advance();
                        Ok(LlvmValue::Const(LlvmConst { value: val, ty }))
                    }
                    Token::LocalId(name) => {
                        let n = name.clone();
                        self.advance();
                        Ok(LlvmValue::Local(n))
                    }
                    Token::GlobalId(name) => {
                        let n = name.clone();
                        self.advance();
                        Ok(LlvmValue::Global(n))
                    }
                    _ => Err(LlvmError::Parse(format!(
                        "expected value, got {:?}",
                        self.current
                    ))),
                }
            }
        }
    }

    fn parse_value_of_type(&mut self, ty: &LlvmType) -> Result<LlvmValue, LlvmError> {
        self.parse_value_of_type_opt(Some(ty))
    }

    fn parse_params(&mut self) -> Result<Vec<(LlvmType, String)>, LlvmError> {
        let mut params = Vec::new();
        self.expect(&Token::OpenParen)?;
        while self.current != Token::CloseParen {
            let ty = self.parse_type()?;
            let name = match &self.current {
                Token::LocalId(n) => {
                    let name = n.clone();
                    self.advance();
                    name
                }
                _ => return Err(LlvmError::Parse(format!("expected local id, got {:?}", self.current))),
            };
            params.push((ty, name));
            if self.current == Token::Comma {
                self.advance();
            }
        }
        self.expect(&Token::CloseParen)?;
        Ok(params)
    }

    fn parse_icmp_predicate(&mut self) -> Result<IcmpPred, LlvmError> {
        let tok = self.current.clone();
        self.advance();
        match tok {
            Token::EqPred => Ok(IcmpPred::Eq),
            Token::NePred => Ok(IcmpPred::Ne),
            Token::SltPred => Ok(IcmpPred::Slt),
            Token::SgtPred => Ok(IcmpPred::Sgt),
            Token::SlePred => Ok(IcmpPred::Sle),
            Token::SgePred => Ok(IcmpPred::Sge),
            _ => Err(LlvmError::UnsupportedIcmpPredicate(format!("{:?}", tok))),
        }
    }

    fn parse_call_args(&mut self) -> Result<Vec<LlvmValue>, LlvmError> {
        let mut args = Vec::new();
        self.expect(&Token::OpenParen)?;
        while self.current != Token::CloseParen {
            let val = self.parse_value()?;
            args.push(val);
            if self.current == Token::Comma {
                self.advance();
            }
        }
        self.expect(&Token::CloseParen)?;
        Ok(args)
    }

    fn parse_gep_indices(&mut self) -> Result<Vec<LlvmValue>, LlvmError> {
        let mut indices = Vec::new();
        if self.current == Token::Comma {
            self.advance();
        }
        loop {
            match &self.current {
                Token::CloseBrace | Token::Label(_) | Token::Eof => break,
                _ => {}
            }
            let idx = self.parse_value()?;
            indices.push(idx);
            if self.current == Token::Comma {
                self.advance();
            } else {
                break;
            }
        }
        Ok(indices)
    }

    fn skip_opt_align(&mut self) {
        if self.current == Token::Comma {
            self.advance();
            if matches!(&self.current, Token::Ident(s) if s == "align") {
                self.advance();
                if matches!(&self.current, Token::Number(_)) {
                    self.advance();
                }
            }
        }
    }

    fn parse_instruction(&mut self) -> Result<LlvmInstr, LlvmError> {
        let dest = match &self.current {
            Token::LocalId(_) => {
                let name = match &self.current {
                    Token::LocalId(n) => n.clone(),
                    _ => unreachable!(),
                };
                self.advance();
                self.expect(&Token::Equals)?;
                Some(name)
            }
            _ => None,
        };

        match &self.current {
            Token::Ident(op) => {
                let op_name = op.clone();
                self.advance();

                while matches!(&self.current, Token::Ident(s) if s == "nsw" || s == "nuw") {
                    self.advance();
                }

                match op_name.as_str() {
                    "add" | "sub" | "mul" | "sdiv" | "udiv" => {
                        let binop_op = match op_name.as_str() {
                            "sdiv" | "udiv" => "div".to_string(),
                            _ => op_name,
                        };
                        let ty = self.parse_type()?;
                        let lhs = self.parse_value()?;
                        self.expect(&Token::Comma)?;
                        let rhs = self.parse_value()?;
                        Ok(LlvmInstr::BinOp {
                            dest,
                            op: binop_op,
                            ty,
                            lhs,
                            rhs,
                        })
                    }
                    "icmp" => {
                        let pred = self.parse_icmp_predicate()?;
                        let ty = self.parse_type()?;
                        let lhs = self.parse_value()?;
                        self.expect(&Token::Comma)?;
                        let rhs = self.parse_value()?;
                        let dest = dest.ok_or_else(|| LlvmError::Parse("icmp requires dest".into()))?;
                        Ok(LlvmInstr::Icmp { dest, pred, ty, lhs, rhs })
                    }
                    "ret" => {
                        if self.current == Token::Void {
                            self.advance();
                            Ok(LlvmInstr::Ret { value: None })
                        } else {
                            let value = self.parse_value()?;
                            Ok(LlvmInstr::Ret { value: Some(value) })
                        }
                    }
                    "br" => {
                        if self.current == Token::Ident("label".to_string()) || self.current == Token::I1 {
                            if self.current == Token::I1 {
                                self.advance();
                                let cond = self.parse_value()?;
                                self.expect(&Token::Comma)?;
                                let true_target = self.parse_label_target()?;
                                self.expect(&Token::Comma)?;
                                let false_target = self.parse_label_target()?;
                                Ok(LlvmInstr::CondBr {
                                    cond,
                                    true_target,
                                    false_target,
                                })
                            } else {
                                self.advance();
                                let target = match &self.current {
                                    Token::LocalId(n) | Token::GlobalId(n) => {
                                        let t = n.clone();
                                        self.advance();
                                        t
                                    }
                                    _ => {
                                        return Err(LlvmError::Parse(format!(
                                            "expected label target, got {:?}",
                                            self.current
                                        )))
                                    }
                                };
                                Ok(LlvmInstr::Br { target })
                            }
                        } else {
                            Err(LlvmError::Parse(format!(
                                "expected 'label' or 'i1' after br, got {:?}",
                                self.current
                            )))
                        }
                    }
                    "alloca" => {
                        let ty = self.parse_type()?;
                        let mut count = 1u32;
                        if self.current == Token::Comma {
                            self.advance();
                            if matches!(&self.current, Token::Ident(s) if s == "align") {
                                self.advance();
                                if let Token::Number(_) = &self.current {
                                    self.advance();
                                }
                            } else {
                                let _count_ty = self.parse_type()?;
                                if let Token::Number(n) = self.current.clone() {
                                    count = n as u32;
                                    self.advance();
                                }
                                self.skip_opt_align();
                            }
                        }
                        let dest = dest.ok_or_else(|| LlvmError::Parse("alloca requires dest".into()))?;
                        Ok(LlvmInstr::Alloca { dest, ty, count })
                    }
                    "load" => {
                        let ty = self.parse_type()?;
                        self.expect(&Token::Comma)?;
                        self.expect(&Token::Ptr)?;
                        let addr = self.parse_value()?;
                        self.skip_opt_align();
                        let dest = dest.ok_or_else(|| LlvmError::Parse("load requires dest".into()))?;
                        Ok(LlvmInstr::Load { dest, ty, addr })
                    }
                    "store" => {
                        let ty = self.parse_type()?;
                        let value = self.parse_value_of_type(&ty)?;
                        self.expect(&Token::Comma)?;
                        self.expect(&Token::Ptr)?;
                        let addr = self.parse_value()?;
                        self.skip_opt_align();
                        Ok(LlvmInstr::Store { ty, value, addr })
                    }
                    "call" => {
                        let return_ty = self.parse_type()?;
                        let callee = match &self.current {
                            Token::GlobalId(n) => {
                                let name = n.clone();
                                self.advance();
                                name
                            }
                            _ => {
                                return Err(LlvmError::Parse(format!(
                                    "expected callee, got {:?}",
                                    self.current
                                )))
                            }
                        };
                        let args = self.parse_call_args()?;
                        Ok(LlvmInstr::Call {
                            dest,
                            return_ty,
                            callee,
                            args,
                        })
                    }
                    "getelementptr" => {
                        if matches!(&self.current, Token::Ident(s) if s == "inbounds") {
                            self.advance();
                        }
                        let elem_ty = self.parse_type()?;
                        self.expect(&Token::Comma)?;
                        self.expect(&Token::Ptr)?;
                        let base = self.parse_value()?;
                        let indices = self.parse_gep_indices()?;
                        let dest =
                            dest.ok_or_else(|| LlvmError::Parse("getelementptr requires dest".into()))?;
                        Ok(LlvmInstr::Gep {
                            dest,
                            elem_ty,
                            base,
                            indices,
                        })
                    }
                    _ => Err(LlvmError::UnsupportedInstruction(op_name.to_string())),
                }
            }
            _ => Err(LlvmError::Parse(format!(
                "expected instruction, got {:?}",
                self.current
            ))),
        }
    }

    fn parse_label_target(&mut self) -> Result<String, LlvmError> {
        self.expect(&Token::Ident("label".into()))?;
        match &self.current {
            Token::LocalId(n) | Token::GlobalId(n) => {
                let target = n.clone();
                self.advance();
                Ok(target)
            }
            _ => Err(LlvmError::Parse(format!("expected label target, got {:?}", self.current))),
        }
    }

    fn parse_block(&mut self) -> Result<LlvmBlock, LlvmError> {
        let label = match &self.current {
            Token::Label(l) => {
                let label = l.clone();
                self.advance();
                label
            }
            _ => return Err(LlvmError::Parse(format!("expected block label, got {:?}", self.current))),
        };

        let mut instructions = Vec::new();
        loop {
            match &self.current {
                Token::CloseBrace => break,
                Token::Label(_) => break,
                Token::Eof => break,
                _ => {}
            }
            let instr = self.parse_instruction()?;
            instructions.push(instr);
        }

        Ok(LlvmBlock { label, instructions })
    }

    fn parse_function(&mut self) -> Result<LlvmFunction, LlvmError> {
        self.expect(&Token::Define)?;
        let return_ty = self.parse_type()?;
        let name = match &self.current {
            Token::GlobalId(n) => {
                let name = n.clone();
                self.advance();
                name
            }
            _ => {
                return Err(LlvmError::Parse(format!(
                    "expected function name, got {:?}",
                    self.current
                )))
            }
        };
        let params = self.parse_params()?;
        self.expect(&Token::OpenBrace)?;

        let mut blocks = Vec::new();
        loop {
            match &self.current {
                Token::CloseBrace => break,
                Token::Label(_) => {
                    let block = self.parse_block()?;
                    blocks.push(block);
                }
                _ => {
                    if blocks.is_empty() {
                        let mut instructions = Vec::new();
                        loop {
                            match &self.current {
                                Token::CloseBrace => break,
                                Token::Label(_) => break,
                                Token::Eof => break,
                                _ => {}
                            }
                            let instr = self.parse_instruction()?;
                            instructions.push(instr);
                        }
                        blocks.push(LlvmBlock {
                            label: "entry".into(),
                            instructions,
                        });
                    } else {
                        return Err(LlvmError::Parse(format!(
                            "expected block label or }} after instructions, got {:?}",
                            self.current
                        )));
                    }
                }
            }
        }

        self.expect(&Token::CloseBrace)?;

        Ok(LlvmFunction {
            name,
            return_ty,
            params,
            blocks,
            is_declaration: false,
        })
    }

    fn parse_declaration(&mut self) -> Result<LlvmFunction, LlvmError> {
        self.expect(&Token::Declare)?;
        let return_ty = self.parse_type()?;
        let name = match &self.current {
            Token::GlobalId(n) => {
                let name = n.clone();
                self.advance();
                name
            }
            _ => {
                return Err(LlvmError::Parse(format!(
                    "expected function name, got {:?}",
                    self.current
                )))
            }
        };
        let params = self.parse_params()?;
        Ok(LlvmFunction {
            name,
            return_ty,
            params,
            blocks: Vec::new(),
            is_declaration: true,
        })
    }

    pub fn parse_program(&mut self) -> Result<LlvmProgram, LlvmError> {
        let mut functions = Vec::new();
        loop {
            match &self.current {
                Token::Eof => break,
                Token::Define => {
                    let func = self.parse_function()?;
                    functions.push(func);
                }
                Token::Declare => {
                    let func = self.parse_declaration()?;
                    functions.push(func);
                }
                _ => {
                    self.advance();
                }
            }
        }
        Ok(LlvmProgram { functions })
    }
}

pub fn parse_llvm(input: &str) -> Result<LlvmProgram, LlvmError> {
    let mut parser = Parser::new(input);
    parser.parse_program()
}
