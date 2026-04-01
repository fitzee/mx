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
                if addr.ty.starts_with('{') || addr.ty.starts_with('[')
                {
                    // If val is a by-value aggregate or scalar (not a pointer), spill to alloca first
                    let src = if val.ty != "ptr" {
                        let spill_ty = if val.ty.starts_with('{') || val.ty.starts_with('[') {
                            val.ty.clone()
                        } else {
                            // Scalar assigned to aggregate (e.g. single-char string "0" → ARRAY)
                            // Spill using the target's aggregate type
                            addr.ty.clone()
                        };
                        let tmp_alloca = self.next_tmp();
                        self.emitln(&format!("  {} = alloca {}", tmp_alloca, spill_ty));
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
                    if s == "void" || s == "i32" && addr.ty == "i8" {
                        addr.ty.clone()
                    } else {
                        s
                    }
                } else {
                    addr.ty.clone()
                };

                let coerced = self.coerce_val(&val, &store_ty);
                self.emitln(&format!("  store {} {}, ptr {}", store_ty, coerced.name, addr.name));
            }

            HirStmtKind::ProcCall { target, args } => {
                // HIR has already expanded open arrays and marked VAR with AddrOf
                let mut arg_str = self.expand_hir_call_args(args);

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
                                if let Some(arg) = args.first() {
                                    // NEW arg may be Place or AddrOf(Place) depending on HIR lowering
                                    let place_ref = match &arg.kind {
                                        HirExprKind::Place(ref p) => Some(p),
                                        HirExprKind::AddrOf(ref p) => Some(p),
                                        _ => None,
                                    };
                                    if let Some(place) = place_ref {
                                        let addr = self.emit_place_addr(place);
                                        let pointee_ty = self.tl_type_str(place.ty);
                                        let size = self.emit_sizeof(&pointee_ty);
                                        // Check if the variable's pointer type has a type descriptor
                                        let td_name = self.sema.symtab.find_type_by_id(place.ty)
                                            .and_then(|tn| {
                                                self.ref_type_descs.get(&tn).cloned()
                                                    .or_else(|| {
                                                        // Try module-prefixed key
                                                        let prefixed = format!("{}_{}", self.module_name, tn);
                                                        self.ref_type_descs.get(&prefixed).cloned()
                                                    })
                                            })
                                            .or_else(|| {
                                                // Try all TypeIds in the alias chain
                                                let mut tid = place.ty;
                                                for _ in 0..20 {
                                                    if let Some(tn) = self.sema.symtab.find_type_by_id(tid) {
                                                        if let Some(td) = self.ref_type_descs.get(&tn) {
                                                            return Some(td.clone());
                                                        }
                                                    }
                                                    match self.sema.types.get(tid) {
                                                        crate::types::Type::Alias { target, .. } => tid = *target,
                                                        _ => break,
                                                    }
                                                }
                                                None
                                            });

                                        if let Some(td) = td_name {
                                            // REF type: use M2_ref_alloc
                                            if !self.declared_fns.contains("M2_ref_alloc") {
                                                self.emit_preambleln("declare ptr @M2_ref_alloc(i64, ptr)");
                                                self.declared_fns.insert("M2_ref_alloc".to_string());
                                            }
                                            let tmp = self.next_tmp();
                                            let td_ref = if td.starts_with('@') { td.clone() } else { format!("@{}", td) };
                                            self.emitln(&format!("  {} = call ptr @M2_ref_alloc(i64 {}, ptr {})", tmp, size, td_ref));
                                            self.emitln(&format!("  store ptr {}, ptr {}", tmp, addr.name));
                                        } else {
                                            // Regular pointer: plain malloc
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
                        let call_name = self.fn_name_map.get(&sid.mangled)
                            .cloned().unwrap_or_else(|| sid.mangled.clone());
                        // Coerce arg types to match declared param types
                        if let Some(params) = self.proc_params.get(&call_name)
                            .or_else(|| self.proc_params.get(&sid.source_name)).cloned()
                        {
                            for (i, param) in params.iter().enumerate() {
                                if i < arg_str.len() && arg_str[i].starts_with("double ") && param.llvm_type == "float" {
                                    let val_name = arg_str[i].strip_prefix("double ").unwrap().to_string();
                                    let tmp = self.next_tmp();
                                    self.emitln(&format!("  {} = fptrunc double {} to float", tmp, val_name));
                                    arg_str[i] = format!("float {}", tmp);
                                }
                            }
                        }
                        self.emitln(&format!("  call void @{}({})",
                            call_name, arg_str.join(", ")));
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

            // Control flow statements are handled by CFG terminators.
            // These should never appear in CFG block statements.
            HirStmtKind::If { .. } | HirStmtKind::While { .. } | HirStmtKind::Repeat { .. }
            | HirStmtKind::For { .. } | HirStmtKind::Loop { .. } | HirStmtKind::Case { .. }
            | HirStmtKind::Return { .. } | HirStmtKind::Exit
            | HirStmtKind::Raise { .. } | HirStmtKind::Retry
            | HirStmtKind::Try { .. } | HirStmtKind::Lock { .. }
            | HirStmtKind::TypeCase { .. } => {
                panic!("unexpected structured control flow in CFG block: {:?}", std::mem::discriminant(&stmt.kind));
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
