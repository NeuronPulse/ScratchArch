use std::collections::HashMap;
use crate::builder::IrBuilder;
use crate::debug::DebugLoc;
use crate::instruction::{CastOp, GepIndex, Terminator};
use crate::r#module::IrModule;
use crate::types::IrType;
use crate::value::{Constant, ValueId};

#[derive(Debug, Clone, PartialEq)]
pub enum TextIrError {
    Parse(String),
    Validation(String),
    UnsupportedVersion(String),
}

impl std::fmt::Display for TextIrError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TextIrError::Parse(s) => write!(f, "parse error: {}", s),
            TextIrError::Validation(s) => write!(f, "validation error: {}", s),
            TextIrError::UnsupportedVersion(s) => write!(f, "unsupported version: {}", s),
        }
    }
}

impl std::error::Error for TextIrError {}

pub fn deserialize(input: &str) -> Result<IrModule, TextIrError> {
    let mut lexer = Lexer::new(input);
    let mut parser = Parser::new(&mut lexer)?;
    parser.parse_module()
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    KwSair,
    KwEntry,
    KwFunc,
    KwParam,
    KwBlock,
    KwConst,
    KwAdd,
    KwSub,
    KwMul,
    KwDiv,
    KwRem,
    KwAnd,
    KwOr,
    KwXor,
    KwShl,
    KwLshr,
    KwAshr,
    KwEq,
    KwLt,
    KwGt,
    KwAlloca,
    KwLoad,
    KwStore,
    KwCall,
    KwPhi,
    KwGep,
    KwCast,
    KwSelect,
    KwBr,
    KwCondBr,
    KwRet,
    KwUnreachable,
    KwZext,
    KwSext,
    KwTrunc,
    KwBitcast,
    KwPtrToInt,
    KwIntToPtr,
    KwVoid,
    KwTrue,
    KwFalse,
    KwField,
    KwLoc,
    TyI1,
    TyI8,
    TyI16,
    TyI32,
    TyI64,
    TyF64,
    TyPtr,
    String(String),
    Number(u64),
    Version(String),
    Global(String),
    Local(String),
    Arrow,
    Bang,
    Colon,
    Comma,
    Equals,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Eof,
}

struct Lexer<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Lexer<'a> {
    fn new(input: &'a str) -> Self {
        Lexer { input, pos: 0 }
    }

    fn peek_char(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn advance_char(&mut self) {
        if let Some(c) = self.peek_char() {
            self.pos += c.len_utf8();
        }
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            match self.peek_char() {
                Some(c) if c.is_whitespace() => self.advance_char(),
                Some(';') => {
                    while let Some(c) = self.peek_char() {
                        self.advance_char();
                        if c == '\n' {
                            break;
                        }
                    }
                }
                _ => break,
            }
        }
    }

    fn read_string(&mut self) -> Result<Token, TextIrError> {
        self.advance_char(); // opening "
        let mut s = String::new();
        loop {
            match self.peek_char() {
                Some('"') => {
                    self.advance_char();
                    break;
                }
                Some('\\') => {
                    self.advance_char();
                    match self.peek_char() {
                        Some('"') => s.push('"'),
                        Some('\\') => s.push('\\'),
                        Some(c) => return Err(TextIrError::Parse(format!("bad escape: \\{}" , c))),
                        None => return Err(TextIrError::Parse("unterminated string".into())),
                    }
                    self.advance_char();
                }
                Some(c) => {
                    s.push(c);
                    self.advance_char();
                }
                None => return Err(TextIrError::Parse("unterminated string".into())),
            }
        }
        Ok(Token::String(s))
    }

    fn read_number(&mut self) -> Token {
        let start = self.pos;
        if self.peek_char() == Some('0') {
            self.advance_char();
            if self.peek_char() == Some('x') || self.peek_char() == Some('X') {
                self.advance_char();
                let hex_start = self.pos;
                while let Some(c) = self.peek_char() {
                    if c.is_ascii_hexdigit() {
                        self.advance_char();
                    } else {
                        break;
                    }
                }
                let hex: String = self.input[hex_start..self.pos].chars().collect();
                let val = u64::from_str_radix(&hex, 16).unwrap_or(0);
                return Token::Number(val);
            }
            if self.peek_char() == Some('.') {
                self.advance_char();
                let rest_start = self.pos;
                while let Some(c) = self.peek_char() {
                    if c.is_ascii_digit() {
                        self.advance_char();
                    } else {
                        break;
                    }
                }
                let rest: String = self.input[rest_start..self.pos].chars().collect();
                return Token::Version(format!("0.{}", rest));
            }
        }
        while let Some(c) = self.peek_char() {
            if c.is_ascii_digit() {
                self.advance_char();
            } else {
                break;
            }
        }
        let num_str: String = self.input[start..self.pos].chars().collect();
        let val = num_str.parse::<u64>().unwrap_or(0);
        Token::Number(val)
    }

    fn read_word(&mut self) -> Token {
        let start = self.pos;
        while let Some(c) = self.peek_char() {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                self.advance_char();
            } else {
                break;
            }
        }
        let word: String = self.input[start..self.pos].chars().collect();
        match word.as_str() {
            "sair" => Token::KwSair,
            "entry" => Token::KwEntry,
            "func" => Token::KwFunc,
            "param" => Token::KwParam,
            "block" => Token::KwBlock,
            "const" => Token::KwConst,
            "add" => Token::KwAdd,
            "sub" => Token::KwSub,
            "mul" => Token::KwMul,
            "div" => Token::KwDiv,
            "rem" => Token::KwRem,
            "and" => Token::KwAnd,
            "or" => Token::KwOr,
            "xor" => Token::KwXor,
            "shl" => Token::KwShl,
            "lshr" => Token::KwLshr,
            "ashr" => Token::KwAshr,
            "eq" => Token::KwEq,
            "lt" => Token::KwLt,
            "gt" => Token::KwGt,
            "alloca" => Token::KwAlloca,
            "load" => Token::KwLoad,
            "store" => Token::KwStore,
            "call" => Token::KwCall,
            "phi" => Token::KwPhi,
            "gep" => Token::KwGep,
            "cast" => Token::KwCast,
            "select" => Token::KwSelect,
            "br" => Token::KwBr,
            "cond_br" => Token::KwCondBr,
            "ret" => Token::KwRet,
            "unreachable" => Token::KwUnreachable,
            "zext" => Token::KwZext,
            "sext" => Token::KwSext,
            "trunc" => Token::KwTrunc,
            "bitcast" => Token::KwBitcast,
            "ptrtoint" => Token::KwPtrToInt,
            "inttoptr" => Token::KwIntToPtr,
            "void" => Token::KwVoid,
            "true" => Token::KwTrue,
            "false" => Token::KwFalse,
            "field" => Token::KwField,
            "loc" => Token::KwLoc,
            "i1" => Token::TyI1,
            "i8" => Token::TyI8,
            "i16" => Token::TyI16,
            "i32" => Token::TyI32,
            "i64" => Token::TyI64,
            "f64" => Token::TyF64,
            "ptr" => Token::TyPtr,
            _ => Token::String(word),
        }
    }

    fn read_global(&mut self) -> Result<Token, TextIrError> {
        self.advance_char(); // @
        let start = self.pos;
        while let Some(c) = self.peek_char() {
            if c.is_alphanumeric() || c == '_' || c == '-' || c == '.' {
                self.advance_char();
            } else {
                break;
            }
        }
        let name: String = self.input[start..self.pos].chars().collect();
        Ok(Token::Global(name))
    }

    fn read_local(&mut self) -> Result<Token, TextIrError> {
        self.advance_char(); // %
        let start = self.pos;
        while let Some(c) = self.peek_char() {
            if c.is_ascii_digit() {
                self.advance_char();
            } else {
                break;
            }
        }
        let num_str: String = self.input[start..self.pos].chars().collect();
        Ok(Token::Local(num_str))
    }

    fn next_token(&mut self) -> Result<Token, TextIrError> {
        self.skip_whitespace_and_comments();
        match self.peek_char() {
            None => Ok(Token::Eof),
            Some('"') => self.read_string(),
            Some('@') => self.read_global(),
            Some('%') => self.read_local(),
            Some('-') => {
                self.advance_char();
                if self.peek_char() == Some('>') {
                    self.advance_char();
                    Ok(Token::Arrow)
                } else {
                    Err(TextIrError::Parse("expected '->'".into()))
                }
            }
            Some(':') => {
                self.advance_char();
                Ok(Token::Colon)
            }
            Some(',') => {
                self.advance_char();
                Ok(Token::Comma)
            }
            Some('=') => {
                self.advance_char();
                Ok(Token::Equals)
            }
            Some('!') => {
                self.advance_char();
                Ok(Token::Bang)
            }
            Some('(') => {
                self.advance_char();
                Ok(Token::LParen)
            }
            Some(')') => {
                self.advance_char();
                Ok(Token::RParen)
            }
            Some('{') => {
                self.advance_char();
                Ok(Token::LBrace)
            }
            Some('}') => {
                self.advance_char();
                Ok(Token::RBrace)
            }
            Some('[') => {
                self.advance_char();
                Ok(Token::LBracket)
            }
            Some(']') => {
                self.advance_char();
                Ok(Token::RBracket)
            }
            Some(c) if c.is_ascii_digit() => Ok(self.read_number()),
            Some(c) if c.is_alphabetic() || c == '_' => Ok(self.read_word()),
            Some(c) => Err(TextIrError::Parse(format!("unexpected character: {}", c))),
        }
    }
}

struct Parser<'a> {
    lexer: &'a mut Lexer<'a>,
    current: Token,
    value_map: HashMap<ValueId, ValueId>,
}

impl<'a> Parser<'a> {
    fn new(lexer: &'a mut Lexer<'a>) -> Result<Self, TextIrError> {
        let current = lexer.next_token()?;
        Ok(Parser {
            lexer,
            current,
            value_map: HashMap::new(),
        })
    }

    fn advance(&mut self) -> Result<(), TextIrError> {
        self.current = self.lexer.next_token()?;
        Ok(())
    }

    fn expect(&mut self, tok: &Token) -> Result<(), TextIrError> {
        if &self.current == tok {
            self.advance()
        } else {
            Err(TextIrError::Parse(format!(
                "expected {:?}, got {:?}",
                tok, self.current
            )))
        }
    }

    fn parse_module(&mut self) -> Result<IrModule, TextIrError> {
        self.expect(&Token::KwSair)?;
        let version = self.parse_version()?;
        if version != "0.1" {
            return Err(TextIrError::UnsupportedVersion(format!(
                "expected 0.1, got {}",
                version
            )));
        }

        self.expect(&Token::KwEntry)?;
        let entry = self.parse_string()?;
        let mut builder = IrBuilder::new(&entry);

        while self.current != Token::Eof {
            self.parse_function(&mut builder)?;
        }

        let module = builder.finish();
        module.validate().map_err(TextIrError::Validation)?;
        Ok(module)
    }

    fn parse_number(&mut self) -> Result<u64, TextIrError> {
        match &self.current {
            Token::Number(n) => {
                let v = *n;
                self.advance()?;
                Ok(v)
            }
            other => Err(TextIrError::Parse(format!("expected number, got {:?}", other))),
        }
    }

    fn parse_version(&mut self) -> Result<String, TextIrError> {
        match &self.current {
            Token::Version(v) => {
                let s = v.clone();
                self.advance()?;
                Ok(s)
            }
            other => Err(TextIrError::Parse(format!("expected version, got {:?}", other))),
        }
    }

    fn parse_string(&mut self) -> Result<String, TextIrError> {
        match &self.current {
            Token::String(s) => {
                let v = s.clone();
                self.advance()?;
                Ok(v)
            }
            other => Err(TextIrError::Parse(format!("expected string, got {:?}", other))),
        }
    }

    fn parse_type(&mut self) -> Result<IrType, TextIrError> {
        let tok = self.current.clone();
        self.advance()?;
        match tok {
            Token::TyI1 => Ok(IrType::I1),
            Token::TyI8 => Ok(IrType::I8),
            Token::TyI16 => Ok(IrType::I16),
            Token::TyI32 => Ok(IrType::I32),
            Token::TyI64 => Ok(IrType::I64),
            Token::TyF64 => Ok(IrType::F64),
            Token::TyPtr => Ok(IrType::Pointer),
            Token::KwVoid => Ok(IrType::Void),
            other => Err(TextIrError::Parse(format!("expected type, got {:?}", other))),
        }
    }

    fn parse_function(&mut self, builder: &mut IrBuilder) -> Result<(), TextIrError> {
        self.expect(&Token::KwFunc)?;
        let name = self.parse_global_name()?;
        self.expect(&Token::Arrow)?;
        let return_ty = self.parse_type()?;

        let declared_entry = if self.current == Token::KwEntry {
            self.advance()?;
            self.parse_string()?
        } else {
            "entry".to_string()
        };

        self.expect(&Token::LBrace)?;
        builder.start_function(name, return_ty);
        self.value_map.clear();

        while self.current == Token::KwParam {
            self.parse_param(builder)?;
        }

        let mut first_block_label: Option<String> = None;
        while self.current == Token::KwBlock {
            let label = self.parse_block(builder)?;
            if first_block_label.is_none() {
                first_block_label = Some(label.clone());
                if label != declared_entry {
                    return Err(TextIrError::Parse(format!(
                        "entry block mismatch: declared '{}', first block is '{}'",
                        declared_entry, label
                    )));
                }
                builder.set_entry_block(&label);
            }
        }

        self.expect(&Token::RBrace)?;
        Ok(())
    }

    fn parse_param(&mut self, builder: &mut IrBuilder) -> Result<(), TextIrError> {
        self.expect(&Token::KwParam)?;
        let ty = self.parse_type()?;
        let textual_id = self.parse_local_id()?;
        let id = builder.add_param(ty, "");
        self.value_map.insert(textual_id, id);
        Ok(())
    }

    fn parse_block(&mut self, builder: &mut IrBuilder) -> Result<String, TextIrError> {
        self.expect(&Token::KwBlock)?;
        let label = self.parse_string()?;
        self.expect(&Token::Colon)?;
        builder.new_block(label.clone());

        while !self.is_terminator_start() {
            let maybe_textual_result = self.peek_local_def()?;
            let result_id = self.parse_instruction(builder)?;
            if let (Some(textual), Some(actual)) = (maybe_textual_result, result_id) {
                self.value_map.insert(textual, actual);
            }
            if self.current == Token::Bang {
                let loc = self.parse_debug_loc()?;
                builder.set_last_debug_loc(Some(loc));
            }
        }

        let term = self.parse_terminator()?;
        builder.set_terminator(term);

        Ok(label)
    }

    fn is_terminator_start(&self) -> bool {
        matches!(
            self.current,
            Token::KwBr | Token::KwCondBr | Token::KwRet | Token::KwUnreachable
        )
    }

    fn peek_local_def(&mut self) -> Result<Option<ValueId>, TextIrError> {
        if let Token::Local(s) = &self.current {
            let textual_id = s.parse::<ValueId>().map_err(|_| {
                TextIrError::Parse(format!("invalid local id: {}", s))
            })?;
            let saved_current = self.current.clone();
            let saved_pos = self.lexer.pos;
            self.advance()?;
            let is_def = self.current == Token::Equals;
            self.current = saved_current;
            self.lexer.pos = saved_pos;
            if is_def {
                self.advance()?; // consume local
                self.expect(&Token::Equals)?;
                Ok(Some(textual_id))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    fn parse_instruction(&mut self, builder: &mut IrBuilder) -> Result<Option<ValueId>, TextIrError> {
        let tok = self.current.clone();
        self.advance()?;
        match tok {
            Token::KwConst => {
                let ty = self.parse_type()?;
                let constant = self.parse_constant(ty)?;
                let id = builder.const_value(constant);
                Ok(Some(id))
            }
            Token::KwAdd => self.parse_binop(builder, |b, t, l, r| b.add(t, l, r)),
            Token::KwSub => self.parse_binop(builder, |b, t, l, r| b.sub(t, l, r)),
            Token::KwMul => self.parse_binop(builder, |b, t, l, r| b.mul(t, l, r)),
            Token::KwDiv => self.parse_binop(builder, |b, t, l, r| b.div(t, l, r)),
            Token::KwRem => self.parse_binop(builder, |b, t, l, r| b.rem(t, l, r)),
            Token::KwAnd => self.parse_binop(builder, |b, t, l, r| b.and(t, l, r)),
            Token::KwOr => self.parse_binop(builder, |b, t, l, r| b.or(t, l, r)),
            Token::KwXor => self.parse_binop(builder, |b, t, l, r| b.xor(t, l, r)),
            Token::KwShl => self.parse_binop(builder, |b, t, l, r| b.shl(t, l, r)),
            Token::KwLshr => self.parse_binop(builder, |b, t, l, r| b.lshr(t, l, r)),
            Token::KwAshr => self.parse_binop(builder, |b, t, l, r| b.ashr(t, l, r)),
            Token::KwEq => self.parse_binop(builder, |b, t, l, r| b.eq(t, l, r)),
            Token::KwLt => self.parse_binop(builder, |b, t, l, r| b.lt(t, l, r)),
            Token::KwGt => self.parse_binop(builder, |b, t, l, r| b.gt(t, l, r)),
            Token::KwCast => {
                let op = self.parse_cast_op()?;
                let from_ty = self.parse_type()?;
                let to_ty = self.parse_type()?;
                let value = self.parse_value_ref()?;
                let id = builder.cast(op, from_ty, to_ty, value);
                Ok(Some(id))
            }
            Token::KwSelect => {
                let ty = self.parse_type()?;
                let condition = self.parse_value_ref()?;
                self.expect(&Token::Comma)?;
                let then_value = self.parse_value_ref()?;
                self.expect(&Token::Comma)?;
                let else_value = self.parse_value_ref()?;
                let id = builder.select(ty, condition, then_value, else_value);
                Ok(Some(id))
            }
            Token::KwAlloca => {
                let ty = self.parse_type()?;
                let count = if self.current == Token::Comma {
                    self.advance()?;
                    self.parse_number()? as u32
                } else {
                    1
                };
                let id = builder.alloca_array(ty, count);
                Ok(Some(id))
            }
            Token::KwLoad => {
                let ty = self.parse_type()?;
                let addr = self.parse_value_ref()?;
                let id = builder.load(ty, addr);
                Ok(Some(id))
            }
            Token::KwStore => {
                let ty = self.parse_type()?;
                let value = self.parse_value_ref()?;
                self.expect(&Token::Comma)?;
                let addr = self.parse_value_ref()?;
                builder.store(ty, value, addr);
                Ok(None)
            }
            Token::KwCall => {
                let ret_ty = self.parse_type()?;
                let callee = self.parse_global_name()?;
                self.expect(&Token::LParen)?;
                let mut args = Vec::new();
                while self.current != Token::RParen {
                    args.push(self.parse_value_ref()?);
                    if self.current == Token::Comma {
                        self.advance()?;
                    }
                }
                self.expect(&Token::RParen)?;
                let id = builder.call(ret_ty, callee, args);
                Ok(id)
            }
            Token::KwPhi => {
                let ty = self.parse_type()?;
                let mut incoming = Vec::new();
                loop {
                    self.expect(&Token::LBracket)?;
                    let label = self.parse_string()?;
                    self.expect(&Token::Comma)?;
                    let value = self.parse_value_ref()?;
                    self.expect(&Token::RBracket)?;
                    incoming.push((value, label));
                    if self.current != Token::Comma {
                        break;
                    }
                    self.advance()?;
                }
                let id = builder.phi(ty, incoming);
                Ok(Some(id))
            }
            Token::KwGep => {
                let elem_ty = self.parse_type()?;
                let base = self.parse_value_ref()?;
                self.expect(&Token::Comma)?;
                let mut indices = Vec::new();
                loop {
                    indices.push(self.parse_gep_index()?);
                    if self.current != Token::Comma {
                        break;
                    }
                    self.advance()?;
                }
                let id = builder.gep(elem_ty, base, indices);
                Ok(Some(id))
            }
            other => Err(TextIrError::Parse(format!("expected instruction, got {:?}", other))),
        }
    }

    fn parse_binop<F>(&mut self, builder: &mut IrBuilder, mut f: F) -> Result<Option<ValueId>, TextIrError>
    where
        F: FnMut(&mut IrBuilder, IrType, ValueId, ValueId) -> ValueId,
    {
        let ty = self.parse_type()?;
        let lhs = self.parse_value_ref()?;
        self.expect(&Token::Comma)?;
        let rhs = self.parse_value_ref()?;
        let id = f(builder, ty, lhs, rhs);
        Ok(Some(id))
    }

    fn parse_terminator(&mut self) -> Result<Terminator, TextIrError> {
        let tok = self.current.clone();
        self.advance()?;
        match tok {
            Token::KwBr => {
                let target = self.parse_string()?;
                Ok(Terminator::Branch { target })
            }
            Token::KwCondBr => {
                let cond = self.parse_value_ref()?;
                self.expect(&Token::Comma)?;
                let true_target = self.parse_string()?;
                self.expect(&Token::Comma)?;
                let false_target = self.parse_string()?;
                Ok(Terminator::CondBranch {
                    condition: cond,
                    true_target,
                    false_target,
                })
            }
            Token::KwRet => {
                if self.current == Token::KwVoid {
                    self.advance()?;
                    Ok(Terminator::Return { value: None })
                } else {
                    let _ty = self.parse_type()?;
                    let value = self.parse_value_ref()?;
                    Ok(Terminator::Return { value: Some(value) })
                }
            }
            Token::KwUnreachable => Ok(Terminator::Unreachable),
            other => Err(TextIrError::Parse(format!("expected terminator, got {:?}", other))),
        }
    }

    fn parse_cast_op(&mut self) -> Result<CastOp, TextIrError> {
        let tok = self.current.clone();
        self.advance()?;
        match tok {
            Token::KwZext => Ok(CastOp::Zext),
            Token::KwSext => Ok(CastOp::Sext),
            Token::KwTrunc => Ok(CastOp::Trunc),
            Token::KwBitcast => Ok(CastOp::Bitcast),
            Token::KwPtrToInt => Ok(CastOp::PtrToInt),
            Token::KwIntToPtr => Ok(CastOp::IntToPtr),
            other => Err(TextIrError::Parse(format!("expected cast op, got {:?}", other))),
        }
    }

    fn parse_value_ref(&mut self) -> Result<ValueId, TextIrError> {
        match &self.current {
            Token::Local(s) => {
                let textual_id = s.parse::<ValueId>().map_err(|_| {
                    TextIrError::Parse(format!("invalid local id: {}", s))
                })?;
                self.advance()?;
                self.value_map
                    .get(&textual_id)
                    .copied()
                    .ok_or_else(|| TextIrError::Parse(format!("undefined value: %{}", textual_id)))
            }
            other => Err(TextIrError::Parse(format!("expected value reference, got {:?}", other))),
        }
    }

    fn parse_gep_index(&mut self) -> Result<GepIndex, TextIrError> {
        if self.current == Token::KwField {
            self.advance()?;
            let n = self.parse_number()? as u32;
            Ok(GepIndex::StructField(n))
        } else {
            let id = self.parse_value_ref()?;
            Ok(GepIndex::Dynamic(id))
        }
    }

    fn parse_debug_loc(&mut self) -> Result<DebugLoc, TextIrError> {
        self.expect(&Token::Bang)?;
        self.expect(&Token::KwLoc)?;
        let file = self.parse_string()?;
        let line = self.parse_number()? as u32;
        let column = if let Token::Number(_) = self.current {
            Some(self.parse_number()? as u32)
        } else {
            None
        };
        Ok(DebugLoc { file, line, column })
    }

    fn parse_constant(&mut self, ty: IrType) -> Result<Constant, TextIrError> {
        match ty {
            IrType::I1 => {
                if self.current == Token::KwTrue {
                    self.advance()?;
                    Ok(Constant::I1(true))
                } else if self.current == Token::KwFalse {
                    self.advance()?;
                    Ok(Constant::I1(false))
                } else {
                    let n = self.parse_number()?;
                    Ok(Constant::I1(n != 0))
                }
            }
            IrType::I8 => {
                let n = self.parse_number()?;
                Ok(Constant::I8(n as u8))
            }
            IrType::I16 => {
                let n = self.parse_number()?;
                Ok(Constant::I16(n as u16))
            }
            IrType::I32 => {
                let n = self.parse_number()?;
                Ok(Constant::I32(n as u32))
            }
            IrType::I64 => {
                let n = self.parse_number()?;
                Ok(Constant::I64(n))
            }
            IrType::F64 => {
                let n = self.parse_number()?;
                Ok(Constant::F64(f64::from_bits(n)))
            }
            other => Err(TextIrError::Parse(format!("cannot represent constant for type {:?}", other))),
        }
    }

    fn parse_global_name(&mut self) -> Result<String, TextIrError> {
        match &self.current {
            Token::Global(s) => {
                let name = s.clone();
                self.advance()?;
                Ok(name)
            }
            other => Err(TextIrError::Parse(format!("expected global name, got {:?}", other))),
        }
    }

    fn parse_local_id(&mut self) -> Result<ValueId, TextIrError> {
        match &self.current {
            Token::Local(s) => {
                let id = s.parse::<ValueId>().map_err(|_| {
                    TextIrError::Parse(format!("invalid local id: {}", s))
                })?;
                self.advance()?;
                Ok(id)
            }
            other => Err(TextIrError::Parse(format!("expected local id, got {:?}", other))),
        }
    }
}

// Extension method for IrBuilder used by the text parser.
impl IrBuilder {
    fn const_value(&mut self, constant: Constant) -> ValueId {
        match constant {
            Constant::I1(v) => self.const_i1(v),
            Constant::I8(v) => self.const_i8(v),
            Constant::I16(v) => self.const_i16(v),
            Constant::I32(v) => self.const_i32(v),
            Constant::I64(v) => self.const_i64(v),
            Constant::F64(v) => self.const_f64(v),
        }
    }
}
