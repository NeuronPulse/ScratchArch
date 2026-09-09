//! SAIR → ScratchGraph lowering.
//!
//! Maps SAIR functions to Scratch custom blocks and SSA values to frame slots
//! inside a runtime call stack. The Scratch target ABI uses a hidden stage list
//! `__scratcharch_stack` and a frame pointer `__scratcharch_fp` so that calls
//! are reentrant.

use std::collections::{HashMap, HashSet};

use scratcharch_ir::function::IrFunction;
use scratcharch_ir::instruction::{CastOp, GepIndex, Instruction as SairInstr, Terminator};
use scratcharch_ir::r#module::{IrModule, STATIC_DATA_BASE};
use scratcharch_ir::types::IrType;
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

        // If any SAIR function uses memory instructions — or the module carries
        // a static-data segment to seed — model memory as a single stage-backed
        // byte heap list.
        let needs_heap = !module.static_data.image.is_empty()
            || module.functions.iter().any(|f| {
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
            // Seed the static-data segment before the first call, mirroring the
            // interpreter's `memory[STATIC_DATA_BASE .. + len] = image`:
            // pad addresses [0, STATIC_DATA_BASE) with zero bytes, then write
            // each image byte at its 1-indexed heap item.
            let mut entry_stmts = vec![
                Stmt::DeleteAllOfList {
                    list: "__scratcharch_stack".to_string(),
                },
                Stmt::SetVariable {
                    var: "__scratcharch_fp".to_string(),
                    value: Expr::number(0.0),
                },
            ];
            if !module.static_data.image.is_empty() {
                let seeded_bytes = STATIC_DATA_BASE as usize + module.static_data.image.len();
                entry_stmts.push(Stmt::Repeat {
                    times: Expr::number(seeded_bytes as f64),
                    body: vec![Stmt::AddToList {
                        list: "__scratcharch_heap".to_string(),
                        value: Expr::number(0.0),
                    }],
                });
                for (i, byte) in module.static_data.image.iter().enumerate() {
                    entry_stmts.push(Stmt::SetListItem {
                        list: "__scratcharch_heap".to_string(),
                        index: Expr::number((STATIC_DATA_BASE as usize + 1 + i) as f64),
                        value: Expr::number(*byte as f64),
                    });
                }
            }
            entry_stmts.extend([
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
            ]);

            stage.add_script(Script::new(EventHat::GreenFlag, entry_stmts));
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
            Terminator::Return { .. } | Terminator::Unreachable => vec![],
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
            Terminator::Unreachable => {
                // Reaching LLVM `unreachable` is a trap. Scratch has no trap, so
                // lowering reports the path rather than inventing a mapping.
                return Err(LowerError::UnsupportedTerminator(format!(
                    "block '{}' ends in 'unreachable' (a trap); no Scratch mapping",
                    block.label
                )));
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
        SairInstr::Load { ty, addr } => {
            // Byte-exact load: recompose `size_in_bytes(ty)` little-endian heap
            // bytes, then normalize the value to the backend's mathematical
            // signed convention (docs/specification/SCRATCH_MEMORY.md).
            let addr_expr = lower_value(func, *addr)?;
            let value = match ty {
                IrType::I1 => Expr::operator(
                    "operator_not",
                    vec![Expr::operator(
                        "operator_equals",
                        vec![heap_byte(&addr_expr, 0), Expr::number(0.0)],
                    )],
                ),
                IrType::I8 => signed_reinterpret(heap_byte(&addr_expr, 0), 8),
                IrType::I16 => signed_reinterpret(recombine_bytes(&addr_expr, 2), 16),
                IrType::I32 => signed_reinterpret(recombine_bytes(&addr_expr, 4), 32),
                // i64 is exact only for stored values in [0, 2^53): the raw
                // non-negative pattern is then exactly representable (SAFE_SUBSET).
                IrType::I64 => recombine_bytes(&addr_expr, 8),
                IrType::Pointer => recombine_bytes(&addr_expr, 4),
                IrType::F64 | IrType::Void => {
                    return Err(LowerError::UnsupportedType(format!(
                        "load of type {ty} is not representable in Scratch bytes"
                    )))
                }
            };
            Ok(vec![Stmt::FrameSet {
                offset: frame_offset(func, result_id),
                value,
            }])
        }
        SairInstr::Store { ty, value, addr } => {
            // Byte-exact store: reduce the value mod its width, split it into
            // little-endian bytes, and write exactly `size_in_bytes(ty)` heap
            // items — neighbouring bytes are preserved.
            let src = lower_value(func, *value)?;
            let addr_expr = lower_value(func, *addr)?;
            let binary = |name: &str, a: Expr, b: Expr| Expr::operator(name, vec![a, b]);
            let num = Expr::number;
            let bytes = match ty {
                IrType::I1 => split_bytes(binary("operator_mod", src, num(2.0)), 1),
                IrType::I8 => split_bytes(binary("operator_mod", src, num(256.0)), 1),
                IrType::I16 => split_bytes(binary("operator_mod", src, num(65536.0)), 2),
                IrType::I32 => split_bytes(binary("operator_mod", src, num(4294967296.0)), 4),
                // i64/ptr: values are (by contract) non-negative f64 in the
                // exact range, so no mod reduction is needed or representable.
                IrType::I64 => split_bytes(src, 8),
                IrType::Pointer => split_bytes(src, 4),
                IrType::F64 | IrType::Void => {
                    return Err(LowerError::UnsupportedType(format!(
                        "store of type {ty} is not representable in Scratch bytes"
                    )))
                }
            };
            let mut stmts = Vec::with_capacity(bytes.len());
            for (k, byte) in bytes.into_iter().enumerate() {
                let index = Expr::operator(
                    "operator_add",
                    vec![addr_expr.clone(), Expr::number(1.0 + k as f64)],
                );
                stmts.push(Stmt::SetListItem {
                    list: "__scratcharch_heap".to_string(),
                    index,
                    value: byte,
                });
            }
            Ok(stmts)
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
        SairInstr::Select {
            condition,
            then_value,
            else_value,
            ..
        } => {
            // `select cond a b` stores a or b into the result slot depending on
            // the i1 condition. Both operands are already materialized in the
            // frame, so an if/else that copies the chosen one is faithful.
            let result_offset = frame_offset(func, result_id);
            Ok(vec![Stmt::If {
                condition: lower_value(func, *condition)?,
                then_body: vec![Stmt::FrameSet {
                    offset: result_offset,
                    value: lower_value(func, *then_value)?,
                }],
                else_body: vec![Stmt::FrameSet {
                    offset: result_offset,
                    value: lower_value(func, *else_value)?,
                }],
            }])
        }
        SairInstr::And { .. }
        | SairInstr::Or { .. }
        | SairInstr::Xor { .. }
        | SairInstr::Shl { .. }
        | SairInstr::Lshr { .. }
        | SairInstr::Ashr { .. } => {
            // Bitwise and shift ops need exact integer bit semantics. The Scratch
            // numeric backend is f64-only and its operator set has no bitwise or
            // shift blocks, so the lowering reports rather than approximating.
            Err(LowerError::UnsupportedInstruction(format!(
                "{} cannot be lowered to Scratch numbers",
                op_name(instr),
            )))
        }
        SairInstr::Cast {
            op,
            from_ty,
            to_ty,
            value,
        } => {
            // Width-changing casts lower to closed-form Scratch arithmetic
            // (docs/design/SCRATCH_NUMERIC_MODEL.md §3.2). Scratch's floor-mod
            // (`operator_mod`) and boolean coercion make every cast exact: mod
            // reduces the source to its low width, the signed-reinterpret
            // predicates shift into the mathematical (two's-complement) range,
            // and pointer/int casts are identity because pointers *are* heap
            // offsets. Constructs with no exact Scratch form are reported, never
            // approximated.
            let value_expr = lower_value(func, *value)?;
            let expr = lower_cast(op, from_ty, to_ty, value_expr)?;
            Ok(vec![Stmt::FrameSet {
                offset: frame_offset(func, result_id),
                value: expr,
            }])
        }
    }
}

/// Lower a SAIR cast to an exact Scratch expression.
///
/// Scratch numbers are f64; every integer up to 2^53 is exact, `operator_mod`
/// is floor-mod (non-negative result for a positive divisor), and booleans
/// coerce to 0/1 in arithmetic. The formulas below use those three facts:
///
/// - **Widening** is exact once the source is reduced to its low width and, for
///   `zext` from a negative i32, shifted up by 2^32 (unsigned reinterpretation).
///   `sext i32 → i64` is identity because Scratch already holds the
///   mathematical signed value.
/// - **`trunc i64 → i32`** reduces mod 2^32 then reinterprets the high bit as
///   signed — the two's-complement truncation.
/// - **`zext i1`** forces numeric coercion (`x + 0`) so a boolean predicate
///   becomes exactly 1 or 0.
/// - **`ptrtoint`/`inttoptr`/`bitcast`** between integers and pointers are
///   identity: an address is an f64 heap offset.
fn lower_cast(op: &CastOp, from_ty: &IrType, to_ty: &IrType, x: Expr) -> Result<Expr, LowerError> {
    // Reject float reinterpretations: identity would silently round, and the
    // numeric model only covers integer/pointer cells.
    if from_ty == &IrType::F64 || to_ty == &IrType::F64 {
        return Err(LowerError::UnsupportedType(format!(
            "{} between {} and {} is not representable in Scratch numbers",
            op.name(),
            from_ty,
            to_ty,
        )));
    }
    let binary = |name: &str, a: Expr, b: Expr| Expr::operator(name, vec![a, b]);
    let num = Expr::number;

    match op {
        CastOp::PtrToInt | CastOp::IntToPtr | CastOp::Bitcast => Ok(x),
        CastOp::Zext => match from_ty.integer_width() {
            Some(1) => Ok(binary("operator_add", x, num(0.0))),
            Some(8) => Ok(binary("operator_mod", x, num(256.0))),
            Some(16) => Ok(binary("operator_mod", x, num(65536.0))),
            Some(32) => Ok(binary(
                "operator_add",
                x.clone(),
                binary(
                    "operator_multiply",
                    num(4294967296.0),
                    binary("operator_lt", x, num(0.0)),
                ),
            )),            _ => Err(LowerError::UnsupportedInstruction(format!(
                "zext {} to {} cannot be lowered to Scratch numbers",
                from_ty, to_ty,
            ))),
        },
        CastOp::Sext => match from_ty.integer_width() {
            Some(1) => Ok(binary(
                "operator_subtract",
                num(0.0),
                binary("operator_gt", x, num(0.0)),
            )),
            Some(8) => {
                let m = binary("operator_mod", x, num(256.0));
                Ok(binary(
                    "operator_subtract",
                    m.clone(),
                    binary(
                        "operator_multiply",
                        num(256.0),
                        binary("operator_gt", m, num(127.0)),
                    ),
                ))
            }
            Some(16) => {
                let m = binary("operator_mod", x, num(65536.0));
                Ok(binary(
                    "operator_subtract",
                    m.clone(),
                    binary(
                        "operator_multiply",
                        num(65536.0),
                        binary("operator_gt", m, num(32767.0)),
                    ),
                ))
            }
            // i32 is already the mathematical signed value; sign extension to
            // i64 is a no-op on the Scratch f64 representation.
            Some(32) => Ok(x),
            _ => Err(LowerError::UnsupportedInstruction(format!(
                "sext {} to {} cannot be lowered to Scratch numbers",
                from_ty, to_ty,
            ))),
        },
        CastOp::Trunc => match to_ty.integer_width() {
            Some(1) => Ok(binary("operator_mod", x, num(2.0))),
            Some(8) => Ok(binary("operator_mod", x, num(256.0))),
            Some(16) => Ok(binary("operator_mod", x, num(65536.0))),
            Some(32) => {
                let m = binary("operator_mod", x, num(4294967296.0));
                Ok(binary(
                    "operator_subtract",
                    m.clone(),
                    binary(
                        "operator_multiply",
                        num(4294967296.0),
                        binary("operator_gt", m, num(2147483647.0)),
                    ),
                ))
            }
            _ => Err(LowerError::UnsupportedInstruction(format!(
                "trunc {} to {} cannot be lowered to Scratch numbers",
                from_ty, to_ty,
            ))),
        },
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

/// The SAIR mnemonic of the bitwise/shift ops (for diagnostics).
fn op_name(instr: &SairInstr) -> &'static str {
    match instr {
        SairInstr::And { .. } => "and",
        SairInstr::Or { .. } => "or",
        SairInstr::Xor { .. } => "xor",
        SairInstr::Shl { .. } => "shl",
        SairInstr::Lshr { .. } => "lshr",
        SairInstr::Ashr { .. } => "ashr",
        _ => "bitwise op",
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
        // Scratch numbers are IEEE doubles: small i64 magnitudes round-trip
        // exactly, values above 2^53 lose low bits (documented Scratch backend
        // limitation, not a silent truncation at SAIR level).
        Constant::I64(v) => Expr::number(*v as f64),
    }
}

// ── Byte-exact memory helpers ───────────────────────────────────────────────
//
// The ScratchGraph heap is byte-addressable (docs/specification/SCRATCH_MEMORY
// .md): one list item per byte, little-endian. Loads/stores decompose a value
// into exactly `size_in_bytes(ty)` bytes; widths come from SAIR types, never
// re-derived here. All operations are exact f64 arithmetic:
//
//   split:   (m − b0) is a multiple of 256, so `/ 256` is exact — no Scratch
//            `floor` operator is required.
//   combine: every term is below 2^53 for the supported widths.
//   signed:  `m − 2^N·(m > 2^(N−1)−1)` reinterprets the raw pattern to the
//            mathematical signed value (Scratch booleans coerce to 0/1).

fn heap_byte(addr: &Expr, k: usize) -> Expr {
    // Byte at 0-based address `addr + k` is 1-indexed list item `addr + k + 1`.
    Expr::ListItem {
        list: "__scratcharch_heap".to_string(),
        index: Box::new(Expr::operator(
            "operator_add",
            vec![addr.clone(), Expr::number(1.0 + k as f64)],
        )),
    }
}

/// Recompose `count` little-endian bytes at `addr` into one value expression
/// `b0 + 256·b1 + 256²·b2 + …`.
fn recombine_bytes(addr: &Expr, count: usize) -> Expr {
    let binary = |name: &str, a: Expr, b: Expr| Expr::operator(name, vec![a, b]);
    let mut sum = Expr::number(0.0);
    for k in 0..count {
        let term = binary(
            "operator_multiply",
            heap_byte(addr, k),
            Expr::number(256.0f64.powi(k as i32)),
        );
        sum = binary("operator_add", sum, term);
    }
    sum
}

/// Split an already-mod-reduced, non-negative value into `count` little-endian
/// byte expressions (LSB first).
fn split_bytes(value: Expr, count: usize) -> Vec<Expr> {
    let binary = |name: &str, a: Expr, b: Expr| Expr::operator(name, vec![a, b]);
    let mut bytes = Vec::with_capacity(count);
    let mut rem = value;
    for k in 0..count {
        let byte = binary("operator_mod", rem.clone(), Expr::number(256.0));
        if k + 1 < count {
            // `rem − byte` is an exact multiple of 256, so the division is exact.
            let shifted = binary("operator_subtract", rem, byte.clone());
            rem = binary("operator_divide", shifted, Expr::number(256.0));
        }
        bytes.push(byte);
    }
    bytes
}

/// Reinterpret raw unsigned bits `m` (in [0, 2^width)) as the mathematical
/// signed value: `m − 2^width` when the top bit is set, else `m`.
fn signed_reinterpret(m: Expr, width: u32) -> Expr {
    let binary = |name: &str, a: Expr, b: Expr| Expr::operator(name, vec![a, b]);
    let modulus = (1u64 << width) as f64;
    let half = (1u64 << (width - 1)) as f64;
    binary(
        "operator_subtract",
        m.clone(),
        binary(
            "operator_multiply",
            Expr::number(modulus),
            binary("operator_gt", m, Expr::number(half - 1.0)),
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::Value as IrValue;
    use scratcharch_ir::builder::IrBuilder;

    /// Minimal Scratch-expression evaluator for formula-level verification.
    ///
    /// Evaluates the exact f64 arithmetic the emitted Scratch blocks perform:
    /// floor-mod, boolean coercion (true/false -> 1/0), and 1-indexed list
    /// reads against a mock byte heap. This proves the split/combine/signed
    /// formulas, not just their shape.
    fn eval_expr(e: &Expr, heap: &[f64]) -> f64 {
        eval_expr_frame(e, heap, &[])
    }

    fn eval_expr_frame(e: &Expr, heap: &[f64], frame: &[f64]) -> f64 {
        match e {
            Expr::Literal(IrValue::Number(v)) => *v,
            Expr::Literal(IrValue::Bool(b)) => {
                if *b {
                    1.0
                } else {
                    0.0
                }
            }
            Expr::ListItem { index, .. } => {
                let i = eval_expr(index, heap);
                heap[(i as usize) - 1]
            }
            Expr::ListLength { .. } => heap.len() as f64,
            Expr::FrameGet { offset } => frame[*offset as usize],
            Expr::Operator { opcode, args } => {
                let mut vals = args.iter().map(|a| eval_expr_frame(a, heap, frame));
                match opcode.as_str() {
                    "operator_add" => vals.next().unwrap() + vals.next().unwrap(),
                    "operator_subtract" => vals.next().unwrap() - vals.next().unwrap(),
                    "operator_multiply" => vals.next().unwrap() * vals.next().unwrap(),
                    "operator_divide" => vals.next().unwrap() / vals.next().unwrap(),
                    "operator_mod" => {
                        let a = vals.next().unwrap();
                        let b = vals.next().unwrap();
                        a - b * (a / b).floor()
                    }
                    "operator_gt" => (vals.next().unwrap() > vals.next().unwrap()) as u8 as f64,
                    "operator_lt" => (vals.next().unwrap() < vals.next().unwrap()) as u8 as f64,
                    "operator_equals" => (vals.next().unwrap() == vals.next().unwrap()) as u8 as f64,
                    "operator_not" => (vals.next().unwrap() == 0.0) as u8 as f64,
                    other => panic!("unexpected opcode {other}"),
                }
            }
            other => panic!("unsupported expr variant in evaluator: {other:?}"),
        }
    }

    fn le_bytes(v: f64, count: usize) -> Vec<f64> {
        let u = v as u64;
        (0..count)
            .map(|k| ((u >> (8 * k)) & 0xFF) as f64)
            .collect()
    }

    #[test]
    fn split_matches_reference_bytes() {
        let values = [
            0.0,
            1.0,
            42.0,
            127.0,
            255.0,
            256.0,
            65535.0,
            65536.0,
            16777215.0,
            16777216.0,
            2147483647.0,
            2147483648.0,
            4294967295.0,
        ];
        for &count in &[1usize, 2, 4, 8] {
            for &v in &values {
                let m = if count == 8 {
                    v
                } else {
                    (v as u64).rem_euclid(1u64 << (8 * count)) as f64
                };
                let bytes = split_bytes(Expr::number(m), count);
                let got: Vec<f64> = bytes.iter().map(|b| eval_expr(b, &[])).collect();
                let expected = le_bytes(m, count);
                assert_eq!(got, expected, "split({m}, {count})");
            }
        }
    }

    #[test]
    fn combine_matches_reference_bytes() {
        // Little-endian combine: item at 1-based index addr+k+1 is byte k.
        let heap: Vec<f64> = (0..16).map(|i| i as f64).collect();
        let v = eval_expr(&recombine_bytes(&Expr::number(0.0), 4), &heap);
        assert_eq!(v, 0.0 + 256.0 + 2.0 * 65536.0 + 3.0 * 16777216.0);
        // Reuse the split formulas in reverse: recombine(addr) with the split
        // bytes planted at addr recovers the value, at every width.
        for &count in &[1usize, 2, 4] {
            let m = (0xDEAD_BEEFu64 % (1u64 << (8 * count))) as f64;
            let bytes = split_bytes(Expr::number(m), count);
            let mut heap = vec![0.0; 16];
            for (k, b) in bytes.iter().enumerate() {
                heap[k] = eval_expr(b, &heap);
            }
            let loaded = recombine_bytes(&Expr::number(0.0), count);
            assert_eq!(eval_expr(&loaded, &heap), m, "combine({count})");
        }
    }

    #[test]
    fn signed_reinterpret_at_boundary() {
        let cases: &[(f64, u32, f64)] = &[
            (0.0, 32, 0.0),
            (2147483647.0, 32, 2147483647.0),
            (2147483648.0, 32, -2147483648.0),
            (4294967295.0, 32, -1.0),
            (65535.0, 16, -1.0),
            (32767.0, 16, 32767.0),
            (32768.0, 16, -32768.0),
            (255.0, 8, -1.0),
            (127.0, 8, 127.0),
            (128.0, 8, -128.0),
        ];
        for (raw, width, expect) in cases {
            let got = eval_expr(&signed_reinterpret(Expr::number(*raw), *width), &[]);
            assert_eq!(got, *expect, "reinterpret {raw} as i{width}");
        }
    }

    #[test]
    fn store_then_load_roundtrips_signed_and_raw_values() {
        // The store reduces mod the width, so the same bytes result whether
        // the carried value is the raw pattern or the mathematical value; the
        // load then normalizes back to the mathematical signed value.
        for &v in &[-5.0f64, -1.0, 0.0, 42.0, 2147483647.0, -2147483648.0] {
            let raw = (v as i64).rem_euclid(1 << 32) as f64;
            for &carried in &[v, raw] {
                let reduced = Expr::operator(
                    "operator_mod",
                    vec![Expr::number(carried), Expr::number(4294967296.0)],
                );
                let bytes = split_bytes(reduced, 4);
                let mut heap = vec![0.0; 8];
                for (k, b) in bytes.iter().enumerate() {
                    heap[k] = eval_expr(b, &heap);
                }
                let loaded = signed_reinterpret(recombine_bytes(&Expr::number(0.0), 4), 32);
                assert_eq!(eval_expr(&loaded, &heap), v, "round-trip of {v} via {carried}");
            }
        }
    }

    #[test]
    fn store_width_matches_type_size() {
        // Structural: a store emits exactly `size_in_bytes(ty)` SetListItems.
        let mut builder = IrBuilder::new("main");
        builder.start_function("main", IrType::I32);
        builder.new_block("entry");
        let cases: &[(IrType, u32, f64)] = &[
            (IrType::I8, 1, 7.0),
            (IrType::I16, 2, 1000.0),
            (IrType::I32, 4, 300.0),
            (IrType::I64, 8, 45.0),
            (IrType::Pointer, 4, 12.0),
        ];
        for (ty, _width, value) in cases {
            let addr = builder.alloca(*ty);
            let c = builder.const_i32(*value as u32);
            builder.store(*ty, c, addr);
        }
        let project = ScratchGraphLowerer::new().lower(&builder.finish()).expect("lower");
        let body = &project.stage.procedures[0].body;
        let heap_sets = body
            .iter()
            .filter(|s| matches!(s, Stmt::SetListItem { list, .. } if list == "__scratcharch_heap"))
            .count();
        assert_eq!(heap_sets, 1 + 2 + 4 + 8 + 4, "one SetListItem per byte");
    }

    #[test]
    fn neighbor_bytes_not_touched_by_i8_store() {
        // Structural guarantee of byte-exactness: an i8 store emits exactly one
        // SetListItem (its own byte), so neighbours are untouched.
        let mut builder = IrBuilder::new("main");
        builder.start_function("main", IrType::I32);
        builder.new_block("entry");
        let addr = builder.alloca(IrType::I32); // 4 bytes
        let byte_val = builder.const_i32(0xAB);
        builder.store(IrType::I8, byte_val, addr);
        let project = ScratchGraphLowerer::new().lower(&builder.finish()).expect("lower");
        let body = &project.stage.procedures[0].body;
        let heap_sets: Vec<&Stmt> = body
            .iter()
            .filter(|s| matches!(s, Stmt::SetListItem { list, .. } if list == "__scratcharch_heap"))
            .collect();
        assert_eq!(heap_sets.len(), 1, "i8 store must touch one byte");
        if let Stmt::SetListItem { index, value, .. } = heap_sets[0] {
            // index = addr + 1 (address 0 -> list item 1).
            let mut frame = [0.0f64; 8];
            frame[2] = 0.0; // addr slot: first alloca returns heap length 0
            frame[3] = 171.0; // stored-value slot
            let i = eval_expr_frame(index, &[], &frame);
            assert_eq!(i, 1.0, "byte address 0 is list item 1");
            // 0xAB mod 256 = 0xAB.
            assert_eq!(eval_expr_frame(value, &[], &frame), 171.0);
        }
    }

    #[test]
    fn static_data_is_seeded_into_entry_script() {
        // A module with a static-data segment must declare the heap and seed
        // image bytes at STATIC_DATA_BASE+1+i, padded below the base.
        let mut builder = IrBuilder::new("main");
        builder.start_function("main", IrType::I32);
        builder.new_block("entry");
        let zero = builder.const_i32(0);
        builder.ret(Some(zero));
        let mut module = builder.finish();
        module.static_data.image = vec![104, 101, 108, 108, 111, 0]; // "hello\0"

        let project = ScratchGraphLowerer::new().lower(&module).expect("lower");
        assert!(project.stage.lists.iter().any(|l| l.name == "__scratcharch_heap"));

        let script = project
            .stage
            .scripts
            .iter()
            .find(|s| matches!(s.entry.hat, EventHat::GreenFlag))
            .expect("green flag script");
        let seeds: Vec<&Stmt> = script
            .entry
            .body
            .iter()
            .filter(|s| matches!(s, Stmt::SetListItem { list, .. } if list == "__scratcharch_heap"))
            .collect();
        assert_eq!(seeds.len(), module.static_data.image.len());
        for (i, stmt) in seeds.iter().enumerate() {
            if let Stmt::SetListItem { index, value, .. } = stmt {
                assert_eq!(
                    eval_expr(index, &[]),
                    (STATIC_DATA_BASE as usize + 1 + i) as f64,
                    "image byte {i} address"
                );
                assert_eq!(eval_expr(value, &[]), module.static_data.image[i] as f64);
            }
        }
        // The zero padding below the base is appended before the image bytes.
        assert!(script
            .entry
            .body
            .iter()
            .any(|s| matches!(s, Stmt::Repeat { .. })));
    }
}
