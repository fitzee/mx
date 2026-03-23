use super::*;

impl LLVMCodeGen {
    // ── Statement generation ────────────────────────────────────────

    pub(crate) fn gen_statement(&mut self, stmt: &Statement) {
        // Set debug source location for this statement
        self.set_debug_loc(&stmt.loc);

        // Update stack frame line number for stack trace
        if stmt.loc.line > 0 {
            if let Some(frame) = self.stack_frame_alloca.clone() {
                let line_ptr = self.next_tmp();
                self.emitln(&format!("  {} = getelementptr inbounds %m2_StackFrame, ptr {}, i32 0, i32 3",
                    line_ptr, frame));
                self.emitln(&format!("  store i32 {}, ptr {}", stmt.loc.line, line_ptr));
            }
        }

        match &stmt.kind {
            StatementKind::Empty => {}

            StatementKind::Assign { desig, expr } => {
                self.gen_assign(desig, expr);
            }

            StatementKind::ProcCall { desig, args } => {
                self.gen_proc_call_stmt(desig, args);
            }

            StatementKind::If { cond, then_body, elsifs, else_body } => {
                self.gen_if(cond, then_body, elsifs, else_body);
            }

            StatementKind::While { cond, body } => {
                self.gen_while(cond, body);
            }

            StatementKind::Repeat { body, cond } => {
                self.gen_repeat(body, cond);
            }

            StatementKind::For { var, start, end, step, body } => {
                self.gen_for(var, start, end, step.as_ref(), body);
            }

            StatementKind::Loop { body } => {
                self.gen_loop(body);
            }

            StatementKind::Exit => {
                if let Some(exit_label) = self.loop_exit_stack.last().cloned() {
                    self.emitln(&format!("  br label %{}", exit_label));
                    let dead = self.next_label("exit.dead");
                    self.emitln(&format!("{}:", dead));
                    self.emitln("  unreachable");
                }
            }

            StatementKind::Return { expr } => {
                // Pop stack frame before returning
                if let Some(ref frame) = self.stack_frame_alloca.clone() {
                    self.emitln(&format!("  call void @m2_stack_pop(ptr {})", frame));
                }
                if let Some(e) = expr {
                    let val = self.gen_expr(e);
                    let ret_ty = self.current_return_type.clone().unwrap_or_else(|| "void".to_string());
                    // Aggregate invariant: gen_expr returns ptr for structs.
                    // For return statements, load the actual struct value.
                    let final_val = if ret_ty.starts_with('{') && val.ty == "ptr" {
                        let tmp = self.next_tmp();
                        self.emitln(&format!("  {} = load {}, ptr {}", tmp, ret_ty, val.name));
                        Val::new(tmp, ret_ty.clone())
                    } else {
                        val
                    };
                    let coerced = self.coerce_val(&final_val, &ret_ty);
                    self.emitln(&format!("  ret {} {}", ret_ty, coerced.name));
                } else {
                    self.emitln("  ret void");
                }
                // Dead code after return — emit unreachable block
                let dead = self.next_label("ret.dead");
                self.emitln(&format!("{}:", dead));
                self.emitln("  unreachable");
            }

            StatementKind::Case { expr, branches, else_body } => {
                self.gen_case(expr, branches, else_body);
            }

            StatementKind::With { desig, body } => {
                let var_name = desig.ident.name.clone();
                let has_deref = desig.selectors.iter().any(|s| matches!(s, Selector::Deref(_)));

                // Resolve the record type via TypeId (new) and type_name (legacy)
                let mut with_tid: Option<crate::types::TypeId> = self.var_types.get(&var_name).copied();
                let mut type_name = self.var_type_names.get(&var_name).cloned().unwrap_or_default();
                let mut field_names: Vec<String> = Vec::new();

                // Resolve through deref if needed
                if has_deref {
                    if let Some(tid) = with_tid {
                        if let Some(target) = self.tl_pointer_target(self.tl_resolve(tid)) {
                            with_tid = Some(self.tl_resolve(target));
                        }
                    }
                    if let Some(target) = self.pointer_target_types.get(&type_name).cloned() {
                        type_name = target;
                    }
                }

                // Get field names — try TypeLowering first, then legacy
                if let Some(tid) = with_tid {
                    if let Some(ref tl) = self.type_lowering {
                        if let Some(layout) = tl.get_record_layout(self.tl_resolve(tid)) {
                            field_names = layout.fields.iter().map(|f| f.name.clone()).collect();
                        }
                    }
                }
                if field_names.is_empty() {
                    // Legacy fallback — also handles nested WITH
                    if type_name.is_empty() {
                        for (_, outer_tn, outer_fields, _, outer_tid) in self.with_stack.iter().rev() {
                            if outer_fields.contains(&var_name) {
                                // Try TypeId-based resolution of outer field type
                                if let Some(otid) = outer_tid {
                                    if let Some(fi) = self.tl_lookup_field(*otid, &var_name) {
                                        with_tid = Some(fi.m2_type);
                                        if let Some(ref tl) = self.type_lowering {
                                            let resolved = tl.resolve_alias(&self.sema.types, fi.m2_type);
                                            if let Some(layout) = tl.get_record_layout(resolved) {
                                                field_names = layout.fields.iter().map(|f| f.name.clone()).collect();
                                            }
                                        }
                                    }
                                }
                                // Legacy: look up field type via string matching
                                if field_names.is_empty() {
                                    if let Some(fields) = self.record_fields.get(outer_tn) {
                                        if let Some((_, ft, _)) = fields.iter().find(|(n, _, _)| n == &var_name) {
                                            for (tn, ty) in &self.type_map {
                                                if ty == ft {
                                                    type_name = tn.clone();
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                }
                                break;
                            }
                        }
                    }
                    if field_names.is_empty() {
                        field_names = self.record_fields.get(&type_name)
                            .map(|fields| fields.iter().map(|(n, _, _)| n.clone()).collect())
                            .unwrap_or_default();
                    }
                }

                let with_var = if self.var_types.contains_key(&var_name)
                    || self.var_type_names.contains_key(&var_name)
                    || self.lookup_local(&var_name).is_some()
                    || self.globals.contains_key(&var_name)
                    || self.globals.contains_key(&self.mangle(&var_name))
                {
                    var_name.clone()
                } else {
                    var_name.clone()
                };
                self.with_stack.push((with_var, type_name, field_names, has_deref, with_tid));
                for stmt in body {
                    self.gen_statement(stmt);
                }
                self.with_stack.pop();
            }

            StatementKind::Raise { expr } => {
                self.declare_exc_runtime();
                if self.in_sjlj_context && self.try_unwind_dest.is_none() {
                    // Inside SjLj-guarded procedure body — use m2_raise (longjmp)
                    let exc_id = if let Some(e) = expr {
                        let val = self.gen_expr(e);
                        self.coerce_val(&val, "i32").name.clone()
                    } else {
                        "1".to_string()
                    };
                    self.emitln(&format!(
                        "  call void @m2_raise(i32 {}, ptr null, ptr null)", exc_id));
                    self.emitln("  unreachable");
                } else {
                    // Inside TRY body or top-level — use native EH (m2_eh_throw)
                    if let Some(e) = expr {
                        let val = self.gen_expr(e);
                        let coerced = self.coerce_val(&val, "i32");
                        if let Some(ref unwind_dest) = self.try_unwind_dest.clone() {
                            let cont = self.next_label("raise.cont");
                            self.emitln(&format!(
                                "  invoke void @m2_eh_throw(i32 {}, ptr null) to label %{} unwind label %{}",
                                coerced.name, cont, unwind_dest));
                            self.emitln(&format!("{}:", cont));
                        } else {
                            self.emitln(&format!(
                                "  call void @m2_eh_throw(i32 {}, ptr null)", coerced.name));
                        }
                    } else {
                        if let Some(ref unwind_dest) = self.try_unwind_dest.clone() {
                            let cont = self.next_label("raise.cont");
                            self.emitln(&format!(
                                "  invoke void @m2_eh_throw(i32 1, ptr null) to label %{} unwind label %{}",
                                cont, unwind_dest));
                            self.emitln(&format!("{}:", cont));
                        } else {
                            self.emitln("  call void @m2_eh_throw(i32 1, ptr null)");
                        }
                    }
                    self.emitln("  unreachable");
                }
            }

            StatementKind::Retry => {
                // SjLj parity: branch back to TRY entry
                if let Some(label) = self.try_entry_label.last().cloned() {
                    self.emitln(&format!("  br label %{}", label));
                    let dead = self.next_label("retry.dead");
                    self.emitln(&format!("{}:", dead));
                }
            }

            StatementKind::Try { body, excepts, finally_body } => {
                self.gen_try_native(body, excepts, finally_body);
            }

            StatementKind::Lock { mutex: _, body } => {
                for stmt in body {
                    self.gen_statement(stmt);
                }
            }

            StatementKind::TypeCase { expr, branches, else_body } => {
                self.gen_typecase(expr, branches, else_body);
            }
        }
    }

    pub(crate) fn gen_assign(&mut self, desig: &Designator, expr: &Expr) {
        // Check if this is a string literal assignment to a char array
        let is_string_assign = matches!(&expr.kind, ExprKind::StringLit(s) if s.len() > 1);
        let is_array_target = self.array_vars.contains(&desig.ident.name)
            || self.char_array_vars.contains(&desig.ident.name);

        if is_string_assign && is_array_target {
            // String to char array: memset + memcpy
            if let ExprKind::StringLit(s) = &expr.kind {
                let addr = self.gen_designator_addr(desig);
                let (str_global, _) = self.intern_string(s);
                let lit_size = s.len() + 1;

                self.emitln(&format!("  call ptr @memcpy(ptr {}, ptr {}, i64 {})",
                    addr.name, str_global, lit_size));
            }
            return;
        }

        let addr = self.gen_designator_addr(desig);

        // Handle assigning a bare procedure name to a variable (function pointer)
        if let ExprKind::Designator(rhs_desig) = &expr.kind {
            if rhs_desig.selectors.is_empty() && addr.ty == "ptr" {
                let rhs_name = &rhs_desig.ident.name;
                let mangled = self.mangle(rhs_name);
                // Check if it's a known procedure
                if self.proc_params.contains_key(&mangled) || self.proc_params.contains_key(rhs_name) {
                    let fn_name = if self.proc_params.contains_key(&mangled) {
                        format!("@{}", mangled)
                    } else if let Some(module) = self.import_map.get(rhs_name) {
                        let orig = self.import_alias_map.get(rhs_name).cloned().unwrap_or_else(|| rhs_name.to_string());
                        let prefixed = format!("{}_{}", module, orig);
                        if let Some(runtime) = self.stdlib_name_map.get(&prefixed) {
                            format!("@{}", runtime)
                        } else {
                            format!("@{}", prefixed)
                        }
                    } else {
                        format!("@{}", mangled)
                    };
                    self.emitln(&format!("  store ptr {}, ptr {}", fn_name, addr.name));
                    return;
                }
            }
        }

        // Handle single-char string assigned to CHAR or integer variable
        let final_val = if let ExprKind::StringLit(s) = &expr.kind {
            if s.len() <= 1 && (addr.ty == "i8" || addr.ty == "i32" || addr.ty == "i64") {
                // Char/subrange assignment: use byte value directly
                let byte_val = if s.is_empty() { 0u8 } else { s.as_bytes()[0] };
                Val::new(format!("{}", byte_val), addr.ty.clone())
            } else {
                self.gen_expr(expr)
            }
        } else {
            self.gen_expr(expr)
        };

        // Handle aggregate-to-aggregate assignment via memcpy.
        // Both source and destination must be aggregates.
        let dest_is_aggregate = addr.type_id
            .map(|tid| is_aggregate(&self.sema.types, tid))
            .unwrap_or_else(|| addr.ty.starts_with('{') || addr.ty.starts_with('%'));
        let src_is_aggregate = final_val.type_id
            .map(|tid| is_aggregate(&self.sema.types, tid))
            .unwrap_or(false);
        if dest_is_aggregate && src_is_aggregate && !addr.ty.contains("float") {
            if final_val.ty == "ptr" {
                // Source is an address (aggregate invariant) — direct memcpy
                self.emit_struct_memcpy(&addr.name, &final_val.name, &addr.ty);
                return;
            } else if final_val.ty.starts_with('{') || final_val.ty.starts_with('%') {
                // Source is a struct SSA value (from function return) — spill then memcpy
                let tmp_alloca = self.next_tmp();
                self.emitln(&format!("  {} = alloca {}", tmp_alloca, final_val.ty));
                self.emitln(&format!("  store {} {}, ptr {}", final_val.ty, final_val.name, tmp_alloca));
                self.emit_struct_memcpy(&addr.name, &tmp_alloca, &addr.ty);
                return;
            }
        }
        // Dest aggregate but source has no TypeId — use LLVM type string as last resort
        if dest_is_aggregate && !addr.ty.contains("float") && final_val.ty == "ptr" {
            self.emit_struct_memcpy(&addr.name, &final_val.name, &addr.ty);
            return;
        }

        // Handle string-to-char-array: memcpy instead of store
        if addr.ty.starts_with('[') && final_val.ty == "ptr" {
            // Array destination, string source — use memcpy
            // Extract size from array type [N x i8]
            if let Some(n_str) = addr.ty.strip_prefix('[').and_then(|s| s.split(' ').next()) {
                if let Ok(n) = n_str.parse::<i64>() {
                    // Compute string length from interned string
                    let str_len = if let ExprKind::StringLit(s) = &expr.kind {
                        (s.len() + 1) as i64 // include NUL
                    } else {
                        n // fallback: copy array size
                    };
                    let copy_len = std::cmp::min(str_len, n);
                    // Zero the array first, then copy
                    self.emitln(&format!("  call ptr @memset(ptr {}, i32 0, i64 {})", addr.name, n));
                    self.emitln(&format!("  call ptr @memcpy(ptr {}, ptr {}, i64 {})",
                        addr.name, final_val.name, copy_len));
                    return;
                }
            }
        }

        // Determine store type from semantic TypeId (source of truth),
        // falling back to addr.ty only when TypeId is unavailable.
        let store_ty_owned: String = if let Some(tid) = addr.type_id {
            let s = self.tl_type_str(tid);
            // Aggregate target with scalar source → use source type
            // (field GEP resolved semantically but legacy type is stale)
            if (s.starts_with('{') || s.starts_with('['))
                && (final_val.ty.starts_with('i') || final_val.ty == "float"
                    || final_val.ty == "double" || final_val.ty == "ptr") {
                final_val.ty.clone()
            } else {
                s
            }
        } else if (addr.ty.starts_with('{') || addr.ty.starts_with('%') || addr.ty.starts_with('['))
            && (final_val.ty.starts_with('i') || final_val.ty == "float"
                || final_val.ty == "double" || final_val.ty == "ptr") {
            final_val.ty.clone()
        } else {
            addr.ty.clone()
        };
        debug_assert!(store_ty_owned != "void",
            "ICE: store type is void — target designator has unresolved TypeId");
        let coerced = self.coerce_val(&final_val, &store_ty_owned);
        self.emitln(&format!("  store {} {}, ptr {}", store_ty_owned, coerced.name, addr.name));
    }

    pub(crate) fn gen_proc_call_stmt(&mut self, desig: &Designator, args: &[Expr]) {
        // Indirect call through pointer dereference or field access on non-module
        // e.g. cp^.genFn(args), rec.callback(args)
        // Must check BEFORE resolve_proc_name to avoid mangling local vars as procs
        let has_deref_or_field = !desig.selectors.is_empty()
            && !self.imported_modules.contains(&desig.ident.name)
            && desig.selectors.iter().any(|s| matches!(s,
                Selector::Deref(_) | Selector::Field(_, _)));
        if has_deref_or_field {
            let fn_ptr = self.gen_designator_load(desig);
            self.gen_indirect_call(&fn_ptr, args);
            return;
        }

        let name = self.resolve_proc_name(desig);

        // Handle built-in procedures
        if builtins::is_builtin_proc(&desig.ident.name) || builtins::is_builtin_proc(&name) {
            let builtin_name = if builtins::is_builtin_proc(&desig.ident.name) {
                &desig.ident.name
            } else {
                &name
            };
            self.gen_builtin_proc_call(builtin_name, args);
            return;
        }

        // Check if this is a proc variable (indirect call through function pointer)
        // A proc variable is a VARIABLE with type "ptr" — not a function definition.
        // We check that it's in locals/globals but NOT in declared_fns (which tracks actual functions).
        let is_proc_var = desig.selectors.is_empty()
            && desig.ident.module.is_none()
            && {
                let mangled = self.mangle(&desig.ident.name);
                let is_local_ptr = self.lookup_local(&desig.ident.name).map(|(_, ty)| ty == "ptr").unwrap_or(false);
                let is_global_ptr = self.globals.get(&desig.ident.name).map(|(_, ty)| ty == "ptr").unwrap_or(false)
                    || self.globals.get(&mangled).map(|(_, ty)| ty == "ptr").unwrap_or(false);
                // Must be a variable, not a known function
                let is_known_fn = self.declared_fns.contains(&mangled)
                    || self.declared_fns.contains(&desig.ident.name);
                (is_local_ptr || is_global_ptr) && !is_known_fn
            };

        if is_proc_var {
            let fn_ptr = self.gen_designator_load(desig);
            self.gen_indirect_call(&fn_ptr, args);
            return;
        }

        // Special handling for non-native Strings module functions that need extra HIGH params.
        // Native Strings module has proper open array params — gen_call handles them correctly.
        if !crate::stdlib::is_native_stdlib("Strings")
            && (name.contains("Strings_Assign") || name.contains("Strings_Concat")
                || name.contains("Strings_Insert") || name.contains("Strings_Delete")
                || name.contains("Strings_Copy"))
        {
            self.gen_strings_call(&name, args);
            return;
        }

        self.gen_call(&name, args, "void");
    }

    fn gen_indirect_call(&mut self, fn_ptr: &Val, args: &[Expr]) {
        // Ensure fn_ptr is a ptr (proc type fields may load as i32 etc.)
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
                    // Compute HIGH from the resolved address type (not the base variable),
                    // so that e.g. arr2d[i] passes the inner array's HIGH, not the outer's.
                    let high = if let Some(n_str) = addr.ty.strip_prefix('[').and_then(|s| s.split(' ').next()) {
                        if let Ok(n) = n_str.parse::<usize>() {
                            format!("{}", n.saturating_sub(1))
                        } else { self.get_array_high(&d.ident.name) }
                    } else { self.get_array_high(&d.ident.name) };
                    arg_strs.push(format!("ptr {}", addr.name));
                    arg_strs.push(format!("i32 {}", high));
                    continue;
                }
            }
            let val = self.gen_expr(arg);
            arg_strs.push(format!("{} {}", val.ty, val.name));
        }
        let args_str = arg_strs.join(", ");
        self.emitln(&format!("  call void {}({})", call_target, args_str));
    }

    // ── Control flow generation ─────────────────────────────────────

    pub(crate) fn gen_if(&mut self, cond: &Expr, then_body: &[Statement],
              elsifs: &[(Expr, Vec<Statement>)], else_body: &Option<Vec<Statement>>) {
        let then_label = self.next_label("if.then");
        let merge_label = self.next_label("if.end");

        let first_else_label = if !elsifs.is_empty() {
            self.next_label("if.elsif")
        } else if else_body.is_some() {
            self.next_label("if.else")
        } else {
            merge_label.clone()
        };

        let cond_val = self.gen_expr_as_i1(cond);
        self.emitln(&format!("  br i1 {}, label %{}, label %{}", cond_val, then_label, first_else_label));

        // Then block
        self.emitln(&format!("{}:", then_label));
        for stmt in then_body {
            self.gen_statement(stmt);
        }
        self.emitln(&format!("  br label %{}", merge_label));

        // Elsif blocks
        let mut current_else = first_else_label;
        for (i, (elsif_cond, elsif_body)) in elsifs.iter().enumerate() {
            self.emitln(&format!("{}:", current_else));
            let elsif_then = self.next_label("elsif.then");
            let next_else = if i + 1 < elsifs.len() {
                self.next_label("if.elsif")
            } else if else_body.is_some() {
                self.next_label("if.else")
            } else {
                merge_label.clone()
            };

            let cond_val = self.gen_expr_as_i1(elsif_cond);
            self.emitln(&format!("  br i1 {}, label %{}, label %{}", cond_val, elsif_then, next_else));

            self.emitln(&format!("{}:", elsif_then));
            for stmt in elsif_body {
                self.gen_statement(stmt);
            }
            self.emitln(&format!("  br label %{}", merge_label));
            current_else = next_else;
        }

        // Else block
        if let Some(else_stmts) = else_body {
            self.emitln(&format!("{}:", current_else));
            for stmt in else_stmts {
                self.gen_statement(stmt);
            }
            self.emitln(&format!("  br label %{}", merge_label));
        }

        // Merge block
        self.emitln(&format!("{}:", merge_label));
    }

    pub(crate) fn gen_while(&mut self, cond: &Expr, body: &[Statement]) {
        let cond_label = self.next_label("while.cond");
        let body_label = self.next_label("while.body");
        let end_label = self.next_label("while.end");

        self.emitln(&format!("  br label %{}", cond_label));
        self.emitln(&format!("{}:", cond_label));

        let cond_val = self.gen_expr_as_i1(cond);
        self.emitln(&format!("  br i1 {}, label %{}, label %{}", cond_val, body_label, end_label));

        self.emitln(&format!("{}:", body_label));
        self.loop_exit_stack.push(end_label.clone());
        for stmt in body {
            self.gen_statement(stmt);
        }
        self.loop_exit_stack.pop();
        self.emitln(&format!("  br label %{}", cond_label));

        self.emitln(&format!("{}:", end_label));
    }

    pub(crate) fn gen_repeat(&mut self, body: &[Statement], cond: &Expr) {
        let body_label = self.next_label("repeat.body");
        let end_label = self.next_label("repeat.end");

        self.emitln(&format!("  br label %{}", body_label));
        self.emitln(&format!("{}:", body_label));

        self.loop_exit_stack.push(end_label.clone());
        for stmt in body {
            self.gen_statement(stmt);
        }
        self.loop_exit_stack.pop();

        let cond_val = self.gen_expr_as_i1(cond);
        self.emitln(&format!("  br i1 {}, label %{}, label %{}", cond_val, end_label, body_label));

        self.emitln(&format!("{}:", end_label));
    }

    pub(crate) fn gen_for(&mut self, var: &str, start: &Expr, end: &Expr, step: Option<&Expr>, body: &[Statement]) {
        // Handle single-char string literals as char values
        let start_val = if let ExprKind::StringLit(s) = &start.kind {
            if s.len() == 1 { Val::new(format!("{}", s.as_bytes()[0]), "i8".to_string()) }
            else { self.gen_expr(start) }
        } else { self.gen_expr(start) };
        let end_val = if let ExprKind::StringLit(s) = &end.kind {
            if s.len() == 1 { Val::new(format!("{}", s.as_bytes()[0]), "i8".to_string()) }
            else { self.gen_expr(end) }
        } else { self.gen_expr(end) };

        let var_addr = self.get_var_addr(var);
        let var_ty = var_addr.ty.clone();
        let start_coerced = self.coerce_val(&start_val, &var_ty);
        let end_coerced = self.coerce_val(&end_val, &var_ty);

        // Determine step
        let step_val = if let Some(s) = step {
            self.gen_expr(s)
        } else {
            Val::new("1", var_ty.clone())
        };
        let step_coerced = self.coerce_val(&step_val, &var_ty);

        // Determine if counting up or down
        let is_negative_step = if let Some(s) = step {
            match &s.kind {
                ExprKind::UnaryOp { op: UnaryOp::Neg, .. } => true,
                ExprKind::IntLit(v) => *v < 0,
                _ => step_val.name.starts_with('-'),
            }
        } else {
            false
        };

        let preheader_label = self.next_label("for.ph");
        let header_label = self.next_label("for.header");
        let latch_label = self.next_label("for.latch");
        let exit_label = self.next_label("for.exit");

        // ── Preheader: evaluate bounds once, skip if empty range ──
        // Guard: if start > end (counting up) or start < end (counting down), skip
        let skip_cmp = self.next_tmp();
        if is_negative_step {
            self.emitln(&format!("  {} = icmp slt {} {}, {}", skip_cmp, var_ty, start_coerced.name, end_coerced.name));
        } else {
            self.emitln(&format!("  {} = icmp sgt {} {}, {}", skip_cmp, var_ty, start_coerced.name, end_coerced.name));
        }
        self.emitln(&format!("  br i1 {}, label %{}, label %{}", skip_cmp, exit_label, preheader_label));

        self.emitln(&format!("{}:", preheader_label));
        // Store start value to loop var (for body code that reads via alloca)
        self.emitln(&format!("  store {} {}, ptr {}", var_ty, start_coerced.name, var_addr.name));
        self.emitln(&format!("  br label %{}", header_label));

        // ── Header: body executes here ──
        self.emitln(&format!("{}:", header_label));

        self.loop_exit_stack.push(exit_label.clone());
        for stmt in body {
            self.gen_statement(stmt);
        }
        self.loop_exit_stack.pop();

        self.emitln(&format!("  br label %{}", latch_label));

        // ── Latch: increment, compare, branch back or exit ──
        self.emitln(&format!("{}:", latch_label));
        let cur = self.next_tmp();
        self.emitln(&format!("  {} = load {}, ptr {}", cur, var_ty, var_addr.name));
        let next = self.next_tmp();
        // nsw: FOR loop induction variables have defined range (no signed overflow)
        self.emitln(&format!("  {} = add nsw {} {}, {}", next, var_ty, cur, step_coerced.name));
        self.emitln(&format!("  store {} {}, ptr {}", var_ty, next, var_addr.name));

        // Exit test: compare AFTER increment (canonical latch-exit pattern)
        let cont_cmp = self.next_tmp();
        if is_negative_step {
            self.emitln(&format!("  {} = icmp sge {} {}, {}", cont_cmp, var_ty, next, end_coerced.name));
        } else {
            self.emitln(&format!("  {} = icmp sle {} {}, {}", cont_cmp, var_ty, next, end_coerced.name));
        }
        self.emitln(&format!("  br i1 {}, label %{}, label %{}", cont_cmp, header_label, exit_label));

        // ── Exit ──
        self.emitln(&format!("{}:", exit_label));
    }

    pub(crate) fn gen_loop(&mut self, body: &[Statement]) {
        let body_label = self.next_label("loop.body");
        let end_label = self.next_label("loop.end");

        self.emitln(&format!("  br label %{}", body_label));
        self.emitln(&format!("{}:", body_label));

        self.loop_exit_stack.push(end_label.clone());
        for stmt in body {
            self.gen_statement(stmt);
        }
        self.loop_exit_stack.pop();

        self.emitln(&format!("  br label %{}", body_label));
        self.emitln(&format!("{}:", end_label));
    }

    pub(crate) fn gen_case(&mut self, expr: &Expr, branches: &[CaseBranch], else_body: &Option<Vec<Statement>>) {
        let val = self.gen_expr(expr);
        let end_label = self.next_label("case.end");
        let default_label = if else_body.is_some() {
            self.next_label("case.else")
        } else {
            end_label.clone()
        };

        // Collect all branch labels
        let mut branch_labels = Vec::new();
        let mut switch_cases = Vec::new();

        for branch in branches {
            let label = self.next_label("case.branch");
            branch_labels.push(label.clone());
            for cl in &branch.labels {
                match cl {
                    CaseLabel::Single(e) => {
                        if let Some(v) = self.const_eval_expr(e) {
                            switch_cases.push((v, label.clone()));
                        }
                    }
                    CaseLabel::Range(lo, hi) => {
                        if let (Some(lo_v), Some(hi_v)) = (self.const_eval_expr(lo), self.const_eval_expr(hi)) {
                            for v in lo_v..=hi_v {
                                switch_cases.push((v, label.clone()));
                            }
                        }
                    }
                }
            }
        }

        // Emit switch instruction
        let coerced = self.coerce_val(&val, "i32");
        let mut switch_str = format!("  switch i32 {}, label %{} [\n", coerced.name, default_label);
        for (case_val, label) in &switch_cases {
            switch_str.push_str(&format!("    i32 {}, label %{}\n", case_val, label));
        }
        switch_str.push_str("  ]");
        self.emitln(&switch_str);

        // Emit branch bodies
        for (i, branch) in branches.iter().enumerate() {
            self.emitln(&format!("{}:", branch_labels[i]));
            for stmt in &branch.body {
                self.gen_statement(stmt);
            }
            self.emitln(&format!("  br label %{}", end_label));
        }

        // Else body
        if let Some(else_stmts) = else_body {
            self.emitln(&format!("{}:", default_label));
            for stmt in else_stmts {
                self.gen_statement(stmt);
            }
            self.emitln(&format!("  br label %{}", end_label));
        }

        self.emitln(&format!("{}:", end_label));
    }

    // ── Exception handling ─────────────────────────────────────────
    //
    // TRY/EXCEPT/FINALLY uses LLVM-native EH (invoke/landingpad).
    // ISO module-body EXCEPT uses SjLj (setjmp/longjmp) for simplicity.

    pub(crate) fn declare_exc_runtime(&mut self) {
        if !self.declared_fns.contains("m2_eh_throw") {
            // LLVM-native EH
            self.emit_preambleln("declare void @m2_eh_throw(i32, ptr)");
            self.emit_preambleln("declare i32 @m2_eh_get_id(ptr)");
            self.declared_fns.insert("m2_eh_throw".to_string());
            self.declared_fns.insert("m2_eh_get_id".to_string());
            // SjLj (for ISO module-body EXCEPT)
            self.emit_preambleln("declare void @m2_raise(i32, ptr, ptr)");
            self.emit_preambleln("declare void @m2_exc_push(ptr)");
            self.emit_preambleln("declare void @m2_exc_pop(ptr)");
            self.emit_preambleln("declare i32 @setjmp(ptr)");
            self.emit_preambleln("declare i32 @m2_exc_get_id(ptr)");
            self.emit_preambleln("declare void @m2_exc_reraise(ptr)");
            self.declared_fns.insert("m2_raise".to_string());
            self.declared_fns.insert("m2_exc_push".to_string());
            self.declared_fns.insert("m2_exc_pop".to_string());
            self.declared_fns.insert("setjmp".to_string());
            self.declared_fns.insert("m2_exc_get_id".to_string());
            self.declared_fns.insert("m2_exc_reraise".to_string());
        }
    }

    fn gen_try_native(&mut self, body: &[Statement],
                      excepts: &[crate::ast::ExceptClause],
                      finally_body: &Option<Vec<Statement>>) {
        self.declare_exc_runtime();
        // Declare personality function
        if !self.declared_fns.contains("m2_eh_personality") {
            self.emit_preambleln("declare i32 @m2_eh_personality(...)");
            self.declared_fns.insert("m2_eh_personality".to_string());
        }

        let landing_label = self.next_label("try.lpad");
        let finally_label = self.next_label("try.finally");
        let end_label = self.next_label("try.end");

        // Set unwind destination so all calls in the body become invoke
        let saved_unwind = self.try_unwind_dest.clone();
        self.try_unwind_dest = Some(landing_label.clone());

        // ── TRY body ──
        self.try_entry_label.push(landing_label.clone());
        for stmt in body {
            self.gen_statement(stmt);
        }
        self.try_entry_label.pop();
        self.try_unwind_dest = saved_unwind.clone();
        self.emitln(&format!("  br label %{}", finally_label));

        // ── Landing pad ──
        self.emitln(&format!("{}:", landing_label));

        // Build landingpad clause list
        let has_finally = finally_body.is_some();
        let has_catch_all = excepts.iter().any(|ec| ec.exception.is_none());

        // Collect type info globals for typed catches
        let mut catch_clauses = Vec::new();
        for ec in excepts {
            if let Some(ref exc_name) = ec.exception {
                let mangled = if let Some(ref m) = exc_name.module {
                    format!("{}_{}", m, exc_name.name)
                } else {
                    self.mangle(&exc_name.name)
                };
                let ti_global = format!("@M2_EXC_{}", mangled);
                // Declare the type info global if not already
                if !self.globals.contains_key(&format!("M2_EXC_{}", mangled)) {
                    let exc_val = self.const_values.get(&mangled)
                        .or_else(|| self.const_values.get(&exc_name.name))
                        .copied()
                        .unwrap_or(0);
                    self.emit_preambleln(&format!(
                        "{} = global i32 {}", ti_global, exc_val));
                    self.globals.insert(format!("M2_EXC_{}", mangled),
                        (ti_global.clone(), "i32".to_string()));
                }
                catch_clauses.push((Some(ti_global), ec));
            } else {
                catch_clauses.push((None, ec)); // catch-all
            }
        }

        // Emit landingpad instruction
        let lp = self.next_tmp();
        // Always add cleanup when there are typed catches without catch-all.
        // This ensures the landing pad is entered even if no catch matches,
        // allowing the dispatch code to resume and propagate to outer handlers.
        let cleanup = if !has_catch_all { " cleanup" } else { "" };
        self.emitln(&format!("  {} = landingpad {{ ptr, i32 }}{}", lp, cleanup));
        for (ti, _) in &catch_clauses {
            if let Some(ref ti_global) = ti {
                self.emitln(&format!("    catch ptr {}", ti_global));
            } else {
                self.emitln("    catch ptr null"); // catch-all
            }
        }
        if catch_clauses.is_empty() && has_finally {
            // FINALLY without EXCEPT: use catch-all so the search phase finds
            // a handler. Without this, cleanup-only landing pads are skipped
            // during search and the unwinder never finds the outer catch.
            self.emitln("    catch ptr null");
        }

        // Extract exception pointer and selector
        let exc_ptr = self.next_tmp();
        self.emitln(&format!("  {} = extractvalue {{ ptr, i32 }} {}, 0", exc_ptr, lp));
        let selector = self.next_tmp();
        self.emitln(&format!("  {} = extractvalue {{ ptr, i32 }} {}, 1", selector, lp));

        // ── Dispatch to handlers ──
        if catch_clauses.is_empty() {
            // No handlers — run FINALLY then resume
            if has_finally {
                self.emitln(&format!("  br label %{}.resume", finally_label));
            } else {
                self.emitln(&format!("  resume {{ ptr, i32 }} {}", lp));
            }
        } else {
            // Create a resume label for the no-match path when there's no catch-all
            let nomatch_label = if !has_catch_all {
                Some(self.next_label("try.nomatch"))
            } else {
                None
            };

            for (i, (ti, ec)) in catch_clauses.iter().enumerate() {
                let handler_label = self.next_label("try.handler");
                let next_label = if i + 1 < catch_clauses.len() {
                    self.next_label("try.next")
                } else if has_finally {
                    finally_label.clone()
                } else if let Some(ref nm) = nomatch_label {
                    nm.clone()
                } else {
                    end_label.clone()
                };

                if let Some(ref ti_global) = ti {
                    // Typed catch — compare selector
                    let expected_sel = self.next_tmp();
                    self.emitln(&format!("  {} = call i32 @llvm.eh.typeid.for(ptr {})",
                        expected_sel, ti_global));
                    if !self.declared_fns.contains("llvm.eh.typeid.for") {
                        self.emit_preambleln("declare i32 @llvm.eh.typeid.for(ptr)");
                        self.declared_fns.insert("llvm.eh.typeid.for".to_string());
                    }
                    let cmp = self.next_tmp();
                    self.emitln(&format!("  {} = icmp eq i32 {}, {}",
                        cmp, selector, expected_sel));
                    self.emitln(&format!("  br i1 {}, label %{}, label %{}",
                        cmp, handler_label, next_label));
                } else {
                    // Catch-all — always matches
                    self.emitln(&format!("  br label %{}", handler_label));
                }

                self.emitln(&format!("{}:", handler_label));
                for stmt in &ec.body {
                    self.gen_statement(stmt);
                }
                self.emitln(&format!("  br label %{}",
                    if has_finally { &finally_label } else { &end_label }));

                if next_label != finally_label && next_label != end_label
                    && nomatch_label.as_ref() != Some(&next_label) {
                    self.emitln(&format!("{}:", next_label));
                }
            }

            // No handler matched — resume unwinding
            if let Some(ref nm) = nomatch_label {
                self.emitln(&format!("{}:", nm));
                if has_finally {
                    self.emitln(&format!("  br label %{}.resume", finally_label));
                } else {
                    // Resume to outer handler or out of the function
                    if !self.declared_fns.contains("_Unwind_Resume") {
                        self.emit_preambleln("declare void @_Unwind_Resume(ptr)");
                        self.declared_fns.insert("_Unwind_Resume".to_string());
                    }
                    if let Some(ref outer_unwind) = saved_unwind {
                        let cont = self.next_label("reraise.unr");
                        self.emitln(&format!(
                            "  invoke void @_Unwind_Resume(ptr {}) to label %{} unwind label %{}",
                            exc_ptr, cont, outer_unwind));
                        self.emitln(&format!("{}:", cont));
                        self.emitln("  unreachable");
                    } else {
                        self.emitln(&format!("  resume {{ ptr, i32 }} {}", lp));
                    }
                }
            }
        }

        // ── FINALLY ──
        // Uses a flag to distinguish normal entry from exception entry.
        // After FINALLY body: if on exception path, resume unwinding.
        let needs_resume = has_finally && !has_catch_all;
        if needs_resume {
            // Alloca a flag: 0 = normal, non-zero = exception
            let flag = self.next_tmp();
            // We need the flag BEFORE the TRY body, so emit it in the preamble.
            // Actually, use the landingpad value: if we arrive from landingpad,
            // the lp value is non-null. Use a separate entry for each path.
            let fin_normal = self.next_label("fin.normal");
            let fin_exc = self.next_label("fin.exc");

            // Normal path → FINALLY → end
            // The branches to finally_label from the TRY body need to go to fin_normal
            // But we already emitted those branches. Use the finally_label for both
            // and distinguish via phi.

            // Simpler: emit two copies of FINALLY — one for normal, one for exc
            self.emitln(&format!("{}:", finally_label));
            if let Some(fin_body) = finally_body {
                for stmt in fin_body {
                    self.gen_statement(stmt);
                }
            }
            self.emitln(&format!("  br label %{}", end_label));

            // Exception cleanup path: runs FINALLY then resumes
            // The unmatched handler path (after all catch checks fail) goes here
            self.emitln(&format!("{}.resume:", finally_label));
            if let Some(fin_body) = finally_body {
                for stmt in fin_body {
                    self.gen_statement(stmt);
                }
            }
            // Re-raise after FINALLY: invoke _Unwind_Resume with the
            // outer TRY's landing pad as unwind destination. This makes
            // the LSDA emit a call site entry at this IP, so the unwinder
            // finds the outer handler when it re-enters this frame.
            if !self.declared_fns.contains("_Unwind_Resume") {
                self.emit_preambleln("declare void @_Unwind_Resume(ptr)");
                self.declared_fns.insert("_Unwind_Resume".to_string());
            }
            if let Some(ref outer_unwind) = saved_unwind {
                // Nested TRY: invoke so outer landing pad catches
                let cont = self.next_label("reraise.unr");
                self.emitln(&format!(
                    "  invoke void @_Unwind_Resume(ptr {}) to label %{} unwind label %{}",
                    exc_ptr, cont, outer_unwind));
                self.emitln(&format!("{}:", cont));
                self.emitln("  unreachable");
            } else {
                // Top-level TRY: resume exits the function
                self.emitln(&format!("  resume {{ ptr, i32 }} {}", lp));
            }
        } else {
            self.emitln(&format!("{}:", finally_label));
            if let Some(fin_body) = finally_body {
                for stmt in fin_body {
                    self.gen_statement(stmt);
                }
            }
            self.emitln(&format!("  br label %{}", end_label));
        }

        self.emitln(&format!("{}:", end_label));
    }

    fn gen_try_sjlj(&mut self, body: &[Statement],
                     excepts: &[crate::ast::ExceptClause],
                     finally_body: &Option<Vec<Statement>>) {
        self.declare_exc_runtime();

        // Allocate exception frame (m2_ExcFrame = { [37 x i32], ptr, i32, ptr, ptr })
        // sizeof(jmp_buf) varies; use an opaque alloca large enough
        let frame = self.next_tmp();
        self.emitln(&format!("  {} = alloca [256 x i8]", frame));

        // Flag to track if exception occurred (for FINALLY re-raise)
        let exc_flag = self.next_tmp();
        self.emitln(&format!("  {} = alloca i32", exc_flag));
        self.emitln(&format!("  store i32 0, ptr {}", exc_flag));

        // Push frame
        self.emitln(&format!("  call void @m2_exc_push(ptr {})", frame));

        // setjmp returns 0 on first call, non-zero on longjmp
        let sjret = self.next_tmp();
        self.emitln(&format!("  {} = call i32 @setjmp(ptr {})", sjret, frame));
        let caught = self.next_tmp();
        self.emitln(&format!("  {} = icmp ne i32 {}, 0", caught, sjret));

        let try_body_label = self.next_label("try.body");
        let handler_label = self.next_label("try.handler");
        let finally_label = self.next_label("try.finally");
        let end_label = self.next_label("try.end");

        self.emitln(&format!("  br i1 {}, label %{}, label %{}", caught, handler_label, try_body_label));

        // TRY body
        self.emitln(&format!("{}:", try_body_label));
        self.try_entry_label.push(try_body_label.clone());
        for stmt in body {
            self.gen_statement(stmt);
        }
        // Pop frame after successful body
        self.emitln(&format!("  call void @m2_exc_pop(ptr {})", frame));
        self.emitln(&format!("  br label %{}", finally_label));
        self.try_entry_label.pop();

        // Handler
        self.emitln(&format!("{}:", handler_label));
        self.emitln(&format!("  call void @m2_exc_pop(ptr {})", frame));

        // Track whether we need to re-raise after FINALLY
        let needs_reraise = excepts.is_empty() && finally_body.is_some();

        if excepts.is_empty() {
            // No handlers — set exception flag, go to finally, then re-raise
            self.emitln(&format!("  store i32 1, ptr {}", exc_flag));
            self.emitln(&format!("  br label %{}", finally_label));
        } else {
            // Get exception_id from frame
            let exc_id = self.next_tmp();
            self.emitln(&format!("  {} = call i32 @m2_exc_get_id(ptr {})", exc_id, frame));

            let mut has_catch_all = false;
            for (i, ec) in excepts.iter().enumerate() {
                if let Some(ref exc_name) = ec.exception {
                    // Typed catch — compare exception_id
                    let expected_name = if let Some(ref m) = exc_name.module {
                        format!("{}_{}", m, exc_name.name)
                    } else {
                        self.mangle(&exc_name.name)
                    };
                    let expected_val = self.const_values.get(&expected_name)
                        .or_else(|| self.const_values.get(&exc_name.name))
                        .copied()
                        .unwrap_or(i as i64 + 100);
                    let cmp = self.next_tmp();
                    self.emitln(&format!("  {} = icmp eq i32 {}, {}", cmp, exc_id, expected_val));
                    let match_label = self.next_label("exc.match");
                    let next_label = self.next_label("exc.next");
                    self.emitln(&format!("  br i1 {}, label %{}, label %{}",
                        cmp, match_label, next_label));

                    self.emitln(&format!("{}:", match_label));
                    for stmt in &ec.body {
                        self.gen_statement(stmt);
                    }
                    self.emitln(&format!("  br label %{}", finally_label));

                    self.emitln(&format!("{}:", next_label));
                } else {
                    // Catch-all
                    has_catch_all = true;
                    for stmt in &ec.body {
                        self.gen_statement(stmt);
                    }
                    self.emitln(&format!("  br label %{}", finally_label));
                }
            }
            if !has_catch_all {
                // No matching handler — re-raise
                self.emitln(&format!("  call void @m2_exc_reraise(ptr {})", frame));
                self.emitln("  unreachable");
            }
        }

        // FINALLY
        self.emitln(&format!("{}:", finally_label));
        if let Some(fin_body) = finally_body {
            for stmt in fin_body {
                self.gen_statement(stmt);
            }
        }
        // After FINALLY: if exception was unhandled, re-raise
        if needs_reraise {
            let flag_val = self.next_tmp();
            self.emitln(&format!("  {} = load i32, ptr {}", flag_val, exc_flag));
            let is_exc = self.next_tmp();
            self.emitln(&format!("  {} = icmp ne i32 {}, 0", is_exc, flag_val));
            let reraise_label = self.next_label("try.reraise");
            self.emitln(&format!("  br i1 {}, label %{}, label %{}",
                is_exc, reraise_label, end_label));
            self.emitln(&format!("{}:", reraise_label));
            self.emitln(&format!("  call void @m2_exc_reraise(ptr {})", frame));
            self.emitln("  unreachable");
        } else {
            self.emitln(&format!("  br label %{}", end_label));
        }

        self.emitln(&format!("{}:", end_label));
    }

    fn gen_typecase(&mut self, expr: &crate::ast::Expr,
                    branches: &[crate::ast::TypeCaseBranch],
                    else_body: &Option<Vec<Statement>>) {
        self.declare_rtti_runtime();
        let val = self.gen_expr(expr);
        let end_label = self.next_label("typecase.end");

        for (i, branch) in branches.iter().enumerate() {
            let match_label = self.next_label("typecase.match");
            let next_label = if i + 1 < branches.len() {
                self.next_label("typecase.next")
            } else if else_body.is_some() {
                self.next_label("typecase.else")
            } else {
                end_label.clone()
            };

            if !branch.types.is_empty() {
                // Build OR of M2_ISA checks for each type in the branch
                let type_name = if let Some(ref m) = branch.types[0].module {
                    format!("{}_{}", m, branch.types[0].name)
                } else {
                    self.mangle(&branch.types[0].name)
                };

                // Look up the type descriptor global
                let td_sym = self.ref_type_descs.get(&type_name).cloned()
                    .unwrap_or_else(|| format!("@M2_TD_{}", type_name));

                let isa_result = self.next_tmp();
                self.emitln(&format!("  {} = call i32 @M2_ISA(ptr {}, ptr {})",
                    isa_result, val.name, td_sym));
                let is_match = self.next_tmp();
                self.emitln(&format!("  {} = icmp ne i32 {}, 0", is_match, isa_result));
                self.emitln(&format!("  br i1 {}, label %{}, label %{}",
                    is_match, match_label, next_label));
            } else {
                self.emitln(&format!("  br label %{}", match_label));
            }

            self.emitln(&format!("{}:", match_label));

            // Variable binding: cast REFANY to the specific type
            if let Some(ref var_name) = branch.var {
                if let Some(first_type) = branch.types.first() {
                    // Create a local variable with the cast pointer
                    let alloca = self.next_tmp();
                    self.emitln(&format!("  {} = alloca ptr", alloca));
                    self.emitln(&format!("  store ptr {}, ptr {}", val.name, alloca));
                    self.locals.last_mut().unwrap().insert(
                        var_name.clone(), (alloca, "ptr".to_string()));
                }
            }

            for stmt in &branch.body {
                self.gen_statement(stmt);
            }
            self.emitln(&format!("  br label %{}", end_label));

            if next_label != end_label {
                self.emitln(&format!("{}:", next_label));
            }
        }

        if let Some(else_stmts) = else_body {
            for stmt in else_stmts {
                self.gen_statement(stmt);
            }
            self.emitln(&format!("  br label %{}", end_label));
        }

        self.emitln(&format!("{}:", end_label));
    }
}
