use super::*;

impl LLVMCodeGen {
    // ── HIR-based declaration generation ────────────────────────────
    // All AST-based methods (gen_type_decls, gen_const_decls, gen_var_decls_global,
    // gen_exception_decls, gen_proc_decl, gen_var_decl_local) removed in Step 3.

    /// Emit constant declarations from prebuilt HirModule.
    pub(crate) fn gen_hir_const_decls(&mut self) {
        let consts = if let Some(ref hir) = self.prebuilt_hir {
            hir.const_decls.clone()
        } else {
            return;
        };
        self.gen_hir_const_decls_from(&consts);
    }

    /// Emit exception declarations from prebuilt HirModule.
    pub(crate) fn gen_hir_exception_decls(&mut self) {
        let exceptions = if let Some(ref hir) = self.prebuilt_hir {
            hir.exception_decls.clone()
        } else {
            return;
        };
        self.gen_hir_exception_decls_from(&exceptions);
    }

    // ── HIR-based emission methods (Step 2: LLVM decoupling) ───────

    /// Emit type declarations from HIR type decls (no AST dependency).
    pub(crate) fn gen_hir_type_decls_from(&mut self, type_decls: &[crate::hir::HirTypeDecl]) {
        use crate::types::Type;
        for td in type_decls {
            let resolved_id = self.resolve_alias_id(td.type_id);
            let llvm_ty = self.llvm_type_for_type_id(resolved_id);
            // Sanitize: "void" in struct/array positions means unresolved → replace with i32
            let llvm_ty = if llvm_ty.contains("void") && llvm_ty != "void" {
                llvm_ty.replace("void", "i32")
            } else {
                llvm_ty
            };
            self.type_map.insert(td.name.clone(), llvm_ty.clone());

            // Track record fields
            match self.sema.types.get(resolved_id) {
                Type::Record { fields, .. } => {
                    let mut field_list: Vec<(String, String, usize)> = Vec::new();
                    let mut idx = 0;
                    for f in fields {
                        let ft = self.llvm_type_for_type_id(f.typ);
                        field_list.push((f.name.clone(), ft, idx));
                        idx += 1;
                    }
                    self.record_fields.insert(td.name.clone(), field_list);
                }
                Type::Array { elem_type, low, high, .. } => {
                    let elem_ty = self.llvm_type_for_type_id(*elem_type);
                    let is_char_array = matches!(self.sema.types.get(*elem_type), Type::Char);
                    let size = (*high - *low + 1) as usize;
                    self.array_types.insert(td.name.clone(), (elem_ty, size));
                    let _ = is_char_array; // may be useful later
                }
                Type::Pointer { base } => {
                    // Track pointer target type name
                    let base_resolved = self.resolve_alias_id(*base);
                    let base_name = self.type_name_for_id(base_resolved);
                    if !base_name.is_empty() {
                        self.pointer_target_types.insert(td.name.clone(), base_name);
                    }
                    // Anonymous inline records pointed to
                    if let Type::Record { fields, .. } = self.sema.types.get(base_resolved) {
                        let record_name = format!("__anon_record_{}", self.anon_record_counter);
                        self.anon_record_counter += 1;
                        let record_ty = self.llvm_type_for_type_id(base_resolved);
                        self.type_map.insert(record_name.clone(), record_ty);
                        let mut field_list = Vec::new();
                        for (idx, f) in fields.iter().enumerate() {
                            let ft = self.llvm_type_for_type_id(f.typ);
                            field_list.push((f.name.clone(), ft, idx));
                        }
                        self.record_fields.insert(record_name.clone(), field_list);
                        self.pointer_target_types.insert(td.name.clone(), record_name);
                    }
                }
                Type::Alias { target, .. } => {
                    // Propagate pointer_target_types through aliases
                    let alias_name = self.type_name_for_id(*target);
                    if let Some(target_rec) = self.pointer_target_types.get(&alias_name).cloned() {
                        self.pointer_target_types.insert(td.name.clone(), target_rec);
                    }
                }
                Type::Enumeration { variants, .. } => {
                    for (i, v) in variants.iter().enumerate() {
                        self.enum_variants.insert(v.clone(), i as i64);
                        self.enum_variants.insert(format!("{}_{}", td.name, v), i as i64);
                    }
                }
                _ => {}
            }

            // Register RTTI type descriptors for REF/OBJECT types (M2+)
            if self.m2plus {
                match self.sema.types.get(resolved_id) {
                    Type::Ref { .. } => {
                        let mangled = self.mangle(&td.name);
                        self.register_type_desc(&mangled, None);
                    }
                    Type::Object { parent, .. } => {
                        let mangled = self.mangle(&td.name);
                        let parent_name = parent.as_ref().map(|pid| self.type_name_for_id(*pid));
                        let parent_mangled = parent_name.as_ref().map(|n| self.mangle(n));
                        self.register_type_desc(&mangled, parent_mangled.as_deref());
                    }
                    _ => {}
                }
            }
        }
    }

    /// Emit constant declarations from a list of HIR const decls.
    pub(crate) fn gen_hir_const_decls_from(&mut self, consts: &[crate::hir::HirConstDecl]) {
        use crate::hir::ConstVal;
        for c in consts {
            match &c.value {
                ConstVal::Integer(v) | ConstVal::EnumVariant(v) => {
                    self.const_values.insert(c.name.clone(), *v);
                    self.const_values.insert(self.mangle(&c.name), *v);
                }
                ConstVal::Boolean(b) => {
                    let v = if *b { 1i64 } else { 0 };
                    self.const_values.insert(c.name.clone(), v);
                    self.const_values.insert(self.mangle(&c.name), v);
                }
                ConstVal::Char(ch) => {
                    self.const_values.insert(c.name.clone(), *ch as i64);
                    self.const_values.insert(self.mangle(&c.name), *ch as i64);
                }
                ConstVal::String(s) => {
                    if s.len() == 1 {
                        // Single-char string constant: treat as CHAR value, not string pointer
                        self.const_values.insert(c.name.clone(), s.as_bytes()[0] as i64);
                        self.const_values.insert(self.mangle(&c.name), s.as_bytes()[0] as i64);
                    } else {
                        let (str_name, _) = self.intern_string(s);
                        let mut mangled = self.mangle(&c.name);
                        if self.globals.contains_key(&mangled) {
                            self.anon_record_counter += 1;
                            mangled = format!("{}_{}", mangled, self.anon_record_counter);
                        }
                        let global_name = format!("@{}", mangled);
                        self.emit_preambleln(&format!("{} = global ptr {}", global_name, str_name));
                        self.globals.insert(c.name.clone(), (global_name.clone(), "ptr".to_string()));
                        self.globals.insert(mangled, (global_name, "ptr".to_string()));
                        self.string_const_lengths.insert(c.name.clone(), s.len());
                    }
                }
                ConstVal::Real(v) => {
                    let mut mangled = self.mangle(&c.name);
                    if self.globals.contains_key(&mangled) {
                        self.anon_record_counter += 1;
                        mangled = format!("{}_{}", mangled, self.anon_record_counter);
                    }
                    let global_name = format!("@{}", mangled);
                    let bits = v.to_bits();
                    self.emit_preambleln(&format!("{} = global double 0x{:016X}", global_name, bits));
                    self.globals.insert(c.name.clone(), (global_name.clone(), "double".to_string()));
                    self.globals.insert(mangled, (global_name, "double".to_string()));
                }
                ConstVal::Set(bits) => {
                    self.const_values.insert(c.name.clone(), *bits as i64);
                    self.const_values.insert(self.mangle(&c.name), *bits as i64);
                }
                ConstVal::Nil => {
                    self.const_values.insert(c.name.clone(), 0);
                    self.const_values.insert(self.mangle(&c.name), 0);
                }
            }
        }
    }

    /// Emit exception declarations from a list of HIR exception decls.
    pub(crate) fn gen_hir_exception_decls_from(&mut self, exceptions: &[crate::hir::HirExceptionDecl]) {
        for e in exceptions {
            self.const_values.insert(e.name.clone(), e.exc_id);
            self.const_values.insert(e.mangled.clone(), e.exc_id);
            let global_name = format!("@{}", e.mangled);
            self.emit_preambleln(&format!("{} = global i32 {}", global_name, e.exc_id));
            self.globals.insert(e.name.clone(), (global_name.clone(), "i32".to_string()));
            self.globals.insert(e.mangled.clone(), (global_name, "i32".to_string()));
        }
    }

    /// Emit global variable declarations from HIR global decls (no AST dependency).
    pub(crate) fn gen_hir_var_decls_global_from(&mut self, global_decls: &[crate::hir::HirGlobalDecl]) {
        use crate::types::Type;
        for g in global_decls {
            let resolved_id = self.resolve_alias_id(g.type_id);
            let llvm_ty = self.llvm_type_for_type_id(resolved_id);
            let is_array = matches!(self.sema.types.get(resolved_id), Type::Array { .. });
            let is_char_array = if let Type::Array { elem_type, .. } = self.sema.types.get(resolved_id) {
                matches!(self.sema.types.get(*elem_type), Type::Char)
            } else {
                false
            };
            let m2_type_name = self.type_name_for_id(g.type_id);

            // Handle inline POINTER TO RECORD
            if let Type::Pointer { base } = self.sema.types.get(resolved_id) {
                let base_resolved = self.resolve_alias_id(*base);
                if let Type::Record { fields, .. } = self.sema.types.get(base_resolved) {
                    let record_name = format!("__anon_record_{}", self.anon_record_counter);
                    self.anon_record_counter += 1;
                    let record_ty = self.llvm_type_for_type_id(base_resolved);
                    self.type_map.insert(record_name.clone(), record_ty);
                    let mut field_list = Vec::new();
                    for (idx, f) in fields.iter().enumerate() {
                        let ft = self.llvm_type_for_type_id(f.typ);
                        field_list.push((f.name.clone(), ft, idx));
                    }
                    self.record_fields.insert(record_name.clone(), field_list);
                    self.var_type_names.insert(g.name.clone(), record_name);
                    self.var_types.insert(g.name.clone(), g.type_id);
                }
            }

            // Track array element type name
            if let Type::Array { elem_type, .. } = self.sema.types.get(resolved_id) {
                let elem_name = self.type_name_for_id(*elem_type);
                if !elem_name.is_empty() {
                    self.array_elem_type_names.insert(g.name.clone(), elem_name);
                }
            }

            let mangled_name = self.mangle(&g.name);
            if self.globals.contains_key(&mangled_name) {
                continue;
            }
            let global_name = format!("@{}", mangled_name);
            let zero = self.llvm_zero_initializer(&llvm_ty);
            self.emit_preambleln(&format!("{} = global {} {}", global_name, llvm_ty, zero));
            self.globals.insert(g.name.clone(), (global_name.clone(), llvm_ty.clone()));
            self.globals.insert(mangled_name, (global_name, llvm_ty.clone()));
            if is_array {
                self.array_vars.insert(g.name.clone());
            }
            if is_char_array {
                self.char_array_vars.insert(g.name.clone());
            }
            if !m2_type_name.is_empty() {
                self.var_type_names.insert(g.name.clone(), m2_type_name);
            }
            self.var_types.insert(g.name.clone(), g.type_id);
        }
    }

    /// Emit a procedure declaration from an HIR proc decl (no AST dependency).
    pub(crate) fn gen_hir_proc_decl(&mut self, proc: &crate::hir::HirProcDecl) -> CompileResult<()> {
        use crate::types::Type;

        let proc_name = if self.parent_proc_stack.is_empty() {
            self.mangle(&proc.sig.name)
        } else {
            let parent = self.parent_proc_stack.last().unwrap();
            format!("{}_{}", parent, proc.sig.name)
        };

        self.declared_fns.insert(proc_name.clone());
        self.declared_fns.insert(proc.sig.name.clone());
        let simple_mangled = self.mangle(&proc.sig.name);
        if simple_mangled != proc_name {
            self.declared_fns.insert(simple_mangled);
        }

        // Return type from TypeId
        let ret_ty = if let Some(ret_id) = proc.sig.return_type {
            self.llvm_type_for_type_id(ret_id)
        } else {
            "void".to_string()
        };
        if ret_ty != "void" {
            self.fn_return_types.insert(proc_name.clone(), ret_ty.clone());
            self.fn_return_types.insert(proc.sig.name.clone(), ret_ty.clone());
        }

        let saved_var_types = self.var_types.clone();

        // Build parameter list from HirParamDecl
        let mut params = Vec::new();
        let mut param_names = Vec::new();
        let mut var_param_set = HashSet::new();
        let mut open_array_set = HashSet::new();
        let mut named_array_set = HashSet::new();
        let mut param_infos = Vec::new();

        for hp in &proc.sig.params {
            let base_ty = self.llvm_type_for_type_id(hp.type_id);
            let m2_type_name = self.type_name_for_id(hp.type_id);

            if hp.is_var || hp.is_open_array {
                let attrs = if hp.is_var { " noalias nocapture" } else { " nocapture readonly" };
                params.push(format!("ptr{} %{}", attrs, hp.name));
                param_names.push((hp.name.clone(), base_ty.clone(), hp.is_var));
                if hp.is_var {
                    var_param_set.insert(hp.name.clone());
                }
            } else if base_ty.starts_with('[') {
                params.push(format!("ptr nocapture readonly %{}", hp.name));
                param_names.push((hp.name.clone(), base_ty.clone(), false));
                named_array_set.insert(hp.name.clone());
            } else {
                params.push(format!("{} noundef %{}", base_ty, hp.name));
                param_names.push((hp.name.clone(), base_ty.clone(), false));
            }

            if !m2_type_name.is_empty() {
                self.var_type_names.insert(hp.name.clone(), m2_type_name);
            }
            self.var_types.insert(hp.name.clone(), hp.type_id);

            if hp.is_open_array {
                params.push(format!("i32 %{}_high", hp.name));
                open_array_set.insert(hp.name.clone());
                // Track char array for open array of CHAR
                let resolved = self.resolve_alias_id(hp.type_id);
                if let Type::OpenArray { elem_type } = self.sema.types.get(resolved) {
                    if matches!(self.sema.types.get(*elem_type), Type::Char) {
                        self.char_array_vars.insert(hp.name.clone());
                    }
                }
            }

            let elem_ty = if hp.is_open_array {
                let resolved = self.resolve_alias_id(hp.type_id);
                if let Type::OpenArray { elem_type } = self.sema.types.get(resolved) {
                    Some(self.llvm_type_for_type_id(*elem_type))
                } else { None }
            } else { None };

            param_infos.push(ParamLLVMInfo {
                name: hp.name.clone(),
                is_var: hp.is_var,
                is_open_array: hp.is_open_array,
                llvm_type: base_ty.clone(),
                open_array_elem_type: elem_ty,
            });
        }

        self.proc_params.insert(proc_name.clone(), param_infos.clone());
        self.proc_params.insert(proc.sig.name.clone(), param_infos);

        // Emit function definition
        let params_str = params.join(", ");
        let dbg_sp = if let Some(ref mut di) = self.di {
            let sp = di.create_subprogram(
                &proc.sig.name, &proc_name, &proc.loc.file, proc.loc.line);
            Some(sp)
        } else { None };
        let personality = if self.m2plus {
            if !self.declared_fns.contains("m2_eh_personality") {
                self.emit_preambleln("declare i32 @m2_eh_personality(...)");
                self.declared_fns.insert("m2_eh_personality".to_string());
            }
            " personality ptr @m2_eh_personality"
        } else { "" };

        // Large function detection: count HIR statements
        let stmt_count = proc.body.as_ref().map(|s| s.len()).unwrap_or(0);
        let is_huge = stmt_count > 200; // HIR stmts are denser than AST
        let has_exceptions = proc.except_handler.is_some() || self.m2plus;
        let fn_attrs = if is_huge {
            " optnone noinline"
        } else if has_exceptions { "" } else { " nounwind" };

        if let Some(sp) = dbg_sp {
            self.emitln(&format!("define {} @{}({}){}{} !dbg !{} {{", ret_ty, proc_name, params_str, fn_attrs, personality, sp));
        } else {
            self.emitln(&format!("define {} @{}({}){}{} {{", ret_ty, proc_name, params_str, fn_attrs, personality));
        }
        self.emitln("bb.entry:");

        self.in_function = true;
        self.tmp_counter = 0;
        self.current_return_type = Some(ret_ty.clone());
        self.locals.push(HashMap::new());
        self.var_params.push(var_param_set.clone());
        self.open_array_params.push(open_array_set.clone());
        self.named_array_params.push(named_array_set);

        // Stack trace
        let frame_alloca = self.next_tmp();
        self.emitln(&format!("  {} = alloca %m2_StackFrame", frame_alloca));
        let proc_str = self.intern_string(&proc_name);
        let file_str = self.intern_string(&proc.loc.file);
        self.emitln(&format!("  call void @m2_stack_push(ptr {}, ptr {}, ptr {})",
            frame_alloca, proc_str.0, file_str.0));
        self.stack_frame_alloca = Some(frame_alloca);

        // Create allocas for params
        for (name, ty, is_var) in &param_names {
            if *is_var {
                let alloca = self.next_tmp();
                self.emitln(&format!("  {} = alloca ptr", alloca));
                self.emitln(&format!("  store ptr %{}, ptr {}", name, alloca));
                self.locals.last_mut().unwrap().insert(name.clone(), (alloca, ty.clone()));
            } else {
                let alloca = self.next_tmp();
                let is_array_param = ty.starts_with('[');
                if is_array_param {
                    self.emitln(&format!("  {} = alloca ptr", alloca));
                    self.emitln(&format!("  store ptr %{}, ptr {}", name, alloca));
                } else {
                    self.emitln(&format!("  {} = alloca {}", alloca, ty));
                    self.emitln(&format!("  store {} %{}, ptr {}", ty, name, alloca));
                }
                self.locals.last_mut().unwrap().insert(name.clone(), (alloca, ty.clone()));
            }
        }

        // Debug declarations for params
        if self.di.is_some() {
            let source_file = self.source_file.clone();
            let proc_line = proc.loc.line;
            if let Some(ref mut di) = self.di {
                di.set_location(proc_line, 0, &source_file);
            }
            let mut arg_no = 1usize;
            for hp in &proc.sig.params {
                let llvm_ty = self.llvm_type_for_type_id(hp.type_id);
                let di_type_id = self.debug_type_for_llvm_type_str(&llvm_ty);
                let var_id = self.di.as_mut().unwrap().create_local_variable(
                    &hp.name, &source_file, proc_line, di_type_id, arg_no,
                );
                if let Some((alloca, _)) = self.locals.last().and_then(|l| l.get(&hp.name)).cloned() {
                    self.emit_dbg_declare(&alloca, var_id);
                }
                arg_no += 1;
            }
        }

        // Open array _high params
        for name in &open_array_set {
            let alloca = self.next_tmp();
            self.emitln(&format!("  {} = alloca i32", alloca));
            self.emitln(&format!("  store i32 %{}_high, ptr {}", name, alloca));
            let high_name = format!("{}_high", name);
            self.locals.last_mut().unwrap().insert(high_name.clone(), (alloca, "i32".to_string()));
            self.var_types.insert(high_name, crate::types::TY_INTEGER);
        }

        // Local declarations from HIR
        for local in &proc.locals {
            self.gen_hir_local_decl(local);
        }

        // Push parent proc name for nested proc name resolution
        self.parent_proc_stack.push(proc_name.clone());

        // Handle closure captures for nested procedures
        let parent_locals: HashSet<String> = self.locals.iter()
            .flat_map(|l| l.keys().cloned())
            .collect();
        let mut nested_procs = Vec::new();
        for nested in &proc.nested_procs {
            let nested_name = format!("{}_{}", proc_name, nested.sig.name);
            self.declared_fns.insert(nested_name.clone());
            self.declared_fns.insert(nested.sig.name.clone());
            let module_name = format!("{}_{}", self.module_name, nested.sig.name);
            if module_name != nested_name {
                self.declared_fns.insert(module_name);
            }

            // Promote captured variables to globals
            for cap in &nested.closure_captures {
                if self.globals.contains_key(&cap.name) {
                    if let Some((addr, ty)) = self.globals.get(&cap.name).cloned() {
                        self.locals.last_mut().unwrap().entry(cap.name.clone())
                            .or_insert((addr, ty));
                    }
                    continue;
                }
                let found = self.locals.iter().rev()
                    .find_map(|l| l.get(&cap.name).cloned());
                if let Some((alloca, ty)) = found {
                    if alloca.starts_with('@') { continue; }
                    let global_name = format!("@{}_{}", proc_name, cap.name);
                    let zero = self.llvm_zero_initializer(&ty);
                    self.emit_preambleln(&format!("{} = global {} {}", global_name, ty, zero));
                    let tmp = self.next_tmp();
                    self.emitln(&format!("  {} = load {}, ptr {}", tmp, ty, alloca));
                    self.emitln(&format!("  store {} {}, ptr {}", ty, tmp, global_name));
                    self.locals.last_mut().unwrap().insert(cap.name.clone(), (global_name.clone(), ty.clone()));
                    self.globals.insert(cap.name.clone(), (global_name, ty));
                }
            }
            nested_procs.push(nested.clone());
        }

        // Body with EXCEPT support
        let has_proc_except = proc.except_handler.is_some();
        if has_proc_except {
            self.declare_exc_runtime();
            let frame = self.next_tmp();
            self.emitln(&format!("  {} = alloca [256 x i8]", frame));
            self.emitln(&format!("  call void @m2_exc_push(ptr {})", frame));
            let sjret = self.next_tmp();
            self.emitln(&format!("  {} = call i32 @setjmp(ptr {})", sjret, frame));
            let caught = self.next_tmp();
            self.emitln(&format!("  {} = icmp ne i32 {}, 0", caught, sjret));
            let body_label = self.next_label("proc.body");
            let except_label = self.next_label("proc.except");
            self.emitln(&format!("  br i1 {}, label %{}, label %{}",
                caught, except_label, body_label));

            self.emitln(&format!("{}:", body_label));
            self.in_sjlj_context = true;
            if let Some(ref body) = proc.body {
                self.gen_hir_statements(body);
            }
            self.in_sjlj_context = false;
            self.emitln(&format!("  call void @m2_exc_pop(ptr {})", frame));
            if let Some(ref sf) = self.stack_frame_alloca.clone() {
                self.emitln(&format!("  call void @m2_stack_pop(ptr {})", sf));
            }
            if ret_ty == "void" {
                self.emitln("  ret void");
            } else {
                let zero = self.llvm_zero_initializer(&ret_ty);
                self.emitln(&format!("  ret {} {}", ret_ty, zero));
            }

            self.emitln(&format!("{}:", except_label));
            self.emitln(&format!("  call void @m2_exc_pop(ptr {})", frame));
            if let Some(ref except_stmts) = proc.except_handler {
                self.gen_hir_statements(except_stmts);
            }
        } else {
            if let Some(ref body) = proc.body {
                self.gen_hir_statements(body);
            }
        }

        // Pop stack frame and return
        if let Some(ref frame) = self.stack_frame_alloca.clone() {
            self.emitln(&format!("  call void @m2_stack_pop(ptr {})", frame));
        }
        if ret_ty == "void" {
            self.emitln("  ret void");
        } else {
            let zero = self.llvm_zero_initializer(&ret_ty);
            self.emitln(&format!("  ret {} {}", ret_ty, zero));
        }

        self.stack_frame_alloca = None;
        self.emitln("}");
        self.emitln("");

        self.locals.pop();
        self.var_params.pop();
        self.open_array_params.pop();
        self.named_array_params.pop();
        self.current_return_type = None;
        self.in_function = false;
        if let Some(ref mut di) = self.di {
            di.leave_scope();
        }

        // Emit nested procedures
        for nested in nested_procs {
            self.gen_hir_proc_decl(&nested)?;
        }
        self.parent_proc_stack.pop();
        self.var_types = saved_var_types;

        Ok(())
    }

    /// Emit a local variable/type/const/exception declaration from HIR.
    pub(crate) fn gen_hir_local_decl(&mut self, local: &crate::hir::HirLocalDecl) {
        use crate::hir::HirLocalDecl as LD;
        use crate::types::Type;
        match local {
            LD::Var { name, type_id } => {
                let resolved_id = self.resolve_alias_id(*type_id);
                let llvm_ty = self.llvm_type_for_type_id(resolved_id);
                let is_array = matches!(self.sema.types.get(resolved_id), Type::Array { .. });
                let is_char_array = if let Type::Array { elem_type, .. } = self.sema.types.get(resolved_id) {
                    matches!(self.sema.types.get(*elem_type), Type::Char)
                } else { false };
                let m2_type_name = self.type_name_for_id(*type_id);

                // Handle inline POINTER TO RECORD
                if let Type::Pointer { base } = self.sema.types.get(resolved_id) {
                    let base_resolved = self.resolve_alias_id(*base);
                    if let Type::Record { fields, .. } = self.sema.types.get(base_resolved) {
                        let record_name = format!("__anon_record_{}", self.anon_record_counter);
                        self.anon_record_counter += 1;
                        let record_ty = self.llvm_type_for_type_id(base_resolved);
                        self.type_map.insert(record_name.clone(), record_ty);
                        let mut field_list = Vec::new();
                        for (idx, f) in fields.iter().enumerate() {
                            let ft = self.llvm_type_for_type_id(f.typ);
                            field_list.push((f.name.clone(), ft, idx));
                        }
                        self.record_fields.insert(record_name.clone(), field_list);
                        self.var_type_names.insert(name.clone(), record_name);
                        self.var_types.insert(name.clone(), *type_id);
                    }
                }

                // Track array element type name
                if let Type::Array { elem_type, .. } = self.sema.types.get(resolved_id) {
                    let elem_name = self.type_name_for_id(*elem_type);
                    if !elem_name.is_empty() {
                        self.array_elem_type_names.insert(name.clone(), elem_name);
                    }
                }

                let alloca = self.next_tmp();
                self.emitln(&format!("  {} = alloca {}", alloca, llvm_ty));
                self.locals.last_mut().unwrap().insert(name.clone(), (alloca, llvm_ty.clone()));
                if is_array {
                    self.array_vars.insert(name.clone());
                }
                if is_char_array {
                    self.char_array_vars.insert(name.clone());
                }
                if !m2_type_name.is_empty() {
                    self.var_type_names.insert(name.clone(), m2_type_name);
                }
                self.var_types.insert(name.clone(), *type_id);
            }
            LD::Type { name, type_id } => {
                let hir_td = crate::hir::HirTypeDecl {
                    name: name.clone(),
                    mangled: self.mangle(name),
                    type_id: *type_id,
                    exported: false,
                };
                self.gen_hir_type_decls_from(&[hir_td]);
            }
            LD::Const(cd) => {
                self.gen_hir_const_decls_from(&[cd.clone()]);
            }
            LD::Exception { name, mangled, exc_id } => {
                let ed = crate::hir::HirExceptionDecl {
                    name: name.clone(),
                    mangled: mangled.clone(),
                    exc_id: *exc_id,
                };
                self.gen_hir_exception_decls_from(&[ed]);
            }
        }
    }

    /// Resolve a TypeId through aliases to the underlying type.
    pub(crate) fn resolve_alias_id(&self, id: crate::types::TypeId) -> crate::types::TypeId {
        use crate::types::Type;
        match self.sema.types.get(id) {
            Type::Alias { target, .. } => self.resolve_alias_id(*target),
            _ => id,
        }
    }

    /// Get the M2 type name for a TypeId (for record field resolution).
    pub(crate) fn type_name_for_id(&self, id: crate::types::TypeId) -> String {
        use crate::types::Type;
        match self.sema.types.get(id) {
            Type::Alias { name, .. } => name.clone(),
            Type::Enumeration { name, .. } => name.clone(),
            Type::Opaque { name, .. } => name.clone(),
            _ => String::new(),
        }
    }
}
