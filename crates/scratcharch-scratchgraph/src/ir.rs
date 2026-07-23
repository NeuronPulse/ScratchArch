//! ScratchGraph IR: a semantic representation of Scratch programs.
//!
//! ScratchGraph is intentionally independent of `project.json` serialization.
//! It models sprites, scripts, procedures, variables, and control flow as
//! nested structures; exporters flatten these into concrete Scratch formats.

/// A Scratch project.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Project {
    pub stage: Stage,
    pub sprites: Vec<Sprite>,
}

/// The Scratch stage.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Stage {
    pub name: String,
    pub variables: Vec<Variable>,
    pub lists: Vec<List>,
    pub broadcasts: Vec<Broadcast>,
    pub scripts: Vec<Script>,
    pub procedures: Vec<Procedure>,
}

/// A Scratch sprite.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Sprite {
    pub name: String,
    pub variables: Vec<Variable>,
    pub lists: Vec<List>,
    pub scripts: Vec<Script>,
    pub procedures: Vec<Procedure>,
}

/// A named variable.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Variable {
    pub id: String,
    pub name: String,
}

/// A named list.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct List {
    pub id: String,
    pub name: String,
}

/// A named broadcast.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Broadcast {
    pub id: String,
    pub name: String,
}

/// A script triggered by an event hat.
#[derive(Debug, Clone, PartialEq)]
pub struct Script {
    pub hat: Hat,
    pub body: Vec<Stmt>,
}

/// Event hats.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Hat {
    GreenFlag,
    BroadcastReceived(String),
    /// Procedure definition trigger; the body is the procedure implementation.
    Procedure { name: String },
}

/// A custom block definition.
#[derive(Debug, Clone, PartialEq)]
pub struct Procedure {
    pub prototype: ProcedurePrototype,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProcedurePrototype {
    pub name: String,
    pub params: Vec<ProcedureParam>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProcedureParam {
    pub name: String,
    pub default: Option<Value>,
}

/// Scratch statement.
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// Evaluate an expression and discard the result.
    Expr(Expr),
    /// Set a variable to an expression value.
    SetVariable { var: String, value: Expr },
    /// Change a variable by an expression value.
    ChangeVariable { var: String, delta: Expr },
    /// Add an item to a list.
    AddToList { list: String, value: Expr },
    /// Delete all items of a list.
    DeleteAllOfList { list: String },
    /// Call a custom block.
    Call { proc: String, args: Vec<Expr> },
    /// If/then/else.
    If {
        condition: Expr,
        then_body: Vec<Stmt>,
        else_body: Vec<Stmt>,
    },
    /// Repeat a fixed number of times.
    Repeat { times: Expr, body: Vec<Stmt> },
    /// Repeat until a condition becomes true.
    RepeatUntil { condition: Expr, body: Vec<Stmt> },
    /// Repeat forever.
    Forever { body: Vec<Stmt> },
    /// Stop execution option.
    Stop { option: StopOption },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopOption {
    ThisScript,
    All,
}

/// Scratch expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(Value),
    Variable(String),
    List(String),
    ProcedureParam(String),
    Operator { opcode: String, args: Vec<Expr> },
}

/// Literal value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Number(f64),
    String(String),
    Bool(bool),
}

impl Project {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_stage(mut self, stage: Stage) -> Self {
        self.stage = stage;
        self
    }

    pub fn add_sprite(&mut self, sprite: Sprite) {
        self.sprites.push(sprite);
    }
}

impl Stage {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    pub fn add_variable(&mut self, var: Variable) {
        self.variables.push(var);
    }

    pub fn add_script(&mut self, script: Script) {
        self.scripts.push(script);
    }

    pub fn add_procedure(&mut self, proc: Procedure) {
        self.procedures.push(proc);
    }
}

impl Sprite {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }
}

impl Variable {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
        }
    }
}

impl Procedure {
    pub fn new(name: impl Into<String>, params: Vec<ProcedureParam>, body: Vec<Stmt>) -> Self {
        Self {
            prototype: ProcedurePrototype {
                name: name.into(),
                params,
            },
            body,
        }
    }
}

impl ProcedureParam {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            default: None,
        }
    }
}

impl Script {
    pub fn new(hat: Hat, body: Vec<Stmt>) -> Self {
        Self { hat, body }
    }
}

impl Expr {
    pub fn variable(name: impl Into<String>) -> Self {
        Expr::Variable(name.into())
    }

    pub fn number(v: f64) -> Self {
        Expr::Literal(Value::Number(v))
    }

    pub fn string(v: impl Into<String>) -> Self {
        Expr::Literal(Value::String(v.into()))
    }

    pub fn bool(v: bool) -> Self {
        Expr::Literal(Value::Bool(v))
    }

    pub fn operator(opcode: impl Into<String>, args: Vec<Expr>) -> Self {
        Expr::Operator {
            opcode: opcode.into(),
            args,
        }
    }
}
