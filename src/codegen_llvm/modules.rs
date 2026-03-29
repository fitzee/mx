use super::*;

impl LLVMCodeGen {
    // ── Program module generation ───────────────────────────────────

    pub(crate) fn gen_program_module(&mut self) -> CompileResult<()> {
        // Get module name and source file from prebuilt HIR
        let (mod_name, source_file) = if let Some(ref hir) = self.prebuilt_hir {
            (hir.name.clone(), hir.source_file.clone())
        } else {
            return Err(CompileError::codegen(
                crate::errors::SourceLoc::new("<codegen>", 0, 0),
                "no prebuilt HIR for program module".to_string(),
            ));
        };
        self.module_name = mod_name.clone();
        self.source_file = source_file.clone();

        // Build import map from HIR imports
        self.build_import_map_from_hir();

        // Initialize debug compile unit
        if let Some(ref mut di) = self.di {
            di.create_compile_unit(&source_file);
        }

        // Register proc_params from module_exports
        for (mod_name, exports) in &self.module_exports.clone() {
            for (proc_name, param_info) in exports {
                let prefixed = format!("{}_{}", mod_name, proc_name);
                self.proc_params.insert(prefixed, param_info.clone());
                if self.foreign_modules.contains(mod_name.as_str()) {
                    self.proc_params.insert(proc_name.clone(), param_info.clone());
                }
            }
        }

        // Declare imports from HIR
        self.declare_imports_from_hir();

        // Declare runtime helpers
        self.declare_runtime_helpers();

        // Process definition-only modules (types, consts, enums)
        // These modules have no .mod file but their types/consts are needed
        self.process_def_only_modules();

        // Fixup: def modules may be processed in arbitrary order, so
        // types that reference enums/types from not-yet-processed modules
        // may contain "void" where they should have "i32". Fix up type_map.
        let fixup_keys: Vec<String> = self.type_map.iter()
            .filter(|(_, v)| v.contains("void"))
            .map(|(k, _)| k.clone())
            .collect();
        for key in fixup_keys {
            if let Some(ty) = self.type_map.get_mut(&key) {
                *ty = ty.replace("void", "i32");
            }
        }
        // Also fixup record_fields
        for (_, fields) in self.record_fields.iter_mut() {
            for (_, ft, _) in fields.iter_mut() {
                if ft.contains("void") {
                    *ft = ft.replace("void", "i32");
                }
            }
        }

        // Generate embedded implementation modules (topologically sorted)
        if !self.pending_module_names.is_empty() {
            let pending_names = std::mem::take(&mut self.pending_module_names);
            let sorted = self.topo_sort_by_deps(&pending_names)?;
            for imp_name in &sorted {
                self.gen_embedded_impl_module_by_name(imp_name)?;
            }
        }

        // Global declarations from prebuilt HIR
        self.gen_hir_const_decls();
        self.gen_hir_exception_decls();

        // Type and variable declarations from prebuilt HIR
        if let Some(ref hir) = self.prebuilt_hir.clone() {
            self.gen_hir_type_decls_from(&hir.type_decls);
            self.gen_hir_var_decls_global_from(&hir.global_decls);
        }

        // Procedure declarations from prebuilt HIR
        if let Some(ref hir) = self.prebuilt_hir.clone() {
            for pd in &hir.proc_decls {
                if !pd.sig.is_nested {
                    self.gen_hir_proc_decl(pd)?;
                }
            }
        }

        // Generate main function
        let main_dbg = if let Some(ref mut di) = self.di {
            let sp = di.create_subprogram("main", "main", &source_file, 1);
            Some(sp)
        } else { None };
        let personality = if self.m2plus {
            if !self.declared_fns.contains("m2_eh_personality") {
                self.emit_preambleln("declare i32 @m2_eh_personality(...)");
                self.declared_fns.insert("m2_eh_personality".to_string());
            }
            " personality ptr @m2_eh_personality"
        } else { "" };
        if let Some(sp) = main_dbg {
            self.emitln(&format!("define i32 @main(i32 %argc, ptr %argv){} !dbg !{} {{", personality, sp));
        } else {
            self.emitln(&format!("define i32 @main(i32 %argc, ptr %argv){} {{", personality));
        }
        self.emitln("bb.entry:");
        self.in_function = true;
        self.tmp_counter = 0;
        self.locals.push(HashMap::new());

        // Store argc/argv
        self.emit_preambleln("@m2_argc = external global i32");
        self.emit_preambleln("@m2_argv = external global ptr");
        self.emitln("  store i32 %argc, ptr @m2_argc");
        self.emitln("  store ptr %argv, ptr @m2_argv");

        // Stack trace: push frame for main module
        let main_frame = self.next_tmp();
        self.emitln(&format!("  {} = alloca %m2_StackFrame", main_frame));
        let main_name_str = self.intern_string(&mod_name);
        let main_file_str = self.intern_string(&source_file);
        self.emitln(&format!("  call void @m2_stack_push(ptr {}, ptr {}, ptr {})",
            main_frame, main_name_str.0, main_file_str.0));
        self.stack_frame_alloca = Some(main_frame);

        // Call embedded module init functions
        for mod_name in &self.embedded_init_modules.clone() {
            self.emitln(&format!("  call void @{}_init()", mod_name));
        }

        // Module body (with ISO EXCEPT/FINALLY support) from prebuilt HIR
        let hir_init = self.prebuilt_hir.as_ref().and_then(|h| h.init_body.clone());
        let hir_except = self.prebuilt_hir.as_ref().and_then(|h| h.except_handler.clone());
        let hir_finally = self.prebuilt_hir.as_ref().and_then(|h| h.finally_handler.clone());

        if hir_except.is_some() || hir_finally.is_some() {
            self.declare_exc_runtime();
            let frame = self.next_tmp();
            self.emitln(&format!("  {} = alloca [256 x i8]", frame));
            self.emitln(&format!("  call void @m2_exc_push(ptr {})", frame));
            let sjret = self.next_tmp();
            self.emitln(&format!("  {} = call i32 @setjmp(ptr {})", sjret, frame));
            let caught = self.next_tmp();
            self.emitln(&format!("  {} = icmp ne i32 {}, 0", caught, sjret));
            let body_label = self.next_label("mod.body");
            let except_label = self.next_label("mod.except");
            self.emitln(&format!("  br i1 {}, label %{}, label %{}",
                caught, except_label, body_label));

            self.emitln(&format!("{}:", body_label));
            self.in_sjlj_context = true;
            if let Some(ref stmts) = hir_init {
                self.gen_hir_statements(stmts);
            }
            self.in_sjlj_context = false;
            self.emitln(&format!("  call void @m2_exc_pop(ptr {})", frame));
            let end_label = self.next_label("mod.end");
            self.emitln(&format!("  br label %{}", end_label));

            self.emitln(&format!("{}:", except_label));
            self.emitln(&format!("  call void @m2_exc_pop(ptr {})", frame));
            if let Some(ref stmts) = hir_except {
                self.gen_hir_statements(stmts);
            }
            self.emitln(&format!("  br label %{}", end_label));

            self.emitln(&format!("{}:", end_label));
            if let Some(ref stmts) = hir_finally {
                self.gen_hir_statements(stmts);
            }
        } else {
            if let Some(ref stmts) = hir_init {
                self.gen_hir_statements(stmts);
            }
        }

        // Pop stack frame and return
        if let Some(ref frame) = self.stack_frame_alloca.clone() {
            self.emitln(&format!("  call void @m2_stack_pop(ptr {})", frame));
        }
        self.stack_frame_alloca = None;
        self.emitln("  ret i32 0");
        self.emitln("}");

        self.locals.pop();
        self.in_function = false;

        Ok(())
    }

    pub(crate) fn gen_implementation_module(&mut self, m: &ImplementationModule) -> CompileResult<()> {
        self.module_name = m.name.clone();
        self.source_file = m.loc.file.clone();
        self.build_import_map(&m.imports);

        // Register proc_params
        for (mod_name, exports) in &self.module_exports.clone() {
            for (proc_name, param_info) in exports {
                let prefixed = format!("{}_{}", mod_name, proc_name);
                self.proc_params.insert(prefixed, param_info.clone());
            }
        }

        self.declare_imports(&m.imports);
        self.declare_runtime_helpers();

        if let Some(pending) = self.pending_modules.take() {
            for imp_mod in &pending {
                self.gen_embedded_impl_module(imp_mod)?;
            }
        }

        self.gen_type_decls(&m.block.decls);
        self.gen_hir_const_decls();
        self.gen_var_decls_global(&m.block.decls);

        for decl in &m.block.decls {
            if let Declaration::Procedure(p) = decl {
                self.gen_proc_decl(p)?;
            }
        }

        // Init function for this module
        if let Some(stmts) = &m.block.body {
            if !stmts.is_empty() {
                let init_name = format!("{}_init", self.module_name);
                self.emitln(&format!("define void @{}() {{", init_name));
                self.emitln("bb.entry:");
                self.in_function = true;
                self.tmp_counter = 0;
                self.locals.push(HashMap::new());

                let mut hb = self.make_hir_builder();
                let hir_stmts = hb.lower_stmts(stmts);
                self.gen_hir_statements(&hir_stmts);

                self.emitln("  ret void");
                self.emitln("}");
                self.locals.pop();
                self.in_function = false;
            }
        }

        Ok(())
    }

    /// Generate an embedded implementation module from HIR (no AST dependency).
    pub(crate) fn gen_embedded_impl_module_by_name(&mut self, imp_name: &str) -> CompileResult<()> {
        let saved_module = self.module_name.clone();
        let saved_import_map = self.import_map.clone();
        let saved_import_alias_map = self.import_alias_map.clone();
        let saved_imported_modules = self.imported_modules.clone();

        self.module_name = imp_name.to_string();

        // Build import map from stored module_imports (both def + impl)
        self.import_map.clear();
        self.import_alias_map.clear();
        if let Some(imports) = self.module_imports.get(imp_name).cloned() {
            for hi in &imports {
                if !hi.is_qualified && !hi.module.is_empty() {
                    self.imported_modules.insert(hi.module.clone());
                    for name in &hi.names {
                        if !name.local_name.is_empty() {
                            self.import_map.insert(name.local_name.clone(), hi.module.clone());
                            if name.name != name.local_name {
                                self.import_alias_map.insert(name.local_name.clone(), name.name.clone());
                            }
                        }
                    }
                } else {
                    for name in &hi.names {
                        self.imported_modules.insert(name.name.clone());
                    }
                }
            }
        }

        // Declare imports from stored module_imports
        if let Some(imports) = self.module_imports.get(imp_name).cloned() {
            self.declare_imports_for(&imports);
        }

        // Look up the HirEmbeddedModule
        let hir_emb = self.prebuilt_hir.as_ref().and_then(|hir| {
            hir.embedded_modules.iter().find(|e| e.name == imp_name).cloned()
        });

        if let Some(ref emb) = hir_emb {
            // Process type/const/var declarations from the definition module first
            if self.def_module_names.contains(imp_name) {
                self.process_def_types_for_module(imp_name);
            }

            // Type, const, exception, variable declarations from HIR
            self.gen_hir_type_decls_from(&emb.type_decls);
            self.gen_hir_const_decls_from(&emb.const_decls);
            self.gen_hir_exception_decls_from(&emb.exception_decls);
            self.gen_hir_var_decls_global_from(&emb.global_decls);

            // Pre-register ALL procedure names before generating bodies
            for pd in &emb.procedures {
                let name = self.mangle(&pd.sig.name);
                self.declared_fns.insert(name);
                self.declared_fns.insert(pd.sig.name.clone());
            }

            // Generate procedure bodies
            for pd in &emb.procedures {
                self.gen_hir_proc_decl(pd)?;
            }

            // Generate init function if there's a module body
            if let Some(ref stmts) = emb.init_body {
                if !stmts.is_empty() {
                    let init_name = format!("{}_init", imp_name);
                    self.emitln(&format!("define void @{}() {{", init_name));
                    self.emitln("bb.entry:");
                    self.in_function = true;
                    self.tmp_counter = 0;
                    self.locals.push(HashMap::new());

                    self.gen_hir_statements(stmts);

                    self.emitln("  ret void");
                    self.emitln("}");
                    self.locals.pop();
                    self.in_function = false;
                    self.embedded_init_modules.push(imp_name.to_string());
                }
            }
        }

        self.module_name = saved_module;
        self.import_map = saved_import_map;
        self.import_alias_map = saved_import_alias_map;
        self.imported_modules = saved_imported_modules;
        Ok(())
    }

    /// Process definition-only modules using sema + def_module_names (no AST).
    fn process_def_only_modules(&mut self) {
        let def_names: Vec<String> = self.def_module_names.iter().cloned().collect();
        for mod_name in &def_names {
            self.process_def_types_for_module(mod_name);
        }
    }

    /// Process types/consts/enums from a definition module's scope using sema.
    fn process_def_types_for_module(&mut self, mod_name: &str) {
        use crate::types::Type;
        if let Some(scope_id) = self.sema.symtab.lookup_module_scope(mod_name) {
            let syms: Vec<_> = self.sema.symtab.symbols_in_scope(scope_id)
                .iter()
                .map(|s| (s.name.clone(), s.typ, s.kind.clone()))
                .collect();
            for (name, type_id, kind) in &syms {
                if matches!(kind, crate::symtab::SymbolKind::Type) {
                    let resolved_id = self.resolve_alias_id(*type_id);
                    let llvm_ty = self.llvm_type_for_type_id(resolved_id);
                    let llvm_ty = if llvm_ty.contains("void") && llvm_ty != "void" {
                        llvm_ty.replace("void", "i32")
                    } else { llvm_ty };
                    self.type_map.insert(name.clone(), llvm_ty.clone());
                    // Register under module-prefixed name
                    self.type_map.insert(format!("{}_{}", mod_name, name), llvm_ty);
                    // Copy record fields under both names
                    if let Type::Record { fields, .. } = self.sema.types.get(resolved_id) {
                        let mut field_list = Vec::new();
                        for (idx, f) in fields.iter().enumerate() {
                            let ft = self.llvm_type_for_type_id(f.typ);
                            field_list.push((f.name.clone(), ft, idx));
                        }
                        self.record_fields.insert(name.clone(), field_list.clone());
                        self.record_fields.insert(format!("{}_{}", mod_name, name), field_list);
                    }
                    // Register enum variants under Module_Variant names
                    if let Type::Enumeration { variants, .. } = self.sema.types.get(resolved_id) {
                        for (i, v) in variants.iter().enumerate() {
                            self.enum_variants.insert(format!("{}_{}", mod_name, v), i as i64);
                        }
                    }
                }
                // Process constants
                if let crate::symtab::SymbolKind::Constant(cv) = kind {
                    use crate::symtab::ConstValue;
                    match cv {
                        ConstValue::Integer(v) => {
                            self.const_values.insert(name.clone(), *v);
                            self.const_values.insert(self.mangle(name), *v);
                        }
                        ConstValue::Boolean(b) => {
                            let v = if *b { 1i64 } else { 0 };
                            self.const_values.insert(name.clone(), v);
                            self.const_values.insert(self.mangle(name), v);
                        }
                        ConstValue::Char(ch) => {
                            self.const_values.insert(name.clone(), *ch as i64);
                            self.const_values.insert(self.mangle(name), *ch as i64);
                        }
                        _ => {}
                    }
                }
                // Process variables from def module
                if matches!(kind, crate::symtab::SymbolKind::Variable) {
                    let resolved_id = self.resolve_alias_id(*type_id);
                    let llvm_ty = self.llvm_type_for_type_id(resolved_id);
                    let mangled_name = self.mangle(name);
                    if !self.globals.contains_key(&mangled_name) {
                        let global_name = format!("@{}", mangled_name);
                        let zero = self.llvm_zero_initializer(&llvm_ty);
                        self.emit_preambleln(&format!("{} = global {} {}", global_name, llvm_ty, zero));
                        self.globals.insert(name.clone(), (global_name.clone(), llvm_ty.clone()));
                        self.globals.insert(mangled_name, (global_name, llvm_ty.clone()));
                        // Track type metadata
                        let m2_type_name = self.type_name_for_id(*type_id);
                        if !m2_type_name.is_empty() {
                            self.var_type_names.insert(name.clone(), m2_type_name);
                        }
                        self.var_types.insert(name.clone(), *type_id);
                        if matches!(self.sema.types.get(resolved_id), Type::Array { .. }) {
                            self.array_vars.insert(name.clone());
                        }
                    }
                }
            }
        }
    }

    // ── Topological sorting ─────────────────────────────────────────

    /// Topologically sort module names using stored import lists.
    fn topo_sort_by_deps(&self, module_names: &[String]) -> CompileResult<Vec<String>> {
        let names: HashSet<String> = module_names.iter().cloned().collect();
        let mut deps: HashMap<String, Vec<String>> = HashMap::new();
        for mod_name in module_names {
            let mut my_deps: Vec<String> = Vec::new();
            if let Some(imports) = self.module_imports.get(mod_name) {
                for hi in imports {
                    if !hi.is_qualified && !hi.module.is_empty() {
                        if names.contains(&hi.module) && !my_deps.contains(&hi.module) {
                            my_deps.push(hi.module.clone());
                        }
                    } else {
                        for n in &hi.names {
                            if names.contains(&n.name) && !my_deps.contains(&n.name) {
                                my_deps.push(n.name.clone());
                            }
                        }
                    }
                }
            }
            deps.insert(mod_name.clone(), my_deps);
        }
        let mut visited = HashSet::new();
        let mut visiting = HashSet::new();
        let mut order = Vec::new();
        for name in module_names {
            Self::topo_visit(name, &deps, &mut visited, &mut visiting, &mut order)
                .map_err(|cycle_desc| {
                    CompileError::codegen(
                        crate::errors::SourceLoc::new("<codegen>", 0, 0),
                        format!("module dependency cycle detected: {}", cycle_desc),
                    )
                })?;
        }
        Ok(order)
    }

    fn topo_visit(
        name: &str,
        deps: &HashMap<String, Vec<String>>,
        visited: &mut HashSet<String>,
        visiting: &mut HashSet<String>,
        order: &mut Vec<String>,
    ) -> Result<(), String> {
        if visited.contains(name) { return Ok(()); }
        if visiting.contains(name) { return Err(name.to_string()); }
        visiting.insert(name.to_string());
        if let Some(d) = deps.get(name) {
            for dep in d {
                Self::topo_visit(dep, deps, visited, visiting, order)?;
            }
        }
        visiting.remove(name);
        visited.insert(name.to_string());
        order.push(name.to_string());
        Ok(())
    }

    // ── Import map building ─────────────────────────────────────────

    /// Build import map from prebuilt HIR imports (no AST dependency).
    pub(crate) fn build_import_map_from_hir(&mut self) {
        if let Some(ref hir) = self.prebuilt_hir {
            let hir_imports = hir.imports.clone();
            for imp in &hir_imports {
                if !imp.is_qualified {
                    // FROM Module IMPORT name1, name2;
                    for name in &imp.names {
                        self.import_map.insert(name.local_name.clone(), imp.module.clone());
                        if name.name != name.local_name {
                            self.import_alias_map.insert(name.local_name.clone(), name.name.clone());
                        }
                    }
                } else {
                    // IMPORT Module1, Module2;
                    for name in &imp.names {
                        self.imported_modules.insert(name.name.clone());
                    }
                }
            }
        }
    }

    /// Declare external functions needed by imports from prebuilt HIR.
    pub(crate) fn declare_imports_from_hir(&mut self) {
        let imports = if let Some(ref hir) = self.prebuilt_hir {
            hir.imports.clone()
        } else {
            return;
        };
        self.declare_imports_for(&imports);
    }

    /// Declare external functions for a list of HIR imports.
    fn declare_imports_for(&mut self, imports: &[crate::hir::HirImport]) {
        for imp in imports {
            if imp.is_qualified {
                // Whole-module import: IMPORT InOut
                for iname in &imp.names {
                    if stdlib::is_stdlib_module(&iname.name) && !stdlib::is_native_stdlib(&iname.name) {
                        let exports = stdlib::get_stdlib_exports(&iname.name);
                        for export in &exports {
                            self.declare_stdlib_function(&iname.name, export);
                        }
                    }
                }
            }
            if !imp.is_qualified && !imp.module.is_empty() {
                if stdlib::is_stdlib_module(&imp.module) && !stdlib::is_native_stdlib(&imp.module) {
                    for iname in &imp.names {
                        self.declare_stdlib_function(&imp.module, &iname.name);
                    }
                } else if self.foreign_modules.contains(imp.module.as_str()) {
                    // Foreign C modules — declare functions with proper signatures
                    for iname in &imp.names {
                        let bare_name = &iname.name;
                        if !self.declared_fns.contains(bare_name) {
                            if let Some(sym) = self.sema.symtab.lookup_any(bare_name) {
                                if let crate::symtab::SymbolKind::Procedure { params, return_type, .. } = &sym.kind {
                                    let ret_ty = if let Some(ret_id) = return_type {
                                        self.llvm_type_for_type_id(*ret_id)
                                    } else { "void".to_string() };
                                    let param_tys: Vec<String> = params.iter()
                                        .map(|p| {
                                            let ty = self.llvm_type_for_type_id(p.typ);
                                            if p.is_var || ty == "ptr" { "ptr".to_string() } else { ty }
                                        })
                                        .collect();
                                    let params_str = param_tys.join(", ");
                                    self.emit_preambleln(&format!("declare {} @{}({})", ret_ty, bare_name, params_str));
                                } else {
                                    self.emit_preambleln(&format!("declare i32 @{}(...)", bare_name));
                                }
                            } else {
                                self.emit_preambleln(&format!("declare i32 @{}(...)", bare_name));
                            }
                            self.declared_fns.insert(bare_name.clone());
                        }
                    }
                }
            }
        }
    }

    // ── Legacy AST-based methods (kept for gen_implementation_module) ─

    pub(crate) fn gen_embedded_impl_module(&mut self, imp: &ImplementationModule) -> CompileResult<()> {
        let saved_module = self.module_name.clone();
        let saved_import_map = self.import_map.clone();
        let saved_import_alias_map = self.import_alias_map.clone();
        let saved_imported_modules = self.imported_modules.clone();

        self.module_name = imp.name.clone();

        self.build_import_map(&imp.imports);
        self.declare_imports(&imp.imports);

        // Process type/const/var declarations from the definition module first
        if let Some(def_mod) = self.def_modules.get(&imp.name).cloned() {
            let def_decls: Vec<Declaration> = def_mod.definitions.iter().filter_map(|d| {
                match d {
                    Definition::Type(td) => Some(Declaration::Type(td.clone())),
                    Definition::Const(cd) => Some(Declaration::Const(cd.clone())),
                    Definition::Var(vd) => Some(Declaration::Var(vd.clone())),
                    _ => None,
                }
            }).collect();
            self.gen_const_decls(&def_decls);
            self.gen_type_decls(&def_decls);
            self.gen_var_decls_global(&def_decls);

            for d in &def_mod.definitions {
                if let Definition::Type(td) = d {
                    if let Some(ty) = self.type_map.get(&td.name).cloned() {
                        let prefixed = format!("{}_{}", imp.name, td.name);
                        self.type_map.insert(prefixed.clone(), ty);
                        if let Some(fields) = self.record_fields.get(&td.name).cloned() {
                            self.record_fields.insert(prefixed, fields);
                        }
                    }
                }
            }
        }

        self.gen_type_decls(&imp.block.decls);
        self.gen_const_decls(&imp.block.decls);
        self.gen_exception_decls(&imp.block.decls);
        self.gen_var_decls_global(&imp.block.decls);

        for decl in &imp.block.decls {
            if let Declaration::Procedure(p) = decl {
                let name = self.mangle(&p.heading.name);
                self.declared_fns.insert(name);
                self.declared_fns.insert(p.heading.name.clone());
            }
        }
        for decl in &imp.block.decls {
            if let Declaration::Procedure(p) = decl {
                self.gen_proc_decl(p)?;
            }
        }

        if let Some(stmts) = &imp.block.body {
            if !stmts.is_empty() {
                let init_name = format!("{}_init", imp.name);
                self.emitln(&format!("define void @{}() {{", init_name));
                self.emitln("bb.entry:");
                self.in_function = true;
                self.tmp_counter = 0;
                self.locals.push(HashMap::new());

                let mut hb = self.make_hir_builder();
                let hir_stmts = hb.lower_stmts(stmts);
                self.gen_hir_statements(&hir_stmts);

                self.emitln("  ret void");
                self.emitln("}");
                self.locals.pop();
                self.in_function = false;
                self.embedded_init_modules.push(imp.name.clone());
            }
        }

        self.module_name = saved_module;
        self.import_map = saved_import_map;
        self.import_alias_map = saved_import_alias_map;
        self.imported_modules = saved_imported_modules;
        Ok(())
    }

    /// Topologically sort implementation modules by dependency order (legacy AST-based).
    pub(crate) fn topo_sort_impl_modules(&self, modules: Vec<ImplementationModule>) -> Vec<ImplementationModule> {
        let mod_map: HashMap<String, &ImplementationModule> = modules.iter()
            .map(|m| (m.name.clone(), m))
            .collect();
        let mut visited = HashSet::new();
        let mut sorted_names = Vec::new();

        fn visit(
            name: &str,
            mod_map: &HashMap<String, &ImplementationModule>,
            visited: &mut HashSet<String>,
            sorted: &mut Vec<String>,
        ) {
            if visited.contains(name) { return; }
            visited.insert(name.to_string());
            if let Some(m) = mod_map.get(name) {
                for imp in &m.imports {
                    if let Some(ref from_mod) = imp.from_module {
                        visit(from_mod, mod_map, visited, sorted);
                    } else {
                        for n in &imp.names {
                            visit(&n.name, mod_map, visited, sorted);
                        }
                    }
                }
            }
            sorted.push(name.to_string());
        }

        for name in mod_map.keys() {
            visit(name, &mod_map, &mut visited, &mut sorted_names);
        }

        sorted_names.iter()
            .filter_map(|name| modules.iter().find(|m| m.name == *name).cloned())
            .collect()
    }

    pub(crate) fn build_import_map(&mut self, imports: &[Import]) {
        for imp in imports {
            if let Some(ref from_mod) = imp.from_module {
                for iname in &imp.names {
                    let local = iname.local_name().to_string();
                    self.import_map.insert(local.clone(), from_mod.clone());
                    if let Some(ref alias) = iname.alias {
                        self.import_alias_map.insert(alias.clone(), iname.name.clone());
                    }
                }
            } else {
                for iname in &imp.names {
                    self.imported_modules.insert(iname.name.clone());
                }
            }
        }
    }

    /// Resolve a name to its module-qualified form.
    pub(crate) fn resolve_name(&self, name: &str) -> String {
        if let Some(module) = self.import_map.get(name) {
            let orig = self.import_alias_map.get(name).cloned().unwrap_or_else(|| name.to_string());
            if stdlib::is_stdlib_module(module) || self.foreign_modules.contains(module.as_str()) {
                return format!("{}_{}", module, orig);
            }
            return format!("{}_{}", module, orig);
        }
        name.to_string()
    }

    /// Declare external functions needed by imported modules (legacy AST-based).
    pub(crate) fn declare_imports(&mut self, imports: &[Import]) {
        for imp in imports {
            if imp.from_module.is_none() {
                for iname in &imp.names {
                    if stdlib::is_stdlib_module(&iname.name) && !stdlib::is_native_stdlib(&iname.name) {
                        let exports = stdlib::get_stdlib_exports(&iname.name);
                        for export in &exports {
                            self.declare_stdlib_function(&iname.name, export);
                        }
                    }
                }
            }
            if let Some(ref from_mod) = imp.from_module {
                if stdlib::is_stdlib_module(from_mod) && !stdlib::is_native_stdlib(from_mod) {
                    for iname in &imp.names {
                        self.declare_stdlib_function(from_mod, &iname.name);
                    }
                } else if self.foreign_modules.contains(from_mod.as_str()) {
                    for iname in &imp.names {
                        let bare_name = &iname.name;
                        if !self.declared_fns.contains(bare_name) {
                            if let Some(sym) = self.sema.symtab.lookup_any(bare_name) {
                                if let crate::symtab::SymbolKind::Procedure { params, return_type, .. } = &sym.kind {
                                    let ret_ty = if let Some(ret_id) = return_type {
                                        self.llvm_type_for_type_id(*ret_id)
                                    } else { "void".to_string() };
                                    let param_tys: Vec<String> = params.iter()
                                        .map(|p| {
                                            let ty = self.llvm_type_for_type_id(p.typ);
                                            if p.is_var || ty == "ptr" { "ptr".to_string() } else { ty }
                                        })
                                        .collect();
                                    let params_str = param_tys.join(", ");
                                    self.emit_preambleln(&format!("declare {} @{}({})", ret_ty, bare_name, params_str));
                                } else {
                                    self.emit_preambleln(&format!("declare i32 @{}(...)", bare_name));
                                }
                            } else {
                                self.emit_preambleln(&format!("declare i32 @{}(...)", bare_name));
                            }
                            self.declared_fns.insert(bare_name.clone());
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn declare_runtime_helpers(&mut self) {
        // printf for basic I/O fallback
        if !self.declared_fns.contains("printf") {
            self.emit_preambleln("declare i32 @printf(ptr, ...) nounwind");
            self.declared_fns.insert("printf".to_string());
        }
        // Memory functions
        if !self.declared_fns.contains("malloc") {
            self.emit_preambleln("declare noalias ptr @malloc(i64) nounwind");
            self.declared_fns.insert("malloc".to_string());
        }
        if !self.declared_fns.contains("free") {
            self.emit_preambleln("declare void @free(ptr nocapture) nounwind");
            self.declared_fns.insert("free".to_string());
        }
        if !self.declared_fns.contains("memcpy") {
            self.emit_preambleln("declare ptr @memcpy(ptr, ptr, i64) nounwind");
            self.declared_fns.insert("memcpy".to_string());
        }
        if !self.declared_fns.contains("memset") {
            self.emit_preambleln("declare ptr @memset(ptr, i32, i64) nounwind");
            self.declared_fns.insert("memset".to_string());
        }
        if !self.declared_fns.contains("exit") {
            self.emit_preambleln("declare void @exit(i32) noreturn nounwind");
            self.declared_fns.insert("exit".to_string());
        }
        if !self.declared_fns.contains("strcmp") {
            self.emit_preambleln("declare i32 @strcmp(ptr nocapture, ptr nocapture) nounwind readonly");
            self.declared_fns.insert("strcmp".to_string());
        }
        if !self.declared_fns.contains("strcpy") {
            self.emit_preambleln("declare ptr @strcpy(ptr, ptr nocapture) nounwind");
            self.declared_fns.insert("strcpy".to_string());
        }
        if !self.declared_fns.contains("strlen") {
            self.emit_preambleln("declare i64 @strlen(ptr nocapture) nounwind readonly");
            self.declared_fns.insert("strlen".to_string());
        }
        // Stack trace support
        if !self.declared_fns.contains("m2_stack_push") {
            self.emit_preambleln("%m2_StackFrame = type { ptr, ptr, ptr, i32 }");
            self.emit_preambleln("@m2_frame_stack = external thread_local global ptr");
            self.emit_preambleln("declare void @m2_stack_push(ptr, ptr, ptr) nounwind");
            self.emit_preambleln("declare void @m2_stack_pop(ptr) nounwind");
            self.declared_fns.insert("m2_stack_push".to_string());
            self.declared_fns.insert("m2_stack_pop".to_string());
        }
        // HALT runtime
        if !self.declared_fns.contains("m2_halt") {
            self.emit_preambleln("declare void @m2_halt() noreturn nounwind");
            self.declared_fns.insert("m2_halt".to_string());
        }
    }
}
