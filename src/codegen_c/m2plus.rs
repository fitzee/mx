use super::*;

impl CodeGen {
    pub(crate) fn gen_object_type(
        &mut self,
        name: &str,
        parent: Option<&QualIdent>,
        fields: &[Field],
        methods: &[MethodDecl],
        overrides: &[OverrideDecl],
    ) {
        let c_name = self.mangle(name);

        // Generate vtable struct
        self.emitln(&format!("typedef struct {}_vtable {{", c_name));
        self.indent += 1;
        // If there's a parent, include parent vtable fields (simplified: embed parent vtable pointer)
        if let Some(p) = parent {
            let parent_c = if let Some(ref m) = p.module {
                format!("{}_{}", m, p.name)
            } else {
                self.mangle(&p.name)
            };
            self.emitln(&format!("{}_vtable _parent;", parent_c));
        }
        // Method function pointers
        for md in methods {
            self.emit_indent();
            let ret = if let Some(rt) = &md.return_type {
                self.type_to_c(rt)
            } else {
                "void".to_string()
            };
            self.emit(&format!("{} (*{})(struct {} *self", ret, md.name, c_name));
            for fp in &md.params {
                let pt = self.type_to_c(&fp.typ);
                for pname in &fp.names {
                    if fp.is_var {
                        self.emit(&format!(", {} *{}", pt, pname));
                    } else {
                        self.emit(&format!(", {} {}", pt, pname));
                    }
                }
            }
            self.emit(");\n");
        }
        self.indent -= 1;
        self.emitln(&format!("}} {}_vtable;", c_name));
        self.newline();

        // Generate instance struct
        self.emitln(&format!("struct {} {{", c_name));
        self.indent += 1;
        self.emitln(&format!("{}_vtable *_vt;", c_name));
        // Include parent fields
        if let Some(p) = parent {
            let parent_c = if let Some(ref m) = p.module {
                format!("{}_{}", m, p.name)
            } else {
                self.mangle(&p.name)
            };
            self.emitln(&format!("/* inherited from {} */", parent_c));
        }
        // Own fields
        for f in fields {
            self.emit_indent();
            let ctype = self.type_to_c(&f.typ);
            let arr_suffix = self.type_array_suffix(&f.typ);
            self.emit(&format!("{} ", ctype));
            for (i, fname) in f.names.iter().enumerate() {
                if i > 0 { self.emit(", "); }
                self.emit(fname);
                if !arr_suffix.is_empty() {
                    self.emit(&arr_suffix);
                }
            }
            self.emit(";\n");
        }
        self.indent -= 1;
        self.emitln("};");
        self.newline();

        // Generate type typedef (pointer to struct, as objects are reference types)
        self.emitln(&format!("typedef struct {} *{};", c_name, c_name));

        // Register type descriptor for RTTI
        let parent_c_sym = parent.map(|p| {
            let pc = if let Some(ref m) = p.module {
                format!("{}_{}", m, p.name)
            } else {
                self.mangle(&p.name)
            };
            self.object_type_descs.get(&pc).cloned()
                .unwrap_or_else(|| format!("M2_TD_{}", pc))
        });
        let td_sym = self.register_type_desc(&c_name, name, parent_c_sym);
        self.object_type_descs.insert(c_name.to_string(), td_sym);

        // Track field names for WITH resolution
        let mut field_names: Vec<String> = fields.iter()
            .flat_map(|f| f.names.clone())
            .collect();
        // Also add method names
        for md in methods {
            field_names.push(md.name.clone());
        }
        self.record_fields.insert(name.to_string(), field_names);
    }

    // ── Modula-2+ Exception Declaration ─────────────────────────────

    pub(crate) fn next_exception_id(&mut self) -> usize {
        self.exception_counter += 1;
        self.exception_counter
    }

    /// Allocate a new unique type ID and register a type descriptor to be emitted.
    /// Returns the C symbol name for the descriptor (e.g. "M2_TD_ModName_TypeName").
    pub(crate) fn register_type_desc(&mut self, type_name: &str, display_name: &str, parent_c_sym: Option<String>) -> String {
        self.type_id_counter += 1;
        let id = self.type_id_counter;
        let depth = if let Some(ref parent) = parent_c_sym {
            // Find parent depth from already-registered descriptors
            self.type_descs.iter()
                .find(|(sym, _, _, _)| sym == parent)
                .map(|(_, _, _, d)| d + 1)
                .unwrap_or(1)
        } else {
            0
        };
        let c_sym = format!("M2_TD_{}", type_name);
        self.type_descs.push((c_sym.clone(), display_name.to_string(), parent_c_sym, depth));
        // Store the ID for later use
        let _ = id;
        c_sym
    }

    /// Emit all registered type descriptors as C globals.
    /// Must be called after all type declarations have been processed.
    /// Parents are always registered before children (due to topo-sorted embedded modules).
    pub(crate) fn emit_type_descs(&mut self) {
        if self.type_descs.is_empty() {
            return;
        }
        let descs = std::mem::take(&mut self.type_descs);
        let mut id = 0usize;
        for (c_sym, display, parent, depth) in &descs {
            id += 1;
            let parent_expr = if let Some(p) = parent {
                format!("&{}", p)
            } else {
                "NULL".to_string()
            };
            self.emitln(&format!(
                "M2_TypeDesc {} = {{ {}, \"{}\", {}, {} }};",
                c_sym, id, display, parent_expr, depth
            ));
        }
        self.newline();
    }

    // M2+ statement codegen (TRY/EXCEPT, LOCK, TYPECASE) is in hir_emit.rs

    /// Scan compilation unit to determine which M2+ runtime features are needed.
    pub(crate) fn scan_m2plus_features(&mut self, unit: &CompilationUnit) {
        match unit {
            CompilationUnit::ProgramModule(m) => {
                self.scan_imports_for_features(&m.imports);
                self.scan_block_for_features(&m.block);
            }
            CompilationUnit::ImplementationModule(m) => {
                self.scan_imports_for_features(&m.imports);
                self.scan_block_for_features(&m.block);
            }
            CompilationUnit::DefinitionModule(m) => {
                self.scan_imports_for_features(&m.imports);
            }
        }
    }

    pub(crate) fn scan_imports_for_features(&mut self, imports: &[Import]) {
        for imp in imports {
            if let Some(ref from_mod) = imp.from_module {
                match from_mod.as_str() {
                    "Thread" | "Mutex" | "Condition"
                    | "THREAD" | "MUTEX" | "CONDITION" => self.uses_threads = true,
                    _ => {}
                }
            }
        }
    }

    pub(crate) fn scan_block_for_features(&mut self, block: &Block) {
        for decl in &block.decls {
            if let Declaration::Type(td) = decl {
                if let Some(ref ty) = td.typ {
                    self.scan_type_for_gc(ty);
                }
            }
        }
        if let Some(ref body) = block.body {
            self.scan_stmts_for_features(body);
        }
    }

    pub(crate) fn scan_type_for_gc(&mut self, ty: &TypeNode) {
        match ty {
            TypeNode::Ref { .. } | TypeNode::RefAny { .. } | TypeNode::Object { .. } => {
                self.uses_gc = true;
            }
            _ => {}
        }
    }

    pub(crate) fn scan_stmts_for_features(&mut self, stmts: &[Statement]) {
        for s in stmts {
            match &s.kind {
                StatementKind::Lock { .. } => self.uses_threads = true,
                _ => {}
            }
        }
    }

}
