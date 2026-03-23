use super::*;

impl LLVMCodeGen {
    // ── Program module generation ───────────────────────────────────

    pub(crate) fn gen_program_module(&mut self, m: &ProgramModule) -> CompileResult<()> {
        self.module_name = m.name.clone();
        self.source_file = m.loc.file.clone();
        self.build_import_map(&m.imports);

        // Initialize debug compile unit
        if let Some(ref mut di) = self.di {
            di.create_compile_unit(&m.loc.file);
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

        // Declare stdlib imports
        self.declare_imports(&m.imports);

        // Declare runtime helpers
        self.declare_runtime_helpers();

        // Process definition-only modules (types, consts, enums)
        // These modules have no .mod file but their types/consts are needed
        for def_mod in self.def_modules.values().cloned().collect::<Vec<_>>() {
            let def_decls: Vec<Declaration> = def_mod.definitions.iter().filter_map(|d| {
                match d {
                    Definition::Type(td) => Some(Declaration::Type(td.clone())),
                    Definition::Const(cd) => Some(Declaration::Const(cd.clone())),
                    _ => None,
                }
            }).collect();
            self.gen_const_decls(&def_decls);
            self.gen_type_decls(&def_decls);
            // Register enum variants and types under module-prefixed names
            for d in &def_mod.definitions {
                if let Definition::Type(td) = d {
                    if let Some(ty) = self.type_map.get(&td.name).cloned() {
                        self.type_map.insert(format!("{}_{}", def_mod.name, td.name), ty);
                    }
                    if let Some(fields) = self.record_fields.get(&td.name).cloned() {
                        self.record_fields.insert(format!("{}_{}", def_mod.name, td.name), fields);
                    }
                    // Register enum variants under Module_Variant names
                    if let Some(ref tn) = td.typ {
                        if let TypeNode::Enumeration { variants, .. } = tn {
                            for (i, v) in variants.iter().enumerate() {
                                self.enum_variants.insert(
                                    format!("{}_{}", def_mod.name, v), i as i64);
                            }
                        }
                    }
                }
            }
        }

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
        if let Some(pending) = self.pending_modules.take() {
            let sorted = self.topo_sort_impl_modules(pending);
            for imp_mod in &sorted {
                self.gen_embedded_impl_module(imp_mod)?;
            }
        }

        // Global declarations
        self.gen_const_decls(&m.block.decls);
        self.gen_exception_decls(&m.block.decls);
        self.gen_type_decls(&m.block.decls);
        self.gen_var_decls_global(&m.block.decls);

        // Procedure declarations (including from nested modules)
        for decl in &m.block.decls {
            match decl {
                Declaration::Procedure(p) => {
                    self.gen_proc_decl(p)?;
                }
                Declaration::Module(local_mod) => {
                    // Nested MODULE — process its declarations
                    self.gen_const_decls(&local_mod.block.decls);
                    self.gen_type_decls(&local_mod.block.decls);
                    self.gen_var_decls_global(&local_mod.block.decls);
                    for d in &local_mod.block.decls {
                        if let Declaration::Procedure(p) = d {
                            self.gen_proc_decl(p)?;
                        }
                    }
                }
                _ => {}
            }
        }

        // Generate main function
        let main_dbg = if let Some(ref mut di) = self.di {
            let sp = di.create_subprogram("main", "main", &m.loc.file, m.loc.line);
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

        // Store argc/argv — defined in the runtime C file, declared here as external
        self.emit_preambleln("@m2_argc = external global i32");
        self.emit_preambleln("@m2_argv = external global ptr");
        self.emitln("  store i32 %argc, ptr @m2_argc");
        self.emitln("  store ptr %argv, ptr @m2_argv");

        // Call embedded module init functions
        for mod_name in &self.embedded_init_modules.clone() {
            self.emitln(&format!("  call void @{}_init()", mod_name));
        }

        // Module body (with ISO EXCEPT/FINALLY support)
        let has_except = m.block.except.is_some();
        let has_finally = m.block.finally.is_some();

        if has_except || has_finally {
            self.declare_exc_runtime();
            // Wrap body in SjLj guard
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
            if let Some(stmts) = &m.block.body {
                for stmt in stmts {
                    self.gen_statement(stmt);
                }
            }
            self.in_sjlj_context = false;
            self.emitln(&format!("  call void @m2_exc_pop(ptr {})", frame));
            let end_label = self.next_label("mod.end");
            self.emitln(&format!("  br label %{}", end_label));

            self.emitln(&format!("{}:", except_label));
            self.emitln(&format!("  call void @m2_exc_pop(ptr {})", frame));
            if let Some(except_stmts) = &m.block.except {
                for stmt in except_stmts {
                    self.gen_statement(stmt);
                }
            }
            self.emitln(&format!("  br label %{}", end_label));

            self.emitln(&format!("{}:", end_label));
            if let Some(finally_stmts) = &m.block.finally {
                for stmt in finally_stmts {
                    self.gen_statement(stmt);
                }
            }
        } else {
            if let Some(stmts) = &m.block.body {
                for stmt in stmts {
                    self.gen_statement(stmt);
                }
            }
        }

        // Ensure we have a terminator
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
        self.gen_const_decls(&m.block.decls);
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

                for stmt in stmts {
                    self.gen_statement(stmt);
                }

                self.emitln("  ret void");
                self.emitln("}");
                self.locals.pop();
                self.in_function = false;
            }
        }

        Ok(())
    }

    pub(crate) fn gen_embedded_impl_module(&mut self, imp: &ImplementationModule) -> CompileResult<()> {
        let saved_module = self.module_name.clone();
        let saved_import_map = self.import_map.clone();
        let saved_import_alias_map = self.import_alias_map.clone();

        self.module_name = imp.name.clone();
        // Note: register_impl_types already called in finalize_all_impl_modules
        // before build_type_lowering, so types are available in sema.

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

            // Also register type info under module-prefixed names for cross-module access
            for d in &def_mod.definitions {
                if let Definition::Type(td) = d {
                    if let Some(ty) = self.type_map.get(&td.name).cloned() {
                        let prefixed = format!("{}_{}", imp.name, td.name);
                        self.type_map.insert(prefixed.clone(), ty);
                        // Copy record fields under prefixed name too
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
                self.gen_proc_decl(p)?;
            }
        }

        // Generate init function if there's a module body
        if let Some(stmts) = &imp.block.body {
            if !stmts.is_empty() {
                let init_name = format!("{}_init", imp.name);
                self.emitln(&format!("define void @{}() {{", init_name));
                self.emitln("bb.entry:");
                self.in_function = true;
                self.tmp_counter = 0;
                self.locals.push(HashMap::new());

                for stmt in stmts {
                    self.gen_statement(stmt);
                }

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
        Ok(())
    }

    /// Topologically sort implementation modules by dependency order.
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

    // ── Import map building ─────────────────────────────────────────

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
        // Check import map
        if let Some(module) = self.import_map.get(name) {
            let orig = self.import_alias_map.get(name).cloned().unwrap_or_else(|| name.to_string());
            if stdlib::is_stdlib_module(module) || self.foreign_modules.contains(module.as_str()) {
                return format!("{}_{}", module, orig);
            }
            return format!("{}_{}", module, orig);
        }
        name.to_string()
    }

    /// Declare external functions needed by imported modules.
    pub(crate) fn declare_imports(&mut self, imports: &[Import]) {
        for imp in imports {
            if imp.from_module.is_none() {
                // Whole-module import: IMPORT InOut
                // For non-native stdlib modules, declare exported function signatures.
                // Native stdlib modules are compiled inline — gen_proc_decl handles everything.
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
                    // Foreign C modules — declare functions with proper signatures
                    for iname in &imp.names {
                        let bare_name = &iname.name;
                        if !self.declared_fns.contains(bare_name) {
                            // Build proper signature from symbol table
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
        // Note: #dbg_declare records (LLVM 19+) don't need a function declaration
    }
}
