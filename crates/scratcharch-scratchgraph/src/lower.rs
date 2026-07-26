//! SAIR → ScratchGraph lowering.
//!
//! Maps SAIR functions to Scratch custom blocks and SSA values to frame slots
//! inside a runtime call stack. The Scratch target ABI uses a hidden stage list
//! `__scratcharch_stack` and a frame pointer `__scratcharch_fp` so that calls
//! are reentrant.

use std::collections::{HashMap, HashSet};

use scratcharch_ir::function::IrFunction;
use scratcharch_ir::instruction::{GepIndex, Instruction as SairInstr, Terminator};
use scratcharch_ir::r#module::IrModule;
use scratcharch_ir::value::{Constant, ValueId};

use crate::ir::{
    EventHat, Expr, List, Procedure, ProcedureParam, Project, Script, Stage, Stmt, StopOption,
    Variable, VariableScope,
};

/// Lowers a SAIR module to a ScratchGraph project.
#[derive(Debug, Clone, Default)]
pub struct ScratchGraphLowerer;

impl ScratchGraphLowerer {
    pub fn new() -> Self {
        Self
    }

    /// Lower the module. The entry function becomes a "when green flag"
    /// script that calls the corresponding Scratch custom block.
    pub fn lower(&self, module: &IrModule) -> Result<Project, LowerError> {
        let mut project = Project::new();
        let mut stage = Stage::new("Stage");

        // Compute frame sizes for all functions before lowering any calls.
        let frame_sizes: HashMap<String, u32> = module
            .functions
            .iter()
            .map(|f| (f.name.clone(), compute_frame_size(f)))
            .collect();

        // Every module uses the runtime call stack.
        stage.add_list(List::new("__scratcharch_stack", "__scratcharch_stack"));
        stage.add_variable(
            Variable::new("__scratcharch_fp", "__scratcharch_fp").with_scope(VariableScope::Temporary),
        );

        // If any SAIR function uses memory instructions, model memory as a
        // single stage-backed heap list.
        let needs_heap = module.functions.iter().any(|f| {
            f.blocks.iter().any(|b| {
                b.instructions.iter().any(|i| {
                    matches!(
                        i,
                        SairInstr::Alloca { .. }
                            | SairInstr::Load { .. }
                            | SairInstr::Store { .. }
                            | SairInstr::Gep { .. }
                    )
                })
            })
        });
        if needs_heap {
            stage.add_list(List::new("__scratcharch_heap", "__scratcharch_heap"));
        }

        for func in &module.functions {
            let proc = lower_function(func, &frame_sizes)?;
            stage.add_procedure(proc);
        }

        // Entry script calls the entry procedure inside a frame.
        if let Some(entry_func) = module.functions.iter().find(|f| f.name == module.entry) {
            let entry_proc_name = entry_func.name.clone();
            let entry_frame_size = frame_sizes
                .get(&entry_proc_name)
                .copied()
                .unwrap_or(2);
            let arg_count = entry_func.params.len();
            let call_args: Vec<Expr> = if arg_count == 0 {
                vec![]
            } else {
                vec![Expr::number(0.0); arg_count]
            };
            stage.add_script(Script::new(
                EventHat::GreenFlag,
                vec![
                    Stmt::DeleteAllOfList {
                        list: "__scratcharch_stack".to_string(),
                    },
                    Stmt::SetVariable {
                        var: "__scratcharch_fp".to_string(),
                        value: Expr::number(0.0),
                    },
                    Stmt::EnterFrame {
                        slots: entry_frame_size,
                    },
                    Stmt::Call {
                        proc: entry_proc_name,
                        args: call_args,
                    },
                    Stmt::SetVariable {
                        var: "__scratcharch_fp".to_string(),
                        value: Expr::FrameGet { offset: 0 },
                    },
                    Stmt::PopFrame {
                        slots: entry_frame_size,
                    },
                    Stmt::Stop {
                        option: StopOption::ThisScript,
                    },
                ],
            ));
        }

        project.stage = stage;
        Ok(project)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum LowerError {
    UnsupportedInstruction(String),
    UnsupportedTerminator(String),
    UnsupportedType(String),
    Validation(String),
}

impl core::fmt::Display for LowerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            LowerError::UnsupportedInstruction(msg) => write!(f, "unsupported instruction: {msg}"),
            LowerError::UnsupportedTerminator(msg) => write!(f, "unsupported terminator: {msg}"),
            LowerError::UnsupportedType(msg) => write!(f, "unsupported type: {msg}"),
            LowerError::Validation(msg) => write!(f, "validation error: {msg}"),
        }
    }
}

fn compute_frame_size(func: &IrFunction) -> u32 {
    let local_count = func.value_counter().saturating_sub(func.params.len());
    2u32.saturating_add(local_count as u32)
}

fn lower_function(func: &IrFunction, frame_sizes: &HashMap<String, u32>) -> Result<Procedure, LowerError> {
    let params: Vec<ProcedureParam> = func
        .params
        .iter()
        .enumerate()
        .map(|(i, (_ty, name))| {
            let param_name = if name.is_empty() {
                format!("arg{i}")
            } else {
                name.clone()
            };
            ProcedureParam::new(param_name)
        })
        .collect();

    let ctx = FuncLowerCtx::new(func, frame_sizes.clone());
    let body = lower_region(func, &ctx, func.entry_block.clone(), &HashSet::new())?;

    let frame_size = compute_frame_size(func);
    Ok(Procedure::new(func.name.clone(), params, body).with_frame_size(frame_size))
}

struct FuncLowerCtx<'a> {
    func: &'a IrFunction,
    #[allow(dead_code)]
    pred_map: HashMap<String, Vec<String>>,
    succ_map: HashMap<String, Vec<String>>,
    /// Maps (block_label, instruction_index) to the global SSA value id.
    result_ids: HashMap<(String, usize), ValueId>,
    frame_sizes: HashMap<String, u32>,
}

impl<'a> FuncLowerCtx<'a> {
    fn new(func: &'a IrFunction, frame_sizes: HashMap<String, u32>) -> Self {
        let mut result_ids = HashMap::new();
        let mut next_id = func.params.len();
        for block in &func.blocks {
            for (idx, instr) in block.instructions.iter().enumerate() {
                if instr.result_type().is_some() {
                    result_ids.insert((block.label.clone(), idx), next_id);
                    next_id += 1;
                }
            }
        }
        Self {
            func,
            pred_map: build_pred_map(func),
            succ_map: build_succ_map(func),
            result_ids,
            frame_sizes,
        }
    }

    fn result_id(&self, block_label: &str, instr_idx: usize) -> Option<ValueId> {
        self.result_ids.get(&(block_label.to_string(), instr_idx)).copied()
    }

    fn block(&self, label: &str) -> Option<&scratcharch_ir::block::BasicBlock> {
        self.func.blocks.iter().find(|b| b.label == label)
    }

    #[allow(dead_code)]
    fn predecessors(&self, label: &str) -> &[String] {
        self.pred_map.get(label).map(|v| v.as_slice()).unwrap_or(&[])
    }

    fn successors(&self, label: &str) -> &[String] {
        self.succ_map.get(label).map(|v| v.as_slice()).unwrap_or(&[])
    }

    fn callee_frame_size(&self, name: &str) -> u32 {
        self.frame_sizes.get(name).copied().unwrap_or(2)
    }
}

fn build_pred_map(func: &IrFunction) -> HashMap<String, Vec<String>> {
    let mut preds: HashMap<String, Vec<String>> = HashMap::new();
    for block in &func.blocks {
        preds.entry(block.label.clone()).or_default();
        for target in block.referenced_labels() {
            preds.entry(target).or_default().push(block.label.clone());
        }
    }
    preds
}

fn build_succ_map(func: &IrFunction) -> HashMap<String, Vec<String>> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    for block in &func.blocks {
        let succs = match &block.terminator {
            Terminator::Branch { target } => vec![target.clone()],
            Terminator::CondBranch {
                true_target,
                false_target,
                ..
            } => vec![true_target.clone(), false_target.clone()],
            Terminator::Return { .. } => vec![],
        };
        map.insert(block.label.clone(), succs);
    }
    map
}

/// Lower a control-flow region starting at `entry`.
/// `visited` tracks blocks already emitted by an enclosing region (e.g. loop bodies).
fn lower_region(
    func: &IrFunction,
    ctx: &FuncLowerCtx,
    entry: String,
    visited: &HashSet<String>,
) -> Result<Vec<Stmt>, LowerError> {
    let mut stmts = Vec::new();
    let mut current = entry;
    let mut local_visited = visited.clone();

    loop {
        if local_visited.contains(&current) {
            break;
        }
        local_visited.insert(current.clone());

        let block = ctx.block(&current).ok_or_else(|| {
            LowerError::Validation(format!(
                "block '{}' not found in function '{}'",
                current, func.name
            ))
        })?;

        // Emit non-phi instructions.
        for (idx, instr) in block.instructions.iter().enumerate() {
            if matches!(instr, SairInstr::Phi { .. }) {
                continue;
            }
            let instr_stmts = lower_instruction(func, ctx, &current, instr, idx)?;
            stmts.extend(instr_stmts);
        }

        match &block.terminator {
            Terminator::Return { value } => {
                if let Some(v) = value {
                    if !func.return_ty.is_void() {
                        stmts.push(Stmt::FrameSet {
                            offset: 1,
                            value: lower_value(func, *v)?,
                        });
                    }
                }
                let local_count = func.value_counter().saturating_sub(func.params.len());
                stmts.push(Stmt::PopFrame {
                    slots: local_count as u32,
                });
                stmts.push(Stmt::Stop {
                    option: StopOption::ThisScript,
                });
                break;
            }
            Terminator::Branch { target } => {
                if local_visited.contains(target) {
                    // Back edge: end of a loop body. Stop here.
                    break;
                }
                emit_phi_copies(func, ctx, &current, target, &mut stmts)?;
                current = target.clone();
            }
            Terminator::CondBranch {
                condition,
                true_target,
                false_target,
            } => {
                // Try to recognize a natural loop.
                if let Some((loop_stmts, exit_label)) = try_lower_simple_loop(
                    func,
                    ctx,
                    &current,
                    condition,
                    true_target,
                    false_target,
                    &local_visited,
                )? {
                    stmts.extend(loop_stmts);
                    if local_visited.contains(&exit_label) {
                        break;
                    }
                    current = exit_label;
                    continue;
                }

                // Diamond if/else: both branches merge at a common successor.
                let merge = find_merge(func, ctx, true_target, false_target, &local_visited);
                let mut branch_visited = local_visited.clone();
                if let Some(ref merge_label) = merge {
                    branch_visited.insert(merge_label.clone());
                }

                let mut then_body = lower_branch_body(func, ctx, true_target, &branch_visited)?;
                let mut else_body = lower_branch_body(func, ctx, false_target, &branch_visited)?;

                if let Some(ref merge_label) = merge {
                    emit_phi_copies(func, ctx, true_target, merge_label, &mut then_body)?;
                    emit_phi_copies(func, ctx, false_target, merge_label, &mut else_body)?;
                }

                stmts.push(Stmt::If {
                    condition: lower_value(func, *condition)?,
                    then_body,
                    else_body,
                });

                if let Some(merge_label) = merge {
                    if local_visited.contains(&merge_label) {
                        break;
                    }
                    current = merge_label;
                } else {
                    break;
                }
            }
        }
    }

    Ok(stmts)
}

fn lower_branch_body(
    func: &IrFunction,
    ctx: &FuncLowerCtx,
    target: &str,
    visited: &HashSet<String>,
) -> Result<Vec<Stmt>, LowerError> {
    if visited.contains(target) {
        return Ok(Vec::new());
    }
    lower_region(func, ctx, target.to_string(), visited)
}

fn find_merge(
    _func: &IrFunction,
    ctx: &FuncLowerCtx,
    true_target: &str,
    false_target: &str,
    visited: &HashSet<String>,
) -> Option<String> {
    // Find the first block reachable from both branches that is not already
    // part of either branch's body.
    let true_reachable = reachable(ctx, true_target, visited);
    let false_reachable = reachable(ctx, false_target, visited);
    true_reachable
        .intersection(&false_reachable)
        .next()
        .cloned()
}

fn reachable(ctx: &FuncLowerCtx, start: &str, visited: &HashSet<String>) -> HashSet<String> {
    let mut result = HashSet::new();
    let mut stack = vec![start.to_string()];
    while let Some(label) = stack.pop() {
        if visited.contains(&label) || !result.insert(label.clone()) {
            continue;
        }
        for succ in ctx.successors(&label) {
            stack.push(succ.clone());
        }
    }
    result
}

/// Detect a natural loop whose header terminates with `cond_br` and lower it
/// to `repeat until`.
///
/// Pattern:
/// - Header H terminates with `cond_br condition, B, E`
/// - Body B can reach H again without passing through E or already-visited blocks
/// - E is the exit (it does not reach H)
fn try_lower_simple_loop(
    func: &IrFunction,
    ctx: &FuncLowerCtx,
    header: &str,
    condition: &ValueId,
    true_target: &str,
    false_target: &str,
    visited: &HashSet<String>,
) -> Result<Option<(Vec<Stmt>, String)>, LowerError> {
    // Determine which target is the loop body by checking reachability back to
    // the header. The other target is the exit.
    let (body_target, exit_target) =
        if reaches_header(ctx, header, true_target, false_target, visited) {
            (true_target, false_target)
        } else if reaches_header(ctx, header, false_target, true_target, visited) {
            (false_target, true_target)
        } else {
            // Neither branch loops back; this is an if/else, not a loop.
            return Ok(None);
        };

    // Lower the body recursively. The header and already-visited blocks act as
    // boundaries so the body stops at the back edge. This allows nested
    // conditionals and nested natural loops inside the body.
    let mut body_visited = visited.clone();
    body_visited.insert(header.to_string());
    let body_stmts = lower_region(func, ctx, body_target.to_string(), &body_visited)?;

    // Emit `repeat until <exit_condition>`.
    let exit_condition = negate(lower_value(func, *condition)?);
    Ok(Some((
        vec![Stmt::RepeatUntil {
            condition: exit_condition,
            body: body_stmts,
        }],
        exit_target.to_string(),
    )))
}

/// Returns true if `start` can reach `header` without passing through `avoid`
/// or any already-visited block (except `header` itself).
fn reaches_header(
    ctx: &FuncLowerCtx,
    header: &str,
    start: &str,
    avoid: &str,
    visited: &HashSet<String>,
) -> bool {
    let mut seen = HashSet::new();
    let mut stack = vec![start.to_string()];
    while let Some(label) = stack.pop() {
        if label == header {
            return true;
        }
        if label == avoid || visited.contains(&label) || !seen.insert(label.clone()) {
            continue;
        }
        for succ in ctx.successors(&label) {
            stack.push(succ.clone());
        }
    }
    false
}

fn negate(expr: Expr) -> Expr {
    Expr::Operator {
        opcode: "operator_not".to_string(),
        args: vec![expr],
    }
}

/// Emit `FrameSet` statements that copy phi operands from `src_block` into the
/// corresponding phi result slots in `dst_block`. This realizes phi nodes by
/// edge-selected copies.
fn emit_phi_copies(
    func: &IrFunction,
    ctx: &FuncLowerCtx,
    src_block: &str,
    dst_block: &str,
    stmts: &mut Vec<Stmt>,
) -> Result<(), LowerError> {
    let block = ctx.block(dst_block).ok_or_else(|| {
        LowerError::Validation(format!(
            "phi target block '{}' not found in function '{}'",
            dst_block, func.name
        ))
    })?;
    for (idx, instr) in block.instructions.iter().enumerate() {
        if let SairInstr::Phi { incoming, .. } = instr {
            let incoming_value = incoming
                .iter()
                .find(|(_, label)| label == src_block)
                .map(|(value, _)| *value)
                .ok_or_else(|| {
                    LowerError::Validation(format!(
                        "phi in block '{}' has no incoming value from '{}'",
                        dst_block, src_block
                    ))
                })?;
            let result_id = ctx.result_id(dst_block, idx).expect("phi instruction has a result");
            stmts.push(Stmt::FrameSet {
                offset: frame_offset(func, result_id),
                value: lower_value(func, incoming_value)?,
            });
        }
    }
    Ok(())
}

fn lower_instruction(
    func: &IrFunction,
    ctx: &FuncLowerCtx,
    block_label: &str,
    instr: &SairInstr,
    instr_idx: usize,
) -> Result<Vec<Stmt>, LowerError> {
    let result_id = ctx
        .result_id(block_label, instr_idx)
        .unwrap_or(func.params.len());
    match instr {
        SairInstr::Add { lhs, rhs, .. }
        | SairInstr::Sub { lhs, rhs, .. }
        | SairInstr::Mul { lhs, rhs, .. }
        | SairInstr::Div { lhs, rhs, .. }
        | SairInstr::Rem { lhs, rhs, .. }
        | SairInstr::Eq { lhs, rhs, .. }
        | SairInstr::Lt { lhs, rhs, .. }
        | SairInstr::Gt { lhs, rhs, .. } => {
            let opcode = binary_opcode(instr);
            let value = Expr::Operator {
                opcode: opcode.to_string(),
                args: vec![lower_value(func, *lhs)?, lower_value(func, *rhs)?],
            };
            Ok(vec![Stmt::FrameSet {
                offset: frame_offset(func, result_id),
                value,
            }])
        }
        SairInstr::Const(c) => {
            let value = lower_const(c);
            Ok(vec![Stmt::FrameSet {
                offset: frame_offset(func, result_id),
                value,
            }])
        }
        SairInstr::Alloca { ty, count } => {
            let size = Expr::number((ty.size_in_bytes() * *count) as f64);
            Ok(vec![Stmt::HeapAlloc {
                result_offset: frame_offset(func, result_id),
                size,
            }])
        }
        SairInstr::Load { ty: _, addr } => {
            let value = Expr::HeapLoad {
                addr: Box::new(lower_value(func, *addr)?),
            };
            Ok(vec![Stmt::FrameSet {
                offset: frame_offset(func, result_id),
                value,
            }])
        }
        SairInstr::Store { value, addr, .. } => {
            let src = lower_value(func, *value)?;
            let addr_expr = lower_value(func, *addr)?;
            // Scratch lists are 1-indexed; heap pointers are 0-based offsets.
            let index = Expr::operator("operator_add", vec![addr_expr, Expr::number(1.0)]);
            Ok(vec![Stmt::SetListItem {
                list: "__scratcharch_heap".to_string(),
                index,
                value: src,
            }])
        }
        SairInstr::Call {
            callee,
            args,
            return_ty,
        } => {
            let call_args: Vec<Expr> = args
                .iter()
                .map(|a| lower_value(func, *a))
                .collect::<Result<Vec<_>, _>>()?;
            let callee_frame_size = ctx.callee_frame_size(callee);
            let mut stmts = vec![
                Stmt::EnterFrame {
                    slots: callee_frame_size,
                },
                Stmt::Call {
                    proc: callee.clone(),
                    args: call_args,
                },
            ];
            if !return_ty.is_void() {
                stmts.push(Stmt::FrameSet {
                    offset: frame_offset(func, result_id),
                    value: Expr::FrameGet { offset: 1 },
                });
            }
            stmts.push(Stmt::SetVariable {
                var: "__scratcharch_fp".to_string(),
                value: Expr::FrameGet { offset: 0 },
            });
            stmts.push(Stmt::PopFrame {
                slots: callee_frame_size,
            });
            Ok(stmts)
        }
        SairInstr::Phi { .. } => {
            // Phi values are handled by variable updates on incoming edges.
            // Direct phi emission is a no-op.
            Ok(vec![])
        }
        SairInstr::Gep {
            elem_ty,
            base,
            indices,
            ..
        } => {
            // Compute the byte offset from the base pointer. Struct-field
            // offsets are approximated using the element type size; full layout
            // with padding is out of scope for v0.3.
            let mut offset = Expr::number(0.0);
            for idx in indices {
                match idx {
                    GepIndex::Dynamic(value_id) => {
                        let index_expr = lower_value(func, *value_id)?;
                        let elem_size = Expr::number(elem_ty.size_in_bytes() as f64);
                        let term = Expr::operator(
                            "operator_multiply",
                            vec![index_expr, elem_size],
                        );
                        offset = Expr::operator("operator_add", vec![offset, term]);
                    }
                    GepIndex::StructField(field_idx) => {
                        let field_offset =
                            Expr::number((*field_idx * elem_ty.size_in_bytes()) as f64);
                        offset = Expr::operator("operator_add", vec![offset, field_offset]);
                    }
                }
            }
            let value = Expr::HeapIndex {
                base: Box::new(lower_value(func, *base)?),
                offset: Box::new(offset),
            };
            Ok(vec![Stmt::FrameSet {
                offset: frame_offset(func, result_id),
                value,
            }])
        }
    }
}

fn binary_opcode(instr: &SairInstr) -> &'static str {
    match instr {
        SairInstr::Add { .. } => "operator_add",
        SairInstr::Sub { .. } => "operator_subtract",
        SairInstr::Mul { .. } => "operator_multiply",
        SairInstr::Div { .. } => "operator_divide",
        SairInstr::Rem { .. } => "operator_mod",
        SairInstr::Eq { .. } => "operator_equals",
        SairInstr::Lt { .. } => "operator_lt",
        SairInstr::Gt { .. } => "operator_gt",
        _ => "operator_add",
    }
}

fn param_name(func: &IrFunction, id: ValueId) -> String {
    let name = &func.params[id].1;
    if name.is_empty() {
        format!("arg{id}")
    } else {
        name.clone()
    }
}

fn local_offset(func: &IrFunction, id: ValueId) -> u32 {
    id.saturating_sub(func.params.len()) as u32
}

fn frame_offset(func: &IrFunction, id: ValueId) -> u32 {
    2u32.saturating_add(local_offset(func, id))
}

fn lower_value(func: &IrFunction, id: ValueId) -> Result<Expr, LowerError> {
    if id < func.params.len() {
        return Ok(Expr::ProcedureParam(param_name(func, id)));
    }
    Ok(Expr::FrameGet {
        offset: frame_offset(func, id),
    })
}

fn lower_const(c: &Constant) -> Expr {
    match c {
        Constant::I1(v) => Expr::bool(*v),
        Constant::I8(v) => Expr::number(*v as f64),
        Constant::I16(v) => Expr::number(*v as f64),
        Constant::I32(v) => Expr::number(*v as f64),
        Constant::F64(v) => Expr::number(*v),
    }
}
