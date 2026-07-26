//! ScratchGraph IR: a semantic representation of Scratch programs.
//!
//! ScratchGraph is intentionally independent of `project.json` serialization.
//! It models sprites, scripts, procedures, variables, lists, broadcasts,
//! control flow, and memory as nested structures; exporters flatten these into
//! concrete Scratch formats.

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
    pub costumes: Vec<Costume>,
    pub sounds: Vec<Sound>,
}

/// A Scratch sprite.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Sprite {
    pub name: String,
    pub variables: Vec<Variable>,
    pub lists: Vec<List>,
    pub broadcasts: Vec<Broadcast>,
    pub scripts: Vec<Script>,
    pub procedures: Vec<Procedure>,
    pub costumes: Vec<Costume>,
    pub sounds: Vec<Sound>,
}

/// Scope of a Scratch variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum VariableScope {
    /// Visible to every sprite and the stage.
    #[default]
    Global,
    /// Owned by a single sprite.
    SpriteLocal,
    /// Compiler-generated temporary, typically global for convenience.
    Temporary,
}

/// A named variable.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Variable {
    pub id: String,
    pub name: String,
    pub scope: VariableScope,
}

/// Scope of a Scratch list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ListScope {
    /// Visible to every sprite and the stage.
    #[default]
    Global,
    /// Owned by a single sprite.
    SpriteLocal,
}

/// A named list.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct List {
    pub id: String,
    pub name: String,
    pub scope: ListScope,
}

/// A named broadcast.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Broadcast {
    pub id: String,
    pub name: String,
}

/// A Scratch costume or backdrop.
#[derive(Debug, Clone, PartialEq)]
pub struct Costume {
    pub name: String,
    /// md5ext identifier (e.g. "abc123.svg").
    pub asset_id: String,
    /// Bitmap (true) or vector/SVG (false).
    pub bitmap: bool,
    pub rotation_center_x: f64,
    pub rotation_center_y: f64,
}

/// A Scratch sound.
#[derive(Debug, Clone, PartialEq)]
pub struct Sound {
    pub name: String,
    /// md5ext identifier (e.g. "abc123.wav").
    pub asset_id: String,
    pub rate: u32,
    pub sample_count: u32,
    /// "wav", "mp3", or other format.
    pub format: String,
}

/// A script triggered by an event hat.
#[derive(Debug, Clone, PartialEq)]
pub struct Script {
    pub entry: ScriptEntry,
}

/// A clear entry point for a script: an event hat plus its body.
#[derive(Debug, Clone, PartialEq)]
pub struct ScriptEntry {
    pub hat: EventHat,
    pub name: Option<String>,
    pub body: Vec<Stmt>,
}

/// Event hats. These are distinct from procedure definitions, which are
/// modeled by [`Procedure`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventHat {
    GreenFlag,
    KeyPressed(String),
    SpriteClicked,
    BroadcastReceived(String),
    CloneStart,
}

/// A custom block definition.
#[derive(Debug, Clone, PartialEq)]
pub struct Procedure {
    pub prototype: ProcedurePrototype,
    pub body: Vec<Stmt>,
    /// Total frame size = 2 (saved FP + return slot) + local_count.
    pub frame_size: u32,
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
    /// Set a list item at an index to a value.
    SetListItem { list: String, index: Expr, value: Expr },
    /// Delete a list item at an index.
    DeleteListItem { list: String, index: Expr },
    /// Insert a value into a list at an index.
    InsertListItem { list: String, index: Expr, value: Expr },
    /// Broadcast a message.
    Broadcast { message: Expr },
    /// Allocate heap memory; result frame slot receives the pointer.
    HeapAlloc { result_offset: u32, size: Expr },
    /// Call a custom block.
    Call { proc: String, args: Vec<Expr> },
    /// Push a new frame of `slots` cells onto the runtime stack.
    EnterFrame { slots: u32 },
    /// Pop `slots` cells from the end of the runtime stack.
    PopFrame { slots: u32 },
    /// Write a value into the current frame at `fp + offset`.
    FrameSet { offset: u32, value: Expr },
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
    /// Read a single item from a list.
    ListItem { list: String, index: Box<Expr> },
    /// Length of a list.
    ListLength { list: String },
    ProcedureParam(String),
    Operator { opcode: String, args: Vec<Expr> },
    /// Read from the heap at the given address.
    HeapLoad { addr: Box<Expr> },
    /// Compute a pointer offset.
    HeapIndex { base: Box<Expr>, offset: Box<Expr> },
    /// Read the current frame pointer `__scratcharch_fp`.
    FrameBase,
    /// Read `__scratcharch_stack[fp + offset]`.
    FrameGet { offset: u32 },
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

    pub fn add_list(&mut self, list: List) {
        self.lists.push(list);
    }

    pub fn add_broadcast(&mut self, broadcast: Broadcast) {
        self.broadcasts.push(broadcast);
    }

    pub fn add_script(&mut self, script: Script) {
        self.scripts.push(script);
    }

    pub fn add_procedure(&mut self, proc: Procedure) {
        self.procedures.push(proc);
    }

    pub fn add_costume(&mut self, costume: Costume) {
        self.costumes.push(costume);
    }

    pub fn add_sound(&mut self, sound: Sound) {
        self.sounds.push(sound);
    }
}

impl Sprite {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    pub fn add_variable(&mut self, var: Variable) {
        self.variables.push(var);
    }

    pub fn add_list(&mut self, list: List) {
        self.lists.push(list);
    }

    pub fn add_broadcast(&mut self, broadcast: Broadcast) {
        self.broadcasts.push(broadcast);
    }

    pub fn add_script(&mut self, script: Script) {
        self.scripts.push(script);
    }

    pub fn add_procedure(&mut self, proc: Procedure) {
        self.procedures.push(proc);
    }

    pub fn add_costume(&mut self, costume: Costume) {
        self.costumes.push(costume);
    }

    pub fn add_sound(&mut self, sound: Sound) {
        self.sounds.push(sound);
    }
}

impl Variable {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            scope: VariableScope::Global,
        }
    }

    pub fn with_scope(mut self, scope: VariableScope) -> Self {
        self.scope = scope;
        self
    }
}

impl List {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            scope: ListScope::Global,
        }
    }

    pub fn with_scope(mut self, scope: ListScope) -> Self {
        self.scope = scope;
        self
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
            frame_size: 0,
        }
    }

    pub fn with_frame_size(mut self, frame_size: u32) -> Self {
        self.frame_size = frame_size;
        self
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
    pub fn new(hat: EventHat, body: Vec<Stmt>) -> Self {
        Self {
            entry: ScriptEntry {
                hat,
                name: None,
                body,
            },
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.entry.name = Some(name.into());
        self
    }

    pub fn hat(&self) -> &EventHat {
        &self.entry.hat
    }

    pub fn body(&self) -> &[Stmt] {
        &self.entry.body
    }
}

impl ScriptEntry {
    pub fn new(hat: EventHat, body: Vec<Stmt>) -> Self {
        Self {
            hat,
            name: None,
            body,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
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

    pub fn list_item(list: impl Into<String>, index: Expr) -> Self {
        Expr::ListItem {
            list: list.into(),
            index: Box::new(index),
        }
    }

    pub fn list_length(list: impl Into<String>) -> Self {
        Expr::ListLength { list: list.into() }
    }

    pub fn heap_load(addr: Expr) -> Self {
        Expr::HeapLoad {
            addr: Box::new(addr),
        }
    }

    pub fn heap_index(base: Expr, offset: Expr) -> Self {
        Expr::HeapIndex {
            base: Box::new(base),
            offset: Box::new(offset),
        }
    }
}
