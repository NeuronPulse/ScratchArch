use std::collections::HashMap;

use crate::errors::LlvmError;

#[derive(Debug, Clone, PartialEq)]
pub enum LlvmType {
    I1,
    I8,
    I16,
    I32,
    I64,
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
    /// An inline `getelementptr` constant expression used directly in an
    /// operand position (`load i8, ptr getelementptr inbounds ([N x T], ptr
    /// @g, i64 0, i64 k)`). Clang emits these when a global sub-object is
    /// addressed directly; it is lowered to the same byte-addressed GEP the
    /// instruction form produces.
    GepConstExpr {
        elem_ty: LlvmType,
        base: Box<LlvmValue>,
        indices: Vec<LlvmValue>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum IcmpPred {
    Eq,
    Ne,
    Slt,
    Sgt,
    Sle,
    Sge,
    Ult,
    Ugt,
    Ule,
    Uge,
}

/// LLVM integer-conversion operators that map onto SAIR `CastOp`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CastOp {
    Zext,
    Sext,
    Trunc,
    Bitcast,
    PtrToInt,
    IntToPtr,
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
    /// Integer conversion: `%d = zext i8 %v to i32` etc.
    Cast {
        dest: String,
        op: CastOp,
        from_ty: LlvmType,
        to_ty: LlvmType,
        value: LlvmValue,
    },
    /// `%d = select i1 %c, <ty> %a, <ty> %b`
    Select {
        dest: String,
        ty: LlvmType,
        cond: LlvmValue,
        then_value: LlvmValue,
        else_value: LlvmValue,
    },
    /// Terminator `switch <ty> %v, label %def [ <ty> c, label %t ... ]`.
    Switch {
        cond_ty: LlvmType,
        cond: LlvmValue,
        default_target: String,
        cases: Vec<(LlvmConst, String)>,
    },
    /// Terminator `unreachable`.
    Unreachable,
    /// `%r = phi <ty> [ <value>, %<pred> ] [ <value>, %<pred> ] ...`
    Phi {
        dest: String,
        ty: LlvmType,
        /// One incoming `(value, predecessor block label)` per predecessor.
        incoming: Vec<(LlvmValue, String)>,
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
    /// Named struct type definitions collected from the module
    /// (`%struct.Pair = type { i32, i32 }`). Field lists are needed to compute
    /// layout offsets in the translator.
    pub struct_types: HashMap<String, Vec<LlvmType>>,
    /// Module-level global variable/constant definitions (`@g = global …`).
    pub globals: Vec<LlvmGlobal>,
}

/// A module-level global variable or constant: `@name = global <ty> <init>`.
#[derive(Debug, Clone, PartialEq)]
pub struct LlvmGlobal {
    pub name: String,
    pub ty: LlvmType,
    /// `true` for `constant` (immutable); `false` for writable `global`.
    pub is_constant: bool,
    pub init: LlvmGlobalInit,
}

/// The static initializer of a global. `Zero` is `zeroinitializer` (and the
/// no-initializer form of `external`/`common` globals), which is legal for any
/// type; the remaining variants carry scalar/pointer/array values.
#[derive(Debug, Clone, PartialEq)]
pub enum LlvmGlobalInit {
    /// `zeroinitializer` (or a global with no explicit initializer).
    Zero,
    /// A typed scalar integer (`i32 41`, `i8 -56`, `i1 true`).
    Scalar(LlvmConst),
    /// `null` — the zero address for a pointer-typed element.
    Null,
    /// `@other` — the address of another global (pointer relocations).
    GlobalRef(String),
    /// `c"…"` — a decoded byte string; pairs with a `[N x i8]` element.
    Bytes(Vec<u8>),
    /// `[<ty> <val>, …]` — a typed constant-array element list.
    Array(Vec<LlvmGlobalInit>),
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
    /// A numeric basic-block label header (`13:`). Clang names every block
    /// with the next unused SSA id and prints it as `%N:`; the lexer only
    /// produces this token for a digit run directly followed by a colon, so it
    /// cannot be confused with an integer literal.
    NumLabel(String),
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
    /// A `c"…"` LLVM string literal, decoded to its raw bytes. Array-of-`i8`
    /// global initializers print this way.
    StringLiteral(Vec<u8>),
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
        // Re-check the current character after every step. In particular, a
        // `;` comment stops *at* the following newline, and that newline must
        // then be consumed by the whitespace arm — not masked by the stale
        // `';'` from before the comment.
        loop {
            match self.peek() {
                Some(';') => {
                    while let Some(c) = self.peek() {
                        if c == '\n' {
                            break;
                        }
                        self.advance();
                    }
                }
                Some(c) if c.is_whitespace() => {
                    self.advance();
                }
                _ => break,
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
        // A digit run immediately followed by a colon is a numeric basic-block
        // label header (`13:`), not an integer literal.
        if self.peek() == Some(':') {
            self.advance();
            return Token::NumLabel(s);
        }
        Token::Number(s.parse::<i64>().unwrap())
    }

    /// Read a `c"…"` string literal with `self.pos` on the opening quote. LLVM
    /// escapes (`\n`, `\t`, `\xx` hex, `\\`, `\"`, …) decode to their byte
    /// values. The string is an array-of-`i8` initializer, so each decoded byte
    /// is one element.
    fn read_string_literal(&mut self) -> Token {
        self.advance(); // opening quote
        let mut bytes = Vec::new();
        loop {
            match self.peek() {
                None => break, // unterminated; accept what we have
                Some('"') => {
                    self.advance();
                    break;
                }
                Some('\\') => {
                    self.advance(); // past the backslash
                    if let Some(esc) = self.peek() {
                        let is_hex = esc.is_ascii_hexdigit();
                        let byte = match esc {
                            'n' => b'\n',
                            't' => b'\t',
                            'r' => b'\r',
                            'a' => 0x07,
                            'b' => 0x08,
                            'f' => 0x0C,
                            'v' => 0x0B,
                            '\\' => b'\\',
                            '\'' => b'\'',
                            '"' => b'"',
                            '?' => b'?',
                            h if is_hex => {
                                let mut v = h.to_digit(16).unwrap() as u16;
                                self.advance();
                                if let Some(h2) = self.peek() {
                                    if h2.is_ascii_hexdigit() {
                                        v = v * 16 + h2.to_digit(16).unwrap() as u16;
                                        self.advance();
                                    }
                                }
                                v as u8
                            }
                            other => other as u8,
                        };
                        bytes.push(byte);
                        if !is_hex {
                            self.advance();
                        }
                    }
                }
                Some(c) => {
                    bytes.push(c as u8);
                    self.advance();
                }
            }
        }
        Token::StringLiteral(bytes)
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

            // A `c"…"` string literal is an array-of-`i8` global initializer.
            // Intercept before the identifier keyword match: `c` followed by a
            // double quote is a string, not the letter `c`.
            if word == "c" && self.peek() == Some('"') {
                return self.read_string_literal();
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
                    // Parse the magnitude as u64 and negate with wrapping so the
                    // LLVM-minimal negative constant `-9223372036854775808`
                    // (i64::MIN) survives the round trip instead of overflowing.
                    let mag: u64 = s.parse().unwrap_or(u64::MAX);
                    Token::Number(mag.wrapping_neg() as i64)
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
                    // `i64` is not a reserved keyword token; recognize it from
                    // the identifier stream.
                    Token::Ident(s) if s == "i64" => Ok(LlvmType::I64),
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
            // An inline `getelementptr` constant expression appears as an
            // operand (address) instead of a named SSA value.
            Token::Ident(s) if s == "getelementptr" => self.parse_gep_const_expr(),
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

    fn parse_params(&mut self, names_required: bool) -> Result<Vec<(LlvmType, String)>, LlvmError> {
        let mut params = Vec::new();
        self.expect(&Token::OpenParen)?;
        while self.current != Token::CloseParen {
            // Variadic marker `...`.
            if matches!(&self.current, Token::Ident(s) if s == "...") {
                self.advance();
                break;
            }
            let ty = self.parse_type()?;
            // Parameter attributes sit between the type and the name
            // (`i32 noundef %0`).
            self.skip_param_attrs();
            let name = match &self.current {
                Token::LocalId(n) => {
                    let name = n.clone();
                    self.advance();
                    name
                }
                _ => {
                    if names_required {
                        return Err(LlvmError::Parse(format!(
                            "expected local id, got {:?}",
                            self.current
                        )));
                    }
                    String::new()
                }
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
            // Unsigned predicates are not reserved keywords; recognize them
            // from the identifier stream.
            Token::Ident(s) if s == "ult" => Ok(IcmpPred::Ult),
            Token::Ident(s) if s == "ugt" => Ok(IcmpPred::Ugt),
            Token::Ident(s) if s == "ule" => Ok(IcmpPred::Ule),
            Token::Ident(s) if s == "uge" => Ok(IcmpPred::Uge),
            _ => Err(LlvmError::UnsupportedIcmpPredicate(format!("{:?}", tok))),
        }
    }

    /// Parameter/argument attributes that may appear between a type and the
    /// value in a parameter list or call (`i32 noundef %0`). Only plain
    /// identifier attributes are handled; anything with an argument list is
    /// left for the caller to reject.
    fn skip_param_attrs(&mut self) {
        while let Token::Ident(s) = &self.current {
            let s = s.clone();
            let known = matches!(
                s.as_str(),
                "noundef"
                    | "signext"
                    | "zeroext"
                    | "noalias"
                    | "nocapture"
                    | "nonnull"
                    | "returned"
                    | "inreg"
                    | "sret"
                    | "swiftself"
                    | "nest"
                    | "readonly"
                    | "writeonly"
                    | "readnone"
                    | "immarg"
                    | "captures"
                    | "align"
            );
            if !known {
                break;
            }
            self.advance();
            // `align N` (clang emits it on call operands: `ptr align 4 %x`)
            // carries a trailing integer that is not an attribute word.
            if s == "align" {
                if let Token::Number(_) = self.current {
                    self.advance();
                }
            }
        }
    }

    /// Skip attribute/`#N` tokens between a function header and its body.
    fn skip_to_open_brace(&mut self) -> Result<(), LlvmError> {
        while self.current != Token::OpenBrace {
            if self.current == Token::Eof {
                return Err(LlvmError::Parse(
                    "reached end of input before function body '{'".into(),
                ));
            }
            self.advance();
        }
        Ok(())
    }

    /// Skip linkage/preemption/calling-convention keyword idents that precede
    /// the return type of a `define`/`declare` (`dso_local`, `internal`, …).
    fn skip_linkage_attrs(&mut self) {
        while let Token::Ident(s) = &self.current {
            let known = matches!(
                s.as_str(),
                "dso_local"
                    | "dso_preemptable"
                    | "internal"
                    | "private"
                    | "external"
                    | "available_externally"
                    | "linkonce"
                    | "linkonce_odr"
                    | "weak"
                    | "weak_odr"
                    | "common"
                    | "appending"
                    | "extern_weak"
                    | "thread_local"
                    | "unnamed_addr"
                    | "local_unnamed_addr"
                    | "ccc"
                    | "fastcc"
                    | "coldcc"
                    | "tailcc"
                    | "nounwind"
                    | "willreturn"
            );
            if !known {
                break;
            }
            self.advance();
        }
    }

    fn parse_call_args(&mut self) -> Result<Vec<LlvmValue>, LlvmError> {
        let mut args = Vec::new();
        self.expect(&Token::OpenParen)?;
        while self.current != Token::CloseParen {
            // Call arguments are written `<ty> <attr>* <value>`; attribute words
            // may also precede the type (`noundef i32 5` in some frontends).
            self.skip_param_attrs();
            match &self.current {
                // A bare value without an explicit type prefix is unusual in
                // calls but tolerated.
                Token::Number(_) | Token::LocalId(_) | Token::GlobalId(_) | Token::True | Token::False => {
                    let val = self.parse_value()?;
                    args.push(val);
                }
                _ => {
                    let ty = self.parse_type()?;
                    self.skip_param_attrs();
                    match &self.current {
                        Token::Number(n) => {
                            let val = *n;
                            self.advance();
                            args.push(LlvmValue::Const(LlvmConst { value: val, ty }));
                        }
                        Token::True => {
                            self.advance();
                            args.push(LlvmValue::Const(LlvmConst { value: 1, ty }));
                        }
                        Token::False => {
                            self.advance();
                            args.push(LlvmValue::Const(LlvmConst { value: 0, ty }));
                        }
                        Token::LocalId(name) => {
                            let n = name.clone();
                            self.advance();
                            args.push(LlvmValue::Local(n));
                        }
                        Token::GlobalId(name) => {
                            let n = name.clone();
                            self.advance();
                            args.push(LlvmValue::Global(n));
                        }
                        _ => {
                            return Err(LlvmError::Parse(format!(
                                "expected call argument value, got {:?}",
                                self.current
                            )))
                        }
                    }
                }
            }
            if self.current == Token::Comma {
                self.advance();
            } else {
                break;
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

    /// Parse an inline `getelementptr` constant expression operand:
    /// `getelementptr inbounds (<elem_ty>, ptr <base>, <idx>…).`
    ///
    /// The printed form wraps the source type, the `ptr` base and the (typed)
    /// index list in one parenthesized group, unlike the instruction form whose
    /// indices run to the end of the line.
    fn parse_gep_const_expr(&mut self) -> Result<LlvmValue, LlvmError> {
        // self.current is `Ident("getelementptr")`.
        self.advance();
        if matches!(&self.current, Token::Ident(s) if s == "inbounds") {
            self.advance();
        }
        self.expect(&Token::OpenParen)?;
        let elem_ty = self.parse_type()?;
        self.expect(&Token::Comma)?;
        self.expect(&Token::Ptr)?;
        let base = self.parse_value()?;
        let mut indices = Vec::new();
        while self.current != Token::CloseParen {
            self.expect(&Token::Comma)?;
            let idx = self.parse_value()?;
            indices.push(idx);
        }
        self.expect(&Token::CloseParen)?;
        Ok(LlvmValue::GepConstExpr {
            elem_ty,
            base: Box::new(base),
            indices,
        })
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

    /// Consume trailing instruction metadata attachments (`, !dbg !19`).
    ///
    /// Called after an instruction has been fully parsed. `skip_opt_align`
    /// (used by store/load/alloca) may already have consumed an `, align N`
    /// group; any further `, ...` groups here are metadata nodes.
    fn skip_trailing_metadata(&mut self) {
        while self.current == Token::Comma {
            self.advance();
            while matches!(&self.current, Token::Ident(_) | Token::Number(_)) {
                self.advance();
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
                    // Raw op is kept so the translator can distinguish signed
                    // (`sdiv`/`srem`) from unsigned (`udiv`/`urem`) integer ops.
                    "add" | "sub" | "mul" | "sdiv" | "udiv" | "rem" | "srem" | "urem"
                    | "and" | "or" | "xor" | "shl" | "lshr" | "ashr" => {
                        let ty = self.parse_type()?;
                        let lhs = self.parse_value_of_type(&ty)?;
                        self.expect(&Token::Comma)?;
                        let rhs = self.parse_value_of_type(&ty)?;
                        Ok(LlvmInstr::BinOp {
                            dest,
                            op: op_name,
                            ty,
                            lhs,
                            rhs,
                        })
                    }
                    "icmp" => {
                        let pred = self.parse_icmp_predicate()?;
                        let ty = self.parse_type()?;
                        let lhs = self.parse_value_of_type(&ty)?;
                        self.expect(&Token::Comma)?;
                        let rhs = self.parse_value_of_type(&ty)?;
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
                            // `call i32 %fp(...)`: the callee is an SSA value, so
                            // this is an indirect call through a function pointer.
                            // SAIR/ISA calls name their callee statically, so the
                            // pipeline has no function-pointer ABI; reject it here
                            // with an explicit diagnostic instead of misparsing or
                            // silently miscompiling. (Documented in
                            // LLVM_COMPATIBILITY.md / LLVM_TRANSLATION.md.)
                            Token::LocalId(name) => {
                                return Err(LlvmError::Parse(format!(
                                    "indirect call through '{}' is unsupported: the SAIR/ISA \
                                     call ABI requires a statically-named callee (no function-\
                                     pointer ABI)",
                                    name
                                )))
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
                    "zext" | "sext" | "trunc" | "bitcast" | "ptrtoint" | "inttoptr" => {
                        let cast_op = match op_name.as_str() {
                            "zext" => CastOp::Zext,
                            "sext" => CastOp::Sext,
                            "trunc" => CastOp::Trunc,
                            "bitcast" => CastOp::Bitcast,
                            "ptrtoint" => CastOp::PtrToInt,
                            _ => CastOp::IntToPtr,
                        };
                        let from_ty = self.parse_type()?;
                        let value = self.parse_value_of_type(&from_ty)?;
                        self.expect(&Token::Ident("to".to_string()))?;
                        let to_ty = self.parse_type()?;
                        let dest = dest.ok_or_else(|| LlvmError::Parse("cast requires dest".into()))?;
                        Ok(LlvmInstr::Cast {
                            dest,
                            op: cast_op,
                            from_ty,
                            to_ty,
                            value,
                        })
                    }
                    "select" => {
                        // `select <condty> <cond>, <ty> <a>, <ty> <b>`
                        let cond_ty = self.parse_type()?;
                        let cond = self.parse_value_of_type(&cond_ty)?;
                        self.expect(&Token::Comma)?;
                        let ty = self.parse_type()?;
                        let then_value = self.parse_value_of_type(&ty)?;
                        self.expect(&Token::Comma)?;
                        let else_value = self.parse_value_of_type(&ty)?;
                        let dest =
                            dest.ok_or_else(|| LlvmError::Parse("select requires dest".into()))?;
                        Ok(LlvmInstr::Select {
                            dest,
                            ty,
                            cond,
                            then_value,
                            else_value,
                        })
                    }
                    "switch" => {
                        let cond_ty = self.parse_type()?;
                        let cond = self.parse_value_of_type(&cond_ty)?;
                        self.expect(&Token::Comma)?;
                        self.expect(&Token::Ident("label".to_string()))?;
                        let default_target = self.parse_bare_label()?;
                        self.expect(&Token::OpenBracket)?;
                        let mut cases = Vec::new();
                        while self.current != Token::CloseBracket {
                            let case_ty = self.parse_type()?;
                            let case_val = self.parse_value_of_type(&case_ty)?;
                            let cv = match case_val {
                                LlvmValue::Const(c) => c,
                                _ => {
                                    return Err(LlvmError::Parse(format!(
                                        "switch case value must be a constant, got {:?}",
                                        case_val
                                    )))
                                }
                            };
                            self.expect(&Token::Comma)?;
                            self.expect(&Token::Ident("label".to_string()))?;
                            let target = self.parse_bare_label()?;
                            cases.push((cv, target));
                            if self.current == Token::Comma {
                                self.advance();
                            }
                        }
                        self.expect(&Token::CloseBracket)?;
                        Ok(LlvmInstr::Switch {
                            cond_ty,
                            cond,
                            default_target,
                            cases,
                        })
                    }
                    "unreachable" => Ok(LlvmInstr::Unreachable),
                    "phi" => {
                        // `%r = phi i32 [ 0, %entry ], [ %inc, %for.inc ]`. Values
                        // are typed by the phi's type (integer literals carry no
                        // prefix of their own).
                        let ty = self.parse_type()?;
                        let mut incoming = Vec::new();
                        loop {
                            // Items are separated by a comma; tolerate a bare
                            // comma before the next `[` (not emitted by clang).
                            if self.current == Token::Comma {
                                self.advance();
                            }
                            if self.current != Token::OpenBracket {
                                break;
                            }
                            self.advance();
                            let value = self.parse_value_of_type(&ty)?;
                            self.expect(&Token::Comma)?;
                            let pred = self.parse_bare_label()?;
                            self.expect(&Token::CloseBracket)?;
                            incoming.push((value, pred));
                        }
                        if incoming.is_empty() {
                            return Err(LlvmError::Parse(
                                "phi requires at least one incoming value".into(),
                            ));
                        }
                        let dest =
                            dest.ok_or_else(|| LlvmError::Parse("phi requires dest".into()))?;
                        Ok(LlvmInstr::Phi {
                            dest,
                            ty,
                            incoming,
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
        self.parse_bare_label()
    }

    /// Read a block reference written as a bare id (`label %6` → `"6"`).
    fn parse_bare_label(&mut self) -> Result<String, LlvmError> {
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
            Token::Label(l) | Token::NumLabel(l) => {
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
                Token::NumLabel(_) => break,
                Token::Eof => break,
                _ => {}
            }
            let instr = self.parse_instruction()?;
            self.skip_trailing_metadata();
            instructions.push(instr);
        }

        Ok(LlvmBlock { label, instructions })
    }

    fn parse_function(&mut self) -> Result<LlvmFunction, LlvmError> {
        self.expect(&Token::Define)?;
        self.skip_linkage_attrs();
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
        let params = self.parse_params(true)?;
        // Attribute group (`#0`), `local_unnamed_addr`, alignment etc. can sit
        // between the parameter list and the opening brace.
        self.skip_to_open_brace()?;
        self.expect(&Token::OpenBrace)?;

        let mut blocks = Vec::new();
        loop {
            match &self.current {
                Token::CloseBrace => break,
                Token::Label(_) | Token::NumLabel(_) => {
                    let block = self.parse_block()?;
                    blocks.push(block);
                }
                _ => {
                    if blocks.is_empty() {
                        // Clang does not print a header for the entry block; it
                        // starts with the first instruction right after `{`.
                        let mut instructions = Vec::new();
                        loop {
                            match &self.current {
                                Token::CloseBrace => break,
                                Token::Label(_) | Token::NumLabel(_) => break,
                                Token::Eof => break,
                                _ => {}
                            }
                            let instr = self.parse_instruction()?;
                            self.skip_trailing_metadata();
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
        self.skip_linkage_attrs();
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
        let params = self.parse_params(false)?;
        // Trailing declaration tokens (attribute groups etc.) are consumed by
        // the module-level loop.
        Ok(LlvmFunction {
            name,
            return_ty,
            params,
            blocks: Vec::new(),
            is_declaration: true,
        })
    }

    fn parse_program(&mut self) -> Result<LlvmProgram, LlvmError> {
        let mut functions = Vec::new();
        let mut struct_types = HashMap::new();
        let mut globals = Vec::new();
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
                Token::GlobalId(name) => {
                    let n = name.clone();
                    let g = self.parse_global(&n)?;
                    globals.push(g);
                }
                Token::LocalId(name) => {
                    // Module-level `%name = type { ... }` struct definitions.
                    // Collect them so the translator can compute field offsets.
                    let n = name.clone();
                    self.advance();
                    if self.current == Token::Equals {
                        self.advance();
                        if matches!(&self.current, Token::Ident(s) if s == "type") {
                            self.advance();
                            if self.current == Token::OpenBrace {
                                let fields = self.parse_struct_def_body()?;
                                struct_types.insert(n, fields);
                            }
                        }
                    }
                }
                _ => {
                    self.advance();
                }
            }
        }
        Ok(LlvmProgram {
            functions,
            struct_types,
            globals,
        })
    }

    /// Parse the `{ <type> [',', <type>]* }` body of a struct type definition,
    /// with `self.current` on the opening brace.
    fn parse_struct_def_body(&mut self) -> Result<Vec<LlvmType>, LlvmError> {
        self.expect(&Token::OpenBrace)?;
        let mut fields = Vec::new();
        while self.current != Token::CloseBrace {
            let ty = self.parse_type()?;
            fields.push(ty);
            if self.current == Token::Comma {
                self.advance();
            }
        }
        self.expect(&Token::CloseBrace)?;
        Ok(fields)
    }

    /// Parse a module-level global definition (`@name = [attrs] global/constant
    /// <ty> [init] [, attrs]`) with `self.current` on the `@name` token.
    fn parse_global(&mut self, name: &str) -> Result<LlvmGlobal, LlvmError> {
        let name_owned = name.to_string();
        self.advance(); // @name
        self.expect(&Token::Equals)?;
        // Skip linkage / visibility / attribute keywords (`dso_local`,
        // `private`, `internal`, `unnamed_addr`, `local_unnamed_addr`,
        // `external`, `common`, `thread_local`, …) down to the `global` /
        // `constant` value keyword.
        let is_constant;
        let mut external = false;
        loop {
            match &self.current {
                Token::Ident(s) if s == "global" || s == "constant" => {
                    is_constant = s == "constant";
                    self.advance();
                    break;
                }
                Token::Ident(s) if s == "external" || s == "common" => {
                    external = true;
                    self.advance();
                }
                Token::Define | Token::Declare | Token::Eof => {
                    return Err(LlvmError::Parse(format!(
                        "malformed global definition for '@{}'",
                        name_owned
                    )))
                }
                _ => self.advance(),
            }
        }
        let ty = self.parse_type()?;
        // Struct/void globals are not modeled. Reject at parse time with an
        // explicit diagnostic, so no unsupported initializer syntax needs
        // consuming afterwards.
        match &ty {
            LlvmType::Struct(_) | LlvmType::UnnamedStruct { .. } | LlvmType::Void => {
                return Err(LlvmError::Parse(format!(
                    "global '@{}' of type {:?} is not supported (global struct/void data is not modeled)",
                    name_owned, ty
                )))
            }
            _ => {}
        }
        // Only an `external`/`common` global may omit its initializer (printed
        // as `@x = external global i32[, attrs]`). Anything else must carry one,
        // and a following `@name` on the same line is that initializer (a pointer
        // relocation), not a new module item — so init-detection is keyed on the
        // `external`/`common` keyword, never on the token stream.
        let init = if external && (self.current == Token::Comma || self.at_module_item_end()) {
            LlvmGlobalInit::Zero
        } else {
            self.parse_global_init(&ty)?
        };
        self.skip_to_module_item();
        Ok(LlvmGlobal {
            name: name_owned,
            ty,
            is_constant,
            init,
        })
    }

    /// Is the parser at a module-level item boundary (start of a `define` /
    /// `declare`, another global or struct definition, or end of input)?
    fn at_module_item_end(&self) -> bool {
        matches!(
            &self.current,
            Token::Eof
                | Token::Define
                | Token::Declare
                | Token::GlobalId(_)
                | Token::LocalId(_)
        )
    }

    /// Consume tokens up to the next module-level item boundary (trailing
    /// global attributes such as `align N`, `section "…"`, `comdat`).
    fn skip_to_module_item(&mut self) {
        while !self.at_module_item_end() {
            self.advance();
        }
    }

    /// Parse a global initializer constant of the given expected type. The
    /// token grammar mirrors LLVM constant expressions: scalar integers, `null`
    /// / `@name` pointer values, `c"…"` strings, typed constant-array lists,
    /// and `zeroinitializer`.
    fn parse_global_init(&mut self, expected: &LlvmType) -> Result<LlvmGlobalInit, LlvmError> {
        match &self.current {
            Token::Number(n) => {
                let value = *n;
                self.advance();
                Ok(LlvmGlobalInit::Scalar(LlvmConst {
                    value,
                    ty: expected.clone(),
                }))
            }
            Token::True => {
                self.advance();
                Ok(LlvmGlobalInit::Scalar(LlvmConst {
                    value: 1,
                    ty: expected.clone(),
                }))
            }
            Token::False => {
                self.advance();
                Ok(LlvmGlobalInit::Scalar(LlvmConst {
                    value: 0,
                    ty: expected.clone(),
                }))
            }
            Token::GlobalId(g) => {
                let g = g.clone();
                self.advance();
                Ok(LlvmGlobalInit::GlobalRef(g))
            }
            Token::Ident(s) if s == "zeroinitializer" => {
                self.advance();
                Ok(LlvmGlobalInit::Zero)
            }
            Token::Ident(s) if s == "null" => {
                self.advance();
                Ok(LlvmGlobalInit::Null)
            }
            Token::Ident(s) if s == "undef" || s == "poison" => Err(LlvmError::Parse(format!(
                "'{}' global initializer is not supported (SAIR has no undefined values)",
                s
            ))),
            Token::StringLiteral(bytes) => {
                let b = bytes.clone();
                self.advance();
                Ok(LlvmGlobalInit::Bytes(b))
            }
            Token::OpenBracket => {
                self.advance(); // '['
                let inner = match expected {
                    LlvmType::Array { inner, .. } => (**inner).clone(),
                    _ => {
                        return Err(LlvmError::Parse(format!(
                            "array constant initializer for non-array type {:?}",
                            expected
                        )))
                    }
                };
                let mut elems = Vec::new();
                while self.current != Token::CloseBracket {
                    if self.at_module_item_end() {
                        return Err(LlvmError::Parse(
                            "unterminated global array initializer".into(),
                        ));
                    }
                    elems.push(self.parse_array_element(&inner)?);
                    if self.current == Token::Comma {
                        self.advance();
                    }
                }
                self.expect(&Token::CloseBracket)?;
                Ok(LlvmGlobalInit::Array(elems))
            }
            Token::OpenBrace => Err(LlvmError::Parse(
                "struct-literal global initializers are not supported".into(),
            )),
            other => Err(LlvmError::Parse(format!(
                "unsupported global initializer token {:?}",
                other
            ))),
        }
    }

    /// Parse one element of a typed constant-array list. Elements may carry an
    /// explicit type header (`[i32 1, ptr null]`) which is preferred, else the
    /// array's inner element type applies.
    fn parse_array_element(&mut self, expected: &LlvmType) -> Result<LlvmGlobalInit, LlvmError> {
        let declared = self.try_parse_element_type();
        let ty = declared.unwrap_or_else(|| expected.clone());
        match &ty {
            LlvmType::Array { .. }
            | LlvmType::Struct(_)
            | LlvmType::UnnamedStruct { .. }
            | LlvmType::Void => {
                return Err(LlvmError::Parse(format!(
                    "nested-aggregate global initializer element of type {:?} is not supported",
                    ty
                )))
            }
            _ => {}
        }
        match &self.current {
            Token::Number(n) => {
                let value = *n;
                self.advance();
                Ok(LlvmGlobalInit::Scalar(LlvmConst {
                    value,
                    ty: ty.clone(),
                }))
            }
            Token::True => {
                self.advance();
                Ok(LlvmGlobalInit::Scalar(LlvmConst {
                    value: 1,
                    ty: ty.clone(),
                }))
            }
            Token::False => {
                self.advance();
                Ok(LlvmGlobalInit::Scalar(LlvmConst {
                    value: 0,
                    ty: ty.clone(),
                }))
            }
            Token::GlobalId(g) => {
                let g = g.clone();
                self.advance();
                Ok(LlvmGlobalInit::GlobalRef(g))
            }
            Token::Ident(s) if s == "null" => {
                self.advance();
                Ok(LlvmGlobalInit::Null)
            }
            Token::Ident(s) if s == "zeroinitializer" => Err(LlvmError::Parse(
                "'zeroinitializer' is not valid inside an array constant list".into(),
            )),
            other => Err(LlvmError::Parse(format!(
                "unsupported global array element token {:?}",
                other
            ))),
        }
    }

    /// If the current token begins a type (used for typed array-constant
    /// elements), parse and return it; otherwise return `None`.
    fn try_parse_element_type(&mut self) -> Option<LlvmType> {
        let is_type = matches!(
            &self.current,
            Token::I1
                | Token::I8
                | Token::I16
                | Token::I32
                | Token::Ptr
                | Token::OpenBracket
                | Token::OpenBrace
                | Token::LocalId(_)
        ) || matches!(&self.current, Token::Ident(s) if s == "i64");
        if is_type {
            self.parse_type().ok()
        } else {
            None
        }
    }
}

pub fn parse_llvm(input: &str) -> Result<LlvmProgram, LlvmError> {
    let mut parser = Parser::new(input);
    parser.parse_program()
}
