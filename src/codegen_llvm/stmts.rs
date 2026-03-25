use super::*;


impl LLVMCodeGen {
    pub(crate) fn declare_exc_runtime(&mut self) {
        if !self.declared_fns.contains("m2_eh_throw") {
            self.emit_preambleln("declare void @m2_eh_throw(i32, ptr)");
            self.emit_preambleln("declare i32 @m2_eh_get_id(ptr)");
            self.declared_fns.insert("m2_eh_throw".to_string());
            self.declared_fns.insert("m2_eh_get_id".to_string());
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

    pub(crate) fn gen_hir_statement(&mut self, stmt: &crate::hir::HirStmt) {
        use crate::hir::*;

        // Update stack frame line number
        if stmt.loc.line > 0 {
            if let Some(frame) = self.stack_frame_alloca.clone() {
                let line_ptr = self.next_tmp();
                self.emitln(&format!("  {} = getelementptr inbounds %m2_StackFrame, ptr {}, i32 0, i32 3",
                    line_ptr, frame));
                self.emitln(&format!("  store i32 {}, ptr {}", stmt.loc.line, line_ptr));
            }
        }

        match &stmt.kind {
            HirStmtKind::Empty => {}

            HirStmtKind::Assign { target, value } => {
                let addr = self.emit_place_addr(target);
                let val = self.gen_hir_expr(value);

                // Aggregate assignment: memcpy
                if (addr.ty.starts_with('{') || addr.ty.starts_with('['))
                    && !addr.ty.contains("float") && !addr.ty.contains("double")
                {
                    // If val is a by-value struct (e.g. from a function call), spill to alloca first
                    let src = if val.ty.starts_with('{') || val.ty.starts_with('[') {
                        let tmp_alloca = self.next_tmp();
                        self.emitln(&format!("  {} = alloca {}", tmp_alloca, val.ty));
                        self.emitln(&format!("  store {} {}, ptr {}", val.ty, val.name, tmp_alloca));
                        tmp_alloca
                    } else {
                        val.name.clone()
                    };
                    self.emit_struct_memcpy(&addr.name, &src, &addr.ty);
                    return;
                }

                let store_ty = if let Some(tid) = addr.type_id {
                    let s = self.tl_type_str(tid);
                    if s == "void" { addr.ty.clone() } else { s }
                } else {
                    addr.ty.clone()
                };

                let coerced = self.coerce_val(&val, &store_ty);
                self.emitln(&format!("  store {} {}, ptr {}", store_ty, coerced.name, addr.name));
            }

            HirStmtKind::ProcCall { target, args } => {
                // HIR has already expanded open arrays and marked VAR with AddrOf
                let arg_str = self.expand_hir_call_args(args);

                match target {
                    HirCallTarget::Direct(sid) if builtins::is_builtin_proc(&sid.source_name) => {
                        // Handle builtin procedure calls (INC, DEC, INCL, EXCL, HALT, etc.)
                        match sid.source_name.as_str() {
                            "INC" | "DEC" => {
                                if let Some(arg) = args.first() {
                                    if let HirExprKind::Place(ref place) = arg.kind {
                                        let addr = self.emit_place_addr(place);
                                        let load_ty = if let Some(tid) = addr.type_id {
                                            self.tl_type_str(tid)
                                        } else { addr.ty.clone() };
                                        let cur = self.next_tmp();
                                        self.emitln(&format!("  {} = load {}, ptr {}", cur, load_ty, addr.name));
                                        let step = if args.len() > 1 {
                                            let s = self.gen_hir_expr(&args[1]);
                                            self.coerce_val(&s, &load_ty)
                                        } else {
                                            Val::new("1", load_ty.clone())
                                        };
                                        let next = self.next_tmp();
                                        let op = if sid.source_name == "INC" { "add" } else { "sub" };
                                        self.emitln(&format!("  {} = {} {} {}, {}", next, op, load_ty, cur, step.name));
                                        self.emitln(&format!("  store {} {}, ptr {}", load_ty, next, addr.name));
                                    }
                                }
                            }
                            "INCL" | "EXCL" => {
                                if args.len() >= 2 {
                                    if let HirExprKind::Place(ref place) = args[0].kind {
                                        let addr = self.emit_place_addr(place);
                                        let cur = self.next_tmp();
                                        self.emitln(&format!("  {} = load i32, ptr {}", cur, addr.name));
                                        let bit_val = self.gen_hir_expr(&args[1]);
                                        let bit = self.next_tmp();
                                        self.emitln(&format!("  {} = shl i32 1, {}", bit, bit_val.name));
                                        let next = self.next_tmp();
                                        if sid.source_name == "INCL" {
                                            self.emitln(&format!("  {} = or i32 {}, {}", next, cur, bit));
                                        } else {
                                            let inv = self.next_tmp();
                                            self.emitln(&format!("  {} = xor i32 {}, -1", inv, bit));
                                            self.emitln(&format!("  {} = and i32 {}, {}", next, cur, inv));
                                        }
                                        self.emitln(&format!("  store i32 {}, ptr {}", next, addr.name));
                                    }
                                }
                            }
                            "HALT" => {
                                self.emitln("  call void @m2_halt()");
                                self.emitln("  unreachable");
                                let dead = self.next_label("halt.dead");
                                self.emitln(&format!("{}:", dead));
                            }
                            "NEW" => {
                                // NEW(p) → p := malloc(sizeof(*p))
                                if let Some(arg) = args.first() {
                                    if let HirExprKind::Place(ref place) = arg.kind {
                                        let addr = self.emit_place_addr(place);
                                        let pointee_ty = self.tl_type_str(place.ty);
                                        let size = self.emit_sizeof(&pointee_ty);
                                        if !self.declared_fns.contains("malloc") {
                                            self.emit_preambleln("declare ptr @malloc(i64) nounwind");
                                            self.declared_fns.insert("malloc".to_string());
                                        }
                                        let tmp = self.next_tmp();
                                        self.emitln(&format!("  {} = call ptr @malloc(i64 {})", tmp, size));
                                        self.emitln(&format!("  store ptr {}, ptr {}", tmp, addr.name));
                                    }
                                }
                            }
                            "DISPOSE" => {
                                // DISPOSE(p) → free(p)
                                if let Some(arg) = args.first() {
                                    if let HirExprKind::Place(ref place) = arg.kind {
                                        let addr = self.emit_place_addr(place);
                                        let loaded = self.next_tmp();
                                        self.emitln(&format!("  {} = load ptr, ptr {}", loaded, addr.name));
                                        if !self.declared_fns.contains("free") {
                                            self.emit_preambleln("declare void @free(ptr) nounwind");
                                            self.declared_fns.insert("free".to_string());
                                        }
                                        self.emitln(&format!("  call void @free(ptr {})", loaded));
                                    }
                                }
                            }
                            _ => {
                                // Other builtins: emit as regular call
                                self.emitln(&format!("  call void @{}({})",
                                    sid.mangled, arg_str.join(", ")));
                            }
                        }
                    }
                    HirCallTarget::Direct(sid) => {
                        self.emitln(&format!("  call void @{}({})",
                            sid.mangled, arg_str.join(", ")));
                    }
                    HirCallTarget::Indirect(callee_expr) => {
                        let fn_ptr = self.gen_hir_expr(callee_expr);
                        let call_target = if fn_ptr.ty != "ptr" {
                            let tmp = self.next_tmp();
                            self.emitln(&format!("  {} = inttoptr {} {} to ptr", tmp, fn_ptr.ty, fn_ptr.name));
                            tmp
                        } else {
                            fn_ptr.name
                        };
                        self.emitln(&format!("  call void {}({})",
                            call_target, arg_str.join(", ")));
                    }
                }
            }

            HirStmtKind::If { cond, then_body, elsifs, else_body } => {
                let cond_val = self.gen_hir_expr(cond);
                let cond_i1 = self.to_i1(&cond_val);
                let then_label = self.next_label("if.then");
                let end_label = self.next_label("if.end");
                let else_label = if elsifs.is_empty() && else_body.is_none() {
                    end_label.clone()
                } else {
                    self.next_label("if.else")
                };

                self.emitln(&format!("  br i1 {}, label %{}, label %{}", cond_i1, then_label, else_label));
                self.emitln(&format!("{}:", then_label));
                for s in then_body { self.gen_hir_statement(s); }
                self.emitln(&format!("  br label %{}", end_label));

                if !elsifs.is_empty() || else_body.is_some() {
                    let mut current_else = else_label;
                    for (elsif_cond, elsif_body) in elsifs {
                        self.emitln(&format!("{}:", current_else));
                        let cv = self.gen_hir_expr(elsif_cond);
                        let ci = self.to_i1(&cv);
                        let elsif_then = self.next_label("elsif.then");
                        let next_else = self.next_label("elsif.else");
                        self.emitln(&format!("  br i1 {}, label %{}, label %{}", ci, elsif_then, next_else));
                        self.emitln(&format!("{}:", elsif_then));
                        for s in elsif_body { self.gen_hir_statement(s); }
                        self.emitln(&format!("  br label %{}", end_label));
                        current_else = next_else;
                    }
                    self.emitln(&format!("{}:", current_else));
                    if let Some(eb) = else_body {
                        for s in eb { self.gen_hir_statement(s); }
                    }
                    self.emitln(&format!("  br label %{}", end_label));
                }

                self.emitln(&format!("{}:", end_label));
            }

            HirStmtKind::While { cond, body } => {
                let cond_label = self.next_label("while.cond");
                let body_label = self.next_label("while.body");
                let end_label = self.next_label("while.end");

                self.emitln(&format!("  br label %{}", cond_label));
                self.emitln(&format!("{}:", cond_label));
                let cv = self.gen_hir_expr(cond);
                let ci = self.to_i1(&cv);
                self.emitln(&format!("  br i1 {}, label %{}, label %{}", ci, body_label, end_label));
                self.emitln(&format!("{}:", body_label));
                self.loop_exit_stack.push(end_label.clone());
                for s in body { self.gen_hir_statement(s); }
                self.loop_exit_stack.pop();
                self.emitln(&format!("  br label %{}", cond_label));
                self.emitln(&format!("{}:", end_label));
            }

            HirStmtKind::Repeat { body, cond } => {
                let body_label = self.next_label("repeat.body");
                let end_label = self.next_label("repeat.end");

                self.emitln(&format!("  br label %{}", body_label));
                self.emitln(&format!("{}:", body_label));
                self.loop_exit_stack.push(end_label.clone());
                for s in body { self.gen_hir_statement(s); }
                self.loop_exit_stack.pop();
                let cv = self.gen_hir_expr(cond);
                let ci = self.to_i1(&cv);
                self.emitln(&format!("  br i1 {}, label %{}, label %{}", ci, end_label, body_label));
                self.emitln(&format!("{}:", end_label));
            }

            HirStmtKind::For { var, var_ty, start, end, step, direction, body } => {
                // Look up the FOR variable directly in locals/globals
                let (var_addr_name, llvm_ty) = if let Some((addr, ty)) = self.lookup_local(var) {
                    (addr.clone(), ty.clone())
                } else if let Some((addr, ty)) = self.globals.get(var).or_else(|| self.globals.get(&self.mangle(var))) {
                    (addr.clone(), ty.clone())
                } else {
                    (format!("@{}", self.mangle(var)), self.tl_type_str(*var_ty))
                };
                let var_addr = Val::with_tid(var_addr_name, llvm_ty.clone(), *var_ty);

                let start_val = self.gen_hir_expr(start);
                let end_val = self.gen_hir_expr(end);
                let start_coerced = self.coerce_val(&start_val, &llvm_ty);
                let end_coerced = self.coerce_val(&end_val, &llvm_ty);

                let step_val = if let Some(s) = step {
                    self.gen_hir_expr(s)
                } else {
                    Val::new("1", llvm_ty.clone())
                };
                let step_coerced = self.coerce_val(&step_val, &llvm_ty);

                let is_down = *direction == ForDirection::Down;
                let is_unsigned = crate::types::is_unsigned_type(&self.sema.types, *var_ty);

                let preheader = self.next_label("for.ph");
                let header = self.next_label("for.header");
                let latch = self.next_label("for.latch");
                let exit = self.next_label("for.exit");

                // Skip guard
                let skip = self.next_tmp();
                if is_down {
                    let cmp = if is_unsigned { "icmp ult" } else { "icmp slt" };
                    self.emitln(&format!("  {} = {} {} {}, {}", skip, cmp, llvm_ty, start_coerced.name, end_coerced.name));
                } else {
                    let cmp = if is_unsigned { "icmp ugt" } else { "icmp sgt" };
                    self.emitln(&format!("  {} = {} {} {}, {}", skip, cmp, llvm_ty, start_coerced.name, end_coerced.name));
                }
                self.emitln(&format!("  br i1 {}, label %{}, label %{}", skip, exit, preheader));

                self.emitln(&format!("{}:", preheader));
                self.emitln(&format!("  store {} {}, ptr {}", llvm_ty, start_coerced.name, var_addr.name));
                self.emitln(&format!("  br label %{}", header));

                self.emitln(&format!("{}:", header));
                self.loop_exit_stack.push(exit.clone());
                for s in body { self.gen_hir_statement(s); }
                self.loop_exit_stack.pop();
                self.emitln(&format!("  br label %{}", latch));

                self.emitln(&format!("{}:", latch));
                let cur = self.next_tmp();
                self.emitln(&format!("  {} = load {}, ptr {}", cur, llvm_ty, var_addr.name));
                let next = self.next_tmp();
                self.emitln(&format!("  {} = add nsw {} {}, {}", next, llvm_ty, cur, step_coerced.name));
                self.emitln(&format!("  store {} {}, ptr {}", llvm_ty, next, var_addr.name));
                let cont = self.next_tmp();
                if is_down {
                    let cmp = if is_unsigned { "icmp uge" } else { "icmp sge" };
                    self.emitln(&format!("  {} = {} {} {}, {}", cont, cmp, llvm_ty, next, end_coerced.name));
                } else {
                    let cmp = if is_unsigned { "icmp ule" } else { "icmp sle" };
                    self.emitln(&format!("  {} = {} {} {}, {}", cont, cmp, llvm_ty, next, end_coerced.name));
                }
                self.emitln(&format!("  br i1 {}, label %{}, label %{}", cont, header, exit));
                self.emitln(&format!("{}:", exit));
            }

            HirStmtKind::Loop { body } => {
                let body_label = self.next_label("loop.body");
                let end_label = self.next_label("loop.end");
                self.emitln(&format!("  br label %{}", body_label));
                self.emitln(&format!("{}:", body_label));
                self.loop_exit_stack.push(end_label.clone());
                for s in body { self.gen_hir_statement(s); }
                self.loop_exit_stack.pop();
                self.emitln(&format!("  br label %{}", body_label));
                self.emitln(&format!("{}:", end_label));
            }

            HirStmtKind::Exit => {
                if let Some(exit_label) = self.loop_exit_stack.last().cloned() {
                    self.emitln(&format!("  br label %{}", exit_label));
                    let dead = self.next_label("exit.dead");
                    self.emitln(&format!("{}:", dead));
                }
            }

            HirStmtKind::Return { expr } => {
                if let Some(ref frame) = self.stack_frame_alloca.clone() {
                    self.emitln(&format!("  call void @m2_stack_pop(ptr {})", frame));
                }
                if let Some(e) = expr {
                    let val = self.gen_hir_expr(e);
                    let ret_ty = self.current_return_type.clone().unwrap_or_else(|| "void".to_string());
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
                let dead = self.next_label("ret.dead");
                self.emitln(&format!("{}:", dead));
            }

            HirStmtKind::Case { expr, branches, else_body } => {
                // Simple implementation: chain of if-else comparisons
                let switch_val = self.gen_hir_expr(expr);
                let end_label = self.next_label("case.end");
                let mut next_label = self.next_label("case.test");
                self.emitln(&format!("  br label %{}", next_label));

                for branch in branches {
                    self.emitln(&format!("{}:", next_label));
                    let body_label = self.next_label("case.body");
                    next_label = self.next_label("case.test");

                    // Test labels: OR chain
                    let mut match_val = None;
                    for label in &branch.labels {
                        let test = match label {
                            HirCaseLabel::Single(e) => {
                                let lv = self.gen_hir_expr(e);
                                let lv_coerced = self.coerce_val(&lv, &switch_val.ty);
                                let cmp = self.next_tmp();
                                self.emitln(&format!("  {} = icmp eq {} {}, {}", cmp, switch_val.ty, switch_val.name, lv_coerced.name));
                                cmp
                            }
                            HirCaseLabel::Range(lo, hi) => {
                                let lo_v = self.gen_hir_expr(lo);
                                let hi_v = self.gen_hir_expr(hi);
                                let lo_c = self.coerce_val(&lo_v, &switch_val.ty);
                                let hi_c = self.coerce_val(&hi_v, &switch_val.ty);
                                let ge = self.next_tmp();
                                self.emitln(&format!("  {} = icmp sge {} {}, {}", ge, switch_val.ty, switch_val.name, lo_c.name));
                                let le = self.next_tmp();
                                self.emitln(&format!("  {} = icmp sle {} {}, {}", le, switch_val.ty, switch_val.name, hi_c.name));
                                let both = self.next_tmp();
                                self.emitln(&format!("  {} = and i1 {}, {}", both, ge, le));
                                both
                            }
                        };
                        match_val = Some(if let Some(prev) = match_val {
                            let combined = self.next_tmp();
                            self.emitln(&format!("  {} = or i1 {}, {}", combined, prev, test));
                            combined
                        } else {
                            test
                        });
                    }
                    if let Some(mv) = match_val {
                        self.emitln(&format!("  br i1 {}, label %{}, label %{}", mv, body_label, next_label));
                    }
                    self.emitln(&format!("{}:", body_label));
                    for s in &branch.body { self.gen_hir_statement(s); }
                    self.emitln(&format!("  br label %{}", end_label));
                }

                self.emitln(&format!("{}:", next_label));
                if let Some(eb) = else_body {
                    for s in eb { self.gen_hir_statement(s); }
                }
                self.emitln(&format!("  br label %{}", end_label));
                self.emitln(&format!("{}:", end_label));
            }

            HirStmtKind::Raise { .. } | HirStmtKind::Retry |
            HirStmtKind::Try { .. } | HirStmtKind::Lock { .. } |
            HirStmtKind::TypeCase { .. } => {
                // M2+ features: delegate to AST-based handlers for now
                // TODO: implement HIR-native M2+ statement generation
                panic!("HIR statement: M2+ features (TRY/LOCK/TYPECASE/RAISE/RETRY) \
                       not yet implemented in HIR statement path");
            }
        }
    }

    /// Generate HIR statements for a statement list.
    pub(crate) fn gen_hir_statements(&mut self, stmts: &[crate::hir::HirStmt]) {
        for stmt in stmts {
            self.gen_hir_statement(stmt);
        }
    }
}
