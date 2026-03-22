use super::*;

impl LLVMCodeGen {
    // ── Declaration generation ──────────────────────────────────────

    pub(crate) fn gen_type_decls(&mut self, decls: &[Declaration]) {
        for decl in decls {
            if let Declaration::Type(td) = decl {
                if let Some(ref tn) = td.typ {
                    let raw_ty = self.llvm_type_for_type_node(tn);
                    // Sanitize: "void" in struct/array positions means an
                    // unresolved imported type — replace with i32.
                    let llvm_ty = if raw_ty.contains("void") && raw_ty != "void" {
                        raw_ty.replace("void", "i32")
                    } else {
                        raw_ty
                    };
                    self.type_map.insert(td.name.clone(), llvm_ty.clone());

                    // Track record fields (including variant parts)
                    if let TypeNode::Record { fields, .. } = tn {
                        let mut field_list = Vec::new();
                        let mut idx = 0;
                        for fl in fields {
                            for f in &fl.fixed {
                                let ft = self.llvm_type_for_type_node(&f.typ);
                                for fname in &f.names {
                                    field_list.push((fname.clone(), ft.clone(), idx));
                                    idx += 1;
                                }
                            }
                            // Handle variant part — tag field + variant fields
                            if let Some(ref vp) = fl.variant {
                                if let Some(ref tag_name) = vp.tag_name {
                                    field_list.push((tag_name.clone(), "i32".to_string(), idx));
                                    idx += 1;
                                }
                                // Variant fields share memory (union).
                                // All fields from all variants map to overlapping
                                // indices starting at the union base.
                                let union_start = idx;
                                let mut max_variant_fields = 0usize;
                                for variant in &vp.variants {
                                    let mut variant_offset = 0usize;
                                    for vfl in &variant.fields {
                                        for f in &vfl.fixed {
                                            let ft = self.llvm_type_for_type_node(&f.typ);
                                            for fname in &f.names {
                                                if !field_list.iter().any(|(n, _, _)| n == fname) {
                                                    field_list.push((fname.clone(), ft.clone(), union_start + variant_offset));
                                                }
                                                variant_offset += 1;
                                            }
                                        }
                                        // Handle nested variant parts (inner CASE)
                                        if let Some(ref inner_vp) = vfl.variant {
                                            if let Some(ref inner_tag) = inner_vp.tag_name {
                                                if !field_list.iter().any(|(n, _, _)| n == inner_tag) {
                                                    field_list.push((inner_tag.clone(), "i32".to_string(), union_start + variant_offset));
                                                }
                                                variant_offset += 1;
                                            }
                                            let inner_union_start = union_start + variant_offset;
                                            let mut inner_max = 0usize;
                                            for iv in &inner_vp.variants {
                                                let mut inner_offset = 0usize;
                                                for ivfl in &iv.fields {
                                                    for f in &ivfl.fixed {
                                                        let ft = self.llvm_type_for_type_node(&f.typ);
                                                        for fname in &f.names {
                                                            if !field_list.iter().any(|(n, _, _)| n == fname) {
                                                                field_list.push((fname.clone(), ft.clone(), inner_union_start + inner_offset));
                                                            }
                                                            inner_offset += 1;
                                                        }
                                                    }
                                                }
                                                if inner_offset > inner_max { inner_max = inner_offset; }
                                            }
                                            variant_offset += inner_max;
                                        }
                                    }
                                    if variant_offset > max_variant_fields {
                                        max_variant_fields = variant_offset;
                                    }
                                }
                                if let Some(ref else_fields) = vp.else_fields {
                                    let mut variant_offset = 0usize;
                                    for efl in else_fields {
                                        for f in &efl.fixed {
                                            let ft = self.llvm_type_for_type_node(&f.typ);
                                            for fname in &f.names {
                                                if !field_list.iter().any(|(n, _, _)| n == fname) {
                                                    field_list.push((fname.clone(), ft.clone(), union_start + variant_offset));
                                                }
                                                variant_offset += 1;
                                            }
                                        }
                                    }
                                    if variant_offset > max_variant_fields {
                                        max_variant_fields = variant_offset;
                                    }
                                }
                                idx = union_start + max_variant_fields;
                            }
                        }
                        self.record_fields.insert(td.name.clone(), field_list);
                    }

                    // Track array types
                    if let TypeNode::Array { elem_type, index_types, .. } = tn {
                        let elem_ty = self.llvm_type_for_type_node(elem_type);
                        let is_char_array = matches!(**elem_type, TypeNode::Named(ref qi) if qi.name == "CHAR");
                        if let Some(idx_tn) = index_types.first() {
                            if let TypeNode::Subrange { low, high, .. } = idx_tn {
                                if let (ExprKind::IntLit(_lo), ExprKind::IntLit(hi)) = (&low.kind, &high.kind) {
                                    let size = (*hi + 1) as usize;
                                    self.array_types.insert(td.name.clone(), (elem_ty, size));
                                    if is_char_array {
                                        // Track for string handling
                                    }
                                }
                            }
                        }
                    }

                    // Track pointer-to-record types (including anonymous inline records)
                    if let TypeNode::Pointer { base, .. } = tn {
                        if let TypeNode::Record { fields, .. } = base.as_ref() {
                            // Generate a synthetic record name for the anonymous record
                            let record_name = format!("__anon_record_{}", self.anon_record_counter);
                            self.anon_record_counter += 1;
                            let record_ty = self.llvm_type_for_type_node(base);
                            self.type_map.insert(record_name.clone(), record_ty);

                            // Register fields for the anonymous record
                            let mut field_list = Vec::new();
                            let mut idx = 0;
                            for fl in fields {
                                for f in &fl.fixed {
                                    let ft = self.llvm_type_for_type_node(&f.typ);
                                    for fname in &f.names {
                                        field_list.push((fname.clone(), ft.clone(), idx));
                                        idx += 1;
                                    }
                                }
                            }
                            self.record_fields.insert(record_name.clone(), field_list);
                            // Map the pointer type name to the record type name
                            self.pointer_target_types.insert(td.name.clone(), record_name);
                        } else if let TypeNode::Named(qi) = base.as_ref() {
                            // POINTER TO SomeRecordType — track the target
                            self.pointer_target_types.insert(td.name.clone(), qi.name.clone());
                        }
                    }

                    // Propagate pointer_target_types through type aliases
                    // e.g., Stack = NodePtr where NodePtr = POINTER TO Node
                    if let TypeNode::Named(qi) = tn {
                        if let Some(target) = self.pointer_target_types.get(&qi.name).cloned() {
                            self.pointer_target_types.insert(td.name.clone(), target);
                        }
                    }

                    // Track enum variants
                    if let TypeNode::Enumeration { variants, .. } = tn {
                        for (i, v) in variants.iter().enumerate() {
                            self.enum_variants.insert(v.clone(), i as i64);
                            self.enum_variants.insert(format!("{}_{}", td.name, v), i as i64);
                        }
                    }

                    // Register RTTI type descriptors for REF/OBJECT types
                    if self.m2plus {
                        match tn {
                            TypeNode::Ref { .. } => {
                                let mangled = self.mangle(&td.name);
                                self.register_type_desc(&mangled, None);
                            }
                            TypeNode::Object { parent, .. } => {
                                let mangled = self.mangle(&td.name);
                                let parent_name = parent.as_ref().map(|qi| {
                                    self.mangle(&qi.name)
                                });
                                let parent_ref = parent_name.as_deref();
                                self.register_type_desc(&mangled, parent_ref);
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn gen_const_decls(&mut self, decls: &[Declaration]) {
        // Multiple passes to resolve forward-referenced constants
        let const_decls: Vec<&ConstDecl> = decls.iter()
            .filter_map(|d| if let Declaration::Const(cd) = d { Some(cd) } else { None })
            .collect();
        let mut resolved = std::collections::HashSet::new();
        for _pass in 0..3 {
            for cd in &const_decls {
                if resolved.contains(&cd.name) { continue; }
                if let Some(v) = self.const_eval_expr(&cd.expr) {
                    self.const_values.insert(cd.name.clone(), v);
                    self.const_values.insert(self.mangle(&cd.name), v);
                    resolved.insert(cd.name.clone());
                }
            }
        }
        // Handle string constants
        for decl in decls {
            if let Declaration::Const(cd) = decl {
                if self.const_values.contains_key(&cd.name) { continue; }
                if let ExprKind::StringLit(s) = &cd.expr.kind {
                    // String constants — create a global alias pointing to the interned string
                    let (str_name, _) = self.intern_string(s);
                    let global_name = format!("@{}", self.mangle(&cd.name));
                    self.emit_preambleln(&format!("{} = global ptr {}", global_name, str_name));
                    self.globals.insert(cd.name.clone(), (global_name.clone(), "ptr".to_string()));
                    let mangled = self.mangle(&cd.name);
                    self.globals.insert(mangled, (global_name, "ptr".to_string()));
                    // Single-char string: also register as integer constant for CHAR comparisons
                    if s.len() == 1 {
                        self.const_values.insert(cd.name.clone(), s.as_bytes()[0] as i64);
                        self.const_values.insert(self.mangle(&cd.name), s.as_bytes()[0] as i64);
                    }
                }
            }
        }
        // Handle REAL/float constants
        for decl in decls {
            if let Declaration::Const(cd) = decl {
                if self.const_values.contains_key(&cd.name) { continue; }
                if self.globals.contains_key(&cd.name) { continue; }
                if let ExprKind::RealLit(v) = &cd.expr.kind {
                    let global_name = format!("@{}", self.mangle(&cd.name));
                    let bits = v.to_bits();
                    self.emit_preambleln(&format!("{} = global double 0x{:016X}", global_name, bits));
                    self.globals.insert(cd.name.clone(), (global_name.clone(), "double".to_string()));
                    let mangled = self.mangle(&cd.name);
                    self.globals.insert(mangled, (global_name, "double".to_string()));
                }
            }
        }
    }

    pub(crate) fn gen_exception_decls(&mut self, decls: &[Declaration]) {
        static NEXT_EXC_ID: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(100);
        for decl in decls {
            if let Declaration::Exception(e) = decl {
                let id = NEXT_EXC_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let mangled = self.mangle(&e.name);
                self.const_values.insert(e.name.clone(), id);
                self.const_values.insert(mangled.clone(), id);
                // Also register as a global constant for RAISE
                let global_name = format!("@{}", mangled);
                self.emit_preambleln(&format!("{} = global i32 {}", global_name, id));
                self.globals.insert(e.name.clone(), (global_name.clone(), "i32".to_string()));
                self.globals.insert(mangled, (global_name, "i32".to_string()));
            }
        }
    }

    pub(crate) fn gen_var_decls_global(&mut self, decls: &[Declaration]) {
        for decl in decls {
            if let Declaration::Var(v) = decl {
                let llvm_ty = self.llvm_type_for_type_node(&v.typ);
                let is_array = matches!(&v.typ, TypeNode::Array { .. });
                let is_char_array = if let TypeNode::Array { elem_type, .. } = &v.typ {
                    matches!(**elem_type, TypeNode::Named(ref qi) if qi.name == "CHAR")
                } else {
                    false
                };

                // Track M2 type name for the variable
                let m2_type_name = match &v.typ {
                    TypeNode::Named(qi) => qi.name.clone(),
                    _ => String::new(),
                };

                // Handle inline POINTER TO RECORD in var declarations
                if let TypeNode::Pointer { base, .. } = &v.typ {
                    if let TypeNode::Record { fields, .. } = base.as_ref() {
                        let record_name = format!("__anon_record_{}", self.anon_record_counter);
                        self.anon_record_counter += 1;
                        let record_ty = self.llvm_type_for_type_node(base);
                        self.type_map.insert(record_name.clone(), record_ty);
                        let mut field_list = Vec::new();
                        let mut idx = 0;
                        for fl in fields {
                            for f in &fl.fixed {
                                let ft = self.llvm_type_for_type_node(&f.typ);
                                for fname in &f.names {
                                    field_list.push((fname.clone(), ft.clone(), idx));
                                    idx += 1;
                                }
                            }
                        }
                        self.record_fields.insert(record_name.clone(), field_list);
                        for vname in &v.names {
                            self.var_type_names.insert(vname.clone(), record_name.clone());
                            if let Some(tid) = self.resolve_type_node_to_id(&v.typ) {
                                self.var_types.insert(vname.clone(), tid);
                            }
                        }
                    }
                }

                // Track array element type name for field resolution after indexing
                if let TypeNode::Array { elem_type, .. } = &v.typ {
                    if let TypeNode::Named(qi) = elem_type.as_ref() {
                        for name in &v.names {
                            self.array_elem_type_names.insert(name.clone(), qi.name.clone());
                        }
                    }
                }

                for name in &v.names {
                    let global_name = format!("@{}", self.mangle(name));
                    let zero = self.llvm_zero_initializer(&llvm_ty);
                    // Emit with debug global variable expression if debug mode
                    let dbg_suffix = if self.di.is_some() {
                        let typ_clone = v.typ.clone();
                        let di_type_id = self.debug_type_for_type_node(&typ_clone);
                        let file = self.source_file.clone();
                        let linkage = self.mangle(name);
                        let gv_id = self.di.as_mut().unwrap().create_global_variable(
                            name, &linkage, &file, v.loc.line, di_type_id,
                        );
                        format!(", !dbg !{}", gv_id)
                    } else {
                        String::new()
                    };
                    self.emit_preambleln(&format!("{} = global {} {}{}",
                        global_name, llvm_ty, zero, dbg_suffix));
                    self.globals.insert(name.clone(), (global_name, llvm_ty.clone()));
                    if is_array {
                        self.array_vars.insert(name.clone());
                    }
                    if is_char_array {
                        self.char_array_vars.insert(name.clone());
                    }
                    if !m2_type_name.is_empty() {
                        self.var_type_names.insert(name.clone(), m2_type_name.clone());
                    }
                    // Track semantic TypeId from the declaration's TypeNode.
                    if let Some(tid) = self.resolve_type_node_to_id(&v.typ) {
                        self.var_types.insert(name.clone(), tid);
                    }
                }
            }
        }
    }

    // ── Procedure generation ────────────────────────────────────────

    pub(crate) fn gen_proc_decl(&mut self, p: &ProcDecl) -> CompileResult<()> {
        // Mangle name including parent proc for nested procedures
        let proc_name = if self.parent_proc_stack.is_empty() {
            self.mangle(&p.heading.name)
        } else {
            // Use the last (most specific) parent name as prefix
            let parent = self.parent_proc_stack.last().unwrap();
            format!("{}_{}", parent, p.heading.name)
        };

        // Track this as a defined function (for proc var vs function distinction)
        self.declared_fns.insert(proc_name.clone());
        self.declared_fns.insert(p.heading.name.clone());
        // Also track the simple mangled name → full name for nested proc resolution
        let simple_mangled = self.mangle(&p.heading.name);
        if simple_mangled != proc_name {
            self.declared_fns.insert(simple_mangled);
        }

        // Determine return type
        let ret_ty = if let Some(ref rt) = p.heading.return_type {
            self.llvm_type_for_type_node(rt)
        } else {
            "void".to_string()
        };

        // Save var_types BEFORE params are registered, so they don't
        // overwrite globals with the same name (e.g. param 'd' vs global 'd').
        let saved_var_types = self.var_types.clone();

        // Resolve parameter TypeIds from the AST heading directly.
        // This is the only reliable source — symtab ParamInfo may contain
        // stale TypeIds from .def processing or finalization.
        let sema_params: Vec<crate::symtab::ParamInfo> = {
            let mut params = Vec::new();
            for fp in &p.heading.params {
                let tid = self.resolve_type_node_to_id(&fp.typ)
                    .unwrap_or(crate::types::TY_VOID);
                for name in &fp.names {
                    params.push(crate::symtab::ParamInfo {
                        name: name.clone(),
                        typ: tid,
                        is_var: fp.is_var,
                    });
                }
            }
            params
        };

        // Build parameter list
        let mut params = Vec::new();
        let mut param_names = Vec::new();
        let mut var_param_set = HashSet::new();
        let mut open_array_set = HashSet::new();
        let mut param_infos = Vec::new();
        let mut sema_param_idx = 0;

        for fp in &p.heading.params {
            let is_open_array = matches!(fp.typ, TypeNode::OpenArray { .. });
            let base_ty = self.llvm_type_for_type_node(&fp.typ);

            // Track M2 type name for field resolution
            let m2_type_name = match &fp.typ {
                TypeNode::Named(qi) => qi.name.clone(),
                _ => String::new(),
            };

            for name in &fp.names {
                if fp.is_var || is_open_array {
                    params.push(format!("ptr %{}", name));
                    param_names.push((name.clone(), base_ty.clone(), fp.is_var));
                    if fp.is_var {
                        var_param_set.insert(name.clone());
                    }
                    // Track open array params (even if also VAR) for element type lookup
                    if is_open_array {
                        // Note: don't add to open_array_set if VAR — the alloca/load
                        // is handled by the VAR path. But we need lookup_open_array_elem_type
                        // to find the element type, so add to open_array_params tracking.
                    }
                } else if base_ty.starts_with('[') {
                    // Named array params: pass as pointer (like C array decay)
                    // HIGH is computed from the type, not passed as a param
                    params.push(format!("ptr %{}", name));
                    param_names.push((name.clone(), base_ty.clone(), false));
                } else {
                    params.push(format!("{} %{}", base_ty, name));
                    param_names.push((name.clone(), base_ty.clone(), false));
                }

                // Register M2 type name for record field resolution
                if !m2_type_name.is_empty() {
                    self.var_type_names.insert(name.clone(), m2_type_name.clone());
                }
                // Track semantic TypeId from sema's resolved parameter info.
                if let Some(pi) = sema_params.get(sema_param_idx) {
                    self.var_types.insert(name.clone(), pi.typ);
                }
                sema_param_idx += 1;

                if is_open_array {
                    params.push(format!("i32 %{}_high", name));
                    open_array_set.insert(name.clone());
                    // Track element type for open array
                    if let TypeNode::OpenArray { elem_type, .. } = &fp.typ {
                        if matches!(**elem_type, TypeNode::Named(ref qi) if qi.name == "CHAR") {
                            self.char_array_vars.insert(name.clone());
                        }
                    }
                }

                let elem_ty = if is_open_array {
                    if let TypeNode::OpenArray { elem_type, .. } = &fp.typ {
                        Some(self.llvm_type_for_type_node(elem_type))
                    } else { None }
                } else { None };
                param_infos.push(ParamLLVMInfo {
                    name: name.clone(),
                    is_var: fp.is_var,
                    is_open_array,
                    llvm_type: base_ty.clone(),
                    open_array_elem_type: elem_ty,
                });
            }
        }

        // Register proc params for call sites
        self.proc_params.insert(proc_name.clone(), param_infos.clone());
        self.proc_params.insert(p.heading.name.clone(), param_infos);

        // Emit function definition
        let params_str = params.join(", ");
        // Attach debug subprogram if debug mode
        let dbg_sp = if let Some(ref mut di) = self.di {
            let sp = di.create_subprogram(
                &p.heading.name, &proc_name, &p.loc.file, p.loc.line);
            Some(sp)
        } else { None };
        let personality = if self.m2plus {
            if !self.declared_fns.contains("m2_eh_personality") {
                self.emit_preambleln("declare i32 @m2_eh_personality(...)");
                self.declared_fns.insert("m2_eh_personality".to_string());
            }
            " personality ptr @m2_eh_personality"
        } else { "" };
        if let Some(sp) = dbg_sp {
            self.emitln(&format!("define {} @{}({}){} !dbg !{} {{", ret_ty, proc_name, params_str, personality, sp));
        } else {
            self.emitln(&format!("define {} @{}({}){} {{", ret_ty, proc_name, params_str, personality));
        }
        self.emitln("bb.entry:");

        self.in_function = true;
        self.tmp_counter = 0;
        self.current_return_type = Some(ret_ty.clone());
        self.locals.push(HashMap::new());
        self.var_params.push(var_param_set.clone());
        self.open_array_params.push(open_array_set.clone());

        // Create allocas for value parameters (not VAR params)
        for (name, ty, is_var) in &param_names {
            if *is_var {
                // VAR params are already pointers — store the pointer itself.
                // Track the *element* type (ty = base type, not ptr) so loads/stores work.
                let alloca = self.next_tmp();
                self.emitln(&format!("  {} = alloca ptr", alloca));
                self.emitln(&format!("  store ptr %{}, ptr {}", name, alloca));
                // ty here is the base type (i32, i8, etc.) — what the pointer points to
                self.locals.last_mut().unwrap().insert(name.clone(), (alloca, ty.clone()));
            } else {
                let alloca = self.next_tmp();
                // Arrays passed as ptr need ptr alloca
                let is_array_param = ty.starts_with('[');
                if is_array_param {
                    self.emitln(&format!("  {} = alloca ptr", alloca));
                    self.emitln(&format!("  store ptr %{}, ptr {}", name, alloca));
                    self.locals.last_mut().unwrap().insert(name.clone(), (alloca, ty.clone()));
                } else {
                    self.emitln(&format!("  {} = alloca {}", alloca, ty));
                    self.emitln(&format!("  store {} %{}, ptr {}", ty, name, alloca));
                    self.locals.last_mut().unwrap().insert(name.clone(), (alloca, ty.clone()));
                }
            }
        }

        // Emit debug declarations for parameters
        if self.di.is_some() {
            let source_file = self.source_file.clone();
            let proc_line = p.loc.line;
            // Set debug location at proc start for param declarations
            if let Some(ref mut di) = self.di {
                di.set_location(proc_line, 0, &source_file);
            }
            let mut arg_no = 1usize;
            for fp in &p.heading.params {
                for name in &fp.names {
                    let typ_clone = fp.typ.clone();
                    let di_type_id = self.debug_type_for_type_node(&typ_clone);
                    let var_id = self.di.as_mut().unwrap().create_local_variable(
                        name, &source_file, proc_line, di_type_id, arg_no,
                    );
                    if let Some((alloca, _)) = self.locals.last().and_then(|l| l.get(name)).cloned() {
                        self.emit_dbg_declare(&alloca, var_id);
                    }
                    arg_no += 1;
                }
            }
        }

        // Open array _high params
        for name in &open_array_set {
            let alloca = self.next_tmp();
            self.emitln(&format!("  {} = alloca i32", alloca));
            self.emitln(&format!("  store i32 %{}_high, ptr {}", name, alloca));
            let high_name = format!("{}_high", name);
            self.locals.last_mut().unwrap().insert(high_name, (alloca, "i32".to_string()));
        }

        // Local variable declarations
        for decl in &p.block.decls {
            if let Declaration::Var(v) = decl {
                self.gen_var_decl_local(v);
            }
        }

        // Local const/type declarations (consts first for type resolution)
        self.gen_const_decls(&p.block.decls);
        self.gen_type_decls(&p.block.decls);

        // Push parent proc name for nested proc name resolution
        self.parent_proc_stack.push(proc_name.clone());

        // Collect nested procedures and detect captured variables.
        // Any variable referenced in a nested proc that's defined in the parent
        // gets promoted to a module-level global so both can access it.
        let mut nested_procs = Vec::new();
        // Collect ALL ancestor locals (not just direct parent) for capture detection
        let parent_locals: HashSet<String> = self.locals.iter()
            .flat_map(|l| l.keys().cloned())
            .collect();
        for decl in &p.block.decls {
            if let Declaration::Procedure(nested_p) = decl {
                let nested_name = format!("{}_{}", proc_name, nested_p.heading.name);
                self.declared_fns.insert(nested_name.clone());
                self.declared_fns.insert(nested_p.heading.name.clone());

                // Find captured variables: names used in nested body that are parent locals
                let captured = self.collect_free_vars(&nested_p.block, &parent_locals);
                for cap_name in &captured {
                    // Skip if already promoted (multiple nested procs capturing same var)
                    if self.globals.contains_key(cap_name) {
                        // Already a global — make sure it's in current locals for the nested proc
                        if let Some((addr, ty)) = self.globals.get(cap_name).cloned() {
                            self.locals.last_mut().unwrap().entry(cap_name.clone())
                                .or_insert((addr, ty));
                        }
                        continue;
                    }
                    // Search all ancestor scopes for the captured variable
                    let found = self.locals.iter().rev()
                        .find_map(|l| l.get(cap_name).cloned());
                    if let Some((alloca, ty)) = found {
                        if alloca.starts_with('@') { continue; } // already a global
                        let global_name = format!("@{}_{}", proc_name, cap_name);
                        let zero = self.llvm_zero_initializer(&ty);
                        self.emit_preambleln(&format!("{} = global {} {}", global_name, ty, zero));
                        // Copy current local value to global
                        let tmp = self.next_tmp();
                        self.emitln(&format!("  {} = load {}, ptr {}", tmp, ty, alloca));
                        self.emitln(&format!("  store {} {}, ptr {}", ty, tmp, global_name));
                        // Replace local with global reference
                        self.locals.last_mut().unwrap().insert(cap_name.clone(), (global_name.clone(), ty.clone()));
                        self.globals.insert(cap_name.clone(), (global_name, ty));
                    }
                }

                nested_procs.push(nested_p.clone());
            }
        }

        // Body (with ISO EXCEPT support for procedure-level handlers)
        let has_proc_except = p.block.except.is_some();
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
            if let Some(stmts) = &p.block.body {
                for stmt in stmts {
                    self.gen_statement(stmt);
                }
            }
            self.in_sjlj_context = false;
            self.emitln(&format!("  call void @m2_exc_pop(ptr {})", frame));
            if ret_ty == "void" {
                self.emitln("  ret void");
            } else {
                let zero = self.llvm_zero_initializer(&ret_ty);
                self.emitln(&format!("  ret {} {}", ret_ty, zero));
            }

            self.emitln(&format!("{}:", except_label));
            self.emitln(&format!("  call void @m2_exc_pop(ptr {})", frame));
            if let Some(except_stmts) = &p.block.except {
                for stmt in except_stmts {
                    self.gen_statement(stmt);
                }
            }
        } else {
            if let Some(stmts) = &p.block.body {
                for stmt in stmts {
                    self.gen_statement(stmt);
                }
            }
        }

        // Ensure function has a terminator
        if ret_ty == "void" {
            self.emitln("  ret void");
        } else {
            let zero = self.llvm_zero_initializer(&ret_ty);
            self.emitln(&format!("  ret {} {}", ret_ty, zero));
        }

        self.emitln("}");
        self.emitln("");

        self.locals.pop();
        self.var_params.pop();
        self.open_array_params.pop();
        self.current_return_type = None;
        self.in_function = false;
        if let Some(ref mut di) = self.di {
            di.leave_scope();
        }
        // parent_proc_stack already has proc_name from before body generation

        // Emit nested procedures as separate top-level functions
        // parent_proc_stack already contains proc_name
        for nested_p in nested_procs {
            self.gen_proc_decl(&nested_p)?;
        }
        self.parent_proc_stack.pop();
        // Restore var_types so procedure params don't leak
        self.var_types = saved_var_types;

        Ok(())
    }

    pub(crate) fn gen_var_decl_local(&mut self, v: &VarDecl) {
        let llvm_ty = self.llvm_type_for_type_node(&v.typ);
        let is_array = matches!(&v.typ, TypeNode::Array { .. });
        let is_char_array = if let TypeNode::Array { elem_type, .. } = &v.typ {
            matches!(**elem_type, TypeNode::Named(ref qi) if qi.name == "CHAR")
        } else {
            false
        };

        let m2_type_name = match &v.typ {
            TypeNode::Named(qi) => qi.name.clone(),
            _ => String::new(),
        };

        // Handle inline POINTER TO RECORD in local var declarations
        if let TypeNode::Pointer { base, .. } = &v.typ {
            if let TypeNode::Record { fields, .. } = base.as_ref() {
                let record_name = format!("__anon_record_{}", self.anon_record_counter);
                self.anon_record_counter += 1;
                let record_ty = self.llvm_type_for_type_node(base);
                self.type_map.insert(record_name.clone(), record_ty);
                let mut field_list = Vec::new();
                let mut idx = 0;
                for fl in fields {
                    for f in &fl.fixed {
                        let ft = self.llvm_type_for_type_node(&f.typ);
                        for fname in &f.names {
                            field_list.push((fname.clone(), ft.clone(), idx));
                            idx += 1;
                        }
                    }
                }
                self.record_fields.insert(record_name.clone(), field_list);
                for vname in &v.names {
                    self.var_type_names.insert(vname.clone(), record_name.clone());
                    if let Some(sym) = self.sema.symtab.lookup_any(vname) {
                        self.var_types.insert(vname.clone(), sym.typ);
                    }
                }
            }
        }

        // Track array element type name
        if let TypeNode::Array { elem_type, .. } = &v.typ {
            if let TypeNode::Named(qi) = elem_type.as_ref() {
                for name in &v.names {
                    self.array_elem_type_names.insert(name.clone(), qi.name.clone());
                }
            }
        }

        for name in &v.names {
            let alloca = self.next_tmp();
            self.emitln(&format!("  {} = alloca {}", alloca, llvm_ty));
            self.locals.last_mut().unwrap().insert(name.clone(), (alloca.clone(), llvm_ty.clone()));
            if is_array {
                self.array_vars.insert(name.clone());
            }
            if is_char_array {
                self.char_array_vars.insert(name.clone());
            }
            if !m2_type_name.is_empty() {
                self.var_type_names.insert(name.clone(), m2_type_name.clone());
            }
            // Track semantic TypeId from the declaration's TypeNode.
            if let Some(tid) = self.resolve_type_node_to_id(&v.typ) {
                self.var_types.insert(name.clone(), tid);
            }

            // Emit debug variable declaration
            if self.di.is_some() {
                let typ_clone = v.typ.clone();
                let di_type_id = self.debug_type_for_type_node(&typ_clone);
                let file = self.source_file.clone();
                let line = v.loc.line;
                let var_id = self.di.as_mut().unwrap().create_local_variable(
                    name, &file, line, di_type_id, 0,
                );
                self.emit_dbg_declare(&alloca, var_id);
            }
        }
    }
}
