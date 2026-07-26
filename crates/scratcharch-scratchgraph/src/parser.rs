//! Basic `project.json` → ScratchGraph AST parser.
//!
//! This module implements the reverse direction of the roundtrip path described
//! in [`docs/design/SCRATCH_ROUNDTRIP.md`]. It does not parse every Scratch
//! block; the goal is to establish the parser architecture and recover enough
//! structure for a future decompiler.
//!
//! Supported constructs:
//! - targets (stage, sprites), variables, lists, broadcasts
//! - event hats and scripts
//! - custom block definitions and calls
//! - arithmetic/comparison expressions
//! - variable/list operations
//! - control blocks (`if`, `if/else`, `repeat`, `repeat until`, `forever`, `stop`)

use serde_json::{json, Value};

use crate::debug::{BlockSourceEntry, SourceMap, TargetSourceMap};
use crate::ir::{
    Broadcast, EventHat, Expr, List, ListScope, Procedure, ProcedureParam, ProcedurePrototype,
    Project, Script, ScriptEntry, Sprite, Stage, Stmt, StopOption, Value as SgValue, Variable,
    VariableScope,
};

/// Errors that can occur while parsing a Scratch 3 project.json file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// Top-level value is not an object.
    NotAnObject,
    /// Missing or malformed `targets` array.
    MissingTargets,
    /// A target is not an object.
    InvalidTarget,
    /// A block is not an object.
    InvalidBlock(String),
    /// Referenced block id does not exist.
    MissingBlock(String),
    /// An opcode is not recognized.
    UnsupportedOpcode(String),
    /// An input/field shape is unexpected.
    InvalidInputShape,
    /// A procedure prototype is malformed.
    InvalidProcedurePrototype,
}

impl core::fmt::Display for ParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ParseError::NotAnObject => write!(f, "project.json root is not an object"),
            ParseError::MissingTargets => write!(f, "missing or malformed targets array"),
            ParseError::InvalidTarget => write!(f, "target is not an object"),
            ParseError::InvalidBlock(id) => write!(f, "block {} is not an object", id),
            ParseError::MissingBlock(id) => write!(f, "referenced block {} not found", id),
            ParseError::UnsupportedOpcode(op) => write!(f, "unsupported opcode: {}", op),
            ParseError::InvalidInputShape => write!(f, "invalid input or field shape"),
            ParseError::InvalidProcedurePrototype => write!(f, "invalid procedure prototype"),
        }
    }
}

impl std::error::Error for ParseError {}

/// Parser for Scratch 3 `project.json`.
#[derive(Debug, Clone, Default)]
pub struct ProjectParser;

impl ProjectParser {
    pub fn new() -> Self {
        Self
    }

    /// Parse a `serde_json::Value` into a ScratchGraph `Project`.
    pub fn parse(&mut self, value: &Value) -> Result<Project, ParseError> {
        let root = value.as_object().ok_or(ParseError::NotAnObject)?;
        let targets = root
            .get("targets")
            .and_then(Value::as_array)
            .ok_or(ParseError::MissingTargets)?;

        let mut stage = None;
        let mut sprites = Vec::new();

        for target in targets {
            let target_obj = target.as_object().ok_or(ParseError::InvalidTarget)?;
            let is_stage = target_obj.get("isStage").and_then(Value::as_bool).unwrap_or(false);
            let name = target_obj
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let variables = self.parse_variables(target_obj.get("variables"));
            let lists = self.parse_lists(target_obj.get("lists"));
            let broadcasts = self.parse_broadcasts(target_obj.get("broadcasts"));
            let blocks = target_obj.get("blocks").cloned().unwrap_or(json!({}));
            let (scripts, procedures) = self.parse_scripts_and_procedures(&blocks)?;

            if is_stage {
                stage = Some(Stage {
                    name,
                    variables,
                    lists,
                    broadcasts,
                    scripts,
                    procedures,
                });
            } else {
                sprites.push(Sprite {
                    name,
                    variables,
                    lists,
                    scripts,
                    procedures,
                });
            }
        }

        let stage = stage.unwrap_or_else(|| Stage {
            name: "Stage".to_string(),
            variables: Vec::new(),
            lists: Vec::new(),
            broadcasts: Vec::new(),
            scripts: Vec::new(),
            procedures: Vec::new(),
        });

        Ok(Project { stage, sprites })
    }

    fn parse_variables(&mut self, value: Option<&Value>) -> Vec<Variable> {
        let mut result = Vec::new();
        if let Some(obj) = value.and_then(Value::as_object) {
            for (id, val) in obj {
                let name = val
                    .as_array()
                    .and_then(|a| a.first())
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                result.push(Variable {
                    id: id.clone(),
                    name,
                    scope: VariableScope::Global,
                });
            }
        }
        result
    }

    fn parse_lists(&mut self, value: Option<&Value>) -> Vec<List> {
        let mut result = Vec::new();
        if let Some(obj) = value.and_then(Value::as_object) {
            for (id, val) in obj {
                let name = val.as_array().and_then(|a| a.first()).and_then(Value::as_str).unwrap_or("").to_string();
                result.push(List {
                    id: id.clone(),
                    name,
                    scope: ListScope::Global,
                });
            }
        }
        result
    }

    fn parse_broadcasts(&mut self, value: Option<&Value>) -> Vec<Broadcast> {
        let mut result = Vec::new();
        if let Some(obj) = value.and_then(Value::as_object) {
            for (id, val) in obj {
                let name = val.as_str().unwrap_or("").to_string();
                result.push(Broadcast {
                    id: id.clone(),
                    name,
                });
            }
        }
        result
    }

    fn parse_scripts_and_procedures(
        &mut self,
        blocks: &Value,
    ) -> Result<(Vec<Script>, Vec<Procedure>), ParseError> {
        let block_obj = blocks.as_object().ok_or_else(|| ParseError::InvalidBlock("blocks".to_string()))?;
        let mut scripts = Vec::new();
        let mut procedures = Vec::new();

        for (id, block_val) in block_obj {
            let block = block_val.as_object().ok_or_else(|| ParseError::InvalidBlock(id.clone()))?;
            let top_level = block.get("topLevel").and_then(Value::as_bool).unwrap_or(false);
            if !top_level {
                continue;
            }
            let opcode = block
                .get("opcode")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();

            match opcode.as_str() {
                "event_whenflagclicked"
                | "event_whenkeypressed"
                | "event_whenthisspriteclicked"
                | "event_whenbroadcastreceived"
                | "event_whencloned" => {
                    let entry = self.parse_script_entry(id, block, block_obj)?;
                    scripts.push(Script { entry });
                }
                "procedures_definition" => {
                    let proc = self.parse_procedure(id, block, block_obj)?;
                    procedures.push(proc);
                }
                _ => {}
            }
        }

        Ok((scripts, procedures))
    }

    fn parse_script_entry(
        &mut self,
        _id: &str,
        block: &serde_json::Map<String, Value>,
        blocks: &serde_json::Map<String, Value>,
    ) -> Result<ScriptEntry, ParseError> {
        let opcode = block.get("opcode").and_then(Value::as_str).unwrap_or("");
        let hat = match opcode {
            "event_whenflagclicked" => EventHat::GreenFlag,
            "event_whenkeypressed" => {
                let key = block
                    .get("fields")
                    .and_then(|f| f.get("KEY_OPTION"))
                    .and_then(Value::as_array)
                    .and_then(|a| a.first())
                    .and_then(Value::as_str)
                    .unwrap_or("space")
                    .to_string();
                EventHat::KeyPressed(key)
            }
            "event_whenthisspriteclicked" => EventHat::SpriteClicked,
            "event_whenbroadcastreceived" => {
                let name = block
                    .get("fields")
                    .and_then(|f| f.get("BROADCAST_OPTION"))
                    .and_then(Value::as_array)
                    .and_then(|a| a.first())
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                EventHat::BroadcastReceived(name)
            }
            "event_whencloned" => EventHat::CloneStart,
            _ => return Err(ParseError::UnsupportedOpcode(opcode.to_string())),
        };

        let body = self.parse_body(block.get("next").and_then(Value::as_str), blocks)?;
        Ok(ScriptEntry::new(hat, body))
    }

    fn parse_procedure(
        &mut self,
        _id: &str,
        block: &serde_json::Map<String, Value>,
        blocks: &serde_json::Map<String, Value>,
    ) -> Result<Procedure, ParseError> {
        let mutation = block
            .get("mutation")
            .and_then(Value::as_object)
            .ok_or(ParseError::InvalidProcedurePrototype)?;
        let proccode = mutation
            .get("proccode")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let name = proccode.split_whitespace().next().unwrap_or("").to_string();
        let arg_names: Vec<String> = mutation
            .get("argumentnames")
            .and_then(Value::as_str)
            .map(|s| serde_json::from_str(s).unwrap_or_default())
            .unwrap_or_default();
        let params: Vec<ProcedureParam> = arg_names
            .into_iter()
            .map(|n| ProcedureParam { name: n, default: None })
            .collect();
        let prototype = ProcedurePrototype { name, params };

        let body = if let Some(next) = block.get("next").and_then(Value::as_str) {
            self.parse_body(Some(next), blocks)?
        } else {
            Vec::new()
        };

        Ok(Procedure {
            prototype,
            body,
            frame_size: 0,
        })
    }

    fn parse_body(
        &mut self,
        next_id: Option<&str>,
        blocks: &serde_json::Map<String, Value>,
    ) -> Result<Vec<Stmt>, ParseError> {
        let mut body = Vec::new();
        let mut current_id = next_id.map(String::from);
        while let Some(id) = current_id {
            let block_val = blocks.get(&id).ok_or_else(|| ParseError::MissingBlock(id.clone()))?;
            let block = block_val.as_object().ok_or_else(|| ParseError::InvalidBlock(id.clone()))?;
            let (stmt, following) = self.parse_stmt(block, blocks)?;
            body.push(stmt);
            current_id = following;
        }
        Ok(body)
    }

    fn parse_stmt(
        &mut self,
        block: &serde_json::Map<String, Value>,
        blocks: &serde_json::Map<String, Value>,
    ) -> Result<(Stmt, Option<String>), ParseError> {
        let opcode = block
            .get("opcode")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let next = block.get("next").and_then(Value::as_str).map(String::from);

        let stmt = match opcode.as_str() {
            "data_setvariableto" => {
                let var = self.field_string(block, "VARIABLE")?;
                let value = self.parse_input(block, "VALUE", blocks)?;
                Stmt::SetVariable { var, value }
            }
            "data_changevariableby" => {
                let var = self.field_string(block, "VARIABLE")?;
                let delta = self.parse_input(block, "VALUE", blocks)?;
                Stmt::ChangeVariable { var, delta }
            }
            "data_addtolist" => {
                let list = self.field_string(block, "LIST")?;
                let value = self.parse_input(block, "ITEM", blocks)?;
                Stmt::AddToList { list, value }
            }
            "data_deletealloflist" => {
                let list = self.field_string(block, "LIST")?;
                Stmt::DeleteAllOfList { list }
            }
            "data_replaceitemoflist" => {
                let list = self.field_string(block, "LIST")?;
                let index = self.parse_input(block, "INDEX", blocks)?;
                let value = self.parse_input(block, "ITEM", blocks)?;
                Stmt::SetListItem { list, index, value }
            }
            "data_deleteitemoflist" => {
                let list = self.field_string(block, "LIST")?;
                let index = self.parse_input(block, "INDEX", blocks)?;
                Stmt::DeleteListItem { list, index }
            }
            "data_insertatlist" => {
                let list = self.field_string(block, "LIST")?;
                let index = self.parse_input(block, "INDEX", blocks)?;
                let value = self.parse_input(block, "ITEM", blocks)?;
                Stmt::InsertListItem { list, index, value }
            }
            "event_broadcast" => {
                let message = self.parse_input(block, "BROADCAST_INPUT", blocks)?;
                Stmt::Broadcast { message }
            }
            "procedures_call" => {
                let mutation = block
                    .get("mutation")
                    .and_then(Value::as_object)
                    .ok_or(ParseError::InvalidProcedurePrototype)?;
                let proccode = mutation
                    .get("proccode")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let proc_name = proccode.split_whitespace().next().unwrap_or("").to_string();
                let arg_ids: Vec<String> = mutation
                    .get("argumentids")
                    .and_then(Value::as_str)
                    .map(|s| serde_json::from_str(s).unwrap_or_default())
                    .unwrap_or_default();
                let args: Result<Vec<Expr>, ParseError> = arg_ids
                    .iter()
                    .map(|id| self.parse_input(block, id, blocks))
                    .collect();
                Stmt::Call { proc: proc_name, args: args? }
            }
            "control_if" => {
                let condition = self.parse_input(block, "CONDITION", blocks)?;
                let substack = self.parse_input_substack(block, "SUBSTACK", blocks)?;
                Stmt::If {
                    condition,
                    then_body: substack,
                    else_body: Vec::new(),
                }
            }
            "control_if_else" => {
                let condition = self.parse_input(block, "CONDITION", blocks)?;
                let then_body = self.parse_input_substack(block, "SUBSTACK", blocks)?;
                let else_body = self.parse_input_substack(block, "SUBSTACK2", blocks)?;
                Stmt::If {
                    condition,
                    then_body,
                    else_body,
                }
            }
            "control_repeat" => {
                let times = self.parse_input(block, "TIMES", blocks)?;
                let body = self.parse_input_substack(block, "SUBSTACK", blocks)?;
                Stmt::Repeat { times, body }
            }
            "control_repeat_until" => {
                let condition = self.parse_input(block, "CONDITION", blocks)?;
                let body = self.parse_input_substack(block, "SUBSTACK", blocks)?;
                Stmt::RepeatUntil { condition, body }
            }
            "control_forever" => {
                let body = self.parse_input_substack(block, "SUBSTACK", blocks)?;
                Stmt::Forever { body }
            }
            "control_stop" => {
                let option = self.field_string(block, "STOP_OPTION")?;
                let opt = match option.as_str() {
                    "all" => StopOption::All,
                    _ => StopOption::ThisScript,
                };
                Stmt::Stop { option: opt }
            }
            _ => return Err(ParseError::UnsupportedOpcode(opcode)),
        };

        Ok((stmt, next))
    }

    fn parse_input(
        &mut self,
        block: &serde_json::Map<String, Value>,
        input_name: &str,
        blocks: &serde_json::Map<String, Value>,
    ) -> Result<Expr, ParseError> {
        let inputs = block.get("inputs").and_then(Value::as_object).unwrap_or(&serde_json::Map::new()).clone();
        let input = inputs.get(input_name).cloned().unwrap_or(json!(null));
        self.parse_input_value(&input, blocks)
    }

    fn parse_input_value(
        &mut self,
        input: &Value,
        blocks: &serde_json::Map<String, Value>,
    ) -> Result<Expr, ParseError> {
        // Input is [shadow_type, value]
        let arr = input.as_array().ok_or(ParseError::InvalidInputShape)?;
        if arr.len() < 2 {
            return Ok(Expr::Literal(SgValue::Number(0.0)));
        }
        let value = &arr[1];

        if let Some(block_id) = value.as_str() {
            return self.parse_expr_block(block_id, blocks);
        }
        if let Some(arr2) = value.as_array() {
            if arr2.len() < 2 {
                return Ok(Expr::Literal(SgValue::Number(0.0)));
            }
            let kind = arr2[0].as_str().unwrap_or("");
            let payload = &arr2[1];
            match kind {
                "number" | "integer" | "positive number" | "whole number" => {
                    let n = payload.as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    Ok(Expr::Literal(SgValue::Number(n)))
                }
                "text" => {
                    let s = payload.as_str().unwrap_or("").to_string();
                    Ok(Expr::Literal(SgValue::String(s)))
                }
                "boolean" => Ok(Expr::Literal(SgValue::Bool(false))),
                _ => Ok(Expr::Literal(SgValue::String(payload.to_string()))),
            }
        } else {
            Ok(Expr::Literal(SgValue::Number(0.0)))
        }
    }

    fn parse_input_substack(
        &mut self,
        block: &serde_json::Map<String, Value>,
        input_name: &str,
        blocks: &serde_json::Map<String, Value>,
    ) -> Result<Vec<Stmt>, ParseError> {
        let inputs = block.get("inputs").and_then(Value::as_object).cloned().unwrap_or_default();
        let input = inputs.get(input_name).cloned().unwrap_or(json!(null));
        let arr = input.as_array().ok_or(ParseError::InvalidInputShape)?;
        if arr.len() < 2 {
            return Ok(Vec::new());
        }
        if let Some(block_id) = arr[1].as_str() {
            self.parse_body(Some(block_id), blocks)
        } else {
            Ok(Vec::new())
        }
    }

    fn parse_expr_block(
        &mut self,
        block_id: &str,
        blocks: &serde_json::Map<String, Value>,
    ) -> Result<Expr, ParseError> {
        let block_val = blocks.get(block_id).ok_or_else(|| ParseError::MissingBlock(block_id.to_string()))?;
        let block = block_val.as_object().ok_or_else(|| ParseError::InvalidBlock(block_id.to_string()))?;
        let opcode = block
            .get("opcode")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();

        match opcode.as_str() {
            "math_number" | "math_positive_number" | "math_integer" | "math_whole_number" => {
                let s = block
                    .get("fields")
                    .and_then(|f| f.get("NUM"))
                    .and_then(Value::as_array)
                    .and_then(|a| a.first())
                    .and_then(Value::as_str)
                    .unwrap_or("0");
                let n = s.parse().unwrap_or(0.0);
                Ok(Expr::Literal(SgValue::Number(n)))
            }
            "text" => {
                let s = block
                    .get("fields")
                    .and_then(|f| f.get("TEXT"))
                    .and_then(Value::as_array)
                    .and_then(|a| a.first())
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                Ok(Expr::Literal(SgValue::String(s)))
            }
            "data_variable" => {
                let name = self.field_string(block, "VARIABLE")?;
                Ok(Expr::Variable(name))
            }
            "data_itemnumoflist" | "data_itemoflist" => {
                let list = self.field_string(block, "LIST")?;
                let index = self.parse_input(block, "INDEX", blocks)?;
                Ok(Expr::ListItem {
                    list,
                    index: Box::new(index),
                })
            }
            "data_lengthoflist" => {
                let list = self.field_string(block, "LIST")?;
                Ok(Expr::ListLength { list })
            }
            "argument_reporter_string_number" | "argument_reporter_boolean" => {
                let name = self.field_string(block, "VALUE")?;
                Ok(Expr::ProcedureParam(name))
            }
            "operator_add" => self.parse_operator(block, "operator_add", blocks),
            "operator_subtract" => self.parse_operator(block, "operator_subtract", blocks),
            "operator_multiply" => self.parse_operator(block, "operator_multiply", blocks),
            "operator_divide" => self.parse_operator(block, "operator_divide", blocks),
            "operator_equals" => self.parse_operator(block, "operator_equals", blocks),
            "operator_lt" => self.parse_operator(block, "operator_lt", blocks),
            "operator_gt" => self.parse_operator(block, "operator_gt", blocks),
            _ => Err(ParseError::UnsupportedOpcode(opcode)),
        }
    }

    fn parse_operator(
        &mut self,
        block: &serde_json::Map<String, Value>,
        opcode: &str,
        blocks: &serde_json::Map<String, Value>,
    ) -> Result<Expr, ParseError> {
        let left = self.parse_input(block, "NUM1", blocks)?;
        let right = self.parse_input(block, "NUM2", blocks)?;
        Ok(Expr::Operator {
            opcode: opcode.to_string(),
            args: vec![left, right],
        })
    }

    fn field_string(
        &self,
        block: &serde_json::Map<String, Value>,
        field_name: &str,
    ) -> Result<String, ParseError> {
        block
            .get("fields")
            .and_then(|f| f.get(field_name))
            .and_then(Value::as_array)
            .and_then(|a| a.first())
            .and_then(Value::as_str)
            .map(|s| s.to_string())
            .ok_or(ParseError::InvalidInputShape)
    }
}

/// Builds a [`SourceMap`] while parsing.
///
/// Call [`SourceMapBuilder::build`] after parsing to retrieve the mapping
/// from JSON block IDs to source locations.
#[derive(Debug, Clone, Default)]
pub struct SourceMapBuilder {
    pub source_map: SourceMap,
}

impl SourceMapBuilder {
    pub fn new() -> Self {
        Self { source_map: Vec::new() }
    }

    /// Parse a `serde_json::Value` and build source maps for all targets.
    pub fn parse(&mut self, value: &Value) -> Result<Project, ParseError> {
        let root = value.as_object().ok_or(ParseError::NotAnObject)?;
        let targets = root.get("targets").and_then(Value::as_array).ok_or(ParseError::MissingTargets)?;
        let mut stage = None;
        let mut sprites = Vec::new();

        for target in targets {
            let target_obj = target.as_object().ok_or(ParseError::InvalidTarget)?;
            let is_stage = target_obj.get("isStage").and_then(Value::as_bool).unwrap_or(false);
            let name = target_obj.get("name").and_then(Value::as_str).unwrap_or("").to_string();
            let variables = ProjectParser::new().parse_variables(target_obj.get("variables"));
            let lists = ProjectParser::new().parse_lists(target_obj.get("lists"));
            let broadcasts = ProjectParser::new().parse_broadcasts(target_obj.get("broadcasts"));
            let blocks = target_obj.get("blocks").cloned().unwrap_or(json!({}));

            let mut target_map = TargetSourceMap::new(&name);
            if let Some(block_obj) = blocks.as_object() {
                for (block_id, block_val) in block_obj {
                    if let Some(block) = block_val.as_object() {
                        let opcode = block.get("opcode").and_then(Value::as_str).unwrap_or("").to_string();
                        let is_top_level = block.get("topLevel").and_then(Value::as_bool).unwrap_or(false);
                        let parent = block.get("parent").and_then(Value::as_str).map(String::from);
                        target_map.blocks.push(BlockSourceEntry {
                            block_id: block_id.clone(),
                            opcode,
                            is_top_level,
                            parent,
                        });
                    }
                }
            }

            let parser = &mut ProjectParser::new();
            let (scripts, procedures) = parser.parse_scripts_and_procedures(&blocks)?;

            if is_stage {
                stage = Some(Stage { name, variables, lists, broadcasts, scripts, procedures });
            } else {
                sprites.push(Sprite { name, variables, lists, scripts, procedures });
            }

            self.source_map.push(target_map);
        }

        let stage = stage.unwrap_or_else(|| Stage {
            name: "Stage".to_string(), variables: Vec::new(), lists: Vec::new(),
            broadcasts: Vec::new(), scripts: Vec::new(), procedures: Vec::new(),
        });

        Ok(Project { stage, sprites })
    }
}

/// Convenience function: parse a JSON string into a ScratchGraph `Project`.
pub fn parse_project_json(value: &Value) -> Result<Project, ParseError> {
    ProjectParser::new().parse(value)
}
