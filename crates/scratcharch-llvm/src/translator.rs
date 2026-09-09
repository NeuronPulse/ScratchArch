//! ScratchGraph LLVM IR → SAIR translator.
//!
//! Translates the LLVM subset produced by the parser into an `IrModule`.
//!
//! # Memory model
//!
//! SAIR memory is flat and byte-addressable with 32-bit addresses. Every LLVM
//! allocation is lowered to a byte array (`i8` alloca) whose size is the
//! aggregate byte size computed from the module's source layout (x86-64 for
//! the clang corpus). Every `getelementptr` is lowered to a single byte offset,
//! folded from the typed index chain, and emitted as one byte-addressed GEP:
//! `gep i8, %base, <byte offset>`. Struct field offsets and array strides are
//! computed by [`scratcharch_target::layout`], the single source of layout
//! arithmetic — nothing here re-derives offsets by hand.
//!
//! # Signed vs unsigned semantics
//!
//! The SAIR `Lt`/`Gt` compare the raw bit patterns of masked values, i.e. they
//! are unsigned on the stored bits. Unsigned LLVM predicates map one-to-one
//! onto those. Signed predicates (`slt`/`sgt`/`sle`/`sge`) are expanded exactly
//! from the sign-bit identity
//!
//! ```text
//! slt(a, b) = (sign(a) != sign(b)) ? sign(a) : (a <u b)
//! ```
//!
//! over unsigned `Lt` + `Eq` + `Select` on `i1` flags, which is exact for
//! negative and mixed-sign operands at every supported width (see
//! [`emit_signed_lt`]). Signed division/remainder are likewise expanded from
//! magnitudes ([`translate_signed_divrem`]).
//!
//! # `phi`
//!
//! LLVM `phi` translates one-to-one to SAIR `Phi` with incoming `(value,
//! predecessor block)` pairs. A back-edge operand is defined in a block that
//! has not been translated yet when the phi is emitted (the loop latch comes
//! textually after the header). Such operands are emitted against a placeholder
//! zero constant and patched to the real value once the whole function is laid
//! out ([`finalize_function`]). Switch dispatch blocks introduced by
//! `translate_switch` become the real predecessors of case/default targets, so
//! the same finalization rewrites any phi incoming whose predecessor was the
//! (now-expanded) switch block.

use std::collections::{HashMap, HashSet};

use scratcharch_ir::builder::IrBuilder;
use scratcharch_ir::instruction::{CastOp as IrCastOp, GepIndex, Instruction as SairInstr};
use scratcharch_ir::r#module::{IrModule, StaticData, STATIC_DATA_BASE};
use scratcharch_ir::types::IrType;
use scratcharch_ir::value::ValueId;
use scratcharch_target::layout::layout_struct;

use crate::errors::LlvmError;
use crate::parser::*;

fn llvm_type_to_ir(ty: &LlvmType) -> Result<IrType, LlvmError> {
    match ty {
        LlvmType::I1 => Ok(IrType::I1),
        LlvmType::I8 => Ok(IrType::I8),
        LlvmType::I16 => Ok(IrType::I16),
        LlvmType::I32 => Ok(IrType::I32),
        LlvmType::I64 => Ok(IrType::I64),
        LlvmType::Ptr => Ok(IrType::Pointer),
        LlvmType::Void => Ok(IrType::Void),
        LlvmType::Array { inner, .. } => llvm_type_to_ir(inner),
        LlvmType::Struct(_) => Ok(IrType::I32),
        LlvmType::UnnamedStruct { .. } => Ok(IrType::I32),
    }
}

/// The field list of a struct type, whether named or unnamed.
fn struct_fields<'a>(
    structs: &'a HashMap<String, Vec<LlvmType>>,
    ty: &'a LlvmType,
) -> Option<&'a Vec<LlvmType>> {
    match ty {
        LlvmType::Struct(name) => structs.get(name),
        LlvmType::UnnamedStruct { fields } => Some(fields),
        _ => None,
    }
}

/// Byte size and alignment of `ty`.
///
/// Scalar sizes follow the SAIR/SA48 type system (the memory model we emit
/// into); aggregate sizes and the pointer width follow the x86-64 source
/// layout that the clang corpus was compiled for. Pointer-typed fields are
/// therefore placed at 8-byte-aligned offsets while a stored pointer value
/// occupies the SAIR pointer size; this stays self-consistent because every
/// access to such a field goes through the same layout offsets.
fn type_size_align(
    structs: &HashMap<String, Vec<LlvmType>>,
    ty: &LlvmType,
) -> Result<(u32, u32), LlvmError> {
    match ty {
        LlvmType::I1 | LlvmType::I8 => Ok((1, 1)),
        LlvmType::I16 => Ok((2, 2)),
        LlvmType::I32 => Ok((4, 4)),
        LlvmType::I64 => Ok((8, 8)),
        LlvmType::Ptr => Ok((8, 8)),
        LlvmType::Void => Err(LlvmError::Translation(
            "cannot compute layout of void type".into(),
        )),
        LlvmType::Array { inner, count } => {
            let (size, align) = type_size_align(structs, inner)?;
            Ok((size * count, align))
        }
        LlvmType::Struct(_) | LlvmType::UnnamedStruct { .. } => {
            let fields = struct_fields(structs, ty).ok_or_else(|| {
                LlvmError::Translation(format!("struct fields not collected for {:?}", ty))
            })?;
            let mut infos = Vec::with_capacity(fields.len());
            for f in fields {
                infos.push(type_size_align(structs, f)?);
            }
            let l = layout_struct(&infos);
            Ok((l.size, l.align))
        }
    }
}

/// Total bytes of an allocation of `count` objects of type `ty`.
fn alloc_byte_size(
    structs: &HashMap<String, Vec<LlvmType>>,
    ty: &LlvmType,
    count: u32,
) -> Result<u32, LlvmError> {
    let (size, _) = type_size_align(structs, ty)?;
    size.checked_mul(count).ok_or_else(|| {
        LlvmError::Translation("allocation size overflow".into())
    })
}

fn const_to_builder(builder: &mut IrBuilder, c: &LlvmConst) -> Result<ValueId, LlvmError> {
    match c.ty {
        LlvmType::I1 => Ok(builder.const_i1(c.value != 0)),
        LlvmType::I8 => Ok(builder.const_i8(c.value as u8)),
        LlvmType::I16 => Ok(builder.const_i16(c.value as u16)),
        LlvmType::I32 => Ok(builder.const_i32(c.value as u32)),
        LlvmType::I64 => Ok(builder.const_i64(c.value as u64)),
        _ => Err(LlvmError::Translation(format!(
            "cannot create constant for type {:?}", c.ty
        ))),
    }
}

/// Module symbol context threaded through instruction translation: data-global
/// addresses plus the set of function names (definitions and declarations).
/// A `@name` operand that hits a data global is that global's address; a name in
/// the function set cannot be used as a value (no function-pointer ABI); any
/// other name is genuinely undefined.
struct ModuleSyms<'a> {
    globals: &'a HashMap<String, u32>,
    functions: &'a HashSet<String>,
}

impl<'a> ModuleSyms<'a> {
    fn global_addr(&self, name: &str) -> Option<u32> {
        self.globals.get(name).copied()
    }
    fn is_function(&self, name: &str) -> bool {
        self.functions.contains(name)
    }
}

/// Resolve a data-global reference to its address as an I32 constant, or fail
/// with an explicit diagnostic (see [`ModuleSyms`] for the three outcomes).
fn global_addr_const(
    syms: &ModuleSyms<'_>,
    name: &str,
) -> Result<LlvmConst, LlvmError> {
    match syms.global_addr(name) {
        Some(addr) => Ok(LlvmConst {
            value: addr as i64,
            ty: LlvmType::I32,
        }),
        None if syms.is_function(name) => Err(LlvmError::Translation(format!(
            "cannot use function '@{}' as an SSA value: taking a function address requires a \
             function-pointer ABI (indirect calls are not supported)",
            name
        ))),
        None => Err(LlvmError::UndefinedValue(format!("@{}", name))),
    }
}

fn resolve_value_or_emit(
    builder: &mut IrBuilder,
    structs: &HashMap<String, Vec<LlvmType>>,
    syms: &ModuleSyms<'_>,
    st: &mut FnState,
    val: &LlvmValue,
) -> Result<ValueId, LlvmError> {
    match val {
        LlvmValue::Const(c) => const_to_builder(builder, c),
        LlvmValue::Local(name) => st
            .values
            .get(name)
            .copied()
            .ok_or_else(|| LlvmError::UndefinedValue(format!("%{}", name))),
        // A global referenced as an SSA value denotes the *address* of that
        // global. Addresses are 4-byte I32 on SA48, so the absolute static
        // address is emitted as a plain I32 constant — accepted as an address
        // by interpreter and VM Load/Store/GEP alike.
        LlvmValue::Global(name) => {
            let c = global_addr_const(syms, name)?;
            const_to_builder(builder, &c)
        }
        // An inline `getelementptr` constant expression (used directly as an
        // address operand) lowers to the same byte-addressed GEP as the
        // instruction form.
        LlvmValue::GepConstExpr { elem_ty, base, indices } => {
            emit_gep_addr(builder, structs, syms, st, elem_ty, base, indices)
        }
    }
}

/// Emit the address computed by a `getelementptr` over `elem_ty` with the given
/// index chain from `base`: `gep i8, <base>, <byte offset>`.
fn emit_gep_addr(
    builder: &mut IrBuilder,
    structs: &HashMap<String, Vec<LlvmType>>,
    syms: &ModuleSyms<'_>,
    st: &mut FnState,
    elem_ty: &LlvmType,
    base: &LlvmValue,
    indices: &[LlvmValue],
) -> Result<ValueId, LlvmError> {
    let base_id = resolve_value_or_emit(builder, structs, syms, st, base)?;
    let offset = emit_gep_byte_offset(builder, structs, syms, st, elem_ty, indices)?;
    Ok(builder.gep(IrType::I8, base_id, vec![GepIndex::Dynamic(offset)]))
}

/// Per-function translation state: the SSA-name → value map, the counter for
/// synthesized switch-dispatch labels, and the deferred work resolved once the
/// whole function is laid out (see [`finalize_function`]).
struct FnState {
    values: HashMap<String, ValueId>,
    switch_counter: usize,
    phi_fixups: Vec<PhiFixup>,
    /// Predecessor-label rewrites introduced by switch expansion. Keyed by
    /// `(source block label, target block label)` → the dispatch block(s) that
    /// now carry that edge. A phi in `target` whose incoming predecessor is the
    /// (expanded) `source` must list the dispatch block instead.
    phi_pred_remap: HashMap<(String, String), Vec<String>>,
}

impl FnState {
    fn new() -> Self {
        FnState {
            values: HashMap::new(),
            switch_counter: 0,
            phi_fixups: Vec::new(),
            phi_pred_remap: HashMap::new(),
        }
    }
}

/// A `phi` incoming operand defined in a block that is translated later (a loop
/// back-edge). The operand was emitted against `placeholder`; once the function
/// is complete the entry for predecessor `pred` is replaced by the real value of
/// `name`.
struct PhiFixup {
    block: String,
    pred: String,
    placeholder: ValueId,
    name: String,
}

/// Lay out the module's globals into a single static data image and build the
/// `name → absolute address` map used wherever a global is referenced.
///
/// Layout mirrors the x86-64 source layout the clang corpus is compiled for:
/// each global starts at the natural alignment of its type (the `align N`
/// attributes in the IR are never semantically observable and are ignored), and
/// each pointer leaf reserves an 8-byte stride of which only the 4-byte SAIR
/// pointer value at the element's start is meaningful. The image is seeded at
/// [`STATIC_DATA_BASE`] (below the stack floor) by each backend; addresses
/// returned here are absolute (base + offset) so instructions can refer to a
/// global with a plain `const_i32`.
///
/// Every leaf is serialized at its natural byte offset with its exact byte
/// count (`IrType::size_in_bytes`), so the produced image is byte-exact:
/// word-granular and sub-word/byte leaves (i1/i8/i16, strings) alike are read
/// exactly by both the interpreter and the VM backend (whose single-limb
/// `Load`/`Store` are width-accurate).
fn layout_globals(program: &LlvmProgram) -> Result<(HashMap<String, u32>, StaticData), LlvmError> {
    let structs = &program.struct_types;
    // Pass 1: assign regions and absolute addresses.
    let mut next = 0usize;
    let mut addr_of: HashMap<String, u32> = HashMap::new();
    for g in &program.globals {
        let (size, align) = type_size_align(structs, &g.ty)?;
        next = align_up(next, align as usize);
        let abs = STATIC_DATA_BASE as usize + next;
        let abs = u32::try_from(abs).map_err(|_| {
            LlvmError::Translation("global static data region exceeds 32-bit address space".into())
        })?;
        addr_of.insert(g.name.clone(), abs);
        next += size as usize;
    }
    // Pass 2: serialize initializers into the byte-exact image.
    let mut image = vec![0u8; next];
    for g in &program.globals {
        let abs = addr_of[&g.name];
        write_global_init(
            &mut image,
            structs,
            (abs - STATIC_DATA_BASE) as usize,
            &g.ty,
            &g.init,
            &addr_of,
        )?;
    }
    Ok((addr_of, StaticData { image }))
}

/// Round `n` up to a multiple of `align` (a power of two).
fn align_up(n: usize, align: usize) -> usize {
    if align <= 1 {
        n
    } else {
        (n + align - 1) & !(align - 1)
    }
}

/// Write a global's static initializer into `image` (which is indexed relative
/// to [`STATIC_DATA_BASE`], i.e. `base` here is the image offset of the value).
fn write_global_init(
    image: &mut [u8],
    structs: &HashMap<String, Vec<LlvmType>>,
    base: usize,
    ty: &LlvmType,
    init: &LlvmGlobalInit,
    addr_of: &HashMap<String, u32>,
) -> Result<(), LlvmError> {
    // `zeroinitializer` (and `external` globals without one) leave the already
    // zero-filled storage untouched.
    if matches!(init, LlvmGlobalInit::Zero) {
        return Ok(());
    }
    match ty {
        LlvmType::Array { inner, count } => {
            let count = *count as usize;
            match init {
                LlvmGlobalInit::Bytes(bytes) => {
                    // `c"…"` strings are exact-size `[N x i8]` arrays.
                    if bytes.len() != count {
                        return Err(LlvmError::Translation(format!(
                            "string initializer holds {} bytes for a {}-byte array",
                            bytes.len(),
                            count
                        )));
                    }
                    image[base..base + count].copy_from_slice(bytes);
                    Ok(())
                }
                LlvmGlobalInit::Array(elems) => {
                    if elems.len() != count {
                        return Err(LlvmError::Translation(format!(
                            "array initializer holds {} elements for a {}-element array",
                            elems.len(),
                            count
                        )));
                    }
                    let (stride, _) = type_size_align(structs, inner)?;
                    for (k, elem) in elems.iter().enumerate() {
                        write_global_init(
                            image,
                            structs,
                            base + k * stride as usize,
                            inner,
                            elem,
                            addr_of,
                        )?;
                    }
                    Ok(())
                }
                other => Err(LlvmError::Translation(format!(
                    "initializer {:?} does not match array type {:?}",
                    other, ty
                ))),
            }
        }
        _ => write_global_scalar(image, structs, base, ty, init, addr_of),
    }
}

/// Write a scalar (or pointer) leaf value of type `ty` at `base`.
fn write_global_scalar(
    image: &mut [u8],
    structs: &HashMap<String, Vec<LlvmType>>,
    base: usize,
    ty: &LlvmType,
    init: &LlvmGlobalInit,
    addr_of: &HashMap<String, u32>,
) -> Result<(), LlvmError> {
    match ty {
        LlvmType::Ptr => {
            let (stride, _) = type_size_align(structs, ty)?;
            // Pointer leaves reserve the x86-64 pointer stride (8) but store the
            // 4-byte SAIR address at the element's start.
            let start = base;
            let end = start + stride as usize;
            match init {
                LlvmGlobalInit::GlobalRef(target) => {
                    let addr = addr_of.get(target).copied().ok_or_else(|| {
                        LlvmError::Translation(format!(
                            "pointer global initializer references undefined global '@{}'",
                            target
                        ))
                    })?;
                    image[start..end][..4].copy_from_slice(&addr.to_le_bytes());
                    Ok(())
                }
                LlvmGlobalInit::Null => Ok(()), // already zero-filled
                other => Err(LlvmError::Translation(format!(
                    "pointer global initializer {:?} is not supported",
                    other
                ))),
            }
        }
        LlvmType::I1 | LlvmType::I8 | LlvmType::I16 | LlvmType::I32 | LlvmType::I64 => {
            let width = int_leaf_width(ty) as usize;
            let scalar = match init {
                LlvmGlobalInit::Scalar(c) => c,
                _ => {
                    return Err(LlvmError::Translation(format!(
                        "integer global initializer {:?} does not match type {:?}",
                        init, ty
                    )))
                }
            };
            let value = scalar.value as u128;
            let bytes = match width {
                1 => vec![value as u8],
                2 => vec![value as u8, (value >> 8) as u8],
                4 => (value as u32).to_le_bytes().to_vec(),
                8 => (value as u64).to_le_bytes().to_vec(),
                _ => unreachable!(),
            };
            image[base..base + width].copy_from_slice(&bytes);
            Ok(())
        }
        other => Err(LlvmError::Translation(format!(
            "global initializer for unsupported type {:?}",
            other
        ))),
    }
}

/// Bytes a single integer leaf occupies in storage (i1 is stored as one byte).
fn int_leaf_width(ty: &LlvmType) -> u32 {
    match ty {
        LlvmType::I1 | LlvmType::I8 => 1,
        LlvmType::I16 => 2,
        LlvmType::I32 => 4,
        LlvmType::I64 => 8,
        _ => 0,
    }
}

pub fn translate(program: &LlvmProgram) -> Result<IrModule, LlvmError> {
    if program.functions.is_empty() {
        return Err(LlvmError::Translation("no functions in LLVM program".into()));
    }

    let (global_addrs, static_data) = layout_globals(program)?;

    // Every function name (definition or declaration) that exists in this
    // module. A `@name` operand that hits a data global is that global's
    // address; a name in this set is a function, so it must be rejected as a
    // value (no function-pointer ABI) — never silently "undefined".
    let functions: HashSet<String> =
        program.functions.iter().map(|f| f.name.clone()).collect();
    let syms = ModuleSyms {
        globals: &global_addrs,
        functions: &functions,
    };

    let entry_name = program
        .functions
        .iter()
        .find(|f| f.name == "main" && !f.is_declaration)
        .map(|f| f.name.clone())
        .or_else(|| {
            program
                .functions
                .iter()
                .rev()
                .find(|f| !f.is_declaration)
                .map(|f| f.name.clone())
        })
        .ok_or_else(|| LlvmError::Translation("no function definitions in LLVM program".into()))?;
    let mut builder = IrBuilder::new(&entry_name);

    for func in &program.functions {
        if func.is_declaration {
            // External references (runtime intrinsics, llvm.* intrinsics) are
            // resolved at call sites by the interpreter.
            continue;
        }

        let return_ty = llvm_type_to_ir(&func.return_ty)?;
        builder.start_function(func.name.clone(), return_ty);

        let mut st = FnState::new();
        for (param_ty, param_name) in &func.params {
            let ir_ty = llvm_type_to_ir(param_ty)?;
            let id = builder.add_param(ir_ty, param_name.clone());
            st.values.insert(param_name.clone(), id);
        }

        for block in &func.blocks {
            builder.new_block(block.label.clone());
            for instr in &block.instructions {
                translate_instruction(
                    &mut builder,
                    &program.struct_types,
                    &syms,
                    &block.label,
                    &mut st,
                    instr,
                )?;
            }
        }

        // Phi operands defined later (back-edges) and phi predecessors that
        // turned into switch dispatch blocks can only be resolved once every
        // block of the function has been laid out.
        finalize_function(&mut builder, &func.name, &st)?;
    }

    let mut module = builder.finish();
    module.static_data = static_data;

    if let Err(e) = module.validate() {
        return Err(LlvmError::Translation(format!("SAIR validation failed: {}", e)));
    }

    Ok(module)
}

fn translate_instruction(
    builder: &mut IrBuilder,
    structs: &HashMap<String, Vec<LlvmType>>,
    syms: &ModuleSyms<'_>,
    block_label: &str,
    st: &mut FnState,
    instr: &LlvmInstr,
) -> Result<(), LlvmError> {
    match instr {
        LlvmInstr::BinOp { dest, op, ty, lhs, rhs } => {
            let ir_ty = llvm_type_to_ir(ty)?;
            let l = resolve_value_or_emit(builder, structs, syms, st, lhs)?;
            let r = resolve_value_or_emit(builder, structs, syms, st, rhs)?;
            let result = match op.as_str() {
                "add" => builder.add(ir_ty, l, r),
                "sub" => builder.sub(ir_ty, l, r),
                "mul" => builder.mul(ir_ty, l, r),
                // SAIR Div/Rem are unsigned floor (LLVM `udiv`/`urem`
                // semantics), so unsigned division maps one-to-one — at any
                // width, including i64.
                "udiv" => builder.div(ir_ty, l, r),
                "urem" => builder.rem(ir_ty, l, r),
                // Signed division/remainder must truncate toward zero and take
                // the dividend's sign, so they are expanded over the unsigned
                // ops from magnitudes.
                "sdiv" | "srem" => translate_signed_divrem(builder, ir_ty, op, l, r)?,
                // Bitwise and shifts map one-to-one. SAIR carries each width
                // exactly (see EXECUTION_MODEL.md §5.7): sub-32 results are
                // masked carriers, i64 is two limbs, and the poison shift
                // region is the deterministic `amount mod width`.
                "and" => builder.and(ir_ty, l, r),
                "or" => builder.or(ir_ty, l, r),
                "xor" => builder.xor(ir_ty, l, r),
                "shl" => builder.shl(ir_ty, l, r),
                "lshr" => builder.lshr(ir_ty, l, r),
                "ashr" => builder.ashr(ir_ty, l, r),
                _ => return Err(LlvmError::UnsupportedInstruction(op.clone())),
            };
            if let Some(name) = dest {
                st.values.insert(name.clone(), result);
            }
        }
        LlvmInstr::Icmp { dest, pred, ty, lhs, rhs } => {
            let ir_ty = llvm_type_to_ir(ty)?;
            let l = resolve_value_or_emit(builder, structs, syms, st, lhs)?;
            let r = resolve_value_or_emit(builder, structs, syms, st, rhs)?;
            // SAIR Lt/Gt compare the raw (unsigned) bit patterns of the
            // operands, so unsigned predicates map one-to-one. Signed
            // predicates are expanded exactly over the unsigned compare via the
            // sign-bit identity (see emit_signed_lt) — never a silent collapse
            // onto the unsigned compare.
            let result = match pred {
                IcmpPred::Eq => builder.eq(ir_ty, l, r),
                IcmpPred::Ne => {
                    let eq = builder.eq(ir_ty, l, r);
                    let zero = builder.const_i1(false);
                    builder.eq(IrType::I1, eq, zero)
                }
                IcmpPred::Ult => builder.lt(ir_ty, l, r),
                IcmpPred::Ugt => builder.gt(ir_ty, l, r),
                IcmpPred::Ule => {
                    let gt = builder.gt(ir_ty, l, r);
                    let zero = builder.const_i1(false);
                    builder.eq(IrType::I1, gt, zero)
                }
                IcmpPred::Uge => {
                    let lt = builder.lt(ir_ty, l, r);
                    let zero = builder.const_i1(false);
                    builder.eq(IrType::I1, lt, zero)
                }
                // slt(a,b) = (sign(a) != sign(b)) ? sign(a) : a <u b.
                IcmpPred::Slt => emit_signed_lt(builder, ir_ty, l, r)?,
                // sgt(a,b) = slt(b,a).
                IcmpPred::Sgt => emit_signed_lt(builder, ir_ty, r, l)?,
                // sle(a,b) = !(b <s a).
                IcmpPred::Sle => {
                    let slt = emit_signed_lt(builder, ir_ty, r, l)?;
                    let zero = builder.const_i1(false);
                    builder.eq(IrType::I1, slt, zero)
                }
                // sge(a,b) = !(a <s b).
                IcmpPred::Sge => {
                    let slt = emit_signed_lt(builder, ir_ty, l, r)?;
                    let zero = builder.const_i1(false);
                    builder.eq(IrType::I1, slt, zero)
                }
            };
            st.values.insert(dest.clone(), result);
        }
        LlvmInstr::Phi { dest, ty, incoming } => {
            let ir_ty = llvm_type_to_ir(ty)?;
            let mut ins: Vec<(ValueId, String)> = Vec::new();
            let mut deferred: Vec<(String, String)> = Vec::new();
            let mut placeholder: Option<ValueId> = None;
            for (value, pred) in incoming {
                match value {
                    LlvmValue::Const(c) => {
                        ins.push((const_to_builder(builder, c)?, pred.clone()));
                    }
                    LlvmValue::Local(name) => match st.values.get(name) {
                        Some(id) => ins.push((*id, pred.clone())),
                        None => {
                            // Back-edge operand defined later in this function.
                            // One placeholder zero const per phi is shared by
                            // every deferred predecessor and patched later.
                            let ph = match placeholder {
                                Some(p) => p,
                                None => {
                                    let z = int_const(builder, ir_ty, 0);
                                    placeholder = Some(z);
                                    z
                                }
                            };
                            ins.push((ph, pred.clone()));
                            deferred.push((name.clone(), pred.clone()));
                        }
                    },
                    LlvmValue::Global(name) => {
                        // A pointer-typed phi incoming that names a data global
                        // takes that global's address; a function name gets the
                        // function-pointer rejection; anything else is undefined.
                        let c = global_addr_const(syms, name)?;
                        ins.push((const_to_builder(builder, &c)?, pred.clone()));
                    }
                    LlvmValue::GepConstExpr { elem_ty, base, indices } => {
                        // A constant global-sub-object address used directly as a
                        // phi incoming.
                        let id = emit_gep_addr(
                            builder,
                            structs,
                            syms,
                            st,
                            elem_ty,
                            base,
                            indices,
                        )?;
                        ins.push((id, pred.clone()));
                    }
                }
            }
            let phi_id = builder.phi(ir_ty, ins);
            st.values.insert(dest.clone(), phi_id);
            if let Some(ph) = placeholder {
                for (name, pred) in deferred {
                    st.phi_fixups.push(PhiFixup {
                        block: block_label.to_string(),
                        pred,
                        placeholder: ph,
                        name,
                    });
                }
            }
        }
        LlvmInstr::Ret { value } => match value {
            Some(v) => {
                let id = resolve_value_or_emit(builder, structs, syms, st, v)?;
                builder.ret(Some(id));
            }
            None => {
                builder.ret(None);
            }
        },
        LlvmInstr::Br { target } => {
            builder.br(target.clone());
        }
        LlvmInstr::CondBr { cond, true_target, false_target } => {
            let cond_id = resolve_value_or_emit(builder, structs, syms, st, cond)?;
            builder.cond_br(cond_id, true_target.clone(), false_target.clone());
        }
        LlvmInstr::Alloca { dest, ty, count } => {
            // Every allocation is a byte array sized by the object layout.
            let bytes = alloc_byte_size(structs, ty, *count)?;
            let result = builder.alloca_array(IrType::I8, bytes);
            st.values.insert(dest.clone(), result);
        }
        LlvmInstr::Load { dest, ty, addr } => {
            let ir_ty = llvm_type_to_ir(ty)?;
            let addr_id = resolve_value_or_emit(builder, structs, syms, st, addr)?;
            let result = builder.load(ir_ty, addr_id);
            st.values.insert(dest.clone(), result);
        }
        LlvmInstr::Store { ty, value, addr } => {
            let ir_ty = llvm_type_to_ir(ty)?;
            let val_id = resolve_value_or_emit(builder, structs, syms, st, value)?;
            let addr_id = resolve_value_or_emit(builder, structs, syms, st, addr)?;
            builder.store(ir_ty, val_id, addr_id);
        }
        LlvmInstr::Call { dest, return_ty, callee, args } => {
            let ir_ret_ty = llvm_type_to_ir(return_ty)?;
            let mut arg_ids = Vec::new();
            for arg in args {
                let id = resolve_value_or_emit(builder, structs, syms, st, arg)?;
                arg_ids.push(id);
            }
            let result = builder.call(ir_ret_ty, callee.clone(), arg_ids);
            if let Some(name) = dest {
                if let Some(id) = result {
                    st.values.insert(name.clone(), id);
                }
            }
        }
        LlvmInstr::Cast { dest, op, from_ty, to_ty, value } => {
            let ir_from = llvm_type_to_ir(from_ty)?;
            let ir_to = llvm_type_to_ir(to_ty)?;
            let v = resolve_value_or_emit(builder, structs, syms, st, value)?;
            let cast_op = match op {
                CastOp::Zext => IrCastOp::Zext,
                CastOp::Sext => IrCastOp::Sext,
                CastOp::Trunc => IrCastOp::Trunc,
                CastOp::Bitcast => IrCastOp::Bitcast,
                CastOp::PtrToInt => IrCastOp::PtrToInt,
                CastOp::IntToPtr => IrCastOp::IntToPtr,
            };
            // bitcast between pointers is a no-op passthrough at the SAIR level.
            let result = builder.cast(cast_op, ir_from, ir_to, v);
            st.values.insert(dest.clone(), result);
        }
        LlvmInstr::Select { dest, ty, cond, then_value, else_value } => {
            let ir_ty = llvm_type_to_ir(ty)?;
            let cond_id = resolve_value_or_emit(builder, structs, syms, st, cond)?;
            let then_id = resolve_value_or_emit(builder, structs, syms, st, then_value)?;
            let else_id = resolve_value_or_emit(builder, structs, syms, st, else_value)?;
            let result = builder.select(ir_ty, cond_id, then_id, else_id);
            st.values.insert(dest.clone(), result);
        }
        LlvmInstr::Switch { cond_ty, cond, default_target, cases } => {
            translate_switch(
                builder,
                structs,
                syms,
                block_label,
                st,
                cond_ty,
                cond,
                default_target,
                cases,
            )?;
        }
        LlvmInstr::Unreachable => {
            builder.unreachable();
        }
        LlvmInstr::Gep { dest, elem_ty, base, indices } => {
            let base_id = resolve_value_or_emit(builder, structs, syms, st, base)?;
            let offset =
                emit_gep_byte_offset(builder, structs, syms, st, elem_ty, indices)?;
            // Address = base + byte offset over i8 elements.
            let result = builder.gep(IrType::I8, base_id, vec![GepIndex::Dynamic(offset)]);
            st.values.insert(dest.clone(), result);
        }
    }
    Ok(())
}

/// Emit the two's-complement signed less-than comparison `a <s b` at width `ty`
/// using only the unsigned SAIR ops:
///
/// ```text
/// sign(x) = (x >=u 2^(N-1))            // top bit set
/// slt(a, b) = (sign(a) != sign(b)) ? sign(a) : (a <u b)
/// ```
///
/// The `i1` flag select is exact under SAIR's `Select` semantics; no
/// approximation is involved at any supported width (i1/i8/i16/i32/i64).
fn emit_signed_lt(
    builder: &mut IrBuilder,
    ty: IrType,
    a: ValueId,
    b: ValueId,
) -> Result<ValueId, LlvmError> {
    let width = ty
        .integer_width()
        .ok_or_else(|| LlvmError::UnsupportedInstruction("signed icmp on non-integer".into()))?;
    let half = int_const(builder, ty, 1u64 << (width - 1));
    let false_c = builder.const_i1(false);
    // sign flag = !(x <u 2^(N-1)), i.e. the top bit is set.
    let a_sign = {
        let a_lt = builder.lt(ty, a, half);
        builder.eq(IrType::I1, a_lt, false_c)
    };
    let b_sign = {
        let b_lt = builder.lt(ty, b, half);
        builder.eq(IrType::I1, b_lt, false_c)
    };
    // xor on i1 == `ne`: same signs -> 0, differing signs -> 1.
    let same = builder.eq(IrType::I1, a_sign, b_sign);
    let differ = builder.eq(IrType::I1, same, false_c);
    let ult = builder.lt(ty, a, b);
    Ok(builder.select(IrType::I1, differ, a_sign, ult))
}

/// Resolve deferred `phi` work now that the whole function has been laid out:
///
/// 1. Replace each placeholder back-edge operand with the real value.
/// 2. Rewrite phi incoming predecessors that were switch-dispatch-expanded to
///    the concrete dispatch blocks that now carry the edge.
fn finalize_function(
    builder: &mut IrBuilder,
    func_name: &str,
    st: &FnState,
) -> Result<(), LlvmError> {
    // (1) Back-edge operand patch.
    for fx in &st.phi_fixups {
        let real = st
            .values
            .get(&fx.name)
            .copied()
            .ok_or_else(|| LlvmError::UndefinedValue(format!("%{}", fx.name)))?;
        let mut patched = false;
        let func = builder
            .module
            .get_function_mut(func_name)
            .expect("function missing during phi fixup");
        let block = func
            .blocks
            .iter_mut()
            .find(|b| b.label == fx.block)
            .expect("phi block missing during fixup");
        for instr in block.instructions.iter_mut() {
            if let SairInstr::Phi { incoming, .. } = instr {
                for (v, l) in incoming.iter_mut() {
                    if *l == fx.pred && *v == fx.placeholder {
                        *v = real;
                        patched = true;
                    }
                }
            }
        }
        if !patched {
            return Err(LlvmError::Translation(format!(
                "phi fixup for predecessor '%{}' of block '{}' not found",
                fx.pred, fx.block
            )));
        }
    }

    // (2) Switch predecessor rewrite.
    if st.phi_pred_remap.is_empty() {
        return Ok(());
    }
    let func = builder
        .module
        .get_function_mut(func_name)
        .expect("function missing during phi predecessor rewrite");
    for block in &mut func.blocks {
        let block_label = block.label.clone();
        for instr in block.instructions.iter_mut() {
            if let SairInstr::Phi { incoming, .. } = instr {
                let old = std::mem::take(incoming);
                let mut out: Vec<(ValueId, String)> = Vec::new();
                for (value, pred) in old {
                    match st.phi_pred_remap.get(&(pred.clone(), block_label.clone())) {
                        Some(dispatch_blocks) => {
                            for d in dispatch_blocks {
                                if !out.iter().any(|(_, l)| l == d) {
                                    out.push((value, d.clone()));
                                }
                            }
                        }
                        None => out.push((value, pred)),
                    }
                }
                *incoming = out;
            }
        }
    }
    Ok(())
}

/// Emit a zero/constant of the exact SAIR type (typed constants are required
/// for the interpreter's per-type reads).
fn int_const(builder: &mut IrBuilder, ty: IrType, val: u64) -> ValueId {
    match ty {
        IrType::I1 => builder.const_i1(val != 0),
        IrType::I8 => builder.const_i8(val as u8),
        IrType::I16 => builder.const_i16(val as u16),
        IrType::I32 => builder.const_i32(val as u32),
        _ => builder.const_i64(val),
    }
}

/// Lower LLVM `sdiv`/`srem` (signed, truncating toward zero) over unsigned SAIR
/// ops. SAIR Div/Rem are unsigned floor, so signed semantics are built from
/// magnitudes: negate operands whose sign bit is set, divide the magnitudes
/// (non-negative ⇒ floor == trunc), and re-apply the sign — `sdiv` from
/// sign-difference, `srem` from the dividend's sign. Sign bits are read with
/// the unsigned compare `x < 2^(N-1)` (top bit clear ⟺ non-negative).
///
/// `INT_MIN` and `INT_MIN / -1` are handled by wrapping arithmetic (the
/// magnitude of `INT_MIN` wraps to itself, and the quotient wraps to
/// `INT_MIN`), consistent with SAIR's no-poison policy.
fn translate_signed_divrem(
    builder: &mut IrBuilder,
    ty: IrType,
    op: &str,
    l: ValueId,
    r: ValueId,
) -> Result<ValueId, LlvmError> {
    let width = ty
        .integer_width()
        .ok_or_else(|| LlvmError::UnsupportedInstruction(op.to_string()))?;
    let zero = int_const(builder, ty, 0);
    let half = int_const(builder, ty, 1u64 << (width - 1));
    // `! (x < 2^(N-1))` — the sign bit is set.
    let false_c = builder.const_i1(false);
    let l_lt_half = builder.lt(ty, l, half);
    let r_lt_half = builder.lt(ty, r, half);
    let l_neg = builder.eq(IrType::I1, l_lt_half, false_c);
    let r_neg = builder.eq(IrType::I1, r_lt_half, false_c);
    let l_negated = builder.sub(ty, zero, l);
    let r_negated = builder.sub(ty, zero, r);
    let l_mag = builder.select(ty, l_neg, l_negated, l);
    let r_mag = builder.select(ty, r_neg, r_negated, r);
    match op {
        "sdiv" => {
            // Signs differ ⟺ exactly one operand is negative ⟹ negate.
            let differ = builder.eq(IrType::I1, l_neg, r_neg); // same signs
            let q = builder.div(ty, l_mag, r_mag);
            let q_neg = builder.sub(ty, zero, q);
            let result_neg = builder.eq(IrType::I1, differ, false_c);
            Ok(builder.select(ty, result_neg, q_neg, q))
        }
        // `srem` takes the sign of the dividend.
        _ => {
            let ur = builder.rem(ty, l_mag, r_mag);
            let ur_neg = builder.sub(ty, zero, ur);
            Ok(builder.select(ty, l_neg, ur_neg, ur))
        }
    }
}

/// Accumulates a byte offset as an i64 expression: a folded constant part plus
/// a list of dynamic (scaled index) value ids.
struct ByteOffset {
    folded: u64,
    dynamic: Vec<ValueId>,
}

impl ByteOffset {
    fn new() -> Self {
        ByteOffset { folded: 0, dynamic: Vec::new() }
    }

    fn add_const(&mut self, v: u64) {
        self.folded = self.folded.wrapping_add(v);
    }

    /// Fold `const_idx * scale` into the constant part (wrapping i64).
    fn add_const_scaled(&mut self, idx: i64, scale: u64) {
        self.folded = self
            .folded
            .wrapping_add((idx as u64).wrapping_mul(scale));
    }

    /// Emit `idx * scale` (wrapping i64) and record it as a dynamic term.
    fn add_dynamic_scaled(
        &mut self,
        builder: &mut IrBuilder,
        idx: ValueId,
        scale: u64,
    ) -> Result<(), LlvmError> {
        let term = if scale == 1 {
            idx
        } else {
            let scale_c = builder.const_i64(scale);
            builder.mul(IrType::I64, scale_c, idx)
        };
        self.dynamic.push(term);
        Ok(())
    }

    /// Materialize the accumulated offset.
    ///
    /// Addresses are 32-bit on SA48, and the interpreter's GEP wraps the index
    /// to a u32, so a fully-constant byte offset is emitted as an i32 constant:
    /// identical semantics, and single-cell so the VM backend can lower it.
    /// As soon as any index is dynamic the offset is built in i64 (clang always
    /// types array indices `i64`); that path is SAIR/interpreter-only and the
    /// single-cell VM reports an explicit diagnostic.
    fn finish(self, builder: &mut IrBuilder) -> ValueId {
        if self.dynamic.is_empty() {
            return builder.const_i32(self.folded as u32);
        }
        let mut acc = builder.const_i64(self.folded);
        for term in self.dynamic {
            acc = builder.add(IrType::I64, acc, term);
        }
        acc
    }
}

/// Fold a `getelementptr` index chain into a byte offset, following the LLVM
/// indexing rule:
///
/// * index 0 advances whole objects of `elem_ty`;
/// * array indices advance by the inner element size;
/// * struct field indices add the layout offset of the field and descend;
/// * scalar indices (pointer arithmetic) advance by the scalar size.
fn emit_gep_byte_offset(
    builder: &mut IrBuilder,
    structs: &HashMap<String, Vec<LlvmType>>,
    syms: &ModuleSyms<'_>,
    st: &mut FnState,
    elem_ty: &LlvmType,
    indices: &[LlvmValue],
) -> Result<ValueId, LlvmError> {
    let mut acc = ByteOffset::new();
    let mut cur = elem_ty.clone();

    for (k, idx) in indices.iter().enumerate() {
        if k == 0 {
            // First index scales by the whole pointed-to object.
            let (size, _) = type_size_align(structs, elem_ty)?;
            add_index_term(builder, structs, syms, st, &mut acc, idx, size)?;
            continue;
        }
        match &cur {
            LlvmType::Array { inner, .. } => {
                let (inner_size, _) = type_size_align(structs, inner)?;
                add_index_term(builder, structs, syms, st, &mut acc, idx, inner_size)?;
                cur = (**inner).clone();
            }
            LlvmType::Struct(_) | LlvmType::UnnamedStruct { .. } => {
                let fields = struct_fields(structs, &cur)
                    .cloned()
                    .ok_or_else(|| LlvmError::Translation(format!("no fields for {:?}", cur)))?;
                let field = match idx {
                    LlvmValue::Const(c) => c.value,
                    _ => {
                        return Err(LlvmError::Translation(
                            "dynamic struct field index is not supported by the flat memory model"
                                .into(),
                        ))
                    }
                };
                let field_index: usize = usize::try_from(field).map_err(|_| {
                    LlvmError::Translation("negative struct field index".into())
                })?;
                let mut infos = Vec::with_capacity(fields.len());
                for f in &fields {
                    infos.push(type_size_align(structs, f)?);
                }
                let layout = layout_struct(&infos);
                let offset = *layout.offsets.get(field_index).ok_or_else(|| {
                    LlvmError::Translation(format!(
                        "struct field index {} out of bounds for {:?}",
                        field_index, cur
                    ))
                })?;
                acc.add_const(offset as u64);
                cur = fields[field_index].clone();
            }
            _ => {
                // Pointer arithmetic over the scalar itself.
                let (size, _) = type_size_align(structs, &cur)?;
                add_index_term(builder, structs, syms, st, &mut acc, idx, size)?;
            }
        }
    }

    Ok(acc.finish(builder))
}

fn add_index_term(
    builder: &mut IrBuilder,
    structs: &HashMap<String, Vec<LlvmType>>,
    syms: &ModuleSyms<'_>,
    st: &mut FnState,
    acc: &mut ByteOffset,
    idx: &LlvmValue,
    scale: u32,
) -> Result<(), LlvmError> {
    let scale = scale as u64;
    match idx {
        LlvmValue::Const(c) => {
            acc.add_const_scaled(c.value, scale);
        }
        _ => {
            let id = resolve_value_or_emit(builder, structs, syms, st, idx)?;
            acc.add_dynamic_scaled(builder, id, scale)?;
        }
    }
    Ok(())
}

/// Lower an LLVM `switch` into a chain of `icmp eq` + conditional branches,
/// ending at the default label. The chain blocks are appended between the
/// source block and whatever follows it, so no Scratch-specific control flow
/// is involved.
///
/// Because the source block is replaced by the dispatch chain, each case and
/// default target is now reached from a specific dispatch block. Those edges
/// are recorded in `st.phi_pred_remap` so that any phi in a target whose
/// incoming predecessor was the switch block lists the dispatch block instead
/// (applied by [`finalize_function`], which runs once every block exists).
#[allow(clippy::too_many_arguments)] // matches the ISA-lowerer helper idiom in scratcharch-ir
fn translate_switch(
    builder: &mut IrBuilder,
    structs: &HashMap<String, Vec<LlvmType>>,
    syms: &ModuleSyms<'_>,
    source_block: &str,
    st: &mut FnState,
    cond_ty: &LlvmType,
    cond: &LlvmValue,
    default_target: &str,
    cases: &[(LlvmConst, String)],
) -> Result<(), LlvmError> {
    let ir_ty = llvm_type_to_ir(cond_ty)?;
    let cond_id = resolve_value_or_emit(builder, structs, syms, st, cond)?;

    if cases.is_empty() {
        builder.br(default_target.to_string());
        return Ok(());
    }

    let mut labels: Vec<String> = Vec::with_capacity(cases.len());
    for _ in cases {
        labels.push(format!("__switch.{}.{}", st.switch_counter, labels.len()));
    }
    st.switch_counter += 1;

    // Route the source block into the first dispatch block.
    builder.br(labels[0].clone());

    let record_edge = |st: &mut FnState, target: &str, dispatch: &str| {
        let entry = st
            .phi_pred_remap
            .entry((source_block.to_string(), target.to_string()))
            .or_default();
        if !entry.iter().any(|d| d == dispatch) {
            entry.push(dispatch.to_string());
        }
    };

    for (i, ((case_val, target), label)) in cases.iter().zip(labels.iter()).enumerate() {
        builder.new_block(label.clone());
        record_edge(st, target, label);
        let case_const = const_to_builder(builder, case_val)?;
        let eq = builder.eq(ir_ty, cond_id, case_const);
        let is_last = i + 1 == cases.len();
        if is_last {
            // The last dispatch's false edge is the default edge.
            record_edge(st, default_target, label);
            builder.cond_br(eq, target.clone(), default_target.to_string());
        } else {
            builder.cond_br(eq, target.clone(), labels[i + 1].clone());
        }
    }
    Ok(())
}
