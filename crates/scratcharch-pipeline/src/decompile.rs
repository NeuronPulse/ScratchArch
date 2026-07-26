use std::collections::HashMap;

use scratcharch_ir::builder::IrBuilder;
use scratcharch_ir::instruction::{Instruction, Terminator};
use scratcharch_ir::types::IrType;
use scratcharch_ir::value::ValueId;
use scratcharch_scratchgraph::ir::{
    Expr, Procedure, Project, Stmt, Value as SgValue,
};

/// Decompiles a ScratchGraph `Project` back to a SAIR `IrModule`.
#[derive(Debug, Clone, Default)]
pub struct Decompiler;

impl Decompiler {
    pub fn new() -> Self {
        Self
    }

    /// Decompile a complete ScratchGraph project into a SAIR module.
    pub fn decompile(&self, project: &Project) -> Result<scratcharch_ir::module::IrModule, String> {
        let mut builder = IrBuilder::new("main");

        for (_sprite_name, procs) in self.all_procedures(project) {
            for proc in &procs {
                self.decompile_procedure(&mut builder, proc)?;
            }
        }

        Ok(builder.finish())
    }

    fn all_procedures<'a>(
        &self,
        project: &'a Project,
    ) -> Vec<(&'a str, Vec<&'a Procedure>)> {
        let mut result = Vec::new();
        let stage_procs: Vec<&Procedure> = project.stage.procedures.iter().collect();
        if !stage_procs.is_empty() {
            result.push((project.stage.name.as_str(), stage_procs));
        }
        for sprite in &project.sprites {
            let sprite_procs: Vec<&Procedure> = sprite.procedures.iter().collect();
            if !sprite_procs.is_empty() {
                result.push((sprite.name.as_str(), sprite_procs));
            }
        }
        result
    }

    fn decompile_procedure(
        &self,
        builder: &mut IrBuilder,
        proc: &Procedure,
    ) -> Result<(), String> {
        let func_name = proc.prototype.name.clone();
        builder.start_function(&func_name, IrType::Void);

        for param in &proc.prototype.params {
            builder.add_param(IrType::I32, &param.name);
        }

        let used_vars = self.collect_variables(proc);
        let var_allocas: HashMap<String, ValueId> = used_vars
            .iter()
            .map(|name| {
                let alloca_id = builder.new_value(IrType::Pointer, None::<&str>);
                (name.clone(), alloca_id)
            })
            .collect();

        let mut ctx = DecompileCtx {
            var_allocas,
            block_counter: 0,
        };

        builder.new_block("entry");

        for _alloca_id in ctx.var_allocas.values() {
            let func = builder.module.get_function_mut(&func_name).unwrap();
            let block = func.blocks.last_mut().unwrap();
            block.push(Instruction::Alloca {
                ty: IrType::I32,
                count: 1,
            });
        }

        self.decompile_stmts(builder, &mut ctx, &proc.body, &func_name)?;

        let func = builder.module.get_function_mut(&func_name).unwrap();
        let last_block = func.blocks.last_mut().unwrap();
        if !matches!(last_block.terminator, Terminator::Return { .. }) {
            last_block.set_terminator(Terminator::Return { value: None });
        }

        Ok(())
    }

    fn decompile_stmts(
        &self,
        builder: &mut IrBuilder,
        ctx: &mut DecompileCtx,
        stmts: &[Stmt],
        func_name: &str,
    ) -> Result<(), String> {
        for stmt in stmts {
            self.decompile_stmt(builder, ctx, stmt, func_name)?;
        }
        Ok(())
    }

    fn decompile_stmt(
        &self,
        builder: &mut IrBuilder,
        ctx: &mut DecompileCtx,
        stmt: &Stmt,
        func_name: &str,
    ) -> Result<(), String> {
        match stmt {
            Stmt::SetVariable { var, value } => {
                let val = self.decompile_expr(builder, ctx, value, func_name)?;
                if let Some(alloca) = ctx.var_allocas.get(var) {
                    builder.store(IrType::I32, val, *alloca);
                }
            }
            Stmt::ChangeVariable { var, delta } => {
                let delta_val = self.decompile_expr(builder, ctx, delta, func_name)?;
                if let Some(alloca) = ctx.var_allocas.get(var) {
                    let current = builder.load(IrType::I32, *alloca);
                    let new_val = builder.add(IrType::I32, current, delta_val);
                    builder.store(IrType::I32, new_val, *alloca);
                }
            }
            Stmt::Call { proc, args } => {
                let mut arg_ids = Vec::new();
                for arg in args {
                    let id = self.decompile_expr(builder, ctx, arg, func_name)?;
                    arg_ids.push(id);
                }
                builder.call(IrType::Void, proc, arg_ids);
            }
            Stmt::Broadcast { message } => {
                let _msg = self.decompile_expr(builder, ctx, message, func_name)?;
            }
            Stmt::If {
                condition,
                then_body,
                else_body,
            } => {
                let cond = self.decompile_expr(builder, ctx, condition, func_name)?;
                let then_label = ctx.fresh_block("then");
                let else_label = ctx.fresh_block("else");
                let merge_label = ctx.fresh_block("merge");

                builder.cond_br(cond, &then_label, &else_label);

                builder.new_block(&then_label);
                self.decompile_stmts(builder, ctx, then_body, func_name)?;
                builder.br(&merge_label);

                builder.new_block(&else_label);
                self.decompile_stmts(builder, ctx, else_body, func_name)?;
                builder.br(&merge_label);

                builder.new_block(&merge_label);
            }
            Stmt::Repeat { times, body } => {
                let _times_val = self.decompile_expr(builder, ctx, times, func_name)?;
                let header_label = ctx.fresh_block("loop_header");
                let body_label = ctx.fresh_block("loop_body");

                builder.br(&header_label);
                builder.new_block(&header_label);
                builder.br(&body_label);

                builder.new_block(&body_label);
                self.decompile_stmts(builder, ctx, body, func_name)?;
                builder.br(&header_label);

                let end_label = ctx.fresh_block("loop_end");
                builder.new_block(&end_label);
            }
            Stmt::RepeatUntil { condition, body } => {
                let header_label = ctx.fresh_block("loop_header");
                let body_label = ctx.fresh_block("loop_body");
                let end_label = ctx.fresh_block("loop_end");

                builder.br(&header_label);

                builder.new_block(&header_label);
                let cond = self.decompile_expr(builder, ctx, condition, func_name)?;
                builder.cond_br(cond, &end_label, &body_label);

                builder.new_block(&body_label);
                self.decompile_stmts(builder, ctx, body, func_name)?;
                builder.br(&header_label);

                builder.new_block(&end_label);
            }
            Stmt::Forever { body } => {
                let body_label = ctx.fresh_block("forever_body");
                builder.br(&body_label);
                builder.new_block(&body_label);
                self.decompile_stmts(builder, ctx, body, func_name)?;
                builder.br(&body_label);
            }
            Stmt::Stop { option: _ } => {
                let func = builder.module.get_function_mut(func_name).unwrap();
                if let Some(block) = func.blocks.last_mut() {
                    block.set_terminator(Terminator::Return { value: None });
                }
                let unreachable = ctx.fresh_block("unreachable");
                builder.new_block(&unreachable);
            }
            Stmt::Expr(_)
            | Stmt::AddToList { .. }
            | Stmt::DeleteAllOfList { .. }
            | Stmt::SetListItem { .. }
            | Stmt::DeleteListItem { .. }
            | Stmt::InsertListItem { .. }
            | Stmt::HeapAlloc { .. }
            | Stmt::EnterFrame { .. }
            | Stmt::PopFrame { .. }
            | Stmt::FrameSet { .. } => {}
        }
        Ok(())
    }

    fn decompile_expr(
        &self,
        builder: &mut IrBuilder,
        ctx: &mut DecompileCtx,
        expr: &Expr,
        func_name: &str,
    ) -> Result<ValueId, String> {
        match expr {
            Expr::Literal(SgValue::Number(n)) => {
                let bits = f64::to_bits(*n) as u32;
                Ok(builder.const_i32(bits))
            }
            Expr::Literal(SgValue::String(s)) => {
                Ok(builder.const_i32(s.len() as u32))
            }
            Expr::Literal(SgValue::Bool(b)) => {
                Ok(builder.const_i1(*b))
            }
            Expr::Variable(name) => {
                if let Some(alloca) = ctx.var_allocas.get(name.as_str()) {
                    Ok(builder.load(IrType::I32, *alloca))
                } else {
                    Ok(builder.const_i32(0))
                }
            }
            Expr::ProcedureParam(name) => {
                let func = builder.module.get_function(func_name).unwrap();
                let idx = func.params.iter().position(|(_, pn)| pn == name);
                if let Some(i) = idx {
                    Ok(i)
                } else {
                    Ok(builder.const_i32(0))
                }
            }
            Expr::Operator { opcode, args } => {
                let left = if !args.is_empty() {
                    self.decompile_expr(builder, ctx, &args[0], func_name)?
                } else {
                    builder.const_i32(0)
                };
                let right = if args.len() > 1 {
                    self.decompile_expr(builder, ctx, &args[1], func_name)?
                } else {
                    builder.const_i32(0)
                };
                match opcode.as_str() {
                    "operator_add" => Ok(builder.add(IrType::I32, left, right)),
                    "operator_subtract" => Ok(builder.sub(IrType::I32, left, right)),
                    "operator_multiply" => Ok(builder.mul(IrType::I32, left, right)),
                    "operator_divide" => Ok(builder.div(IrType::I32, left, right)),
                    "operator_equals" => Ok(builder.eq(IrType::I32, left, right)),
                    "operator_lt" => Ok(builder.lt(IrType::I32, left, right)),
                    "operator_gt" => Ok(builder.gt(IrType::I32, left, right)),
                    _ => Ok(builder.add(IrType::I32, left, right)),
                }
            }
            _ => Ok(builder.const_i32(0)),
        }
    }

    fn collect_variables(&self, proc: &Procedure) -> Vec<String> {
        let mut vars = Vec::new();
        self.collect_vars_from_stmts(&proc.body, &mut vars);
        vars.sort();
        vars.dedup();
        vars
    }

    fn collect_vars_from_stmts(&self, stmts: &[Stmt], vars: &mut Vec<String>) {
        for stmt in stmts {
            match stmt {
                Stmt::SetVariable { var, value } => {
                    vars.push(var.clone());
                    self.collect_vars_from_expr(value, vars);
                }
                Stmt::ChangeVariable { var, delta } => {
                    vars.push(var.clone());
                    self.collect_vars_from_expr(delta, vars);
                }
                Stmt::Call { args, .. } => {
                    for arg in args {
                        self.collect_vars_from_expr(arg, vars);
                    }
                }
                Stmt::If {
                    condition,
                    then_body,
                    else_body,
                } => {
                    self.collect_vars_from_expr(condition, vars);
                    self.collect_vars_from_stmts(then_body, vars);
                    self.collect_vars_from_stmts(else_body, vars);
                }
                Stmt::Repeat { times, body } => {
                    self.collect_vars_from_expr(times, vars);
                    self.collect_vars_from_stmts(body, vars);
                }
                Stmt::RepeatUntil { condition, body } => {
                    self.collect_vars_from_expr(condition, vars);
                    self.collect_vars_from_stmts(body, vars);
                }
                Stmt::Forever { body } => {
                    self.collect_vars_from_stmts(body, vars);
                }
                _ => {}
            }
        }
    }

    fn collect_vars_from_expr(&self, expr: &Expr, vars: &mut Vec<String>) {
        match expr {
            Expr::Variable(name) => {
                vars.push(name.clone());
            }
            Expr::Operator { args, .. } => {
                for arg in args {
                    self.collect_vars_from_expr(arg, vars);
                }
            }
            _ => {}
        }
    }
}

struct DecompileCtx {
    var_allocas: HashMap<String, ValueId>,
    block_counter: u32,
}

impl DecompileCtx {
    fn fresh_block(&mut self, prefix: &str) -> String {
        let label = format!("{}_{}", prefix, self.block_counter);
        self.block_counter += 1;
        label
    }
}
