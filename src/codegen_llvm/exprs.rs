use super::*;


impl LLVMCodeGen {
    /// Convert a Val to i1 (boolean) for branch conditions.
    pub(crate) fn to_i1(&mut self, val: &Val) -> String {
        if val.ty == "i1" {
            return val.name.clone();
        }
        let tmp = self.next_tmp();
        if val.ty == "ptr" {
            self.emitln(&format!("  {} = icmp ne ptr {}, null", tmp, val.name));
        } else {
            self.emitln(&format!("  {} = icmp ne {} {}, 0", tmp, val.ty, val.name));
        }
        tmp
    }

    pub(crate) fn gen_hir_expr(&mut self, expr: &crate::hir::HirExpr) -> Val {
        use crate::hir::*;
        match &expr.kind {
            HirExprKind::IntLit(v) => {
                if *v > i32::MAX as i64 || *v < i32::MIN as i64 {
                    Val::with_tid(format!("{}", v), "i64".to_string(), expr.ty)
                } else {
                    Val::with_tid(format!("{}", v), self.tl_type_str(expr.ty), expr.ty)
                }
            }

            HirExprKind::RealLit(v) => {
                // LLVM hex float format is always 64-bit (double encoding).
                // If the target type is float, fptrunc from double.
                let hex = format!("0x{:016X}", v.to_bits());
                let target_ty = self.tl_type_str(expr.ty);
                if target_ty == "float" {
                    let tmp = self.next_tmp();
                    self.emitln(&format!("  {} = fptrunc double {} to float", tmp, hex));
                    Val::with_tid(tmp, "float".to_string(), expr.ty)
                } else {
                    Val::with_tid(hex, "double".to_string(), expr.ty)
                }
            }

            HirExprKind::StringLit(ref s) => {
                // Single-char string with explicit CHAR type: produce char value
                if expr.ty == crate::types::TY_CHAR && s.len() <= 1 {
                    let ch_val = if s.is_empty() { 0u32 } else { s.as_bytes()[0] as u32 };
                    return Val::with_tid(format!("{}", ch_val), "i8".to_string(), crate::types::TY_CHAR);
                }
                let (name, _len) = self.intern_string(s);
                Val::with_tid(name, "ptr".to_string(), expr.ty)
            }

            HirExprKind::CharLit(c) => {
                Val::with_tid(format!("{}", *c as u32), "i8".to_string(), expr.ty)
            }

            HirExprKind::BoolLit(b) => {
                Val::with_tid(if *b { "1" } else { "0" }, "i32".to_string(), expr.ty)
            }

            HirExprKind::NilLit => {
                Val::with_tid("null", "ptr".to_string(), expr.ty)
            }

            HirExprKind::Place(place) => {
                // Constants without projections are unwrapped to literals.
                // Constants WITH projections (e.g., "ABCDEF"[i]) need the
                // address emitted then loaded like any other place.
                if let crate::hir::PlaceBase::Constant(_) = &place.base {
                    if place.projections.is_empty() {
                        return self.emit_place_addr(place);
                    }
                    // Fall through to normal addr+load path for indexed constants
                }

                // FuncRef used as a value (procedure variable): the address is the value
                if let crate::hir::PlaceBase::FuncRef(ref sid) = place.base {
                    return Val::with_tid(format!("@{}", sid.mangled), "ptr".to_string(), sid.ty);
                }

                // Open array params: emit_place_addr already loaded the ptr
                // from the alloca — don't load again, the ptr IS the value.
                if let crate::hir::PlaceBase::Local(ref sid) = place.base {
                    if place.projections.is_empty() && sid.is_open_array {
                        let addr = self.emit_place_addr(place);
                        return Val::new(addr.name, "ptr".to_string());
                    }
                }

                let addr = self.emit_place_addr(place);

                // Load/stay boundary: aggregates stay as addresses
                if let Some(tid) = addr.type_id {
                    if is_aggregate(&self.sema.types, tid) {
                        return Val { name: addr.name, ty: "ptr".into(), type_id: addr.type_id };
                    }
                }
                if (addr.ty.starts_with('{') || addr.ty.starts_with('['))
                    && !addr.ty.contains("float") && !addr.ty.contains("double")
                {
                    return Val { name: addr.name, ty: "ptr".into(), type_id: addr.type_id };
                }

                // Load the value
                let load_ty = if let Some(tid) = addr.type_id {
                    let s = self.tl_type_str(tid);
                    if s == "void" { addr.ty.clone() } else { s }
                } else {
                    addr.ty.clone()
                };

                let tmp = self.next_tmp();
                let is_boolean = addr.type_id == Some(crate::types::TY_BOOLEAN);
                if is_boolean && load_ty == "i32" {
                    self.emitln(&format!("  {} = load {}, ptr {}, !range !{{i32 0, i32 2}}", tmp, load_ty, addr.name));
                } else {
                    self.emitln(&format!("  {} = load {}, ptr {}", tmp, load_ty, addr.name));
                }
                Val { name: tmp, ty: load_ty, type_id: addr.type_id }
            }

            HirExprKind::TypeTransfer(ref arg) => {
                let val = self.gen_hir_expr(arg);
                let target_ty = self.tl_type_str(expr.ty);
                let mut result = self.coerce_val(&val, &target_ty);
                // Preserve the target TypeId so subsequent conversions
                // know the signedness (e.g., CARDINAL → LONGCARD uses zext)
                result.type_id = Some(expr.ty);
                return result;
            }

            HirExprKind::DirectCall { target, args } => {
                let name = &target.source_name;

                // Handle builtins that need special codegen
                if builtins::is_builtin_proc(name) {
                    match name.as_str() {
                        "ADR" => {
                            if let Some(arg) = args.first() {
                                if let HirExprKind::Place(ref place) = arg.kind {
                                    let addr = self.emit_place_addr(place);
                                    return Val::with_tid(addr.name, "ptr".to_string(), crate::types::TY_ADDRESS);
                                }
                                // Non-place arg (e.g., string literal)
                                let val = self.gen_hir_expr(arg);
                                return Val::with_tid(val.name, "ptr".to_string(), crate::types::TY_ADDRESS);
                            }
                        }
                        "HIGH" => {
                            if let Some(arg) = args.first() {
                                if let HirExprKind::Place(ref place) = arg.kind {
                                    if let crate::hir::PlaceBase::Local(ref sid) | crate::hir::PlaceBase::Global(ref sid) = place.base {
                                        let high = self.get_array_high(&sid.source_name);
                                        return Val::with_tid(high, "i32".to_string(), expr.ty);
                                    }
                                }
                            }
                            return Val::with_tid("0", "i32".to_string(), expr.ty);
                        }
                        "ABS" => {
                            if let Some(arg) = args.first() {
                                let val = self.gen_hir_expr(arg);
                                if Self::is_float_type(&val.ty) {
                                    let tmp = self.next_tmp();
                                    let intrinsic = if val.ty == "double" { "llvm.fabs.f64" } else { "llvm.fabs.f32" };
                                    self.emitln(&format!("  {} = call {} @{}({} {})", tmp, val.ty, intrinsic, val.ty, val.name));
                                    return Val::with_tid(tmp, val.ty, expr.ty);
                                } else {
                                    // Integer abs: (x ^ (x >> 31)) - (x >> 31)
                                    let shift = self.next_tmp();
                                    self.emitln(&format!("  {} = ashr {} {}, 31", shift, val.ty, val.name));
                                    let xor = self.next_tmp();
                                    self.emitln(&format!("  {} = xor {} {}, {}", xor, val.ty, val.name, shift));
                                    let tmp = self.next_tmp();
                                    self.emitln(&format!("  {} = sub {} {}, {}", tmp, val.ty, xor, shift));
                                    return Val::with_tid(tmp, val.ty, expr.ty);
                                }
                            }
                        }
                        "CAP" => {
                            if let Some(arg) = args.first() {
                                let val = self.gen_hir_expr(arg);
                                let v32 = self.coerce_val(&val, "i32");
                                let tmp = self.next_tmp();
                                self.emitln(&format!("  {} = and i32 {}, -33", tmp, v32.name)); // ~0x20
                                return Val::with_tid(tmp, "i32".to_string(), expr.ty);
                            }
                        }
                        "ODD" => {
                            if let Some(arg) = args.first() {
                                let val = self.gen_hir_expr(arg);
                                let tmp = self.next_tmp();
                                self.emitln(&format!("  {} = and {} {}, 1", tmp, val.ty, val.name));
                                return Val::with_tid(tmp, "i32".to_string(), expr.ty);
                            }
                        }
                        "SIZE" | "TSIZE" => {
                            // For now, return 4 (i32 size) as default
                            if let Some(arg) = args.first() {
                                if let HirExprKind::Place(ref place) = arg.kind {
                                    let ty_str = self.tl_type_str(place.ty);
                                    let size = self.emit_sizeof(&ty_str);
                                    let tmp = self.next_tmp();
                                    self.emitln(&format!("  {} = trunc i64 {} to i32", tmp, size));
                                    return Val::with_tid(tmp, "i32".to_string(), expr.ty);
                                }
                            }
                            return Val::with_tid("4", "i32".to_string(), expr.ty);
                        }
                        "MAX" | "MIN" => {
                            // Type queries — handled as constants during lowering ideally
                            // For now, default values
                            if name == "MAX" {
                                return Val::with_tid(format!("{}", i32::MAX), "i32".to_string(), expr.ty);
                            } else {
                                return Val::with_tid(format!("{}", i32::MIN), "i32".to_string(), expr.ty);
                            }
                        }
                        "BAND" | "BOR" | "BXOR" => {
                            if args.len() >= 2 {
                                let a = self.gen_hir_expr(&args[0]);
                                let b = self.gen_hir_expr(&args[1]);
                                let a32 = self.coerce_val(&a, "i32");
                                let b32 = self.coerce_val(&b, "i32");
                                let op = match name.as_str() {
                                    "BAND" => "and", "BOR" => "or", _ => "xor",
                                };
                                let tmp = self.next_tmp();
                                self.emitln(&format!("  {} = {} i32 {}, {}", tmp, op, a32.name, b32.name));
                                return Val::with_tid(tmp, "i32".to_string(), expr.ty);
                            }
                        }
                        "BNOT" => {
                            if let Some(arg) = args.first() {
                                let val = self.gen_hir_expr(arg);
                                let v32 = self.coerce_val(&val, "i32");
                                let tmp = self.next_tmp();
                                self.emitln(&format!("  {} = xor i32 {}, -1", tmp, v32.name));
                                return Val::with_tid(tmp, "i32".to_string(), expr.ty);
                            }
                        }
                        "SHL" | "SHIFT" => {
                            if args.len() >= 2 {
                                let val = self.gen_hir_expr(&args[0]);
                                let shift = self.gen_hir_expr(&args[1]);
                                let v32 = self.coerce_val(&val, "i32");
                                let s32 = self.coerce_val(&shift, "i32");
                                let tmp = self.next_tmp();
                                self.emitln(&format!("  {} = shl i32 {}, {}", tmp, v32.name, s32.name));
                                return Val::with_tid(tmp, "i32".to_string(), expr.ty);
                            }
                        }
                        "SHR" => {
                            if args.len() >= 2 {
                                let val = self.gen_hir_expr(&args[0]);
                                let shift = self.gen_hir_expr(&args[1]);
                                let v32 = self.coerce_val(&val, "i32");
                                let s32 = self.coerce_val(&shift, "i32");
                                let tmp = self.next_tmp();
                                self.emitln(&format!("  {} = lshr i32 {}, {}", tmp, v32.name, s32.name));
                                return Val::with_tid(tmp, "i32".to_string(), expr.ty);
                            }
                        }
                        "ROTATE" => {
                            if args.len() >= 2 {
                                let val = self.gen_hir_expr(&args[0]);
                                let shift = self.gen_hir_expr(&args[1]);
                                let v32 = self.coerce_val(&val, "i32");
                                let s32 = self.coerce_val(&shift, "i32");
                                // left rotate: (x << n) | (x >> (32 - n))
                                let shl = self.next_tmp();
                                self.emitln(&format!("  {} = shl i32 {}, {}", shl, v32.name, s32.name));
                                let sub = self.next_tmp();
                                self.emitln(&format!("  {} = sub i32 32, {}", sub, s32.name));
                                let shr = self.next_tmp();
                                self.emitln(&format!("  {} = lshr i32 {}, {}", shr, v32.name, sub));
                                let tmp = self.next_tmp();
                                self.emitln(&format!("  {} = or i32 {}, {}", tmp, shl, shr));
                                return Val::with_tid(tmp, "i32".to_string(), expr.ty);
                            }
                        }
                        "CMPLX" => {
                            // Construct { float, float } struct inline
                            if args.len() >= 2 {
                                let re = self.gen_hir_expr(&args[0]);
                                let im = self.gen_hir_expr(&args[1]);
                                let tmp1 = self.next_tmp();
                                self.emitln(&format!("  {} = insertvalue {{ float, float }} undef, float {}, 0", tmp1, re.name));
                                let tmp2 = self.next_tmp();
                                self.emitln(&format!("  {} = insertvalue {{ float, float }} {}, float {}, 1", tmp2, tmp1, im.name));
                                return Val::with_tid(tmp2, "{ float, float }".to_string(), expr.ty);
                            }
                        }
                        "RE" => {
                            if let Some(arg) = args.first() {
                                let val = self.gen_hir_expr(arg);
                                let tmp = self.next_tmp();
                                self.emitln(&format!("  {} = extractvalue {{ float, float }} {}, 0", tmp, val.name));
                                return Val::with_tid(tmp, "float".to_string(), expr.ty);
                            }
                        }
                        "IM" => {
                            if let Some(arg) = args.first() {
                                let val = self.gen_hir_expr(arg);
                                let tmp = self.next_tmp();
                                self.emitln(&format!("  {} = extractvalue {{ float, float }} {}, 1", tmp, val.name));
                                return Val::with_tid(tmp, "float".to_string(), expr.ty);
                            }
                        }
                        _ => {
                            // Other builtins: evaluate args and emit as regular call
                        }
                    }
                }

                let ret_ty = self.tl_type_str(expr.ty);
                let mut arg_str = self.expand_hir_call_args(args);
                let call_name = self.fn_name_map.get(&target.mangled)
                    .cloned().unwrap_or_else(|| target.mangled.clone());
                // Coerce arg types to match declared param types
                if let Some(params) = self.proc_params.get(&call_name).or_else(|| self.proc_params.get(&target.source_name)).cloned() {
                    for (i, param) in params.iter().enumerate() {
                        if i < arg_str.len() {
                            // Check for double→float coercion needed
                            if arg_str[i].starts_with("double ") && param.llvm_type == "float" {
                                let val_name = arg_str[i].strip_prefix("double ").unwrap();
                                let tmp = self.next_tmp();
                                self.emitln(&format!("  {} = fptrunc double {} to float", tmp, val_name));
                                arg_str[i] = format!("float {}", tmp);
                            }
                        }
                    }
                }
                let tmp = self.next_tmp();
                if ret_ty == "void" {
                    self.emitln(&format!("  call void @{}({})",
                        call_name, arg_str.join(", ")));
                    Val::with_tid("void", "void".to_string(), expr.ty)
                } else {
                    self.emitln(&format!("  {} = call {} @{}({})",
                        tmp, ret_ty, call_name, arg_str.join(", ")));
                    Val::with_tid(tmp, ret_ty, expr.ty)
                }
            }

            HirExprKind::IndirectCall { callee, args } => {
                let fn_ptr = self.gen_hir_expr(callee);
                let ret_ty = self.tl_type_str(expr.ty);
                let arg_vals: Vec<Val> = args.iter()
                    .map(|a| self.gen_hir_expr(a))
                    .collect();
                let arg_str: Vec<String> = arg_vals.iter()
                    .map(|v| format!("{} {}", v.ty, v.name))
                    .collect();
                let call_target = if fn_ptr.ty != "ptr" {
                    let tmp = self.next_tmp();
                    self.emitln(&format!("  {} = inttoptr {} {} to ptr", tmp, fn_ptr.ty, fn_ptr.name));
                    tmp
                } else {
                    fn_ptr.name
                };
                let tmp = self.next_tmp();
                if ret_ty == "void" {
                    self.emitln(&format!("  call void {}({})",
                        call_target, arg_str.join(", ")));
                    Val::with_tid("void", "void".to_string(), expr.ty)
                } else {
                    self.emitln(&format!("  {} = call {} {}({})",
                        tmp, ret_ty, call_target, arg_str.join(", ")));
                    Val::with_tid(tmp, ret_ty, expr.ty)
                }
            }

            HirExprKind::UnaryOp { op, operand } => {
                let val = self.gen_hir_expr(operand);
                match op {
                    UnaryOp::Neg => {
                        let tmp = self.next_tmp();
                        // COMPLEX negation
                        let operand_resolved = self.resolve_alias_id(operand.ty);
                        if matches!(self.sema.types.get(operand_resolved),
                            crate::types::Type::Complex | crate::types::Type::LongComplex)
                        {
                            let func = "m2_complex_neg";
                            if !self.declared_fns.contains(func) {
                                self.emit_preambleln(&format!("declare {} @{}({} %a)", val.ty, func, val.ty));
                                self.declared_fns.insert(func.to_string());
                            }
                            self.emitln(&format!("  {} = call {} @{}({} {})",
                                tmp, val.ty, func, val.ty, val.name));
                            Val::with_tid(tmp, val.ty, expr.ty)
                        } else if Self::is_float_type(&val.ty) {
                            self.emitln(&format!("  {} = fneg {} {}", tmp, val.ty, val.name));
                            Val::with_tid(tmp, val.ty, expr.ty)
                        } else {
                            let neg_ty = if val.ty == "ptr" { "i64" } else { &val.ty };
                            let coerced = self.coerce_val(&val, neg_ty);
                            self.emitln(&format!("  {} = sub {} 0, {}", tmp, neg_ty, coerced.name));
                            Val::with_tid(tmp, neg_ty.to_string(), expr.ty)
                        }
                    }
                    UnaryOp::Pos => val,
                }
            }

            HirExprKind::BinaryOp { op, left, right } => {
                // Short-circuit AND/OR: don't evaluate RHS if LHS determines result
                if *op == BinaryOp::And || *op == BinaryOp::Or {
                    let lhs = self.gen_hir_expr(left);
                    let lhs_bool = self.to_i1(&lhs);
                    let eval_rhs_label = self.next_label("sc.rhs");
                    let merge_label = self.next_label("sc.merge");
                    let current_block = self.current_block.clone();
                    if *op == BinaryOp::And {
                        // AND: if lhs is false, skip rhs (result = false)
                        self.emitln(&format!("  br i1 {}, label %{}, label %{}",
                            lhs_bool, eval_rhs_label, merge_label));
                    } else {
                        // OR: if lhs is true, skip rhs (result = true)
                        self.emitln(&format!("  br i1 {}, label %{}, label %{}",
                            lhs_bool, merge_label, eval_rhs_label));
                    }
                    self.emitln(&format!("{}:", eval_rhs_label));
                    self.current_block = eval_rhs_label.clone();
                    let rhs = self.gen_hir_expr(right);
                    let rhs_bool = self.to_i1(&rhs);
                    let rhs_block = self.current_block.clone();
                    self.emitln(&format!("  br label %{}", merge_label));
                    self.emitln(&format!("{}:", merge_label));
                    self.current_block = merge_label.clone();
                    let result = self.next_tmp();
                    if *op == BinaryOp::And {
                        // AND: false from lhs-block, rhs_bool from rhs-block
                        self.emitln(&format!("  {} = phi i1 [ false, %{} ], [ {}, %{} ]",
                            result, current_block, rhs_bool, rhs_block));
                    } else {
                        // OR: true from lhs-block, rhs_bool from rhs-block
                        self.emitln(&format!("  {} = phi i1 [ true, %{} ], [ {}, %{} ]",
                            result, current_block, rhs_bool, rhs_block));
                    }
                    let ext = self.next_tmp();
                    self.emitln(&format!("  {} = zext i1 {} to i32", ext, result));
                    Val::with_tid(ext, "i32".to_string(), expr.ty)
                } else {
                    let lhs = self.gen_hir_expr(left);
                    let rhs = self.gen_hir_expr(right);
                    // COMPLEX: delegate to runtime C helpers
                    let lhs_resolved = self.resolve_alias_id(left.ty);
                    if matches!(self.sema.types.get(lhs_resolved),
                        crate::types::Type::Complex | crate::types::Type::LongComplex)
                    {
                        self.gen_complex_binary_op(*op, &lhs, &rhs, expr.ty)
                    } else {
                        self.gen_hir_binary_op(*op, &lhs, &rhs, left.ty, right.ty, expr.ty)
                    }
                }
            }

            HirExprKind::Not(operand) => {
                let val = self.gen_hir_expr(operand);
                let tmp = self.next_tmp();
                self.emitln(&format!("  {} = xor {} {}, 1", tmp, val.ty, val.name));
                Val::with_tid(tmp, val.ty, expr.ty)
            }

            HirExprKind::SetConstructor { elements } => {
                let mut result = Val::with_tid("0", "i32".to_string(), expr.ty);
                for elem in elements {
                    match elem {
                        HirSetElement::Single(e) => {
                            let v = self.gen_hir_expr(e);
                            let bit = self.next_tmp();
                            self.emitln(&format!("  {} = shl i32 1, {}", bit, v.name));
                            let new_result = self.next_tmp();
                            self.emitln(&format!("  {} = or i32 {}, {}", new_result, result.name, bit));
                            result = Val::with_tid(new_result, "i32".to_string(), expr.ty);
                        }
                        HirSetElement::Range(lo, hi) => {
                            let lo_v = self.gen_hir_expr(lo);
                            let hi_v = self.gen_hir_expr(hi);
                            let diff = self.next_tmp();
                            self.emitln(&format!("  {} = sub i32 {}, {}", diff, hi_v.name, lo_v.name));
                            let diff1 = self.next_tmp();
                            self.emitln(&format!("  {} = add i32 {}, 1", diff1, diff));
                            let shifted = self.next_tmp();
                            self.emitln(&format!("  {} = shl i32 1, {}", shifted, diff1));
                            let mask_raw = self.next_tmp();
                            self.emitln(&format!("  {} = sub i32 {}, 1", mask_raw, shifted));
                            let mask = self.next_tmp();
                            self.emitln(&format!("  {} = shl i32 {}, {}", mask, mask_raw, lo_v.name));
                            let new_result = self.next_tmp();
                            self.emitln(&format!("  {} = or i32 {}, {}", new_result, result.name, mask));
                            result = Val::with_tid(new_result, "i32".to_string(), expr.ty);
                        }
                    }
                }
                result
            }

            HirExprKind::Deref(inner) => {
                let val = self.gen_hir_expr(inner);
                let deref_ty = self.tl_type_str(expr.ty);
                let tmp = self.next_tmp();
                self.emitln(&format!("  {} = load {}, ptr {}", tmp, deref_ty, val.name));
                Val::with_tid(tmp, deref_ty, expr.ty)
            }

            HirExprKind::AddrOf(ref place) => {
                // Emit the address of the place, don't load the value.
                // Used for VAR parameter passing.
                let addr = self.emit_place_addr(place);
                Val::with_tid(addr.name, "ptr".to_string(), expr.ty)
            }
        }
    }

    /// Generate a binary operation from pre-evaluated HIR operands.
    /// `lhs_ty` and `rhs_ty` are the semantic TypeIds of the operands,
    /// used to select signed vs unsigned operations.
    fn gen_hir_binary_op(&mut self, op: BinaryOp, lhs: &Val, rhs: &Val,
                         lhs_ty: TypeId, rhs_ty: TypeId, result_ty: TypeId) -> Val {
        // Determine if either operand is unsigned (CARDINAL, LONGCARD, etc.)
        let is_unsigned = crate::types::is_unsigned_type(&self.sema.types, lhs_ty)
            || crate::types::is_unsigned_type(&self.sema.types, rhs_ty);

        // Handle pointer arithmetic: ptr ± int → getelementptr
        if (op == BinaryOp::Add || op == BinaryOp::Sub)
            && (lhs.ty == "ptr" || rhs.ty == "ptr")
            && (lhs.ty != rhs.ty)
        {
            let (ptr_val, int_val, is_sub) = if lhs.ty == "ptr" {
                (lhs, rhs, op == BinaryOp::Sub)
            } else {
                (rhs, lhs, false)
            };
            let idx = if is_sub {
                let neg = self.next_tmp();
                self.emitln(&format!("  {} = sub i32 0, {}", neg, int_val.name));
                Val::new(neg, "i32".to_string())
            } else {
                int_val.clone()
            };
            let idx64 = self.coerce_val(&idx, "i64");
            let tmp = self.next_tmp();
            self.emitln(&format!("  {} = getelementptr inbounds i8, ptr {}, i64 {}", tmp, ptr_val.name, idx64.name));
            return Val::with_tid(tmp, "ptr".to_string(), result_ty);
        }

        // Pointer-to-pointer arithmetic needs integer conversion
        let raw_common = self.common_type(&lhs.ty, &rhs.ty);
        let common = if raw_common == "ptr" { "i64".to_string() } else { raw_common };
        let l = self.coerce_val(lhs, &common);
        let r = self.coerce_val(rhs, &common);
        let tmp = self.next_tmp();

        let result_llvm_ty = self.tl_type_str(result_ty);

        match op {
            // Arithmetic
            BinaryOp::Add => {
                if Self::is_float_type(&common) {
                    self.emitln(&format!("  {} = fadd {} {}, {}", tmp, common, l.name, r.name));
                } else if self.is_set_tid(Some(lhs_ty)) || self.is_set_tid(Some(rhs_ty)) {
                    self.emitln(&format!("  {} = or {} {}, {}", tmp, common, l.name, r.name));
                } else {
                    self.emitln(&format!("  {} = add {} {}, {}", tmp, common, l.name, r.name));
                }
                Val::with_tid(tmp, common, result_ty)
            }
            BinaryOp::Sub => {
                if Self::is_float_type(&common) {
                    self.emitln(&format!("  {} = fsub {} {}, {}", tmp, common, l.name, r.name));
                } else if self.is_set_tid(Some(lhs_ty)) || self.is_set_tid(Some(rhs_ty)) {
                    // Set difference: a - b = a & ~b
                    let not_tmp = self.next_tmp();
                    self.emitln(&format!("  {} = xor {} {}, -1", not_tmp, common, r.name));
                    self.emitln(&format!("  {} = and {} {}, {}", tmp, common, l.name, not_tmp));
                } else {
                    self.emitln(&format!("  {} = sub {} {}, {}", tmp, common, l.name, r.name));
                }
                Val::with_tid(tmp, common, result_ty)
            }
            BinaryOp::Mul => {
                if Self::is_float_type(&common) {
                    self.emitln(&format!("  {} = fmul {} {}, {}", tmp, common, l.name, r.name));
                } else if self.is_set_tid(Some(lhs_ty)) || self.is_set_tid(Some(rhs_ty)) {
                    self.emitln(&format!("  {} = and {} {}, {}", tmp, common, l.name, r.name));
                } else {
                    self.emitln(&format!("  {} = mul {} {}, {}", tmp, common, l.name, r.name));
                }
                Val::with_tid(tmp, common, result_ty)
            }
            BinaryOp::RealDiv if self.is_set_tid(Some(lhs_ty)) || self.is_set_tid(Some(rhs_ty)) => {
                self.emitln(&format!("  {} = xor {} {}, {}", tmp, common, l.name, r.name));
                Val::with_tid(tmp, common, result_ty)
            }
            BinaryOp::RealDiv => {
                let float_ty = if common == "double" { "double" } else { "float" };
                let fl = self.coerce_val(&l, float_ty);
                let fr = self.coerce_val(&r, float_ty);
                self.emitln(&format!("  {} = fdiv {} {}, {}", tmp, float_ty, fl.name, fr.name));
                Val::with_tid(tmp, float_ty.to_string(), result_ty)
            }
            BinaryOp::IntDiv => {
                if is_unsigned {
                    self.emitln(&format!("  {} = udiv {} {}, {}", tmp, common, l.name, r.name));
                } else if common == "i64" {
                    // PIM4 floored division for LONGINT
                    if !self.declared_fns.contains("m2_div64") {
                        self.emit_preambleln("declare i64 @m2_div64(i64, i64)");
                        self.declared_fns.insert("m2_div64".to_string());
                    }
                    self.emitln(&format!("  {} = call i64 @m2_div64({} {}, {} {})",
                        tmp, common, l.name, common, r.name));
                } else {
                    // PIM4 floored division
                    if !self.declared_fns.contains("m2_div") {
                        self.emit_preambleln("declare i32 @m2_div(i32, i32)");
                        self.declared_fns.insert("m2_div".to_string());
                    }
                    self.emitln(&format!("  {} = call i32 @m2_div({} {}, {} {})",
                        tmp, common, l.name, common, r.name));
                }
                Val::with_tid(tmp, common, result_ty)
            }
            BinaryOp::Mod => {
                if is_unsigned {
                    self.emitln(&format!("  {} = urem {} {}, {}", tmp, common, l.name, r.name));
                } else if common == "i64" {
                    if !self.declared_fns.contains("m2_mod64") {
                        self.emit_preambleln("declare i64 @m2_mod64(i64, i64)");
                        self.declared_fns.insert("m2_mod64".to_string());
                    }
                    self.emitln(&format!("  {} = call i64 @m2_mod64({} {}, {} {})",
                        tmp, common, l.name, common, r.name));
                } else {
                    if !self.declared_fns.contains("m2_mod") {
                        self.emit_preambleln("declare i32 @m2_mod(i32, i32)");
                        self.declared_fns.insert("m2_mod".to_string());
                    }
                    self.emitln(&format!("  {} = call i32 @m2_mod({} {}, {} {})",
                        tmp, common, l.name, common, r.name));
                }
                Val::with_tid(tmp, common, result_ty)
            }
            // Logical
            BinaryOp::And => {
                self.emitln(&format!("  {} = and {} {}, {}", tmp, common, l.name, r.name));
                Val::with_tid(tmp, result_llvm_ty, result_ty)
            }
            BinaryOp::Or => {
                self.emitln(&format!("  {} = or {} {}, {}", tmp, common, l.name, r.name));
                Val::with_tid(tmp, result_llvm_ty, result_ty)
            }
            // Comparison
            BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Le |
            BinaryOp::Gt | BinaryOp::Ge => {
                let cmp_op = if Self::is_float_type(&common) {
                    match op {
                        BinaryOp::Eq => "fcmp oeq",
                        BinaryOp::Ne => "fcmp une",
                        BinaryOp::Lt => "fcmp olt",
                        BinaryOp::Le => "fcmp ole",
                        BinaryOp::Gt => "fcmp ogt",
                        BinaryOp::Ge => "fcmp oge",
                        _ => unreachable!(),
                    }
                } else if is_unsigned {
                    match op {
                        BinaryOp::Eq => "icmp eq",
                        BinaryOp::Ne => "icmp ne",
                        BinaryOp::Lt => "icmp ult",
                        BinaryOp::Le => "icmp ule",
                        BinaryOp::Gt => "icmp ugt",
                        BinaryOp::Ge => "icmp uge",
                        _ => unreachable!(),
                    }
                } else {
                    match op {
                        BinaryOp::Eq => "icmp eq",
                        BinaryOp::Ne => "icmp ne",
                        BinaryOp::Lt => "icmp slt",
                        BinaryOp::Le => "icmp sle",
                        BinaryOp::Gt => "icmp sgt",
                        BinaryOp::Ge => "icmp sge",
                        _ => unreachable!(),
                    }
                };
                self.emitln(&format!("  {} = {} {} {}, {}", tmp, cmp_op, common, l.name, r.name));
                // Zero-extend i1 to i32 for Modula-2 BOOLEAN
                let ext = self.next_tmp();
                self.emitln(&format!("  {} = zext i1 {} to i32", ext, tmp));
                Val::with_tid(ext, "i32".to_string(), result_ty)
            }
            BinaryOp::In => {
                // IN: test bit in set — (set >> val) & 1
                let shifted = self.next_tmp();
                self.emitln(&format!("  {} = lshr i32 {}, {}", shifted, r.name, l.name));
                self.emitln(&format!("  {} = and i32 {}, 1", tmp, shifted));
                Val::with_tid(tmp, "i32".to_string(), result_ty)
            }
        }
    }

    /// Generate LLVM call args from HIR expressions.
    /// The HIR has already:
    /// - expanded open array params to (value, high) pairs
    /// - wrapped VAR params as AddrOf(Place) expressions
    /// This function just evaluates each arg — gen_hir_expr handles all cases.
    /// Generate LLVM call args from HIR expressions.
    /// The HIR has already expanded open arrays to (value, high) pairs
    /// and wrapped VAR params as AddrOf. No backend state needed.
    pub(crate) fn expand_hir_call_args(
        &mut self,
        args: &[crate::hir::HirExpr],
    ) -> Vec<String> {
        args.iter().map(|a| {
            // VAR parameters: always pass as ptr, never load the struct.
            // AddrOf wraps explicit VAR args; Place with is_var_param covers
            // passing a VAR param through to another function.
            let is_var_param = matches!(a.kind, crate::hir::HirExprKind::AddrOf(_))
                || matches!(&a.kind, crate::hir::HirExprKind::Place(p)
                    if matches!(&p.base, crate::hir::PlaceBase::Local(sid) if sid.is_var_param));
            let val = self.gen_hir_expr(a);
            // By-value struct args: gen_hir_expr returns "ptr" for aggregates,
            // but the callee expects the struct value. Load it.
            // Skip VAR params — those must stay as pointers.
            if val.ty == "ptr" && !is_var_param {
                // Check if the underlying type is a record/aggregate that needs
                // loading for by-value passing. Try type_id first, then fall back
                // to checking the LLVM type from type_lowering.
                let record_ty = val.type_id.and_then(|tid| {
                    let resolved = self.tl_resolve(tid);
                    if crate::codegen_llvm::is_aggregate(&self.sema.types, resolved) {
                        let s = self.tl_type_str(resolved);
                        if s.starts_with('{') { return Some(s); }
                    }
                    // type_id may be wrong (e.g., TY_ADDRESS for a record var).
                    // Try looking up the variable name in the alloca to get the actual type.
                    None
                }).or_else(|| {
                    // Check if the alloca/global type is a struct
                    let name = val.name.trim_start_matches('@').trim_start_matches('%');
                    if let Some((_, ty)) = self.globals.get(name) {
                        if ty.starts_with('{') { return Some(ty.clone()); }
                    }
                    if let Some((_, ty)) = self.lookup_local(name) {
                        if ty.starts_with('{') { return Some(ty.clone()); }
                    }
                    None
                });
                if let Some(actual_ty) = record_ty {
                    let tmp = self.next_tmp();
                    self.emitln(&format!("  {} = load {}, ptr {}", tmp, actual_ty, val.name));
                    return format!("{} {}", actual_ty, tmp);
                }
            }
            format!("{} {}", val.ty, val.name)
        }).collect()
    }

    /// Emit a COMPLEX binary operation via runtime C helper.
    fn gen_complex_binary_op(&mut self, op: BinaryOp, lhs: &Val, rhs: &Val, result_ty: crate::types::TypeId) -> Val {
        let func = match op {
            BinaryOp::Add => "m2_complex_add",
            BinaryOp::Sub => "m2_complex_sub",
            BinaryOp::Mul => "m2_complex_mul",
            BinaryOp::RealDiv => "m2_complex_div",
            BinaryOp::Eq => "m2_complex_eq",
            BinaryOp::Ne => "m2_complex_eq", // negate after
            _ => {
                // Fallback for unsupported ops
                let tmp = self.next_tmp();
                self.emitln(&format!("  {} = alloca {}", tmp, lhs.ty));
                return Val::with_tid(tmp, lhs.ty.clone(), result_ty);
            }
        };
        let ty = &lhs.ty; // e.g., "{ float, float }"
        if !self.declared_fns.contains(func) {
            if func == "m2_complex_eq" {
                self.emit_preambleln(&format!("declare i32 @{}({} %a, {} %b)", func, ty, ty));
            } else {
                self.emit_preambleln(&format!("declare {} @{}({} %a, {} %b)", ty, func, ty, ty));
            }
            self.declared_fns.insert(func.to_string());
        }
        let tmp = self.next_tmp();
        if op == BinaryOp::Eq {
            self.emitln(&format!("  {} = call i32 @{}({} {}, {} {})",
                tmp, func, ty, lhs.name, ty, rhs.name));
            Val::with_tid(tmp, "i32".to_string(), result_ty)
        } else if op == BinaryOp::Ne {
            let eq_tmp = self.next_tmp();
            self.emitln(&format!("  {} = call i32 @{}({} {}, {} {})",
                eq_tmp, func, ty, lhs.name, ty, rhs.name));
            self.emitln(&format!("  {} = xor i32 {}, 1", tmp, eq_tmp));
            Val::with_tid(tmp, "i32".to_string(), result_ty)
        } else {
            self.emitln(&format!("  {} = call {} @{}({} {}, {} {})",
                tmp, ty, func, ty, lhs.name, ty, rhs.name));
            Val::with_tid(tmp, ty.clone(), result_ty)
        }
    }

    /// Convert an HIR expression to i1 for use in branch conditions.
    pub(crate) fn gen_hir_expr_as_i1(&mut self, expr: &crate::hir::HirExpr) -> String {
        let val = self.gen_hir_expr(expr);
        self.to_i1(&val)
    }
}
