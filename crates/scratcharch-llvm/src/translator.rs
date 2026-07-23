use std::collections::HashMap;
use scratcharch_ir::builder::IrBuilder;
use scratcharch_ir::instruction::GepIndex;
use scratcharch_ir::r#module::IrModule;
use scratcharch_ir::types::IrType;
use scratcharch_ir::value::ValueId;
use crate::errors::LlvmError;
use crate::parser::*;

fn llvm_type_to_ir(ty: &LlvmType) -> Result<IrType, LlvmError> {
    match ty {
        LlvmType::I1 => Ok(IrType::I1),
        LlvmType::I8 => Ok(IrType::I8),
        LlvmType::I16 => Ok(IrType::I16),
        LlvmType::I32 => Ok(IrType::I32),
        LlvmType::Ptr => Ok(IrType::Pointer),
        LlvmType::Void => Ok(IrType::Void),
        LlvmType::Array { inner, .. } => llvm_type_to_ir(inner),
        LlvmType::Struct(_) => Ok(IrType::I32),
        LlvmType::UnnamedStruct { .. } => Ok(IrType::I32),
    }
}

fn extract_array_info(ty: &LlvmType) -> (IrType, u32) {
    match ty {
        LlvmType::Array { inner, count } => {
            let (inner_ir, inner_count) = extract_array_info(inner);
            (inner_ir, *count * inner_count)
        }
        LlvmType::UnnamedStruct { fields } => {
            (IrType::I32, fields.len() as u32)
        }
        other => (llvm_type_to_ir(other).unwrap_or(IrType::I32), 1),
    }
}

fn is_struct_type(ty: &LlvmType) -> bool {
    matches!(ty, LlvmType::Struct(_) | LlvmType::UnnamedStruct { .. })
}

fn const_to_builder(builder: &mut IrBuilder, c: &LlvmConst) -> Result<ValueId, LlvmError> {
    match c.ty {
        LlvmType::I1 => Ok(builder.const_i1(c.value != 0)),
        LlvmType::I8 => Ok(builder.const_i8(c.value as u8)),
        LlvmType::I16 => Ok(builder.const_i16(c.value as u16)),
        LlvmType::I32 => Ok(builder.const_i32(c.value as u32)),
        _ => Err(LlvmError::Translation(format!(
            "cannot create constant for type {:?}", c.ty
        ))),
    }
}

pub fn translate(program: &LlvmProgram) -> Result<IrModule, LlvmError> {
    if program.functions.is_empty() {
        return Err(LlvmError::Translation("no functions in LLVM program".into()));
    }

    // Use "main" as entry if present; otherwise use the last defined function.
    // Declarations are not valid entry points.
    let entry_name = program.functions.iter()
        .find(|f| f.name == "main" && !f.is_declaration)
        .map(|f| f.name.clone())
        .or_else(|| program.functions.iter().rev().find(|f| !f.is_declaration).map(|f| f.name.clone()))
        .ok_or_else(|| LlvmError::Translation("no function definitions in LLVM program".into()))?;
    let mut builder = IrBuilder::new(&entry_name);

    for func in &program.functions {
        if func.is_declaration {
            // Declarations are references to external functions (e.g. runtime
            // intrinsics). They have no body and are resolved at call sites by
            // the SAIR interpreter's runtime intrinsic dispatcher.
            continue;
        }

        let return_ty = llvm_type_to_ir(&func.return_ty)?;
        builder.start_function(func.name.clone(), return_ty);

        let mut value_map: HashMap<String, ValueId> = HashMap::new();
        for (param_ty, param_name) in &func.params {
            let ir_ty = llvm_type_to_ir(param_ty)?;
            let id = builder.add_param(ir_ty, param_name.clone());
            value_map.insert(param_name.clone(), id);
        }

        for block in &func.blocks {
            builder.new_block(block.label.clone());
            for instr in &block.instructions {
                translate_instruction(&mut builder, &mut value_map, instr)?;
            }
        }
    }

    let module = builder.finish();

    if let Err(e) = module.validate() {
        return Err(LlvmError::Translation(format!("SAIR validation failed: {}", e)));
    }

    Ok(module)
}

fn resolve_value_or_emit(
    builder: &mut IrBuilder,
    value_map: &HashMap<String, ValueId>,
    val: &LlvmValue,
) -> Result<ValueId, LlvmError> {
    match val {
        LlvmValue::Const(c) => const_to_builder(builder, c),
        LlvmValue::Local(name) => {
            value_map.get(name)
                .copied()
                .ok_or_else(|| LlvmError::UndefinedValue(format!("%{}", name)))
        }
        LlvmValue::Global(name) => {
            Err(LlvmError::Translation(format!(
                "cannot use global '@{}' as an SSA value", name
            )))
        }
    }
}

fn translate_instruction(
    builder: &mut IrBuilder,
    value_map: &mut HashMap<String, ValueId>,
    instr: &LlvmInstr,
) -> Result<(), LlvmError> {
    match instr {
        LlvmInstr::BinOp { dest, op, ty, lhs, rhs } => {
            let ir_ty = llvm_type_to_ir(ty)?;
            let l = resolve_value_or_emit(builder, value_map, lhs)?;
            let r = resolve_value_or_emit(builder, value_map, rhs)?;
            let result = match op.as_str() {
                "add" => builder.add(ir_ty, l, r),
                "sub" => builder.sub(ir_ty, l, r),
                "mul" => builder.mul(ir_ty, l, r),
                "div" => builder.div(ir_ty, l, r),
                _ => return Err(LlvmError::UnsupportedInstruction(op.clone())),
            };
            if let Some(name) = dest {
                value_map.insert(name.clone(), result);
            }
        }
        LlvmInstr::Icmp { dest, pred, ty, lhs, rhs } => {
            let ir_ty = llvm_type_to_ir(ty)?;
            let l = resolve_value_or_emit(builder, value_map, lhs)?;
            let r = resolve_value_or_emit(builder, value_map, rhs)?;
            let result = match pred {
                IcmpPred::Eq => builder.eq(ir_ty, l, r),
                IcmpPred::Ne => {
                    let eq = builder.eq(ir_ty, l, r);
                    let zero = builder.const_i1(false);
                    builder.eq(IrType::I1, eq, zero)
                }
                IcmpPred::Slt => builder.lt(ir_ty, l, r),
                IcmpPred::Sgt => builder.gt(ir_ty, l, r),
                IcmpPred::Sle => {
                    let gt = builder.gt(ir_ty, l, r);
                    let zero = builder.const_i1(false);
                    builder.eq(IrType::I1, gt, zero)
                }
                IcmpPred::Sge => {
                    let lt = builder.lt(ir_ty, l, r);
                    let zero = builder.const_i1(false);
                    builder.eq(IrType::I1, lt, zero)
                }
            };
            value_map.insert(dest.clone(), result);
        }
        LlvmInstr::Ret { value } => {
            match value {
                Some(v) => {
                    let id = resolve_value_or_emit(builder, value_map, v)?;
                    builder.ret(Some(id));
                }
                None => {
                    builder.ret(None);
                }
            }
        }
        LlvmInstr::Br { target } => {
            builder.br(target.clone());
        }
        LlvmInstr::CondBr { cond, true_target, false_target } => {
            let cond_id = resolve_value_or_emit(builder, value_map, cond)?;
            builder.cond_br(cond_id, true_target.clone(), false_target.clone());
        }
        LlvmInstr::Alloca { dest, ty, count } => {
            let (ir_ty, total_count) = extract_array_info(ty);
            let total = total_count * count;
            let result = if total > 1 {
                builder.alloca_array(ir_ty, total)
            } else {
                builder.alloca(ir_ty)
            };
            value_map.insert(dest.clone(), result);
        }
        LlvmInstr::Load { dest, ty, addr } => {
            let ir_ty = llvm_type_to_ir(ty)?;
            let addr_id = resolve_value_or_emit(builder, value_map, addr)?;
            let result = builder.load(ir_ty, addr_id);
            value_map.insert(dest.clone(), result);
        }
        LlvmInstr::Store { ty, value, addr } => {
            let ir_ty = llvm_type_to_ir(ty)?;
            let val_id = resolve_value_or_emit(builder, value_map, value)?;
            let addr_id = resolve_value_or_emit(builder, value_map, addr)?;
            builder.store(ir_ty, val_id, addr_id);
        }
        LlvmInstr::Call { dest, return_ty, callee, args } => {
            let ir_ret_ty = llvm_type_to_ir(return_ty)?;
            let mut arg_ids = Vec::new();
            for arg in args {
                let id = resolve_value_or_emit(builder, value_map, arg)?;
                arg_ids.push(id);
            }
            let result = builder.call(ir_ret_ty, callee.clone(), arg_ids);
            if let Some(name) = dest {
                if let Some(id) = result {
                    value_map.insert(name.clone(), id);
                }
            }
        }
        LlvmInstr::Gep { dest, elem_ty, base, indices } => {
            let (ir_elem_ty, _) = extract_array_info(elem_ty);
            let base_id = resolve_value_or_emit(builder, value_map, base)?;
            let gep_indices = if matches!(elem_ty, LlvmType::Array { .. }) {
                let mut result = Vec::new();
                for (i, idx) in indices.iter().enumerate() {
                    if i == 0 {
                        continue;
                    }
                    let idx_id = resolve_value_or_emit(builder, value_map, idx)?;
                    result.push(GepIndex::Dynamic(idx_id));
                }
                result
            } else if is_struct_type(elem_ty) {
                let mut result = Vec::new();
                for (i, idx) in indices.iter().enumerate() {
                    if i == 0 {
                        continue;
                    }
                    if let LlvmValue::Const(c) = idx {
                        result.push(GepIndex::StructField(c.value as u32));
                    } else {
                        let idx_id = resolve_value_or_emit(builder, value_map, idx)?;
                        result.push(GepIndex::Dynamic(idx_id));
                    }
                }
                result
            } else {
                let mut result = Vec::new();
                for idx in indices {
                    let idx_id = resolve_value_or_emit(builder, value_map, idx)?;
                    result.push(GepIndex::Dynamic(idx_id));
                }
                result
            };
            let result = builder.gep(ir_elem_ty, base_id, gep_indices);
            value_map.insert(dest.clone(), result);
        }
    }
    Ok(())
}
