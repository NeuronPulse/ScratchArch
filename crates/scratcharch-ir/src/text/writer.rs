use std::collections::HashMap;
use std::fmt::Write as _;
use crate::block::BasicBlock;
use crate::debug::DebugLoc;
use crate::function::IrFunction;
use crate::instruction::{GepIndex, Instruction, Terminator};
use crate::r#module::IrModule;
use crate::types::IrType;
use crate::value::{Constant, ValueId};

/// Serialize an `IrModule` to the canonical SAIR text format.
pub fn serialize(module: &IrModule) -> String {
    let mut out = String::new();
    writeln!(out, "sair 0.1").unwrap();
    writeln!(out, "entry \"{}\"", escape_string(&module.entry)).unwrap();
    out.push('\n');

    for func in &module.functions {
        write_function(&mut out, func);
        out.push('\n');
    }

    out
}

fn write_function(out: &mut String, func: &IrFunction) {
    let value_map = build_value_map(func);

    writeln!(
        out,
        "func @{} -> {} entry \"{}\" {{",
        escape_string(&func.name),
        func.return_ty,
        escape_string(&func.entry_block)
    )
    .unwrap();

    for (i, (ty, _name)) in func.params.iter().enumerate() {
        writeln!(out, "  param {} %{}", ty, i).unwrap();
    }

    let mut textual_result_id = func.params.len();
    for block in &func.blocks {
        write_block(out, block, func.return_ty, &value_map, &mut textual_result_id);
    }

    writeln!(out, "}}").unwrap();
}

/// Map internal ValueIds to dense textual ids.
///
/// Under the IrBuilder convention, parameters receive ids `0..p-1` and each
/// result-producing instruction receives the next sequential id. This function
/// maps those internal ids to dense textual ids (`%0`, `%1`, ...).
fn build_value_map(func: &IrFunction) -> HashMap<ValueId, ValueId> {
    let mut map = HashMap::new();
    for i in 0..func.params.len() {
        map.insert(i, i);
    }

    let mut next_id = func.params.len();
    for block in &func.blocks {
        for instr in &block.instructions {
            if instr.result_type().is_some() {
                map.insert(next_id, next_id);
                next_id += 1;
            }
        }
    }

    map
}

fn write_block(
    out: &mut String,
    block: &BasicBlock,
    return_ty: IrType,
    value_map: &HashMap<ValueId, ValueId>,
    textual_result_id: &mut ValueId,
) {
    writeln!(out, "  block \"{}\":", escape_string(&block.label)).unwrap();

    for (i, instr) in block.instructions.iter().enumerate() {
        let result_id = if instr.result_type().is_some() {
            let id = *textual_result_id;
            *textual_result_id += 1;
            Some(id)
        } else {
            None
        };
        write_instruction(out, instr, result_id, value_map, block.debug_loc(i));
    }

    write_terminator(out, return_ty, &block.terminator, value_map);
}

fn write_instruction(
    out: &mut String,
    instr: &Instruction,
    result_id: Option<ValueId>,
    value_map: &HashMap<ValueId, ValueId>,
    debug: Option<&DebugLoc>,
) {
    if let Some(id) = result_id {
        write!(out, "    %{} = ", id).unwrap();
    } else {
        write!(out, "    ").unwrap();
    }

    match instr {
        Instruction::Add { ty, lhs, rhs } => {
            write!(out, "add {} {}, {}", ty, val(*lhs, value_map), val(*rhs, value_map)).unwrap();
        }
        Instruction::Sub { ty, lhs, rhs } => {
            write!(out, "sub {} {}, {}", ty, val(*lhs, value_map), val(*rhs, value_map)).unwrap();
        }
        Instruction::Mul { ty, lhs, rhs } => {
            write!(out, "mul {} {}, {}", ty, val(*lhs, value_map), val(*rhs, value_map)).unwrap();
        }
        Instruction::Div { ty, lhs, rhs } => {
            write!(out, "div {} {}, {}", ty, val(*lhs, value_map), val(*rhs, value_map)).unwrap();
        }
        Instruction::Rem { ty, lhs, rhs } => {
            write!(out, "rem {} {}, {}", ty, val(*lhs, value_map), val(*rhs, value_map)).unwrap();
        }
        Instruction::Eq { ty, lhs, rhs } => {
            write!(out, "eq {} {}, {}", ty, val(*lhs, value_map), val(*rhs, value_map)).unwrap();
        }
        Instruction::Lt { ty, lhs, rhs } => {
            write!(out, "lt {} {}, {}", ty, val(*lhs, value_map), val(*rhs, value_map)).unwrap();
        }
        Instruction::Gt { ty, lhs, rhs } => {
            write!(out, "gt {} {}, {}", ty, val(*lhs, value_map), val(*rhs, value_map)).unwrap();
        }
        Instruction::Cast { op, from_ty, to_ty, value } => {
            write!(
                out,
                "cast {} {} {} {}",
                op.name(),
                from_ty,
                to_ty,
                val(*value, value_map)
            )
            .unwrap();
        }
        Instruction::Select { ty, condition, then_value, else_value } => {
            write!(
                out,
                "select {} {}, {}, {}",
                ty,
                val(*condition, value_map),
                val(*then_value, value_map),
                val(*else_value, value_map)
            )
            .unwrap();
        }
        Instruction::Const(c) => {
            write!(out, "const {}", const_to_string(c)).unwrap();
        }
        Instruction::Alloca { ty, count } => {
            if *count == 1 {
                write!(out, "alloca {}", ty).unwrap();
            } else {
                write!(out, "alloca {}, {}", ty, count).unwrap();
            }
        }
        Instruction::Load { ty, addr } => {
            write!(out, "load {} {}", ty, val(*addr, value_map)).unwrap();
        }
        Instruction::Store { ty, value, addr } => {
            write!(out, "store {} {}, {}", ty, val(*value, value_map), val(*addr, value_map)).unwrap();
        }
        Instruction::Call { return_ty, callee, args } => {
            let args_str = args
                .iter()
                .map(|a| val(*a, value_map))
                .collect::<Vec<_>>()
                .join(", ");
            write!(out, "call {} @{}({})", return_ty, escape_string(callee), args_str).unwrap();
        }
        Instruction::Phi { ty, incoming } => {
            let pairs = incoming
                .iter()
                .map(|(v, l)| format!("[\"{}\", {}]", escape_string(l), val(*v, value_map)))
                .collect::<Vec<_>>()
                .join(", ");
            write!(out, "phi {} {}", ty, pairs).unwrap();
        }
        Instruction::Gep { elem_ty, base, indices, .. } => {
            let idx_str = indices
                .iter()
                .map(|i| gep_index_to_string(i, value_map))
                .collect::<Vec<_>>()
                .join(", ");
            write!(out, "gep {} {}, {}", elem_ty, val(*base, value_map), idx_str).unwrap();
        }
    }
    write_debug_loc(out, debug);
    writeln!(out).unwrap();
}

fn write_debug_loc(out: &mut String, debug: Option<&DebugLoc>) {
    if let Some(loc) = debug {
        write!(out, " !loc \"{}\" {}", escape_string(&loc.file), loc.line).unwrap();
        if let Some(col) = loc.column {
            write!(out, " {}", col).unwrap();
        }
    }
}

fn write_terminator(
    out: &mut String,
    return_ty: IrType,
    term: &Terminator,
    value_map: &HashMap<ValueId, ValueId>,
) {
    match term {
        Terminator::Branch { target } => {
            writeln!(out, "    br \"{}\"", escape_string(target)).unwrap();
        }
        Terminator::CondBranch { condition, true_target, false_target } => {
            writeln!(
                out,
                "    cond_br {}, \"{}\", \"{}\"",
                val(*condition, value_map),
                escape_string(true_target),
                escape_string(false_target)
            )
            .unwrap();
        }
        Terminator::Return { value: Some(v) } => {
            writeln!(out, "    ret {} {}", return_ty, val(*v, value_map)).unwrap();
        }
        Terminator::Return { value: None } => {
            writeln!(out, "    ret void").unwrap();
        }
        Terminator::Unreachable => {
            writeln!(out, "    unreachable").unwrap();
        }
    }
}

fn val(id: ValueId, value_map: &HashMap<ValueId, ValueId>) -> String {
    format!("%{}", find_text_id(value_map, id))
}

fn find_text_id(value_map: &HashMap<ValueId, ValueId>, id: ValueId) -> ValueId {
    *value_map.get(&id).unwrap_or(&id)
}

fn const_to_string(c: &Constant) -> String {
    match c {
        Constant::I1(v) => {
            if *v {
                "i1 true".to_string()
            } else {
                "i1 false".to_string()
            }
        }
        Constant::I8(v) => format!("i8 {}", v),
        Constant::I16(v) => format!("i16 {}", v),
        Constant::I32(v) => format!("i32 {}", v),
        Constant::I64(v) => format!("i64 {}", v),
        Constant::F64(v) => format!("f64 0x{:016x}", v.to_bits()),
    }
}

fn gep_index_to_string(index: &GepIndex, value_map: &HashMap<ValueId, ValueId>) -> String {
    match index {
        GepIndex::Dynamic(id) => val(*id, value_map),
        GepIndex::StructField(n) => format!("field {}", n),
    }
}

fn escape_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
