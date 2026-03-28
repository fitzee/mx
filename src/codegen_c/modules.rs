use super::*;

impl CodeGen {
    pub(crate) fn gen_program_module(&mut self) -> CompileResult<()> {
        // module_name and import_map already set by post_sema_generate
        self.emit_preamble_for_imports()?;

        let mod_name = self.module_name.clone();
        if self.multi_tu {
            self.emit(&format!("/* MX_MAIN_BEGIN {} */\n", mod_name));
        }
        self.emitln(&format!("/* Module {} */", mod_name));
        self.newline();

        // Structural declarations from prebuilt HIR
        self.emit_hir_record_forward_decls();
        self.emit_hir_type_decls();
        self.emit_hir_const_decls();

        // Emit M2+ type descriptors (after all types are declared)
        if self.m2plus {
            self.emit_type_descs();
        }

        // Forward declarations for procedures from HIR (includes local module procs)
        self.emit_hir_forward_decls();
        self.newline();

        // Emit global variable declarations from HIR (includes local module vars)
        self.emit_hir_global_decls();
        // Emit procedure bodies from HIR proc declarations (non-nested only)
        let proc_names: Vec<String> = self.prebuilt_hir.as_ref()
            .map(|hir| hir.proc_decls.iter()
                .filter(|pd| !pd.sig.is_nested)
                .map(|pd| pd.sig.name.clone())
                .collect())
            .unwrap_or_default();
        for name in &proc_names {
            self.gen_proc_by_name(name);
        }

        // ISO Modula-2: generate FINALLY handler from prebuilt HIR
        let finally_body = self.prebuilt_hir.as_ref().and_then(|h| h.finally_handler.clone());
        if let Some(stmts) = finally_body {
            self.emitln("static void m2_finally_handler(void) {");
            self.indent += 1;
            for stmt in &stmts { self.emit_hir_stmt(stmt); }
            self.indent -= 1;
            self.emitln("}");
            self.newline();
        }

        // ISO Modula-2: generate EXCEPT handler from prebuilt HIR
        let except_body = self.prebuilt_hir.as_ref().and_then(|h| h.except_handler.clone());
        if let Some(stmts) = except_body {
            self.emitln("static void m2_except_handler(void) {");
            self.indent += 1;
            for stmt in &stmts { self.emit_hir_stmt(stmt); }
            self.indent -= 1;
            self.emitln("}");
            self.newline();
        }

        // Generate main function
        self.emitln("int main(int _m2_argc, char **_m2_argv) {");
        self.indent += 1;
        self.emitln("m2_argc = _m2_argc; m2_argv = _m2_argv;");
        if self.debug_mode {
            self.emitln("setvbuf(stdout, NULL, _IONBF, 0);");
        }

        // Register FINALLY handler with atexit
        let has_finally = self.prebuilt_hir.as_ref()
            .and_then(|h| h.finally_handler.as_ref()).is_some();
        if has_finally {
            self.emitln("atexit(m2_finally_handler);");
        }

        // Call embedded module init functions (in dependency order)
        for mod_name in &self.embedded_init_modules.clone() {
            self.emitln(&format!("{}_init();", mod_name));
        }

        // Initialize local (nested) modules — run their BEGIN bodies from HIR
        let local_inits = self.prebuilt_hir.as_ref()
            .map(|h| h.local_module_inits.clone())
            .unwrap_or_default();
        for (mod_name, stmts) in &local_inits {
            self.emitln(&format!("/* Init local module {} */", mod_name));
            for stmt in stmts { self.emit_hir_stmt(stmt); }
        }

        self.in_module_body = true;
        // Use prebuilt HIR init body
        let prebuilt_init = self.prebuilt_hir.as_ref()
            .and_then(|hir| hir.init_body.clone());
        if let Some(body) = prebuilt_init {
            for stmt in &body {
                self.emit_hir_stmt(stmt);
            }
        }
        self.in_module_body = false;
        self.emitln("return 0;");
        self.indent -= 1;
        self.emitln("}");
        if self.multi_tu {
            self.emit("/* MX_MAIN_END */\n");
        }
        Ok(())
    }

    pub(crate) fn gen_definition_module(&mut self) {
        let mod_name = self.module_name.clone();
        self.emitln(&format!("/* Definition Module {} */", mod_name));
        self.emitln(&format!("#ifndef {}_H", mod_name.to_uppercase()));
        self.emitln(&format!("#define {}_H", mod_name.to_uppercase()));
        self.newline();

        // Forward struct declarations for record types (and POINTER TO RECORD)
        let def_scope = self.sema.symtab.lookup_module_scope(&mod_name);
        {
            let type_syms: Vec<(String, crate::types::TypeId)> = def_scope.map(|scope_id| {
                self.sema.symtab.symbols_in_scope(scope_id).iter()
                    .filter(|s| matches!(s.kind, crate::symtab::SymbolKind::Type))
                    .map(|s| (s.name.clone(), s.typ))
                    .collect()
            }).unwrap_or_default();
            for (name, tid) in &type_syms {
                let resolved = self.resolve_hir_alias(*tid);
                let cn = self.mangle(name);
                match self.sema.types.get(resolved) {
                    crate::types::Type::Record { .. } => {
                        self.emitln(&format!("typedef struct {} {};", cn, cn));
                    }
                    crate::types::Type::Pointer { base } => {
                        let base_resolved = self.resolve_hir_alias(*base);
                        if matches!(self.sema.types.get(base_resolved), crate::types::Type::Record { .. }) {
                            let tag = format!("{}_r", cn);
                            self.emitln(&format!("typedef struct {} {};", tag, tag));
                            self.emitln(&format!("typedef {} *{};", tag, cn));
                        }
                    }
                    _ => {}
                }
            }
        }
        // Emit declarations from sema scope (no AST Definition iteration)
        if let Some(scope_id) = def_scope {
            // Collect symbols to avoid borrow conflict with self.emit*
            let syms: Vec<(String, crate::symtab::SymbolKind, crate::types::TypeId, bool)> =
                self.sema.symtab.symbols_in_scope(scope_id).iter()
                    .map(|s| (s.name.clone(), s.kind.clone(), s.typ, s.exported))
                    .collect();
            for (name, kind, typ, exported) in &syms {
                match kind {
                    crate::symtab::SymbolKind::Constant(cv) => {
                        let val = crate::hir_build::const_value_to_hir(cv);
                        let hc = crate::hir::HirConstDecl {
                            name: name.clone(),
                            mangled: self.mangle(name),
                            value: val.clone(),
                            type_id: *typ,
                            exported: *exported,
                            c_type: crate::hir_build::const_val_c_type(&val),
                        };
                        self.gen_hir_const_decl(&hc);
                    }
                    crate::symtab::SymbolKind::Type => {
                        if *typ != crate::types::TY_VOID {
                            self.gen_type_decl_from_id(name, *typ);
                        }
                    }
                    crate::symtab::SymbolKind::Variable => {
                        let ctype = self.type_id_to_c(*typ);
                        let arr_suffix = self.type_id_array_suffix(*typ);
                        self.emit_indent();
                        self.emitln(&format!("extern {} {}{};", ctype, self.mangle(name), arr_suffix));
                    }
                    crate::symtab::SymbolKind::Procedure { .. } => {
                        // Build HirProcSig from sema scope
                        if let Some(sym) = self.sema.symtab.lookup_in_scope(scope_id, name) {
                            if let crate::symtab::SymbolKind::Procedure { params, return_type, .. } = &sym.kind {
                                let sig = crate::hir::HirProcSig {
                                    name: name.clone(),
                                    mangled: format!("{}_{}", mod_name, name),
                                    module: mod_name.clone(),
                                    params: params.iter().map(|p| {
                                        let resolved = self.resolve_hir_alias(p.typ);
                                        let is_open = matches!(self.sema.types.get(resolved), crate::types::Type::OpenArray { .. });
                                        crate::hir::HirParamDecl {
                                            name: p.name.clone(),
                                            type_id: p.typ,
                                            is_var: p.is_var,
                                            is_open_array: is_open,
                                            is_proc_type: matches!(self.sema.types.get(resolved), crate::types::Type::ProcedureType { .. }),
                                            is_char: p.typ == crate::types::TY_CHAR,
                                            needs_high: is_open,
                                        }
                                    }).collect(),
                                    return_type: *return_type,
                                    exported: *exported,
                                    is_foreign: false,
                                    export_c_name: None,
                                    is_nested: false,
                                    parent_proc: None,
                                    has_closure_env: false,
                                };
                                self.register_hir_proc_params(&sig);
                                self.gen_hir_proc_prototype(&sig);
                                self.emit(";\n");
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        self.newline();
        self.emitln("#endif");
    }

    pub(crate) fn gen_implementation_module(&mut self) -> CompileResult<()> {
        // module_name, import_map, unit_proc_decls already set by post_sema_generate
        self.emit_preamble_for_imports()?;

        let mod_name = self.module_name.clone();
        self.emitln(&format!("/* Implementation Module {} */", mod_name));
        self.newline();

        // Emit types and constants from the corresponding definition module.
        // The implementation module's scope includes all .def exports, but
        // m2c must emit them explicitly in the generated C.
        let impl_type_names: std::collections::HashSet<String> = if let Some(ref hir) = self.prebuilt_hir {
            hir.type_decls.iter().map(|td| td.name.clone()).collect()
        } else {
            std::collections::HashSet::new()
        };
        if let Some(def_mod) = self.def_modules.get(&mod_name).cloned() {
            // Forward struct declarations from the definition module via sema types
            let def_scope = self.sema.symtab.lookup_module_scope(&mod_name);
            let def_types: Vec<(String, crate::types::TypeId)> = def_scope.map(|scope_id| {
                self.sema.symtab.symbols_in_scope(scope_id).iter()
                    .filter(|s| matches!(s.kind, crate::symtab::SymbolKind::Type))
                    .map(|s| (s.name.clone(), s.typ))
                    .collect()
            }).unwrap_or_default();
            for (name, tid) in &def_types {
                if !impl_type_names.contains(name) {
                    let resolved = self.resolve_hir_alias(*tid);
                    let cn = self.mangle(name);
                    match self.sema.types.get(resolved) {
                        crate::types::Type::Record { .. } => {
                            self.emitln(&format!("typedef struct {} {};", cn, cn));
                        }
                        crate::types::Type::Pointer { base } => {
                            let base_resolved = self.resolve_hir_alias(*base);
                            if matches!(self.sema.types.get(base_resolved), crate::types::Type::Record { .. }) {
                                let tag = format!("{}_r", cn);
                                self.emitln(&format!("typedef struct {} {};", tag, tag));
                                self.emitln(&format!("typedef {} *{};", tag, cn));
                            }
                        }
                        _ => {}
                    }
                }
            }
            // Emit type and const declarations from the definition module via sema scope
            {
                let def_scope = self.sema.symtab.lookup_module_scope(&mod_name);
                let def_syms: Vec<(String, crate::symtab::SymbolKind, crate::types::TypeId, bool)> =
                    def_scope.map(|sid| {
                        self.sema.symtab.symbols_in_scope(sid).iter()
                            .map(|s| (s.name.clone(), s.kind.clone(), s.typ, s.exported))
                            .collect()
                    }).unwrap_or_default();
                let mod_name = mod_name.clone();
                for (name, kind, typ, exported) in &def_syms {
                    match kind {
                        crate::symtab::SymbolKind::Type if !impl_type_names.contains(name) => {
                            if *typ != crate::types::TY_VOID {
                                self.gen_type_decl_from_id(name, *typ);
                            }
                        }
                        crate::symtab::SymbolKind::Constant(cv) => {
                            let val = crate::hir_build::const_value_to_hir(cv);
                            let hc = crate::hir::HirConstDecl {
                                name: name.clone(),
                                mangled: format!("{}_{}", mod_name, name),
                                value: val.clone(),
                                type_id: *typ,
                                exported: *exported,
                                c_type: crate::hir_build::const_val_c_type(&val),
                            };
                            self.gen_hir_const_decl(&hc);
                        }
                        _ => {}
                    }
                }
            }
        }

        // Structural declarations from prebuilt HIR
        self.emit_hir_record_forward_decls();
        self.emit_hir_type_decls();
        self.emit_hir_const_decls();

        // Emit M2+ type descriptors (after all types are declared)
        if self.m2plus {
            self.emit_type_descs();
        }

        self.emit_hir_forward_decls();
        self.newline();

        // Pass 1: Emit global variable declarations from HIR
        self.emit_hir_global_decls();
        // Emit procedure bodies from HIR proc declarations (non-nested only)
        let proc_names: Vec<String> = self.prebuilt_hir.as_ref()
            .map(|hir| hir.proc_decls.iter()
                .filter(|pd| !pd.sig.is_nested)
                .map(|pd| pd.sig.name.clone())
                .collect())
            .unwrap_or_default();
        for name in &proc_names {
            self.gen_proc_by_name(name);
        }

        // Module body = initialization function
        let has_init = self.prebuilt_hir.as_ref()
            .and_then(|h| h.init_body.as_ref()).is_some();
        if has_init {
            self.emitln(&format!("void {}_init(void) {{", self.mangle(&mod_name)));
            self.indent += 1;
            // Use prebuilt HIR init body
            if let Some(body) = self.prebuilt_hir.as_ref().and_then(|h| h.init_body.clone()) {
                for stmt in &body { self.emit_hir_stmt(stmt); }
            }
            self.indent -= 1;
            self.emitln("}");
        }
        Ok(())
    }

    // ── Forward declarations ────────────────────────────────────────

    /// Generate C code for an imported implementation module, embedded in the main program.
    /// All top-level procedure names are prefixed with `ModuleName_`.
    pub(crate) fn gen_embedded_implementation(&mut self, imp: &ImplementationModule) {
        let ctx = self.save_embedded_context();

        // Look up the HirEmbeddedModule for this implementation module
        let hir_emb = self.prebuilt_hir.as_ref().and_then(|hir| {
            hir.embedded_modules.iter().find(|e| e.name == imp.name).cloned()
        });

        self.module_name = imp.name.clone();
        self.import_map.clear();
        self.import_alias_map.clear();
        if let Some(def_mod) = self.def_modules.get(&imp.name).cloned() {
            self.build_import_map(&def_mod.imports);
        }
        self.build_import_map(&imp.imports);

        // Track local procedure and variable names from HIR
        self.embedded_local_procs.clear();
        self.embedded_local_vars.clear();
        if let Some(ref emb) = hir_emb {
            for pd in &emb.procedures {
                if pd.sig.export_c_name.is_none() {
                    self.embedded_local_procs.insert(pd.sig.name.clone());
                }
            }
            for g in &emb.global_decls {
                self.embedded_local_vars.insert(g.name.clone());
            }
            for c in &emb.const_decls {
                self.embedded_local_vars.insert(c.name.clone());
            }
        }

        if self.multi_tu {
            self.emit(&format!("/* MX_MODULE_BEGIN {} */\n", imp.name));
        }
        self.emitln(&format!("/* Imported Module {} */", imp.name));
        self.newline();

        // Set generating_for_module early so all types get module-prefixed C names,
        // including forward struct declarations.
        self.generating_for_module = Some(imp.name.clone());

        // Pre-register ALL type names from this embedded module in embedded_enum_types.
        // This is needed so that forward references (e.g., POINTER TO Record declared
        // before the Record type) resolve to the correct prefixed C name.
        if let Some(scope_id) = self.sema.symtab.lookup_module_scope(&imp.name) {
            let type_names: Vec<String> = self.sema.symtab.symbols_in_scope(scope_id).iter()
                .filter(|s| matches!(s.kind, crate::symtab::SymbolKind::Type))
                .map(|s| s.name.clone())
                .collect();
            for name in &type_names {
                let prefixed = format!("{}_{}", imp.name, self.mangle(name));
                self.embedded_enum_types.insert(prefixed);
            }
        }
        if let Some(ref emb) = hir_emb {
            for td in &emb.type_decls {
                let prefixed = format!("{}_{}", imp.name, self.mangle(&td.name));
                self.embedded_enum_types.insert(prefixed);
            }
        }

        // Forward declare all record types as structs (to allow pointer-to-struct typedefs)

        // From the definition module:
        if let Some(def_mod) = self.def_modules.get(&imp.name).cloned() {
            let impl_type_names: HashSet<String> = hir_emb.as_ref()
                .map(|emb| emb.type_decls.iter().map(|td| td.name.clone()).collect())
                .unwrap_or_default();
            {
                let def_scope = self.sema.symtab.lookup_module_scope(&imp.name);
                let def_types: Vec<(String, crate::types::TypeId)> = def_scope.map(|sid| {
                    self.sema.symtab.symbols_in_scope(sid).iter()
                        .filter(|s| matches!(s.kind, crate::symtab::SymbolKind::Type)
                            && !impl_type_names.contains(&s.name))
                        .map(|s| (s.name.clone(), s.typ))
                        .collect()
                }).unwrap_or_default();
                for (name, tid) in &def_types {
                    let resolved = self.resolve_hir_alias(*tid);
                    if matches!(self.sema.types.get(resolved), crate::types::Type::Record { .. }) {
                        let cn = self.type_decl_c_name(name);
                        self.emitln(&format!("typedef struct {} {};", cn, cn));
                    }
                }
            }
        }
        // From the implementation block — record forward decls via TypeId
        // Only emit for direct Record types. Pointer-to-Record with inline
        // record body is handled by gen_type_decl; pointer-to-named-record
        // uses the named record's own forward decl.
        if let Some(ref emb) = hir_emb {
            for td in &emb.type_decls {
                let resolved = self.resolve_hir_alias(td.type_id);
                if matches!(self.sema.types.get(resolved), crate::types::Type::Record { .. }) {
                    let cn = self.type_decl_c_name(&td.name);
                    self.emitln(&format!("typedef struct {} {};", cn, cn));
                }
            }
        }

        // Emit type and const declarations from the def module via sema scope
        if self.def_modules.contains_key(&imp.name) {
            let def_scope = self.sema.symtab.lookup_module_scope(&imp.name);
            let def_syms: Vec<(String, crate::symtab::SymbolKind, crate::types::TypeId, bool)> =
                def_scope.map(|sid| {
                    self.sema.symtab.symbols_in_scope(sid).iter()
                        .map(|s| (s.name.clone(), s.kind.clone(), s.typ, s.exported))
                        .collect()
                }).unwrap_or_default();
            // Register def-module constants and exported VARs as local vars
            for (name, kind, _, _) in &def_syms {
                match kind {
                    crate::symtab::SymbolKind::Constant(_) => {
                        self.embedded_local_vars.insert(name.clone());
                    }
                    crate::symtab::SymbolKind::Variable => {
                        self.embedded_local_vars.insert(name.clone());
                    }
                    _ => {}
                }
            }
            let impl_type_names: HashSet<String> = if let Some(ref emb) = hir_emb {
                emb.type_decls.iter().map(|td| td.name.clone()).collect()
            } else {
                HashSet::new()
            };
            let imp_name = imp.name.clone();
            for (name, kind, typ, exported) in &def_syms {
                match kind {
                    crate::symtab::SymbolKind::Type if !impl_type_names.contains(name) => {
                        if *typ != crate::types::TY_VOID {
                            self.gen_type_decl_from_id(name, *typ);
                        }
                    }
                    crate::symtab::SymbolKind::Constant(cv) => {
                        let val = crate::hir_build::const_value_to_hir(cv);
                        let hc = crate::hir::HirConstDecl {
                            name: name.clone(),
                            mangled: format!("{}_{}", imp_name, name),
                            value: val.clone(),
                            type_id: *typ,
                            exported: *exported,
                            c_type: crate::hir_build::const_val_c_type(&val),
                        };
                        self.gen_hir_const_decl(&hc);
                    }
                    _ => {}
                }
            }
            // Exception declarations from the definition module (M2+ only)
            if let Some(def_mod) = self.def_modules.get(&imp.name).cloned() {
                for d in &def_mod.definitions {
                    if let Definition::Exception(e) = d {
                        self.exception_names.insert(e.name.clone());
                        let mangled = format!("M2_EXC_{}", self.mangle(&e.name));
                        self.emitln(&format!("#define {} __COUNTER__", mangled));
                    }
                }
            }
        }

        // Type, const, and exception declarations from impl block via HIR
        if let Some(ref emb) = hir_emb {
            for td in &emb.type_decls {
                self.gen_type_decl_from_id(&td.name, td.type_id);
            }
            for c in &emb.const_decls {
                self.gen_hir_const_decl(c);
            }
            for e in &emb.exception_decls {
                self.exception_names.insert(e.name.clone());
                self.emitln(&format!("#define {} {}", e.mangled, e.exc_id));
            }
        }
        self.generating_for_module = None;

        // Emit M2+ type descriptors for types declared in this embedded module
        if self.m2plus {
            self.emit_type_descs();
        }

        // Forward declarations for procedures (with module prefix) from HIR
        if let Some(ref emb) = hir_emb {
            for pd in &emb.procedures {
                self.register_hir_proc_params(&pd.sig);
                let prefixed_name = format!("{}_{}", imp.name, pd.sig.name);
                if let Some(info) = self.proc_params.get(&pd.sig.name).cloned() {
                    self.proc_params.insert(prefixed_name, info);
                }
                // Emit embedded module prototype via TypeId resolver
                let ret_type = match pd.sig.return_type {
                    Some(rt) => self.type_id_to_c(rt),
                    None => "void".to_string(),
                };
                let static_prefix = if pd.sig.export_c_name.is_some() || self.multi_tu { "" } else { "static " };
                let c_name = if let Some(ref ecn) = pd.sig.export_c_name {
                    ecn.clone()
                } else {
                    format!("{}_{}", imp.name, pd.sig.name)
                };
                self.emit_indent();
                self.emit(&format!("{}{} {}(", static_prefix, ret_type, c_name));
                if pd.sig.params.is_empty() {
                    self.emit("void");
                } else {
                    let mut first = true;
                    for p in &pd.sig.params {
                        if !first { self.emit(", "); }
                        first = false;
                        let c_param = self.mangle(&p.name);
                        let resolved_tid = self.resolve_hir_alias(p.type_id);
                        let is_proc = p.is_proc_type
                            || matches!(self.sema.types.get(resolved_tid), crate::types::Type::ProcedureType { .. });
                        if p.is_open_array {
                            let c_type = self.type_id_to_c(p.type_id);
                            self.emit(&format!("{} *{}, uint32_t {}_high", c_type, c_param, c_param));
                        } else if is_proc {
                            let decl = self.proc_type_decl_from_id(p.type_id, &c_param, p.is_var);
                            self.emit(&decl);
                        } else {
                            let c_type = self.type_id_to_c(p.type_id);
                            if p.is_var {
                                self.emit(&format!("{} *{}", c_type, c_param));
                            } else {
                                self.emit(&format!("{} {}", c_type, c_param));
                            }
                        }
                    }
                }
                self.emit(");\n");
            }
        }
        // In multi-TU mode, emit extern declarations for exported vars and init function.
        // This allows other TUs (including main) to reference these symbols.
        if self.multi_tu {
            if let Some(scope_id) = self.sema.symtab.lookup_module_scope(&imp.name) {
                let var_syms: Vec<(String, crate::types::TypeId)> = self.sema.symtab.symbols_in_scope(scope_id).iter()
                    .filter(|s| matches!(s.kind, crate::symtab::SymbolKind::Variable) && s.exported)
                    .map(|s| (s.name.clone(), s.typ))
                    .collect();
                for (name, tid) in &var_syms {
                    let ctype = self.type_id_to_c(*tid);
                    let array_suffix = self.type_id_array_suffix(*tid);
                    let c_name = format!("{}_{}", imp.name, name);
                    self.emitln(&format!("extern {} {}{};", ctype, c_name, array_suffix));
                }
            }
            // Always emit init prototype — harmless if module has no body
            self.emitln(&format!("extern void {}_init(void);", imp.name));
        }

        self.newline();

        // Emit the MODULE_DEFS marker — everything below here is the module "body"
        // (var definitions, proc bodies, init function) that goes into this module's TU.
        if self.multi_tu {
            self.emit(&format!("/* MX_MODULE_DEFS {} */\n", imp.name));
        }

        // Variable declarations from definition module (exported VARs) via sema scope
        if let Some(scope_id) = self.sema.symtab.lookup_module_scope(&imp.name) {
            let var_syms: Vec<(String, crate::types::TypeId)> = self.sema.symtab.symbols_in_scope(scope_id).iter()
                .filter(|s| matches!(s.kind, crate::symtab::SymbolKind::Variable) && s.exported)
                .map(|s| (s.name.clone(), s.typ))
                .collect();
            for (name, tid) in &var_syms {
                let g = crate::hir::HirGlobalDecl {
                    name: name.clone(),
                    mangled: format!("{}_{}", imp.name, name),
                    type_id: *tid,
                    exported: true,
                    c_type: String::new(),
                    c_array_suffix: String::new(),
                    is_proc_type: false,
                };
                self.gen_hir_global_decl(&g);
            }
        }

        // Variable declarations from implementation module via HIR
        if let Some(ref emb) = hir_emb {
            for g in &emb.global_decls {
                self.gen_hir_global_decl(g);
            }
        }

        // Procedure bodies (with module prefix) — iterate from HIR proc sigs
        let emb_procs: Vec<crate::hir::HirProcSig> = hir_emb.as_ref()
            .map(|e| e.procedures.iter().map(|pd| pd.sig.clone()).collect())
            .unwrap_or_default();
        for sig in &emb_procs {
            {
                let emb_sig: Option<crate::hir::HirProcSig> = Some(sig.clone());
                {
                    let static_prefix = if sig.export_c_name.is_some() || self.multi_tu { "" } else { "static " };
                    let ret_type = match sig.return_type {
                        Some(rt) => self.type_id_to_c(rt),
                        None => "void".to_string(),
                    };
                    let c_name = if let Some(ref ecn) = sig.export_c_name {
                        ecn.clone()
                    } else {
                        format!("{}_{}", imp.name, sig.name)
                    };
                    self.emit(&format!("{}{} {}(", static_prefix, ret_type, c_name));
                    if sig.params.is_empty() {
                        self.emit("void");
                    } else {
                        let mut first = true;
                        for sp in &sig.params {
                            if !first { self.emit(", "); }
                            first = false;
                            let c_param = self.mangle(&sp.name);
                            let resolved_tid = self.resolve_hir_alias(sp.type_id);
                            let is_proc = sp.is_proc_type
                                || matches!(self.sema.types.get(resolved_tid), crate::types::Type::ProcedureType { .. });
                            if sp.is_open_array {
                                let c_type = self.type_id_to_c(sp.type_id);
                                self.emit(&format!("{} *{}, uint32_t {}_high", c_type, c_param, c_param));
                            } else if is_proc {
                                let decl = self.proc_type_decl_from_id(sp.type_id, &c_param, sp.is_var);
                                self.emit(&decl);
                            } else {
                                let c_type = self.type_id_to_c(sp.type_id);
                                if sp.is_var {
                                    self.emit(&format!("{} *{}", c_type, c_param));
                                } else {
                                    self.emit(&format!("{} {}", c_type, c_param));
                                }
                            }
                        }
                    }
                }
                self.emit(") {\n");
                self.indent += 1;

                // Track VAR and open array params for body codegen (from HIR sig)
                let mut param_vars = HashMap::new();
                let mut oa_params = HashSet::new();
                let mut na_params = HashSet::new();
                if let Some(ref sig) = emb_sig {
                    for hp in &sig.params {
                        let resolved = self.resolve_hir_alias(hp.type_id);
                        let is_open = matches!(self.sema.types.get(resolved), crate::types::Type::OpenArray { .. });
                        let mangled = self.mangle(&hp.name);
                        if is_open {
                            oa_params.insert(mangled);
                            let high_name = format!("{}_high", &hp.name);
                            self.var_types.insert(high_name, "uint32_t".to_string());
                        } else if hp.is_var {
                            param_vars.insert(hp.name.clone(), true);
                        }
                        if let Some(type_name) = self.type_id_source_name(hp.type_id) {
                            self.var_types.insert(hp.name.clone(), type_name);
                        }
                        if !hp.is_var && !is_open {
                            if matches!(self.sema.types.get(resolved), crate::types::Type::Array { .. }) {
                                na_params.insert(hp.name.clone());
                            }
                        }
                    }
                }
                self.var_params.push(param_vars);
                self.open_array_params.push(oa_params);
                let saved_var_tracking = self.save_var_tracking();
                self.named_array_value_params.push(na_params);
                self.parent_proc_stack.push(sig.name.clone());

                // Local declarations from HirProc.locals (populated by build_proc with correct scope)
                {
                    let mod_name = self.module_name.clone();
                    let proc_locals = self.prebuilt_hir.as_ref().and_then(|hir| {
                        hir.procedures.iter()
                            .find(|hp| hp.name.source_name == sig.name
                                && hp.name.module.as_deref() == Some(mod_name.as_str()))
                            .map(|hp| hp.locals.clone())
                    });
                    if let Some(ref locals) = proc_locals {
                        for local in locals {
                            match local {
                                crate::hir::HirLocalDecl::Var { name, type_id } => {
                                    let resolved = self.resolve_hir_alias(*type_id);
                                    let c_name = self.mangle(name);
                                    let is_proc = matches!(self.sema.types.get(resolved), crate::types::Type::ProcedureType { .. });
                                    let is_ptr_to_arr = if let crate::types::Type::Pointer { base } = self.sema.types.get(resolved) {
                                        matches!(self.sema.types.get(self.resolve_hir_alias(*base)), crate::types::Type::Array { .. })
                                    } else { false };
                                    if is_proc {
                                        self.emit_indent();
                                        let d = self.proc_type_decl_from_id(resolved, &c_name, false);
                                        self.emit(&format!("{};\n", d));
                                    } else if is_ptr_to_arr {
                                        if let crate::types::Type::Pointer { base } = self.sema.types.get(resolved) {
                                            let (elem_c, arr_suffix) = self.field_type_and_suffix(*base);
                                            self.emit_indent();
                                            self.emit(&format!("{} (*{}){};\n", elem_c, c_name, arr_suffix));
                                        }
                                    } else {
                                        let (ctype, arr_suffix) = self.field_type_and_suffix(resolved);
                                        self.emit_indent();
                                        self.emit(&format!("{} {}{};\n", ctype, c_name, arr_suffix));
                                    }
                                }
                                crate::hir::HirLocalDecl::Type { name, type_id } => {
                                    self.gen_type_decl_from_id(name, *type_id);
                                }
                                crate::hir::HirLocalDecl::Const(hc) => {
                                    self.gen_hir_const_decl(hc);
                                }
                                crate::hir::HirLocalDecl::Exception { name, mangled, exc_id } => {
                                    self.exception_names.insert(name.clone());
                                    self.emitln(&format!("#define {} {}", mangled, exc_id));
                                }
                            }
                        }
                    }
                }

                // Body statements — use prebuilt HIR
                let prebuilt_body = self.prebuilt_hir.as_ref().and_then(|hir| {
                    hir.procedures.iter()
                        .find(|hp| hp.name.source_name == sig.name
                            && hp.name.module.as_deref() == Some(&self.module_name))
                        .and_then(|hp| hp.body.clone())
                });
                if let Some(body) = prebuilt_body {
                    for s in &body {
                        self.emit_hir_stmt(s);
                    }
                }

                self.parent_proc_stack.pop();
                self.restore_var_tracking(saved_var_tracking);
                self.var_params.pop();
                self.open_array_params.pop();
                self.named_array_value_params.pop();
                self.indent -= 1;
                self.emitln("}");
                self.newline();
            }
        }

        // Module initialization body from HIR
        let init_body = if let Some(ref emb) = hir_emb {
            emb.init_body.clone()
        } else {
            self.prebuilt_hir.as_ref().and_then(|hir| {
                hir.embedded_init_bodies.iter()
                    .find(|(name, _)| name == &imp.name)
                    .map(|(_, body)| body.clone())
            })
        };
        if let Some(body) = init_body {
            if self.multi_tu {
                self.emitln(&format!("void {}_init(void) {{", imp.name));
            } else {
                self.emitln(&format!("static void {}_init(void) {{", imp.name));
            }
            self.indent += 1;
            for stmt in &body {
                self.emit_hir_stmt(stmt);
            }
            self.indent -= 1;
            self.emitln("}");
            self.newline();
            self.embedded_init_modules.push(imp.name.clone());
        } else if imp.block.body.is_some() {
            // Empty init function still needed if AST says there's a body
            if self.multi_tu {
                self.emitln(&format!("void {}_init(void) {{", imp.name));
            } else {
                self.emitln(&format!("static void {}_init(void) {{", imp.name));
            }
            self.emitln("}");
            self.newline();
            self.embedded_init_modules.push(imp.name.clone());
        }

        if self.multi_tu {
            self.emit(&format!("/* MX_MODULE_END {} */\n", imp.name));
        }

        self.restore_embedded_context(ctx, &imp.name);
    }

    /// Snapshot the mutable state that gen_embedded_implementation needs to save/restore.
    pub(crate) fn save_embedded_context(&self) -> EmbeddedModuleContext {
        EmbeddedModuleContext {
            module_name: self.module_name.clone(),
            import_map: self.import_map.clone(),
            import_alias_map: self.import_alias_map.clone(),
            var_params: self.var_params.clone(),
            open_array_params: self.open_array_params.clone(),
            named_array_value_params: self.named_array_value_params.clone(),
            proc_params: self.proc_params.clone(),
            var_tracking: self.save_var_tracking(),
            typeid_c_names: self.typeid_c_names.clone(),
        }
    }

    /// Restore state after embedded implementation generation.
    /// Preserves module-prefixed proc_params and typeid_c_names registered during generation.
    pub(crate) fn restore_embedded_context(&mut self, ctx: EmbeddedModuleContext, embedded_module_name: &str) {
        // Extract module-prefixed proc params before restoring (these must survive)
        let prefix = format!("{}_", embedded_module_name);
        let module_proc_params: HashMap<String, Vec<ParamCodegenInfo>> = self.proc_params.iter()
            .filter(|(k, _)| k.starts_with(&prefix))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        // Preserve all typeid_c_names registered during this module's generation
        let new_typeid_names = self.typeid_c_names.clone();

        self.module_name = ctx.module_name;
        self.import_map = ctx.import_map;
        self.import_alias_map = ctx.import_alias_map;
        self.var_params = ctx.var_params;
        self.open_array_params = ctx.open_array_params;
        self.named_array_value_params = ctx.named_array_value_params;
        self.proc_params = ctx.proc_params;
        self.restore_var_tracking(ctx.var_tracking);
        self.embedded_local_procs.clear();
        self.embedded_local_vars.clear();

        // Merge back the module-prefixed proc param info
        self.proc_params.extend(module_proc_params);
        // Merge back typeid_c_names (accumulative — types from all processed modules)
        self.typeid_c_names = new_typeid_names;
    }

    /// Topologically sort implementation modules so dependencies come before dependents.
    /// Also considers imports from corresponding .def files so that type dependencies
    /// (e.g. `FROM Gfx IMPORT Renderer;` in Font.def) are properly ordered.
    /// Returns an error if a dependency cycle is detected.
    pub(crate) fn topo_sort_modules(modules: Vec<ImplementationModule>, def_modules: &HashMap<String, crate::ast::DefinitionModule>) -> CompileResult<Vec<ImplementationModule>> {
        let names: HashSet<String> = modules.iter().map(|m| m.name.clone()).collect();
        let mut deps: HashMap<String, Vec<String>> = HashMap::new();
        for m in &modules {
            let mut my_deps = Vec::new();
            // Collect deps from implementation module imports
            for imp in &m.imports {
                if let Some(ref from_mod) = imp.from_module {
                    if names.contains(from_mod) {
                        my_deps.push(from_mod.clone());
                    }
                } else {
                    for name in &imp.names {
                        if names.contains(&name.name) {
                            my_deps.push(name.name.clone());
                        }
                    }
                }
            }
            // Also collect deps from the corresponding definition module imports
            if let Some(def_mod) = def_modules.get(&m.name) {
                for imp in &def_mod.imports {
                    if let Some(ref from_mod) = imp.from_module {
                        if names.contains(from_mod) && !my_deps.contains(from_mod) {
                            my_deps.push(from_mod.clone());
                        }
                    } else {
                        for name in &imp.names {
                            if names.contains(&name.name) && !my_deps.contains(&name.name) {
                                my_deps.push(name.name.clone());
                            }
                        }
                    }
                }
            }
            deps.insert(m.name.clone(), my_deps);
        }
        let mut visited = HashSet::new();
        let mut visiting = HashSet::new();
        fn visit(
            name: &str,
            deps: &HashMap<String, Vec<String>>,
            visited: &mut HashSet<String>,
            visiting: &mut HashSet<String>,
            order: &mut Vec<String>,
        ) -> Result<(), String> {
            if visited.contains(name) {
                return Ok(());
            }
            if visiting.contains(name) {
                return Err(name.to_string());
            }
            visiting.insert(name.to_string());
            if let Some(d) = deps.get(name) {
                for dep in d {
                    visit(dep, deps, visited, visiting, order).map_err(|cycle_node| {
                        if cycle_node == name {
                            // We've come full circle; build the cycle description
                            format!("{} -> {}", name, dep)
                        } else {
                            format!("{} -> {}", name, cycle_node)
                        }
                    })?;
                }
            }
            visiting.remove(name);
            visited.insert(name.to_string());
            order.push(name.to_string());
            Ok(())
        }
        let mut order = Vec::new();
        for m in &modules {
            visit(&m.name, &deps, &mut visited, &mut visiting, &mut order)
                .map_err(|cycle_desc| {
                    CompileError::codegen(
                        crate::errors::SourceLoc::new("<codegen>", 0, 0),
                        format!("module dependency cycle detected: {}", cycle_desc),
                    )
                })?;
        }
        let pos: HashMap<String, usize> = order.iter().enumerate().map(|(i, n)| (n.clone(), i)).collect();
        let mut result = modules;
        result.sort_by_key(|m| pos.get(&m.name).copied().unwrap_or(usize::MAX));
        Ok(result)
    }

    /// Build import map from HIR imports (no AST dependency).
    pub(crate) fn build_import_map_from_hir(&mut self) {
        if let Some(ref hir) = self.prebuilt_hir {
            let hir_imports = hir.imports.clone();
            for imp in &hir_imports {
                if !imp.is_qualified {
                    // FROM Module IMPORT name1, name2;
                    self.imported_modules.insert(imp.module.clone());
                    for name in &imp.names {
                        self.import_map.insert(name.local_name.clone(), imp.module.clone());
                        if name.name != name.local_name {
                            self.import_alias_map.insert(name.local_name.clone(), name.name.clone());
                        }
                        // Register stdlib proc params
                        if stdlib::is_stdlib_module(&imp.module) && !stdlib::is_native_stdlib(&imp.module) {
                            if let Some(params) = stdlib::get_stdlib_proc_params(&imp.module, &name.name) {
                                let info: Vec<ParamCodegenInfo> = params.into_iter().map(|(pname, is_var, is_char, is_open_array)| {
                                    ParamCodegenInfo { name: pname, is_var, is_char, is_open_array }
                                }).collect();
                                let prefixed = format!("{}_{}", imp.module, name.name);
                                self.proc_params.insert(prefixed, info.clone());
                                self.proc_params.insert(name.local_name.clone(), info);
                            }
                        }
                        // Enum variant import
                        if let Some(scope_id) = self.sema.symtab.lookup_module_scope(&imp.module) {
                            if let Some(sym) = self.sema.symtab.lookup_in_scope(scope_id, &name.name) {
                                let resolved = self.resolve_hir_alias(sym.typ);
                                if let crate::types::Type::Enumeration { variants, .. } = self.sema.types.get(resolved) {
                                    for v in variants {
                                        self.import_map.entry(v.clone()).or_insert(imp.module.clone());
                                    }
                                }
                            }
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

    pub(crate) fn build_import_map(&mut self, imports: &[Import]) {
        // Collect enum variant names to add to import_map after the main loop.
        // When importing an enum type, its variant names are implicitly in scope.
        let mut extra_variants: Vec<(String, String)> = Vec::new();
        for imp in imports {
            if let Some(from_mod) = &imp.from_module {
                // FROM Module IMPORT name1, name2;
                // Also register the module name so Module.Proc() syntax works
                self.imported_modules.insert(from_mod.clone());
                for import_name in &imp.names {
                    let original = &import_name.name;
                    let local = import_name.local_name().to_string();
                    self.import_map.insert(local.clone(), from_mod.clone());
                    // Track alias→original mapping if aliased
                    if import_name.alias.is_some() {
                        self.import_alias_map.insert(local.clone(), original.clone());
                    }
                    // Register stdlib proc params for codegen (is_char, is_var, etc.)
                    if stdlib::is_stdlib_module(from_mod) && !stdlib::is_native_stdlib(from_mod) {
                        if let Some(params) = stdlib::get_stdlib_proc_params(from_mod, original) {
                            let info: Vec<ParamCodegenInfo> = params.into_iter().map(|(pname, is_var, is_char, is_open_array)| {
                                ParamCodegenInfo { name: pname, is_var, is_char, is_open_array }
                            }).collect();
                            let prefixed = format!("{}_{}", from_mod, original);
                            self.proc_params.insert(prefixed, info.clone());
                            self.proc_params.insert(local.clone(), info);
                        }
                    }
                    // If this imported name is an enum type, also import its variant names
                    if let Some(scope_id) = self.sema.symtab.lookup_module_scope(from_mod) {
                        if let Some(sym) = self.sema.symtab.lookup_in_scope(scope_id, original) {
                            let resolved = self.resolve_hir_alias(sym.typ);
                            if let crate::types::Type::Enumeration { variants, .. } = self.sema.types.get(resolved) {
                                for v in variants {
                                    extra_variants.push((v.clone(), from_mod.clone()));
                                }
                            }
                        }
                    }
                }
            } else {
                // IMPORT Module1, Module2;  (whole-module / qualified import)
                for import_name in &imp.names {
                    self.imported_modules.insert(import_name.name.clone());
                }
            }
        }
        for (variant_name, module_name) in extra_variants {
            self.import_map.entry(variant_name).or_insert(module_name);
        }
    }

    /// Register proc_params from module_exports, emit foreign extern decls,
    /// and generate embedded implementations for pending imported modules.
    /// Shared by gen_program_module and gen_implementation_module.
    pub(crate) fn emit_preamble_for_imports(&mut self) -> CompileResult<()> {
        for (mod_name, exports) in &self.module_exports.clone() {
            for (proc_name, param_info) in exports {
                let prefixed = format!("{}_{}", mod_name, proc_name);
                self.proc_params.insert(prefixed, param_info.clone());
                if self.foreign_modules.contains(mod_name.as_str()) {
                    self.proc_params.insert(proc_name.clone(), param_info.clone());
                }
            }
        }

        self.gen_foreign_extern_decls();

        if let Some(pending) = self.pending_modules.take() {
            let sorted = Self::topo_sort_modules(pending, &self.def_modules)?;
            let embedded_names: std::collections::HashSet<String> =
                sorted.iter().map(|m| m.name.clone()).collect();

            // Emit types and constants for definition-only modules (no .mod counterpart)
            // BEFORE embedded implementations, since embedded modules may reference these types.
            // These are registered def modules with no matching implementation module,
            // e.g. pure type-definition modules like "PdcTypes.def".
            let def_only_modules: Vec<String> = self.def_modules.keys()
                .filter(|name| {
                    !embedded_names.contains(name.as_str())
                        && !self.foreign_modules.contains(name.as_str())
                        && name.as_str() != self.module_name
                })
                .cloned()
                .collect();
            if !def_only_modules.is_empty() {
                let mut def_only_sorted = def_only_modules;
                def_only_sorted.sort();
                for mod_name in &def_only_sorted {
                    if let Some(def_mod) = self.def_modules.get(mod_name).cloned() {
                        let saved_module_name = self.module_name.clone();
                        let saved_import_map = self.import_map.clone();
                        let saved_import_alias_map = self.import_alias_map.clone();

                        self.module_name = mod_name.clone();
                        self.emitln(&format!("/* Definition-only module {} */", mod_name));
                        self.generating_for_module = Some(mod_name.clone());

                        // Build import scope so intra-module type references resolve
                        self.import_map.clear();
                        self.import_alias_map.clear();
                        self.build_import_map(&def_mod.imports);

                        // Pre-register type names, forward-declare records, emit types+consts from sema scope
                        let donly_scope = self.sema.symtab.lookup_module_scope(mod_name);
                        let donly_syms: Vec<(String, crate::symtab::SymbolKind, crate::types::TypeId, bool)> =
                            donly_scope.map(|sid| {
                                self.sema.symtab.symbols_in_scope(sid).iter()
                                    .map(|s| (s.name.clone(), s.kind.clone(), s.typ, s.exported))
                                    .collect()
                            }).unwrap_or_default();
                        // Pre-register type names
                        for (name, kind, _, _) in &donly_syms {
                            if matches!(kind, crate::symtab::SymbolKind::Type) {
                                let prefixed = format!("{}_{}", mod_name, self.mangle(name));
                                self.embedded_enum_types.insert(prefixed);
                            }
                        }
                        // Forward declare record types
                        for (name, kind, typ, _) in &donly_syms {
                            if matches!(kind, crate::symtab::SymbolKind::Type) {
                                let resolved = self.resolve_hir_alias(*typ);
                                if matches!(self.sema.types.get(resolved), crate::types::Type::Record { .. }) {
                                    let cn = self.type_decl_c_name(name);
                                    self.emitln(&format!("typedef struct {} {};", cn, cn));
                                }
                            }
                        }
                        // Emit type and constant declarations
                        for (name, kind, typ, exported) in &donly_syms {
                            match kind {
                                crate::symtab::SymbolKind::Type => {
                                    if *typ != crate::types::TY_VOID {
                                        self.gen_type_decl_from_id(name, *typ);
                                    }
                                }
                                crate::symtab::SymbolKind::Constant(cv) => {
                                    let val = crate::hir_build::const_value_to_hir(cv);
                                    let hc = crate::hir::HirConstDecl {
                                        name: name.clone(),
                                        mangled: format!("{}_{}", mod_name, name),
                                        value: val.clone(),
                                        type_id: *typ,
                                        exported: *exported,
                                        c_type: crate::hir_build::const_val_c_type(&val),
                                    };
                                    self.gen_hir_const_decl(&hc);
                                }
                                _ => {}
                            }
                        }

                        self.generating_for_module = None;
                        self.module_name = saved_module_name;
                        self.import_map = saved_import_map;
                        self.import_alias_map = saved_import_alias_map;
                        self.newline();
                    }
                }
            }

            for imp_mod in &sorted {
                self.gen_embedded_implementation(imp_mod);
            }
        }
        Ok(())
    }

    /// Emit forward declarations for procedures from prebuilt HirModule.
    pub(crate) fn emit_hir_forward_decls(&mut self) {
        let procs = if let Some(ref hir) = self.prebuilt_hir {
            hir.proc_decls.clone()
        } else {
            return;
        };
        for pd in &procs {
            // Skip nested procs — they get forward-declared by gen_proc_decl
            // after computing their mangled names
            if pd.sig.is_nested { continue; }
            self.register_hir_proc_params(&pd.sig);
            self.gen_hir_proc_prototype(&pd.sig);
            self.emit(";\n");
        }
    }

    /// Emit record forward declarations from prebuilt HirModule.
    pub(crate) fn emit_hir_record_forward_decls(&mut self) {
        let types = if let Some(ref hir) = self.prebuilt_hir {
            hir.type_decls.clone()
        } else {
            return;
        };
        for td in &types {
            let resolved = self.resolve_hir_alias(td.type_id);
            // Only forward-declare direct Record types. Pointer-to-Record with
            // inline record body gets its _r tag from gen_type_decl.
            if matches!(self.sema.types.get(resolved), crate::types::Type::Record { .. }) {
                let cn = self.type_decl_c_name(&td.name);
                self.emitln(&format!("typedef struct {} {};", cn, cn));
            }
        }
    }

    /// Emit const declarations from prebuilt HirModule.
    /// Since ConstVal is fully evaluated, no topological sort is needed.
    pub(crate) fn emit_hir_const_decls(&mut self) {
        let consts = if let Some(ref hir) = self.prebuilt_hir {
            hir.const_decls.clone()
        } else {
            return;
        };
        for c in &consts {
            self.gen_hir_const_decl(c);
        }
    }

    /// Emit type declarations from prebuilt HirModule using TypeId resolution.
    pub(crate) fn emit_hir_type_decls(&mut self) {
        let types = if let Some(ref hir) = self.prebuilt_hir {
            hir.type_decls.clone()
        } else {
            return;
        };
        for td in &types {
            self.gen_type_decl_from_id(&td.name, td.type_id);
        }
    }

    /// Emit global variable declarations from prebuilt HirModule.
    pub(crate) fn emit_hir_global_decls(&mut self) {
        let globals = if let Some(ref hir) = self.prebuilt_hir {
            hir.global_decls.clone()
        } else {
            return;
        };
        for g in &globals {
            self.gen_hir_global_decl(g);
        }
    }

    // ── Program module ──────────────────────────────────────────────

}
