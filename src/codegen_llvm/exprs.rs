use super::*;

impl LLVMCodeGen {
    // ── Expression generation ───────────────────────────────────────

    pub(crate) fn gen_expr(&mut self, expr: &Expr) -> Val {
        match &expr.kind {
            ExprKind::IntLit(v) => {
                // Use i64 for values that don't fit in i32
                if *v > i32::MAX as i64 || *v < i32::MIN as i64 {
                    Val::new(format!("{}", v), "i64".to_string())
                } else {
                    Val::new(format!("{}", v), "i32".to_string())
                }
            }

            ExprKind::RealLit(v) => {
                // LLVM requires hex float format for exact representation
                let bits = (*v).to_bits();
                Val::new(format!("0x{:016X}", bits), "double".to_string())
            }

            ExprKind::StringLit(s) => {
                let (name, _len) = self.intern_string(s);
                Val::new(name, "ptr".to_string())
            }

            ExprKind::CharLit(c) => Val::new(format!("{}", *c as u32), "i8".to_string()),

            ExprKind::BoolLit(b) => Val::new(if *b { "1" } else { "0" }, "i32".to_string()),

            ExprKind::NilLit => Val::new("null", "ptr".to_string()),

            ExprKind::Designator(d) => self.gen_designator_load(d),

            ExprKind::FuncCall { desig, args } => {
                self.gen_func_call_expr(desig, args)
            }

            ExprKind::UnaryOp { op, operand } => {
                let val = self.gen_expr(operand);
                match op {
                    UnaryOp::Neg => {
                        if val.ty.contains("float, float") || val.ty.contains("double, double") {
                            // Complex negation: negate both parts
                            let ft = if val.ty.contains("double") { "double" } else { "float" };
                            let ct = val.ty.clone();
                            let re = self.next_tmp();
                            self.emitln(&format!("  {} = extractvalue {} {}, 0", re, ct, val.name));
                            let im = self.next_tmp();
                            self.emitln(&format!("  {} = extractvalue {} {}, 1", im, ct, val.name));
                            let nre = self.next_tmp();
                            self.emitln(&format!("  {} = fneg {} {}", nre, ft, re));
                            let nim = self.next_tmp();
                            self.emitln(&format!("  {} = fneg {} {}", nim, ft, im));
                            let t1 = self.next_tmp();
                            self.emitln(&format!("  {} = insertvalue {} undef, {} {}, 0", t1, ct, ft, nre));
                            let t2 = self.next_tmp();
                            self.emitln(&format!("  {} = insertvalue {} {}, {} {}, 1", t2, ct, t1, ft, nim));
                            Val::new(t2, ct)
                        } else {
                            let tmp = self.next_tmp();
                            if Self::is_float_type(&val.ty) {
                                self.emitln(&format!("  {} = fneg {} {}", tmp, val.ty, val.name));
                                Val::new(tmp, val.ty)
                            } else {
                                // Pointer arithmetic needs integer conversion
                                let neg_ty = if val.ty == "ptr" { "i64" } else { &val.ty };
                                let coerced = self.coerce_val(&val, neg_ty);
                                self.emitln(&format!("  {} = sub {} 0, {}", tmp, neg_ty, coerced.name));
                                Val::new(tmp, neg_ty.to_string())
                            }
                        }
                    }
                    UnaryOp::Pos => val, // no-op
                }
            }

            ExprKind::BinaryOp { op, left, right } => {
                self.gen_binary_op(*op, left, right)
            }

            ExprKind::Not(operand) => {
                let val = self.gen_expr(operand);
                let tmp = self.next_tmp();
                // NOT on boolean (i32): result = val XOR 1
                self.emitln(&format!("  {} = xor {} {}, 1", tmp, val.ty, val.name));
                Val::new(tmp, val.ty)
            }

            ExprKind::SetConstructor { elements, .. } => {
                // Build set value by OR-ing bits
                let mut result = Val::new("0", "i32".to_string());
                for elem in elements {
                    match elem {
                        SetElement::Single(e) => {
                            let v = self.gen_expr(e);
                            let bit = self.next_tmp();
                            self.emitln(&format!("  {} = shl i32 1, {}", bit, v.name));
                            let new_result = self.next_tmp();
                            self.emitln(&format!("  {} = or i32 {}, {}", new_result, result.name, bit));
                            result = Val::new(new_result, "i32".to_string());
                        }
                        SetElement::Range(lo, hi) => {
                            let lo_v = self.gen_expr(lo);
                            let hi_v = self.gen_expr(hi);
                            // Build mask from lo..hi
                            // mask = ((1 << (hi - lo + 1)) - 1) << lo
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
                            result = Val::new(new_result, "i32".to_string());
                        }
                    }
                }
                result
            }

            ExprKind::Deref(inner) => {
                let val = self.gen_expr(inner);
                // Dereference: load through pointer
                let deref_ty = if val.ty == "ptr" {
                    // Try TypeId-based resolution first, then legacy
                    let tid_result: Option<String> = (|| {
                        let name = match &inner.kind {
                            ExprKind::FuncCall { desig, .. } => &desig.ident.name,
                            ExprKind::Designator(d) => &d.ident.name,
                            _ => return None,
                        };
                        // Try var_types first, then symtab (for functions)
                        let tid = self.var_types.get(name.as_str()).copied()
                            .or_else(|| self.sema.symtab.lookup_any(name).and_then(|sym| {
                                // For procedures, get the return type
                                if let crate::symtab::SymbolKind::Procedure { return_type, .. } = &sym.kind {
                                    *return_type
                                } else {
                                    Some(sym.typ)
                                }
                            }))?;
                        let resolved = self.tl_resolve(tid);
                        let target = self.tl_pointer_target(resolved)?;
                        Some(self.tl_type_str(self.tl_resolve(target)))
                    })();
                    tid_result.unwrap_or_else(|| "i32".to_string())
                } else { val.ty.clone() };
                let tmp = self.next_tmp();
                self.emitln(&format!("  {} = load {}, ptr {}", tmp, deref_ty, val.name));
                Val::new(tmp, deref_ty)
            }
        }
    }

    pub(crate) fn gen_func_call_expr(&mut self, desig: &Designator, args: &[Expr]) -> Val {
        // Indirect call through pointer dereference or field access on non-module
        // e.g. result := cp^.predFn(args)
        let has_deref_or_field = !desig.selectors.is_empty()
            && !self.imported_modules.contains(&desig.ident.name)
            && desig.selectors.iter().any(|s| matches!(s,
                Selector::Deref(_) | Selector::Field(_, _)));
        if has_deref_or_field {
            let fn_ptr = self.gen_designator_load(desig);
            let call_target = if fn_ptr.ty != "ptr" {
                let tmp = self.next_tmp();
                self.emitln(&format!("  {} = inttoptr {} {} to ptr", tmp, fn_ptr.ty, fn_ptr.name));
                tmp
            } else {
                fn_ptr.name.clone()
            };
            let mut arg_strs = Vec::new();
            for arg in args {
                if let ExprKind::Designator(d) = &arg.kind {
                    let addr = self.gen_designator_addr(d);
                    if addr.ty.starts_with('{') || addr.ty.starts_with('%') {
                        arg_strs.push(format!("ptr {}", addr.name));
                        continue;
                    }
                    if addr.ty.starts_with('[') {
                        let high = self.get_array_high(&d.ident.name);
                        arg_strs.push(format!("ptr {}", addr.name));
                        arg_strs.push(format!("i32 {}", high));
                        continue;
                    }
                }
                let val = self.gen_expr(arg);
                arg_strs.push(format!("{} {}", val.ty, val.name));
            }
            let args_str = arg_strs.join(", ");
            let ret_ty = "i32"; // default for indirect function calls
            let tmp = self.next_tmp();
            self.emitln(&format!("  {} = call {} {}({})", tmp, ret_ty, call_target, args_str));
            return Val::new(tmp, ret_ty.to_string());
        }

        let name = self.resolve_proc_name(desig);
        let actual_name = &desig.ident.name;

        // Check if bare name is a proc variable (not a function definition)
        if desig.selectors.is_empty() && desig.ident.module.is_none() {
            let mangled = self.mangle(actual_name);
            let is_local_ptr = self.lookup_local(actual_name).map(|(_, ty)| ty == "ptr").unwrap_or(false);
            let is_global_ptr = self.globals.get(actual_name).map(|(_, ty)| ty == "ptr").unwrap_or(false)
                || self.globals.get(&mangled).map(|(_, ty)| ty == "ptr").unwrap_or(false);
            let is_known_fn = self.declared_fns.contains(&mangled)
                || self.declared_fns.contains(actual_name);
            let is_proc_var = (is_local_ptr || is_global_ptr) && !is_known_fn;
            if is_proc_var {
                let fn_ptr = self.gen_designator_load(desig);
                let mut arg_strs = Vec::new();
                for arg in args {
                    let val = self.gen_expr(arg);
                    arg_strs.push(format!("{} {}", val.ty, val.name));
                }
                let args_str = arg_strs.join(", ");
                let (ret_ty, _ret_tid) = self.infer_call_return_type(&name, actual_name);
                if ret_ty == "void" {
                    self.emitln(&format!("  call void {}({})", fn_ptr.name, args_str));
                    return Val::new("", "void".to_string());
                }
                let tmp = self.next_tmp();
                self.emitln(&format!("  {} = call {} {}({})", tmp, ret_ty, fn_ptr.name, args_str));
                return Val::new(tmp, ret_ty);
            }
        }

        // Indirect call through function pointer (designator with selectors like ops[i](10))
        // But NOT for Module.Proc patterns (already resolved by resolve_proc_name)
        let is_mod_qual = !desig.selectors.is_empty()
            && self.imported_modules.contains(&desig.ident.name);
        if !desig.selectors.is_empty() && !is_mod_qual {
            // The designator resolves to a function pointer — load it and call indirectly
            let fn_ptr = self.gen_designator_load(desig);
            if fn_ptr.ty == "ptr" {
                // Build argument list
                let mut arg_strs = Vec::new();
                for arg in args {
                    let val = self.gen_expr(arg);
                    arg_strs.push(format!("{} {}", val.ty, val.name));
                }
                let args_str = arg_strs.join(", ");
                // Default return type for indirect calls
                let ret_ty = "i32".to_string();
                let tmp = self.next_tmp();
                self.emitln(&format!("  {} = call {} {}({})", tmp, ret_ty, fn_ptr.name, args_str));
                return Val::new(tmp, ret_ty);
            }
        }

        // Type transfer functions: INTEGER(x), CARDINAL(x), etc.
        if args.len() == 1 && desig.selectors.is_empty() && desig.ident.module.is_none() {
            let target_ty: Option<String> = match actual_name.as_str() {
                "INTEGER" => Some("i32".into()),
                "CARDINAL" => Some("i32".into()),
                "LONGINT" => Some("i64".into()),
                "LONGCARD" => Some("i64".into()),
                "REAL" => Some("float".into()),
                "LONGREAL" => Some("double".into()),
                "BOOLEAN" => Some("i32".into()),
                "CHAR" => Some("i8".into()),
                "ADDRESS" => Some("ptr".into()),
                "WORD" => Some("i32".into()),
                "BYTE" => Some("i8".into()),
                "BITSET" => Some("i32".into()),
                _ => {
                    // Try symtab → TypeLowering first
                    self.sema.symtab.lookup_any(actual_name)
                        .filter(|sym| matches!(sym.kind, crate::symtab::SymbolKind::Type))
                        .map(|sym| self.tl_type_str(sym.typ))
                        .or_else(|| self.type_map.get(actual_name).cloned())
                },
            };
            if let Some(ref target) = target_ty {
                let val = self.gen_expr(&args[0]);
                return self.coerce_val(&val, target);
            }
        }

        // Built-in function calls
        if builtins::is_builtin_proc(actual_name) {
            return self.gen_builtin_func_call(actual_name, args);
        }

        // Determine return type from sema
        let (ret_ty, ret_tid) = self.infer_call_return_type(&name, actual_name);
        let mut val = self.gen_call(&name, args, &ret_ty);
        val.type_id = ret_tid;
        val
    }

    pub(crate) fn infer_call_return_type(&self, full_name: &str, base_name: &str) -> (String, Option<crate::types::TypeId>) {
        // Check fn_return_types first (canonical source from gen_proc_decl and declare_stdlib_function)
        if let Some(ret_ty) = self.fn_return_types.get(full_name) {
            return (ret_ty.clone(), None);
        }
        // Try symbol table with base name
        if let Some(sym) = self.sema.symtab.lookup_any(base_name) {
            if let crate::symtab::SymbolKind::Procedure { return_type, .. } = &sym.kind {
                if let Some(ret_id) = return_type {
                    return (self.llvm_type_for_type_id(*ret_id), Some(*ret_id));
                }
                return ("void".to_string(), None);
            }
        }
        // Default for unknown functions
        ("i32".to_string(), None)
    }

    pub(crate) fn gen_builtin_func_call(&mut self, name: &str, args: &[Expr]) -> Val {
        match name {
            "ABS" => {
                if let Some(arg) = args.first() {
                    let val = self.gen_expr(arg);
                    if Self::is_float_type(&val.ty) {
                        let tmp = self.next_tmp();
                        let intrinsic = if val.ty == "float" { "llvm.fabs.f32" } else { "llvm.fabs.f64" };
                        if !self.declared_fns.contains(intrinsic) {
                            self.emit_preambleln(&format!("declare {} @{}({})", val.ty, intrinsic, val.ty));
                            self.declared_fns.insert(intrinsic.to_string());
                        }
                        self.emitln(&format!("  {} = call {} @{}({} {})", tmp, val.ty, intrinsic, val.ty, val.name));
                        Val::new(tmp, val.ty)
                    } else {
                        // Integer ABS: (x ^ (x >> 31)) - (x >> 31)
                        let shift = self.next_tmp();
                        self.emitln(&format!("  {} = ashr {} {}, 31", shift, val.ty, val.name));
                        let xor = self.next_tmp();
                        self.emitln(&format!("  {} = xor {} {}, {}", xor, val.ty, val.name, shift));
                        let result = self.next_tmp();
                        self.emitln(&format!("  {} = sub {} {}, {}", result, val.ty, xor, shift));
                        Val::new(result, val.ty)
                    }
                } else {
                    Val::new("0", "i32".to_string())
                }
            }
            "ODD" => {
                if let Some(arg) = args.first() {
                    let val = self.gen_expr(arg);
                    let tmp = self.next_tmp();
                    self.emitln(&format!("  {} = and {} {}, 1", tmp, val.ty, val.name));
                    Val::new(tmp, "i32".to_string())
                } else {
                    Val::new("0", "i32".to_string())
                }
            }
            "ORD" => {
                if let Some(arg) = args.first() {
                    // Handle single-char string literal directly
                    if let ExprKind::StringLit(s) = &arg.kind {
                        if s.len() == 1 {
                            return Val::new(format!("{}", s.as_bytes()[0] as i32), "i32".to_string());
                        }
                    }
                    let val = self.gen_expr(arg);
                    self.coerce_val(&val, "i32")
                } else {
                    Val::new("0", "i32".to_string())
                }
            }
            "CHR" => {
                if let Some(arg) = args.first() {
                    if let ExprKind::StringLit(s) = &arg.kind {
                        if s.len() == 1 {
                            return Val::new(format!("{}", s.as_bytes()[0]), "i8".to_string());
                        }
                    }
                    let val = self.gen_expr(arg);
                    self.coerce_val(&val, "i8")
                } else {
                    Val::new("0", "i8".to_string())
                }
            }
            "CAP" => {
                // Convert lowercase to uppercase
                if let Some(arg) = args.first() {
                    // Handle single-char string literal
                    if let ExprKind::StringLit(s) = &arg.kind {
                        if s.len() == 1 {
                            let ch = s.as_bytes()[0];
                            let result = if ch >= b'a' && ch <= b'z' { ch - 32 } else { ch };
                            return Val::new(format!("{}", result), "i8".to_string());
                        }
                    }
                    let val = self.gen_expr(arg);
                    let v32 = self.coerce_val(&val, "i32");
                    // if c >= 'a' && c <= 'z' then c - 32 else c
                    let ge_a = self.next_tmp();
                    self.emitln(&format!("  {} = icmp sge i32 {}, 97", ge_a, v32.name));
                    let le_z = self.next_tmp();
                    self.emitln(&format!("  {} = icmp sle i32 {}, 122", le_z, v32.name));
                    let is_lower = self.next_tmp();
                    self.emitln(&format!("  {} = and i1 {}, {}", is_lower, ge_a, le_z));
                    let upper = self.next_tmp();
                    self.emitln(&format!("  {} = sub i32 {}, 32", upper, v32.name));
                    let result = self.next_tmp();
                    self.emitln(&format!("  {} = select i1 {}, i32 {}, i32 {}", result, is_lower, upper, v32.name));
                    let r8 = self.coerce_val(&Val::new(result, "i32".to_string()), "i8");
                    r8
                } else {
                    Val::new("0", "i8".to_string())
                }
            }
            "HIGH" => {
                if let Some(arg) = args.first() {
                    if let ExprKind::Designator(d) = &arg.kind {
                        // For simple names, use get_array_high
                        // For field selectors (rec.field), resolve through the designator
                        if d.selectors.is_empty() {
                            let high = self.get_array_high(&d.ident.name);
                            return Val::new(high, "i32".to_string());
                        } else {
                            // Get the address and extract HIGH from the LLVM type
                            let addr = self.gen_designator_addr(d);
                            if addr.ty.starts_with('[') {
                                if let Some(n_str) = addr.ty.strip_prefix('[').and_then(|s| s.split(' ').next()) {
                                    if let Ok(n) = n_str.parse::<usize>() {
                                        return Val::new(format!("{}", n - 1), "i32".to_string());
                                    }
                                }
                            }
                            let high = self.get_array_high(&d.ident.name);
                            return Val::new(high, "i32".to_string());
                        }
                    }
                }
                Val::new("0", "i32".to_string())
            }
            "SIZE" | "TSIZE" => {
                if let Some(arg) = args.first() {
                    if let ExprKind::Designator(d) = &arg.kind {
                        let size = match d.ident.name.as_str() {
                            "INTEGER" | "CARDINAL" | "BITSET" | "WORD" | "REAL" => "4",
                            "LONGINT" | "LONGCARD" | "LONGREAL" | "ADDRESS" => "8",
                            "CHAR" | "BYTE" => "1",
                            "BOOLEAN" => "4",
                            _ => "4",
                        };
                        return Val::new(size, "i32".to_string());
                    }
                }
                Val::new("4", "i32".to_string())
            }
            "FLOAT" => {
                if let Some(arg) = args.first() {
                    let val = self.gen_expr(arg);
                    self.coerce_val(&val, "float")
                } else {
                    Val::new("0.0", "float".to_string())
                }
            }
            "LFLOAT" | "LONG" => {
                if let Some(arg) = args.first() {
                    let val = self.gen_expr(arg);
                    self.coerce_val(&val, "double")
                } else {
                    Val::new("0.0", "double".to_string())
                }
            }
            "SHORT" => {
                if let Some(arg) = args.first() {
                    let val = self.gen_expr(arg);
                    if val.ty == "double" {
                        self.coerce_val(&val, "float")
                    } else if val.ty == "i64" {
                        self.coerce_val(&val, "i32")
                    } else {
                        val
                    }
                } else {
                    Val::new("0", "i32".to_string())
                }
            }
            "TRUNC" => {
                if let Some(arg) = args.first() {
                    let val = self.gen_expr(arg);
                    self.coerce_val(&val, "i32")
                } else {
                    Val::new("0", "i32".to_string())
                }
            }
            "ADR" => {
                if let Some(arg) = args.first() {
                    if let ExprKind::Designator(d) = &arg.kind {
                        let addr = self.gen_designator_addr(d);
                        return Val::new(addr.name, "ptr".to_string());
                    }
                    if let ExprKind::StringLit(s) = &arg.kind {
                        let (str_name, _) = self.intern_string(s);
                        return Val::new(str_name, "ptr".to_string());
                    }
                    // For other expressions, evaluate and return as ptr
                    let val = self.gen_expr(arg);
                    return Val::new(val.name, "ptr".to_string());
                }
                Val::new("null", "ptr".to_string())
            }
            "VAL" => {
                // VAL(Type, expr) — type transfer
                if args.len() >= 2 {
                    let val = self.gen_expr(&args[1]);
                    // First arg is a type name — resolve it
                    if let ExprKind::Designator(d) = &args[0].kind {
                        let ty = self.llvm_type_for_name(&d.ident.name);
                        return self.coerce_val(&val, &ty);
                    }
                    return val;
                }
                Val::new("0", "i32".to_string())
            }
            "MAX" => {
                // MAX(Type) — return max value for type
                if let Some(arg) = args.first() {
                    if let ExprKind::Designator(d) = &arg.kind {
                        match d.ident.name.as_str() {
                            "INTEGER" | "LONGINT" => return Val::new("2147483647", "i32".to_string()),
                            "CARDINAL" => return Val::new("4294967295", "i64".to_string()),
                            "LONGCARD" => return Val::new("4294967295", "i64".to_string()),
                            "CHAR" => return Val::new("255", "i8".to_string()),
                            "BOOLEAN" => return Val::new("1", "i32".to_string()),
                            _ => {}
                        }
                    }
                }
                Val::new("2147483647", "i32".to_string())
            }
            "MIN" => {
                if let Some(arg) = args.first() {
                    if let ExprKind::Designator(d) = &arg.kind {
                        match d.ident.name.as_str() {
                            "INTEGER" | "LONGINT" => return Val::new("-2147483648", "i32".to_string()),
                            "CARDINAL" | "LONGCARD" => return Val::new("0", "i32".to_string()),
                            "CHAR" => return Val::new("0", "i8".to_string()),
                            "BOOLEAN" => return Val::new("0", "i32".to_string()),
                            _ => {}
                        }
                    }
                }
                Val::new("-2147483648", "i32".to_string())
            }
            "SHL" | "SHIFT" => {
                if args.len() >= 2 {
                    let val = self.gen_expr(&args[0]);
                    let amount = self.gen_expr(&args[1]);
                    let tmp = self.next_tmp();
                    self.emitln(&format!("  {} = shl {} {}, {}", tmp, val.ty, val.name, amount.name));
                    Val::new(tmp, val.ty)
                } else {
                    Val::new("0", "i32".to_string())
                }
            }
            "SHR" => {
                if args.len() >= 2 {
                    let val = self.gen_expr(&args[0]);
                    let amount = self.gen_expr(&args[1]);
                    let tmp = self.next_tmp();
                    self.emitln(&format!("  {} = lshr {} {}, {}", tmp, val.ty, val.name, amount.name));
                    Val::new(tmp, val.ty)
                } else {
                    Val::new("0", "i32".to_string())
                }
            }
            "BAND" => {
                if args.len() >= 2 {
                    let a = self.gen_expr(&args[0]);
                    let b = self.gen_expr(&args[1]);
                    let tmp = self.next_tmp();
                    self.emitln(&format!("  {} = and {} {}, {}", tmp, a.ty, a.name, b.name));
                    Val::new(tmp, a.ty)
                } else {
                    Val::new("0", "i32".to_string())
                }
            }
            "BOR" => {
                if args.len() >= 2 {
                    let a = self.gen_expr(&args[0]);
                    let b = self.gen_expr(&args[1]);
                    let tmp = self.next_tmp();
                    self.emitln(&format!("  {} = or {} {}, {}", tmp, a.ty, a.name, b.name));
                    Val::new(tmp, a.ty)
                } else {
                    Val::new("0", "i32".to_string())
                }
            }
            "BXOR" => {
                if args.len() >= 2 {
                    let a = self.gen_expr(&args[0]);
                    let b = self.gen_expr(&args[1]);
                    let tmp = self.next_tmp();
                    self.emitln(&format!("  {} = xor {} {}, {}", tmp, a.ty, a.name, b.name));
                    Val::new(tmp, a.ty)
                } else {
                    Val::new("0", "i32".to_string())
                }
            }
            "BNOT" => {
                if let Some(arg) = args.first() {
                    let val = self.gen_expr(arg);
                    let tmp = self.next_tmp();
                    self.emitln(&format!("  {} = xor {} {}, -1", tmp, val.ty, val.name));
                    Val::new(tmp, val.ty)
                } else {
                    Val::new("0", "i32".to_string())
                }
            }
            "RE" => {
                // RE(z) — extract real part of COMPLEX
                if let Some(arg) = args.first() {
                    let val = self.gen_expr(arg);
                    let tmp = self.next_tmp();
                    self.emitln(&format!("  {} = extractvalue {{ float, float }} {}, 0", tmp, val.name));
                    Val::new(tmp, "float".to_string())
                } else { Val::new("0.0", "float".to_string()) }
            }
            "IM" => {
                // IM(z) — extract imaginary part of COMPLEX
                if let Some(arg) = args.first() {
                    let val = self.gen_expr(arg);
                    let tmp = self.next_tmp();
                    self.emitln(&format!("  {} = extractvalue {{ float, float }} {}, 1", tmp, val.name));
                    Val::new(tmp, "float".to_string())
                } else { Val::new("0.0", "float".to_string()) }
            }
            "CMPLX" => {
                // CMPLX(re, im) — construct COMPLEX value
                if args.len() >= 2 {
                    let re = self.gen_expr(&args[0]);
                    let im = self.gen_expr(&args[1]);
                    let re_f = self.coerce_val(&re, "float");
                    let im_f = self.coerce_val(&im, "float");
                    let tmp1 = self.next_tmp();
                    self.emitln(&format!("  {} = insertvalue {{ float, float }} undef, float {}, 0", tmp1, re_f.name));
                    let tmp2 = self.next_tmp();
                    self.emitln(&format!("  {} = insertvalue {{ float, float }} {}, float {}, 1", tmp2, tmp1, im_f.name));
                    Val::new(tmp2, "{ float, float }".to_string())
                } else { Val::new("undef", "{ float, float }".to_string()) }
            }
            _ => {
                // Unknown builtin — try calling it
                let full_name = name.to_string();
                self.gen_call(&full_name, args, "i32")
            }
        }
    }

    pub(crate) fn gen_builtin_proc_call(&mut self, name: &str, args: &[Expr]) {
        match name {
            "INC" => {
                if let Some(arg) = args.first() {
                    if let ExprKind::Designator(d) = &arg.kind {
                        let addr = self.gen_designator_addr(d);
                        let cur = self.next_tmp();
                        self.emitln(&format!("  {} = load {}, ptr {}", cur, addr.ty, addr.name));
                        let step = if args.len() > 1 {
                            self.gen_expr(&args[1])
                        } else {
                            Val::new("1", addr.ty.clone())
                        };
                        let result = self.next_tmp();
                        self.emitln(&format!("  {} = add {} {}, {}", result, addr.ty, cur, step.name));
                        self.emitln(&format!("  store {} {}, ptr {}", addr.ty, result, addr.name));
                    }
                }
            }
            "DEC" => {
                if let Some(arg) = args.first() {
                    if let ExprKind::Designator(d) = &arg.kind {
                        let addr = self.gen_designator_addr(d);
                        let cur = self.next_tmp();
                        self.emitln(&format!("  {} = load {}, ptr {}", cur, addr.ty, addr.name));
                        let step = if args.len() > 1 {
                            self.gen_expr(&args[1])
                        } else {
                            Val::new("1", addr.ty.clone())
                        };
                        let result = self.next_tmp();
                        self.emitln(&format!("  {} = sub {} {}, {}", result, addr.ty, cur, step.name));
                        self.emitln(&format!("  store {} {}, ptr {}", addr.ty, result, addr.name));
                    }
                }
            }
            "HALT" => {
                self.emitln("  call void @exit(i32 0)");
                self.emitln("  unreachable");
            }
            "NEW" => {
                if let Some(arg) = args.first() {
                    if let ExprKind::Designator(d) = &arg.kind {
                        let addr = self.gen_designator_addr(d);
                        // Check if the variable has a RTTI type descriptor
                        let var_name = &d.ident.name;
                        let var_type = self.var_type_names.get(var_name).cloned()
                            .unwrap_or_else(|| self.mangle(var_name));
                        let td_sym = self.ref_type_descs.get(&var_type).cloned()
                            .or_else(|| self.ref_type_descs.get(&self.mangle(&var_type)).cloned());

                        if let Some(td) = td_sym {
                            // Typed allocation with RTTI header
                            self.declare_rtti_runtime();
                            // Compute payload size from the pointed-to type
                            let elem_size = self.emit_sizeof(&addr.ty);
                            let ptr = self.next_tmp();
                            self.emitln(&format!("  {} = call ptr @M2_ref_alloc(i64 {}, ptr {})",
                                ptr, elem_size, td));
                            self.emitln(&format!("  store ptr {}, ptr {}", ptr, addr.name));
                        } else {
                            // Plain allocation (no RTTI)
                            let ptr = self.next_tmp();
                            self.emitln(&format!("  {} = call ptr @malloc(i64 256)", ptr));
                            self.emitln(&format!("  store ptr {}, ptr {}", ptr, addr.name));
                        }
                    }
                }
            }
            "DISPOSE" => {
                if let Some(arg) = args.first() {
                    let val = self.gen_expr(arg);
                    // Check if typed ref — use M2_ref_free
                    if self.m2plus {
                        self.declare_rtti_runtime();
                        self.emitln(&format!("  call void @M2_ref_free(ptr {})", val.name));
                    } else {
                        self.emitln(&format!("  call void @free(ptr {})", val.name));
                    }
                }
            }
            "INCL" => {
                // INCL(set, element) — set bit
                if args.len() >= 2 {
                    if let ExprKind::Designator(d) = &args[0].kind {
                        let addr = self.gen_designator_addr(d);
                        let cur = self.next_tmp();
                        self.emitln(&format!("  {} = load i32, ptr {}", cur, addr.name));
                        let elem = self.gen_expr(&args[1]);
                        let bit = self.next_tmp();
                        self.emitln(&format!("  {} = shl i32 1, {}", bit, elem.name));
                        let result = self.next_tmp();
                        self.emitln(&format!("  {} = or i32 {}, {}", result, cur, bit));
                        self.emitln(&format!("  store i32 {}, ptr {}", result, addr.name));
                    }
                }
            }
            "EXCL" => {
                // EXCL(set, element) — clear bit
                if args.len() >= 2 {
                    if let ExprKind::Designator(d) = &args[0].kind {
                        let addr = self.gen_designator_addr(d);
                        let cur = self.next_tmp();
                        self.emitln(&format!("  {} = load i32, ptr {}", cur, addr.name));
                        let elem = self.gen_expr(&args[1]);
                        let bit = self.next_tmp();
                        self.emitln(&format!("  {} = shl i32 1, {}", bit, elem.name));
                        let not_bit = self.next_tmp();
                        self.emitln(&format!("  {} = xor i32 {}, -1", not_bit, bit));
                        let result = self.next_tmp();
                        self.emitln(&format!("  {} = and i32 {}, {}", result, cur, not_bit));
                        self.emitln(&format!("  store i32 {}, ptr {}", result, addr.name));
                    }
                }
            }
            _ => {
                // Unknown builtin — emit as a regular call
                let full_name = self.mangle(name);
                self.gen_call(&full_name, args, "void");
            }
        }
    }

    pub(crate) fn gen_binary_op(&mut self, op: BinaryOp, left: &Expr, right: &Expr) -> Val {
        // Short-circuit evaluation for AND / OR
        if op == BinaryOp::And {
            return self.gen_short_circuit_and(left, right);
        }
        if op == BinaryOp::Or {
            return self.gen_short_circuit_or(left, right);
        }

        let lval = self.gen_expr(left);
        let rval = self.gen_expr(right);

        // Coerce single-char string constants to char values for comparison
        let lval = self.coerce_string_to_char(&lval).unwrap_or(lval);
        let rval = self.coerce_string_to_char(&rval).unwrap_or(rval);

        // Handle COMPLEX arithmetic
        let is_complex = lval.ty.contains("float, float") || rval.ty.contains("float, float")
            || lval.ty.contains("double, double") || rval.ty.contains("double, double");
        if is_complex {
            return self.gen_complex_binop(op, &lval, &rval);
        }

        // Handle pointer arithmetic: ptr + int or int + ptr → getelementptr
        if (op == BinaryOp::Add || op == BinaryOp::Sub)
            && (lval.ty == "ptr" || rval.ty == "ptr")
            && (lval.ty != rval.ty)
        {
            let (ptr_val, int_val, is_sub) = if lval.ty == "ptr" {
                (&lval, &rval, op == BinaryOp::Sub)
            } else {
                (&rval, &lval, false)
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
            return Val::new(tmp, "ptr".to_string());
        }

        // Determine common type. Pointer arithmetic needs integer conversion.
        let raw_common = self.common_type(&lval.ty, &rval.ty);
        let common_ty = if raw_common == "ptr" { "i64".to_string() } else { raw_common };
        let l = self.coerce_val(&lval, &common_ty);
        let r = self.coerce_val(&rval, &common_ty);

        let is_float = Self::is_float_type(&common_ty);
        let tmp = self.next_tmp();

        // Set operations (i32 bitwise)
        if op == BinaryOp::In {
            // x IN s → ((s >> x) & 1) != 0
            let shift = self.next_tmp();
            self.emitln(&format!("  {} = lshr i32 {}, {}", shift, r.name, l.name));
            let bit = self.next_tmp();
            self.emitln(&format!("  {} = and i32 {}, 1", bit, shift));
            return Val::new(bit, "i32".to_string());
        }

        match op {
            BinaryOp::Add => {
                if is_float {
                    self.emitln(&format!("  {} = fadd {} {}, {}", tmp, common_ty, l.name, r.name));
                } else {
                    self.emitln(&format!("  {} = add {} {}, {}", tmp, common_ty, l.name, r.name));
                }
                Val::new(tmp, common_ty)
            }
            BinaryOp::Sub => {
                if is_float {
                    self.emitln(&format!("  {} = fsub {} {}, {}", tmp, common_ty, l.name, r.name));
                } else {
                    self.emitln(&format!("  {} = sub {} {}, {}", tmp, common_ty, l.name, r.name));
                }
                Val::new(tmp, common_ty)
            }
            BinaryOp::Mul => {
                if is_float {
                    self.emitln(&format!("  {} = fmul {} {}, {}", tmp, common_ty, l.name, r.name));
                } else {
                    self.emitln(&format!("  {} = mul {} {}, {}", tmp, common_ty, l.name, r.name));
                }
                Val::new(tmp, common_ty)
            }
            BinaryOp::RealDiv => {
                if is_float {
                    self.emitln(&format!("  {} = fdiv {} {}, {}", tmp, common_ty, l.name, r.name));
                } else {
                    // Integer to float division
                    let lf = self.coerce_val(&l, "double");
                    let rf = self.coerce_val(&r, "double");
                    self.emitln(&format!("  {} = fdiv double {}, {}", tmp, lf.name, rf.name));
                    return Val::new(tmp, "double".to_string());
                }
                Val::new(tmp, common_ty)
            }
            BinaryOp::IntDiv => {
                // PIM4 floored division via runtime helper
                if !self.declared_fns.contains("m2_div") {
                    self.emit_preambleln("declare i32 @m2_div(i32, i32)");
                    self.emit_preambleln("declare i64 @m2_div64(i64, i64)");
                    self.declared_fns.insert("m2_div".to_string());
                    self.declared_fns.insert("m2_div64".to_string());
                }
                if common_ty == "i64" {
                    self.emitln(&format!("  {} = call i64 @m2_div64(i64 {}, i64 {})", tmp, l.name, r.name));
                } else {
                    let l32 = self.coerce_val(&l, "i32");
                    let r32 = self.coerce_val(&r, "i32");
                    self.emitln(&format!("  {} = call i32 @m2_div(i32 {}, i32 {})", tmp, l32.name, r32.name));
                }
                Val::new(tmp, common_ty)
            }
            BinaryOp::Mod => {
                // PIM4 floored MOD via runtime helper
                if !self.declared_fns.contains("m2_mod") {
                    self.emit_preambleln("declare i32 @m2_mod(i32, i32)");
                    self.emit_preambleln("declare i64 @m2_mod64(i64, i64)");
                    self.declared_fns.insert("m2_mod".to_string());
                    self.declared_fns.insert("m2_mod64".to_string());
                }
                if common_ty == "i64" {
                    self.emitln(&format!("  {} = call i64 @m2_mod64(i64 {}, i64 {})", tmp, l.name, r.name));
                } else {
                    let l32 = self.coerce_val(&l, "i32");
                    let r32 = self.coerce_val(&r, "i32");
                    self.emitln(&format!("  {} = call i32 @m2_mod(i32 {}, i32 {})", tmp, l32.name, r32.name));
                }
                Val::new(tmp, common_ty)
            }
            BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Le |
            BinaryOp::Gt | BinaryOp::Ge => {
                let cmp_result = self.gen_comparison(op, &l, &r, &common_ty);
                // Extend i1 to i32 for M2 boolean
                let ext = self.next_tmp();
                self.emitln(&format!("  {} = zext i1 {} to i32", ext, cmp_result));
                Val::new(ext, "i32".to_string())
            }
            BinaryOp::And | BinaryOp::Or => {
                unreachable!("handled above with short-circuit")
            }
            BinaryOp::In => {
                unreachable!("handled above")
            }
        }
    }

    pub(crate) fn gen_complex_binop(&mut self, op: BinaryOp, l: &Val, r: &Val) -> Val {
        let ft = if l.ty.contains("double") || r.ty.contains("double") { "double" } else { "float" };
        let ct = format!("{{ {}, {} }}", ft, ft);

        // Extract real and imaginary parts
        let lr = self.next_tmp();
        self.emitln(&format!("  {} = extractvalue {} {}, 0", lr, ct, l.name));
        let li = self.next_tmp();
        self.emitln(&format!("  {} = extractvalue {} {}, 1", li, ct, l.name));
        let rr = self.next_tmp();
        self.emitln(&format!("  {} = extractvalue {} {}, 0", rr, ct, r.name));
        let ri = self.next_tmp();
        self.emitln(&format!("  {} = extractvalue {} {}, 1", ri, ct, r.name));

        match op {
            BinaryOp::Add => {
                let re = self.next_tmp();
                self.emitln(&format!("  {} = fadd {} {}, {}", re, ft, lr, rr));
                let im = self.next_tmp();
                self.emitln(&format!("  {} = fadd {} {}, {}", im, ft, li, ri));
                let t1 = self.next_tmp();
                self.emitln(&format!("  {} = insertvalue {} undef, {} {}, 0", t1, ct, ft, re));
                let t2 = self.next_tmp();
                self.emitln(&format!("  {} = insertvalue {} {}, {} {}, 1", t2, ct, t1, ft, im));
                Val::new(t2, ct)
            }
            BinaryOp::Sub => {
                let re = self.next_tmp();
                self.emitln(&format!("  {} = fsub {} {}, {}", re, ft, lr, rr));
                let im = self.next_tmp();
                self.emitln(&format!("  {} = fsub {} {}, {}", im, ft, li, ri));
                let t1 = self.next_tmp();
                self.emitln(&format!("  {} = insertvalue {} undef, {} {}, 0", t1, ct, ft, re));
                let t2 = self.next_tmp();
                self.emitln(&format!("  {} = insertvalue {} {}, {} {}, 1", t2, ct, t1, ft, im));
                Val::new(t2, ct)
            }
            BinaryOp::Mul => {
                // (a+bi)(c+di) = (ac-bd) + (ad+bc)i
                let ac = self.next_tmp();
                self.emitln(&format!("  {} = fmul {} {}, {}", ac, ft, lr, rr));
                let bd = self.next_tmp();
                self.emitln(&format!("  {} = fmul {} {}, {}", bd, ft, li, ri));
                let ad = self.next_tmp();
                self.emitln(&format!("  {} = fmul {} {}, {}", ad, ft, lr, ri));
                let bc = self.next_tmp();
                self.emitln(&format!("  {} = fmul {} {}, {}", bc, ft, li, rr));
                let re = self.next_tmp();
                self.emitln(&format!("  {} = fsub {} {}, {}", re, ft, ac, bd));
                let im = self.next_tmp();
                self.emitln(&format!("  {} = fadd {} {}, {}", im, ft, ad, bc));
                let t1 = self.next_tmp();
                self.emitln(&format!("  {} = insertvalue {} undef, {} {}, 0", t1, ct, ft, re));
                let t2 = self.next_tmp();
                self.emitln(&format!("  {} = insertvalue {} {}, {} {}, 1", t2, ct, t1, ft, im));
                Val::new(t2, ct)
            }
            BinaryOp::RealDiv => {
                // (a+bi)/(c+di) = ((ac+bd) + (bc-ad)i) / (c^2+d^2)
                let ac = self.next_tmp();
                self.emitln(&format!("  {} = fmul {} {}, {}", ac, ft, lr, rr));
                let bd = self.next_tmp();
                self.emitln(&format!("  {} = fmul {} {}, {}", bd, ft, li, ri));
                let bc = self.next_tmp();
                self.emitln(&format!("  {} = fmul {} {}, {}", bc, ft, li, rr));
                let ad = self.next_tmp();
                self.emitln(&format!("  {} = fmul {} {}, {}", ad, ft, lr, ri));
                let cc = self.next_tmp();
                self.emitln(&format!("  {} = fmul {} {}, {}", cc, ft, rr, rr));
                let dd = self.next_tmp();
                self.emitln(&format!("  {} = fmul {} {}, {}", dd, ft, ri, ri));
                let denom = self.next_tmp();
                self.emitln(&format!("  {} = fadd {} {}, {}", denom, ft, cc, dd));
                let re_num = self.next_tmp();
                self.emitln(&format!("  {} = fadd {} {}, {}", re_num, ft, ac, bd));
                let im_num = self.next_tmp();
                self.emitln(&format!("  {} = fsub {} {}, {}", im_num, ft, bc, ad));
                let re = self.next_tmp();
                self.emitln(&format!("  {} = fdiv {} {}, {}", re, ft, re_num, denom));
                let im = self.next_tmp();
                self.emitln(&format!("  {} = fdiv {} {}, {}", im, ft, im_num, denom));
                let t1 = self.next_tmp();
                self.emitln(&format!("  {} = insertvalue {} undef, {} {}, 0", t1, ct, ft, re));
                let t2 = self.next_tmp();
                self.emitln(&format!("  {} = insertvalue {} {}, {} {}, 1", t2, ct, t1, ft, im));
                Val::new(t2, ct)
            }
            BinaryOp::Eq => {
                let re_eq = self.next_tmp();
                self.emitln(&format!("  {} = fcmp oeq {} {}, {}", re_eq, ft, lr, rr));
                let im_eq = self.next_tmp();
                self.emitln(&format!("  {} = fcmp oeq {} {}, {}", im_eq, ft, li, ri));
                let both = self.next_tmp();
                self.emitln(&format!("  {} = and i1 {}, {}", both, re_eq, im_eq));
                let ext = self.next_tmp();
                self.emitln(&format!("  {} = zext i1 {} to i32", ext, both));
                Val::new(ext, "i32".to_string())
            }
            BinaryOp::Ne => {
                let re_eq = self.next_tmp();
                self.emitln(&format!("  {} = fcmp one {} {}, {}", re_eq, ft, lr, rr));
                let im_eq = self.next_tmp();
                self.emitln(&format!("  {} = fcmp one {} {}, {}", im_eq, ft, li, ri));
                let either = self.next_tmp();
                self.emitln(&format!("  {} = or i1 {}, {}", either, re_eq, im_eq));
                let ext = self.next_tmp();
                self.emitln(&format!("  {} = zext i1 {} to i32", ext, either));
                Val::new(ext, "i32".to_string())
            }
            _ => {
                // Other ops not defined for complex
                Val::new("undef", ct)
            }
        }
    }

    pub(crate) fn gen_comparison(&mut self, op: BinaryOp, l: &Val, r: &Val, ty: &str) -> String {
        let tmp = self.next_tmp();
        let is_float = Self::is_float_type(ty);

        let (icmp, fcmp) = match op {
            BinaryOp::Eq => ("eq", "oeq"),
            BinaryOp::Ne => ("ne", "one"),
            BinaryOp::Lt => ("slt", "olt"),
            BinaryOp::Le => ("sle", "ole"),
            BinaryOp::Gt => ("sgt", "ogt"),
            BinaryOp::Ge => ("sge", "oge"),
            _ => unreachable!(),
        };

        if is_float {
            self.emitln(&format!("  {} = fcmp {} {} {}, {}", tmp, fcmp, ty, l.name, r.name));
        } else if ty == "ptr" {
            // Pointer comparison
            self.emitln(&format!("  {} = icmp {} ptr {}, {}", tmp, icmp, l.name, r.name));
        } else {
            self.emitln(&format!("  {} = icmp {} {} {}, {}", tmp, icmp, ty, l.name, r.name));
        }
        tmp
    }

    pub(crate) fn gen_short_circuit_and(&mut self, left: &Expr, right: &Expr) -> Val {
        // Use alloca-based approach to avoid phi node block naming issues
        let result_alloca = self.next_tmp();
        self.emitln(&format!("  {} = alloca i32", result_alloca));
        self.emitln(&format!("  store i32 0, ptr {}", result_alloca)); // default false

        let rhs_label = self.next_label("and.rhs");
        let merge_label = self.next_label("and.merge");

        let lval = self.gen_expr(left);
        let l_bool = self.to_i1(&lval);
        self.emitln(&format!("  br i1 {}, label %{}, label %{}", l_bool, rhs_label, merge_label));

        self.emitln(&format!("{}:", rhs_label));
        let rval = self.gen_expr(right);
        let r_bool = self.to_i1(&rval);
        let r_ext = self.next_tmp();
        self.emitln(&format!("  {} = zext i1 {} to i32", r_ext, r_bool));
        self.emitln(&format!("  store i32 {}, ptr {}", r_ext, result_alloca));
        self.emitln(&format!("  br label %{}", merge_label));

        self.emitln(&format!("{}:", merge_label));
        let result = self.next_tmp();
        self.emitln(&format!("  {} = load i32, ptr {}", result, result_alloca));
        Val::new(result, "i32".to_string())
    }

    pub(crate) fn gen_short_circuit_or(&mut self, left: &Expr, right: &Expr) -> Val {
        let result_alloca = self.next_tmp();
        self.emitln(&format!("  {} = alloca i32", result_alloca));
        self.emitln(&format!("  store i32 1, ptr {}", result_alloca)); // default true

        let rhs_label = self.next_label("or.rhs");
        let merge_label = self.next_label("or.merge");

        let lval = self.gen_expr(left);
        let l_bool = self.to_i1(&lval);
        self.emitln(&format!("  br i1 {}, label %{}, label %{}", l_bool, merge_label, rhs_label));

        self.emitln(&format!("{}:", rhs_label));
        let rval = self.gen_expr(right);
        let r_bool = self.to_i1(&rval);
        let r_ext = self.next_tmp();
        self.emitln(&format!("  {} = zext i1 {} to i32", r_ext, r_bool));
        self.emitln(&format!("  store i32 {}, ptr {}", r_ext, result_alloca));
        self.emitln(&format!("  br label %{}", merge_label));

        self.emitln(&format!("{}:", merge_label));
        let result = self.next_tmp();
        self.emitln(&format!("  {} = load i32, ptr {}", result, result_alloca));
        Val::new(result, "i32".to_string())
    }

    pub(crate) fn to_i1(&mut self, val: &Val) -> String {
        if val.ty == "i1" {
            return val.name.clone();
        }
        let tmp = self.next_tmp();
        if Self::is_float_type(&val.ty) {
            self.emitln(&format!("  {} = fcmp one {} {}, 0.0", tmp, val.ty, val.name));
        } else {
            let zero = if val.ty == "ptr" { "null" } else { "0" };
            self.emitln(&format!("  {} = icmp ne {} {}, {}", tmp, val.ty, val.name, zero));
        }
        tmp
    }

    /// Convert an expression to i1 for use in branch conditions.
    pub(crate) fn gen_expr_as_i1(&mut self, expr: &Expr) -> String {
        let val = self.gen_expr(expr);
        self.to_i1(&val)
    }
}
