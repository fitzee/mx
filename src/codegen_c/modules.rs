use super::*;

impl CodeGen {
    pub(crate) fn gen_program_module(&mut self, m: &ProgramModule) -> CompileResult<()> {
        self.module_name = m.name.clone();
        self.build_import_map(&m.imports);

        self.emit_preamble_for_imports()?;

        if self.multi_tu {
            self.emit(&format!("/* MX_MAIN_BEGIN {} */\n", m.name));
        }
        self.emitln(&format!("/* Module {} */", m.name));
        self.newline();

        // Structural declarations from prebuilt HIR
        self.emit_hir_record_forward_decls();
        self.emit_hir_type_decls();
        self.emit_hir_const_decls();

        // Emit M2+ type descriptors (after all types are declared)
        if self.m2plus {
            self.emit_type_descs();
        }

        // Forward declarations for procedures from HIR
        self.emit_hir_forward_decls();
        // Also forward-declare procs from nested local modules (still AST-driven)
        for decl in &m.block.decls {
            if let Declaration::Module(local_mod) = decl {
                for d in &local_mod.block.decls {
                    if let Declaration::Procedure(p) = d {
                        self.register_proc_params(&p.heading);
                        self.gen_proc_prototype(&p.heading);
                        self.emit(";\n");
                    }
                }
            }
        }
        self.newline();

        // Pass 1: Emit global variable declarations from HIR
        self.emit_hir_global_decls();
        // Also emit vars from nested local modules (still AST-driven)
        for decl in &m.block.decls {
            if let Declaration::Module(local_mod) = decl {
                for d in &local_mod.block.decls {
                    if let Declaration::Var(v) = d {
                        self.gen_var_decl(v);
                    }
                }
            }
        }
        // Pass 2: Emit Procedures and Modules (skip Var/Const/Type)
        for decl in &m.block.decls {
            match decl {
                Declaration::Const(_) | Declaration::Type(_) | Declaration::Var(_) => {}
                _ => self.gen_declaration(decl),
            }
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
        if m.block.finally.is_some() {
            self.emitln("atexit(m2_finally_handler);");
        }

        // Call embedded module init functions (in dependency order)
        for mod_name in &self.embedded_init_modules.clone() {
            self.emitln(&format!("{}_init();", mod_name));
        }

        // Initialize local (nested) modules — run their BEGIN bodies
        for decl in &m.block.decls {
            if let Declaration::Module(local_mod) = decl {
                if let Some(stmts) = &local_mod.block.body {
                    self.emitln(&format!("/* Init local module {} */", local_mod.name));
                    let mut hb = self.make_hir_builder();
                    let hir_stmts = hb.lower_stmts(stmts);
                    for stmt in &hir_stmts { self.emit_hir_stmt(stmt); }
                }
            }
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

    pub(crate) fn gen_definition_module(&mut self, m: &DefinitionModule) {
        self.module_name = m.name.clone();
        self.emitln(&format!("/* Definition Module {} */", m.name));
        self.emitln(&format!("#ifndef {}_H", m.name.to_uppercase()));
        self.emitln(&format!("#define {}_H", m.name.to_uppercase()));
        self.newline();

        // Forward struct declarations for record types (and POINTER TO RECORD)
        for def in &m.definitions {
            if let Definition::Type(t) = def {
                let cn = self.mangle(&t.name);
                match &t.typ {
                    Some(TypeNode::Record { .. }) => {
                        self.emitln(&format!("typedef struct {} {};", cn, cn));
                    }
                    Some(TypeNode::Pointer { base, .. }) if matches!(&**base, TypeNode::Record { .. }) => {
                        let tag = format!("{}_r", cn);
                        self.emitln(&format!("typedef struct {} {};", tag, tag));
                        self.emitln(&format!("typedef {} *{};", tag, cn));
                    }
                    _ => {}
                }
            }
        }

        for def in &m.definitions {
            match def {
                Definition::Const(c) => self.gen_const_decl(c),
                Definition::Type(t) => self.gen_type_decl(t),
                Definition::Var(v) => {
                    self.emit_indent();
                    self.emit("extern ");
                    let ctype = self.type_to_c(&v.typ);
                    for (i, name) in v.names.iter().enumerate() {
                        if i > 0 {
                            self.emit(", ");
                        }
                        self.emit(&format!("{} {}", ctype, self.mangle(name)));
                    }
                    self.emit(";\n");
                }
                Definition::Procedure(h) => {
                    self.gen_proc_prototype(h);
                    self.emit(";\n");
                }
                Definition::Exception(e) => {
                    // Exception declaration: generate unique integer constant
                    self.exception_names.insert(e.name.clone());
                    self.emitln(&format!("#define M2_EXC_{} __COUNTER__", self.mangle(&e.name)));
                }
            }
        }

        self.newline();
        self.emitln("#endif");
    }

    pub(crate) fn gen_implementation_module(&mut self, m: &ImplementationModule) -> CompileResult<()> {
        self.module_name = m.name.clone();
        self.build_import_map(&m.imports);

        self.emit_preamble_for_imports()?;

        self.emitln(&format!("/* Implementation Module {} */", m.name));
        self.newline();

        // Emit types and constants from the corresponding definition module.
        // The implementation module's scope includes all .def exports, but
        // m2c must emit them explicitly in the generated C.
        let impl_type_names: std::collections::HashSet<String> = m.block.decls.iter()
            .filter_map(|d| if let Declaration::Type(t) = d { Some(t.name.clone()) } else { None })
            .collect();
        if let Some(def_mod) = self.def_modules.get(&m.name).cloned() {
            // Forward struct declarations from the definition module
            for d in &def_mod.definitions {
                if let Definition::Type(t) = d {
                    if !impl_type_names.contains(&t.name) {
                        let cn = self.mangle(&t.name);
                        match &t.typ {
                            Some(TypeNode::Record { .. }) => {
                                self.emitln(&format!("typedef struct {} {};", cn, cn));
                            }
                            Some(TypeNode::Pointer { base, .. }) if matches!(&**base, TypeNode::Record { .. }) => {
                                let tag = format!("{}_r", cn);
                                self.emitln(&format!("typedef struct {} {};", tag, tag));
                                self.emitln(&format!("typedef {} *{};", tag, cn));
                            }
                            _ => {}
                        }
                    }
                }
            }
            // Emit type and const declarations from the definition module
            for d in &def_mod.definitions {
                match d {
                    Definition::Type(t) => {
                        if !impl_type_names.contains(&t.name) {
                            self.gen_type_decl(t);
                        }
                    }
                    Definition::Const(c) => self.gen_const_decl(c),
                    _ => {}
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
        // Pass 2: Emit Procedures and Modules (skip Var/Const/Type)
        for decl in &m.block.decls {
            match decl {
                Declaration::Const(_) | Declaration::Type(_) | Declaration::Var(_) => {}
                _ => self.gen_declaration(decl),
            }
        }

        // Module body = initialization function
        if let Some(stmts) = &m.block.body {
            self.emit_line_directive(&m.block.loc);
            self.emitln(&format!("void {}_init(void) {{", self.mangle(&m.name)));
            self.indent += 1;
            // Use prebuilt HIR init body
            let prebuilt = self.prebuilt_hir.as_ref()
                .and_then(|hir| hir.init_body.clone());
            if let Some(body) = prebuilt {
                for stmt in &body { self.emit_hir_stmt(stmt); }
            } else {
                let mut hb = self.make_hir_builder();
                let hir_stmts = hb.lower_stmts(stmts);
                for stmt in &hir_stmts { self.emit_hir_stmt(stmt); }
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
        } else {
            // Fallback to AST if no HIR available
            for decl in &imp.block.decls {
                match decl {
                    Declaration::Procedure(p) => {
                        if p.heading.export_c_name.is_none() {
                            self.embedded_local_procs.insert(p.heading.name.clone());
                        }
                    }
                    Declaration::Var(v) => {
                        for name in &v.names {
                            self.embedded_local_vars.insert(name.clone());
                        }
                    }
                    Declaration::Const(c) => {
                        self.embedded_local_vars.insert(c.name.clone());
                    }
                    _ => {}
                }
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
        if let Some(def_mod) = self.def_modules.get(&imp.name).cloned() {
            for d in &def_mod.definitions {
                if let Definition::Type(t) = d {
                    let prefixed = format!("{}_{}", imp.name, self.mangle(&t.name));
                    self.embedded_enum_types.insert(prefixed);
                }
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
            let impl_type_names: HashSet<String> = if let Some(ref emb) = hir_emb {
                emb.type_decls.iter().map(|td| td.name.clone()).collect()
            } else {
                imp.block.decls.iter()
                    .filter_map(|d| if let Declaration::Type(t) = d { Some(t.name.clone()) } else { None })
                    .collect()
            };
            for d in &def_mod.definitions {
                if let Definition::Type(t) = d {
                    if !impl_type_names.contains(&t.name) {
                        if matches!(&t.typ, Some(TypeNode::Record { .. })) {
                            let cn = self.type_decl_c_name(&t.name);
                            self.emitln(&format!("typedef struct {} {};", cn, cn));
                        }
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
        } else {
            self.emit_record_forward_decls(&imp.block.decls);
        }

        // Emit type and const declarations from the corresponding definition module,
        // but skip types that are redefined in the implementation module.
        if let Some(def_mod) = self.def_modules.get(&imp.name).cloned() {
            // Register def-module constants and exported VARs as local vars for module-prefixed references
            for d in &def_mod.definitions {
                match d {
                    Definition::Const(c) => {
                        self.embedded_local_vars.insert(c.name.clone());
                    }
                    Definition::Var(v) => {
                        for name in &v.names {
                            self.embedded_local_vars.insert(name.clone());
                        }
                    }
                    _ => {}
                }
            }
            let impl_type_names: HashSet<String> = if let Some(ref emb) = hir_emb {
                emb.type_decls.iter().map(|td| td.name.clone()).collect()
            } else {
                imp.block.decls.iter()
                    .filter_map(|d| if let Declaration::Type(t) = d { Some(t.name.clone()) } else { None })
                    .collect()
            };
            for d in &def_mod.definitions {
                match d {
                    Definition::Type(t) => {
                        if !impl_type_names.contains(&t.name) {
                            self.gen_type_decl(t);
                        }
                    }
                    Definition::Const(c) => self.gen_const_decl(c),
                    Definition::Exception(e) => self.gen_exception_decl(e),
                    _ => {}
                }
            }
        }

        // Type, const, and exception declarations from impl block
        // Still AST-driven — gen_const_decl needs try_eval_const_int + const_int_values
        // registration for array bounds, and gen_type_decl needs full AST context.
        self.emit_type_const_exception_decls(&imp.block.decls);
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
        } else {
            for decl in &imp.block.decls {
                if let Declaration::Procedure(p) = decl {
                    self.register_proc_params(&p.heading);
                    self.gen_proc_prototype(&p.heading);
                    self.emit(";\n");
                }
            }
        }

        // In multi-TU mode, emit extern declarations for exported vars and init function.
        // This allows other TUs (including main) to reference these symbols.
        if self.multi_tu {
            if let Some(def_mod) = self.def_modules.get(&imp.name).cloned() {
                for d in &def_mod.definitions {
                    if let Definition::Var(v) = d {
                        let ctype = self.type_to_c(&v.typ);
                        let array_suffix = self.type_array_suffix(&v.typ);
                        for name in &v.names {
                            let c_name = format!("{}_{}", imp.name, name);
                            self.emitln(&format!("extern {} {}{};", ctype, c_name, array_suffix));
                        }
                    }
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

        // Variable declarations from definition module (exported VARs)
        if let Some(def_mod) = self.def_modules.get(&imp.name).cloned() {
            for d in &def_mod.definitions {
                if let Definition::Var(v) = d {
                    self.gen_var_decl(v);
                }
            }
        }

        // Variable declarations from implementation module via HIR
        if let Some(ref emb) = hir_emb {
            for g in &emb.global_decls {
                self.gen_hir_global_decl(g);
            }
        } else {
            for decl in &imp.block.decls {
                if let Declaration::Var(v) = decl {
                    self.gen_var_decl(v);
                }
            }
        }

        // Procedure bodies (with module prefix) — prototype via TypeId to match forward decl
        for decl in &imp.block.decls {
            if let Declaration::Procedure(p) = decl {
                let emb_sig = hir_emb.as_ref().and_then(|e| {
                    e.procedures.iter()
                        .find(|pd| pd.sig.name == p.heading.name)
                        .map(|pd| pd.sig.clone())
                });
                if let Some(ref sig) = emb_sig {
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
                } else {
                    // Fallback to AST prototype
                    let ret_type = if let Some(rt) = &p.heading.return_type {
                        self.type_to_c(rt)
                    } else {
                        "void".to_string()
                    };
                    if let Some(ref ecn) = p.heading.export_c_name {
                        self.emit(&format!("{} {}", ret_type, ecn));
                    } else if self.multi_tu {
                        self.emit(&format!("{} {}_{}", ret_type, imp.name, p.heading.name));
                    } else {
                        self.emit(&format!("static {} {}_{}", ret_type, imp.name, p.heading.name));
                    }
                    self.emit("(");
                    if p.heading.params.is_empty() {
                        self.emit("void");
                    } else {
                        let mut first = true;
                        for fp in &p.heading.params {
                            let ctype = self.type_to_c(&fp.typ);
                            let is_open = matches!(fp.typ, TypeNode::OpenArray { .. });
                            for name in &fp.names {
                                if !first { self.emit(", "); }
                                first = false;
                                let c_param = self.mangle(name);
                                if is_open {
                                    self.emit(&format!("{} *{}, uint32_t {}_high", ctype, c_param, c_param));
                                } else if fp.is_var {
                                    self.emit(&format!("{} *{}", ctype, c_param));
                                } else {
                                    self.emit(&format!("{} {}", ctype, c_param));
                                }
                            }
                        }
                    }
                }
                self.emit(") {\n");
                self.indent += 1;

                // Track VAR and open array params for body codegen (from AST)
                let mut param_vars = HashMap::new();
                let mut oa_params = HashSet::new();
                for fp in &p.heading.params {
                    let is_open = matches!(fp.typ, TypeNode::OpenArray { .. });
                    for name in &fp.names {
                        let c_param = self.mangle(name);
                        if is_open { oa_params.insert(c_param); }
                        else if fp.is_var { param_vars.insert(name.clone(), true); }
                    }
                }
                self.var_params.push(param_vars);
                self.open_array_params.push(oa_params);
                let saved_var_tracking = self.save_var_tracking();
                let mut na_params = HashSet::new();

                // Register param type names and _high companions
                for fp in &p.heading.params {
                    if matches!(fp.typ, TypeNode::OpenArray { .. }) {
                        for name in &fp.names {
                            let high_name = format!("{}_high", name);
                            self.var_types.insert(high_name, "uint32_t".to_string());
                        }
                    }
                    if let TypeNode::Named(qi) = &fp.typ {
                        if qi.module.is_none() {
                            for name in &fp.names {
                                self.var_types.insert(name.clone(), qi.name.clone());
                            }
                        }
                    }
                    // Track named-array value params (array decays to pointer in C)
                    if !fp.is_var && !matches!(fp.typ, TypeNode::OpenArray { .. }) {
                        if let TypeNode::Named(qi) = &fp.typ {
                            if self.array_types.contains(&qi.name) {
                                for name in &fp.names {
                                    na_params.insert(name.clone());
                                }
                            }
                        }
                    }
                }
                self.named_array_value_params.push(na_params);
                self.parent_proc_stack.push(p.heading.name.clone());

                // Local declarations
                for d in &p.block.decls {
                    self.gen_declaration(d);
                }

                // Body statements — use prebuilt HIR
                let prebuilt_body = self.prebuilt_hir.as_ref().and_then(|hir| {
                    hir.procedures.iter()
                        .find(|hp| hp.name.source_name == p.heading.name
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
                    if let Some(def_mod) = self.def_modules.get(from_mod.as_str()) {
                        for d in &def_mod.definitions {
                            if let Definition::Type(t) = d {
                                if t.name == *original {
                                    if let Some(TypeNode::Enumeration { variants, .. }) = &t.typ {
                                        for v in variants {
                                            extra_variants.push((v.clone(), from_mod.clone()));
                                        }
                                    }
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

                        // Pre-register type names in embedded_enum_types
                        for d in &def_mod.definitions {
                            if let Definition::Type(t) = d {
                                let prefixed = format!("{}_{}", mod_name, self.mangle(&t.name));
                                self.embedded_enum_types.insert(prefixed);
                            }
                        }

                        // Forward declare record types
                        for d in &def_mod.definitions {
                            if let Definition::Type(t) = d {
                                if matches!(&t.typ, Some(TypeNode::Record { .. })) {
                                    let cn = self.type_decl_c_name(&t.name);
                                    self.emitln(&format!("typedef struct {} {};", cn, cn));
                                }
                            }
                        }

                        // Emit type and constant declarations
                        for d in &def_mod.definitions {
                            match d {
                                Definition::Type(t) => self.gen_type_decl(t),
                                Definition::Const(c) => self.gen_const_decl(c),
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

    /// Emit forward struct declarations for all record types in a declaration list.
    /// Uses type_decl_c_name, which automatically handles module prefixing for
    /// embedded modules (via generating_for_module).
    pub(crate) fn emit_record_forward_decls(&mut self, decls: &[Declaration]) {
        for decl in decls {
            if let Declaration::Type(t) = decl {
                let cn = self.type_decl_c_name(&t.name);
                match &t.typ {
                    Some(TypeNode::Record { .. }) => {
                        self.emitln(&format!("typedef struct {} {};", cn, cn));
                    }
                    Some(TypeNode::Pointer { base, .. }) if matches!(&**base, TypeNode::Record { .. }) => {
                        let tag = format!("{}_r", cn);
                        self.emitln(&format!("typedef struct {} {};", tag, tag));
                        self.emitln(&format!("typedef {} *{};", tag, cn));
                    }
                    _ => {}
                }
            }
        }
    }

    /// Emit forward declarations for procedures from prebuilt HirModule.
    pub(crate) fn emit_hir_forward_decls(&mut self) {
        let procs = if let Some(ref hir) = self.prebuilt_hir {
            hir.proc_decls.clone()
        } else {
            return;
        };
        for pd in &procs {
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

    /// Emit type, const, and exception declarations from a declaration list.
    /// Used by embedded implementation gen which also handles exceptions inline.
    pub(crate) fn emit_type_const_exception_decls(&mut self, decls: &[Declaration]) {
        for decl in decls {
            match decl {
                Declaration::Const(c) => self.gen_const_decl(c),
                Declaration::Type(t) => self.gen_type_decl(t),
                Declaration::Exception(e) => self.gen_exception_decl(e),
                _ => {}
            }
        }
    }

    // ── Program module ──────────────────────────────────────────────

}
