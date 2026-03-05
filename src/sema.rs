use std::collections::HashSet;

use crate::analyze::{Reference, ReferenceIndex, ScopeMap};
use crate::ast::*;
use crate::builtins;
use crate::errors::{CompileError, CompileResult, SourceLoc};
use crate::symtab::*;
use crate::types::*;

pub struct SemanticAnalyzer {
    pub types: TypeRegistry,
    pub symtab: SymbolTable,
    errors: Vec<CompileError>,
    current_scope: usize,
    scope_stack: Vec<usize>,
    in_loop: usize,
    current_proc_return: Option<TypeId>,
    pub foreign_modules: HashSet<String>,
    /// Scope map for LSP: maps source regions to scope IDs.
    scope_map: ScopeMap,
    /// Stack of (scope_id, start_line, start_col) for scope span recording.
    scope_start_stack: Vec<(usize, usize, usize)>,
    /// Tracks the last source position seen (for estimating scope end).
    last_line: usize,
    last_col: usize,
    /// Reference index: all symbol references with resolved identity.
    ref_index: ReferenceIndex,
}

impl SemanticAnalyzer {
    pub fn new() -> Self {
        let mut sa = Self {
            types: TypeRegistry::new(),
            symtab: SymbolTable::new(),
            errors: Vec::new(),
            current_scope: 0,
            scope_stack: vec![0],
            in_loop: 0,
            current_proc_return: None,
            foreign_modules: HashSet::new(),
            scope_map: ScopeMap::new(),
            scope_start_stack: Vec::new(),
            last_line: 0,
            last_col: 0,
            ref_index: ReferenceIndex::new(),
        };
        sa.register_builtins();
        sa
    }

    /// Consume the analyzer and return all semantic artifacts.
    /// Used by the LSP analysis path (no codegen needed).
    pub fn into_results(self) -> (SymbolTable, TypeRegistry, ScopeMap, ReferenceIndex, Vec<CompileError>) {
        (self.symtab, self.types, self.scope_map, self.ref_index, self.errors)
    }

    fn register_builtins(&mut self) {
        builtins::register_builtin_types(&mut self.symtab, &self.types, 0);
        builtins::register_builtin_procs(&mut self.symtab, &self.types, 0);
    }

    /// Pre-register an external definition module so its types and procedures
    /// are available when analyzing imports in the main program module.
    pub fn register_def_module(&mut self, def: &DefinitionModule) {
        self.analyze_definition_module(def);
    }

    /// Reset position-dependent artifacts (scope_map, ref_index, etc.)
    /// after pre-registering .def modules. Their scope spans and refs refer
    /// to positions in other files and must not interfere with the main file.
    pub fn reset_position_artifacts(&mut self) {
        self.scope_map = ScopeMap::new();
        self.ref_index = ReferenceIndex::new();
        self.scope_start_stack.clear();
        self.last_line = 0;
        self.last_col = 0;
    }

    pub fn analyze(&mut self, unit: &CompilationUnit) -> Result<(), Vec<CompileError>> {
        match unit {
            CompilationUnit::ProgramModule(m) => self.analyze_program_module(m),
            CompilationUnit::DefinitionModule(m) => self.analyze_definition_module(m),
            CompilationUnit::ImplementationModule(m) => self.analyze_implementation_module(m),
        }
        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(self.errors.clone())
        }
    }

    fn error(&mut self, loc: &SourceLoc, msg: impl Into<String>) {
        self.errors
            .push(CompileError::semantic(loc.clone(), msg.into()));
    }

    fn enter_scope(&mut self, name: &str) -> usize {
        let id = self.symtab.push_scope(name);
        self.scope_stack.push(id);
        self.current_scope = id;
        id
    }

    fn leave_scope(&mut self) {
        self.symtab.pop_scope();
        self.scope_stack.pop();
        self.current_scope = *self.scope_stack.last().unwrap_or(&0);
    }

    fn define_sym(&mut self, mut sym: Symbol, loc: &SourceLoc) {
        let name = sym.name.clone();
        let name_len = name.len();
        sym.loc = loc.clone();
        self.update_last_pos(loc);
        // Record definition reference
        if loc.line > 0 {
            self.ref_index.push(Reference {
                line: loc.line,
                col: loc.col,
                len: name_len,
                def_scope: self.current_scope,
                name: name.clone(),
                is_definition: true,
            });
        }
        if let Err(msg) = self.symtab.define(self.current_scope, sym) {
            self.error(loc, msg);
        }
    }

    fn update_last_pos(&mut self, loc: &SourceLoc) {
        if loc.line > self.last_line || (loc.line == self.last_line && loc.col > self.last_col) {
            self.last_line = loc.line;
            self.last_col = loc.col;
        }
    }

    /// Record a use-reference to a resolved symbol.
    fn record_use_ref(&mut self, loc: &SourceLoc, name: &str, def_scope: usize) {
        if loc.line > 0 && !name.is_empty() {
            self.ref_index.push(Reference {
                line: loc.line,
                col: loc.col,
                len: name.len(),
                def_scope,
                name: name.to_string(),
                is_definition: false,
            });
        }
    }

    /// Enter a scope and record its start position for the ScopeMap.
    fn enter_scope_at(&mut self, name: &str, loc: &SourceLoc) -> usize {
        let scope_id = self.enter_scope(name);
        self.scope_start_stack.push((scope_id, loc.line, loc.col));
        self.update_last_pos(loc);
        scope_id
    }

    /// Leave a scope and record its span in the ScopeMap.
    fn leave_scope_at(&mut self) {
        if let Some((scope_id, start_line, start_col)) = self.scope_start_stack.pop() {
            self.scope_map.push(scope_id, start_line, start_col, self.last_line, self.last_col);
        }
        self.leave_scope();
    }

    // ── Module analysis ─────────────────────────────────────────────

    fn analyze_program_module(&mut self, m: &ProgramModule) {
        let scope_id = self.enter_scope_at(&m.name, &m.loc);
        self.process_imports(&m.imports);
        self.analyze_block(&m.block);
        self.leave_scope_at();

        // Register module in parent scope
        let sym = Symbol {
            name: m.name.clone(),
            kind: SymbolKind::Module { scope_id },
            typ: TY_VOID,
            exported: false,
            module: None,
            loc: SourceLoc::default(),
            doc: None,
        };
        self.define_sym(sym, &m.loc);

        // Re-export names from EXPORT clause into the parent (current) scope
        if let Some(ref export) = m.export {
            for name in &export.names {
                if let Some(sym) = self.symtab.lookup_in_scope_direct(scope_id, name) {
                    let mut forwarded = sym.clone();
                    forwarded.exported = false; // not exported further
                    self.define_sym(forwarded, &export.loc);
                } else {
                    self.error(&export.loc, format!(
                        "exported name '{}' not found in local module '{}'", name, m.name
                    ));
                }
            }
        }
    }

    fn analyze_definition_module(&mut self, m: &DefinitionModule) {
        if m.foreign_lang.is_some() {
            self.foreign_modules.insert(m.name.clone());
        }
        let scope_id = self.enter_scope_at(&m.name, &m.loc);
        self.process_imports(&m.imports);

        // In PIM4, all names in a definition module are exported by default.
        // The EXPORT QUALIFIED clause is optional and redundant.
        let has_export = m.export.is_some();
        let exported_names: Vec<String> = if has_export {
            m.export.as_ref().unwrap().names.clone()
        } else {
            Vec::new()
        };
        let export_all = !has_export;  // If no EXPORT clause, everything is exported

        for def in &m.definitions {
            match def {
                Definition::Const(c) => {
                    let typ = self.analyze_expr(&c.expr);
                    let val = self.eval_const_expr(&c.expr);
                    let sym = Symbol {
                        name: c.name.clone(),
                        kind: SymbolKind::Constant(val),
                        typ,
                        exported: export_all || exported_names.contains(&c.name),
                        module: Some(m.name.clone()),
                        loc: SourceLoc::default(),
                        doc: c.doc.clone(),
                    };
                    self.define_sym(sym, &c.loc);
                }
                Definition::Type(t) => {
                    let type_id = if let Some(tn) = &t.typ {
                        self.resolve_type_node(tn)
                    } else {
                        // Opaque type
                        self.types.register(Type::Opaque {
                            name: t.name.clone(),
                            module: m.name.clone(),
                        })
                    };
                    let sym = Symbol {
                        name: t.name.clone(),
                        kind: SymbolKind::Type,
                        typ: type_id,
                        exported: export_all || exported_names.contains(&t.name),
                        module: Some(m.name.clone()),
                        loc: SourceLoc::default(),
                        doc: t.doc.clone(),
                    };
                    self.define_sym(sym, &t.loc);
                }
                Definition::Var(v) => {
                    let type_id = self.resolve_type_node(&v.typ);
                    for (i, name) in v.names.iter().enumerate() {
                        let sym = Symbol {
                            name: name.clone(),
                            kind: SymbolKind::Variable,
                            typ: type_id,
                            exported: export_all || exported_names.contains(name),
                            module: Some(m.name.clone()),
                            loc: SourceLoc::default(),
                            doc: v.doc.clone(),
                        };
                        let loc = v.name_locs.get(i).unwrap_or(&v.loc);
                        self.define_sym(sym, loc);
                    }
                }
                Definition::Procedure(h) => {
                    let (params, ret) = self.analyze_proc_heading(h);
                    let sym = Symbol {
                        name: h.name.clone(),
                        kind: SymbolKind::Procedure {
                            params,
                            return_type: ret,
                            is_builtin: false,
                        },
                        typ: TY_VOID,
                        exported: export_all || exported_names.contains(&h.name),
                        module: Some(m.name.clone()),
                        loc: SourceLoc::default(),
                        doc: h.doc.clone(),
                    };
                    self.define_sym(sym, &h.loc);
                }
                Definition::Exception(e) => {
                    let type_id = self.types.register(Type::Exception { name: e.name.clone() });
                    let sym = Symbol {
                        name: e.name.clone(),
                        kind: SymbolKind::Constant(ConstValue::Integer(type_id as i64)),
                        typ: type_id,
                        exported: export_all || exported_names.contains(&e.name),
                        module: Some(m.name.clone()),
                        loc: SourceLoc::default(),
                        doc: e.doc.clone(),
                    };
                    self.define_sym(sym, &e.loc);
                }
            }
        }
        self.leave_scope_at();

        let sym = Symbol {
            name: m.name.clone(),
            kind: SymbolKind::Module { scope_id },
            typ: TY_VOID,
            exported: false,
            module: None,
            loc: SourceLoc::default(),
            doc: None,
        };
        self.define_sym(sym, &m.loc);
    }

    fn analyze_implementation_module(&mut self, m: &ImplementationModule) {
        let scope_id = self.enter_scope_at(&m.name, &m.loc);

        // Import types, constants, and exceptions from the own definition module.
        // In Modula-2, an implementation module implicitly sees these names
        // from its corresponding .def. Procedures and variables are skipped
        // because the .mod re-declares/implements them.
        if let Some(def_sym) = self.symtab.lookup(&m.name).cloned() {
            if let SymbolKind::Module { scope_id: def_scope } = def_sym.kind {
                let def_symbols: Vec<Symbol> = self.symtab
                    .scope_symbols(def_scope)
                    .filter(|s| s.exported && matches!(s.kind,
                        SymbolKind::Type | SymbolKind::Constant(_)))
                    .cloned()
                    .collect();
                for sym in def_symbols {
                    let _ = self.symtab.define(scope_id, sym);
                }
            }
        }

        self.process_imports(&m.imports);
        self.analyze_block(&m.block);
        self.leave_scope_at();

        // Only register the module symbol if not already present from the .def
        if self.symtab.lookup(&m.name).is_none() {
            let sym = Symbol {
                name: m.name.clone(),
                kind: SymbolKind::Module { scope_id },
                typ: TY_VOID,
                exported: false,
                module: None,
                loc: SourceLoc::default(),
                doc: None,
            };
            self.define_sym(sym, &m.loc);
        }
    }

    fn process_imports(&mut self, imports: &[Import]) {
        for imp in imports {
            if let Some(from_mod) = &imp.from_module {
                // Check if this module was already registered (e.g. from a .def file)
                let existing_scope = self.symtab.lookup_in_scope(self.current_scope, from_mod)
                    .and_then(|sym| if let SymbolKind::Module { scope_id } = &sym.kind {
                        Some(*scope_id)
                    } else {
                        None
                    });

                let scope_id = if let Some(sid) = existing_scope {
                    sid
                } else {
                    // FROM Module IMPORT names -- register as stdlib stubs
                    let sid = self.enter_scope(from_mod);
                    crate::stdlib::register_module(&mut self.symtab, &mut self.types, sid, from_mod);
                    self.leave_scope();

                    // Register module
                    let mod_sym = Symbol {
                        name: from_mod.clone(),
                        kind: SymbolKind::Module { scope_id: sid },
                        typ: TY_VOID,
                        exported: false,
                        module: None,
                        loc: SourceLoc::default(),
                        doc: None,
                    };
                    let _ = self.symtab.define(self.current_scope, mod_sym);
                    sid
                };

                // Import specific names into current scope
                for import_name in &imp.names {
                    let local = import_name.local_name().to_string();
                    if let Some(sym) = self.symtab.lookup_in_scope(scope_id, &import_name.name) {
                        let imported = sym.clone();
                        let _ = self.symtab.define(self.current_scope, Symbol {
                            name: local,
                            kind: imported.kind,
                            typ: imported.typ,
                            exported: false,
                            module: Some(from_mod.clone()),
                            loc: imported.loc,
                            doc: imported.doc,
                        });
                    } else {
                        // Register as unknown procedure stub
                        let sym = Symbol {
                            name: local,
                            kind: SymbolKind::Procedure {
                                params: vec![],
                                return_type: None,
                                is_builtin: false,
                            },
                            typ: TY_VOID,
                            exported: false,
                            module: Some(from_mod.clone()),
                            loc: SourceLoc::default(),
                            doc: None,
                        };
                        let _ = self.symtab.define(self.current_scope, sym);
                    }
                }
            } else {
                // IMPORT Module1, Module2, ...
                for import_name in &imp.names {
                    let name = &import_name.name;
                    // Check if this module was pre-registered (e.g., from a .def file)
                    if self.symtab.lookup(name).is_some() {
                        continue;
                    }
                    let scope_id = self.enter_scope(name);
                    crate::stdlib::register_module(&mut self.symtab, &mut self.types, scope_id, name);
                    self.leave_scope();

                    let sym = Symbol {
                        name: name.clone(),
                        kind: SymbolKind::Module { scope_id },
                        typ: TY_VOID,
                        exported: false,
                        module: None,
                        loc: SourceLoc::default(),
                        doc: None,
                    };
                    let _ = self.symtab.define(self.current_scope, sym);
                }
            }
        }
    }

    // ── Block / declarations ────────────────────────────────────────

    fn analyze_block(&mut self, block: &Block) {
        self.update_last_pos(&block.loc);
        // First pass: register all type names as placeholders for forward references
        for decl in &block.decls {
            if let Declaration::Type(t) = decl {
                // Register a placeholder type
                let placeholder_id = self.types.register(Type::Opaque {
                    name: t.name.clone(),
                    module: String::new(),
                });
                let sym = Symbol {
                    name: t.name.clone(),
                    kind: SymbolKind::Type,
                    typ: placeholder_id,
                    exported: false,
                    module: None,
                    loc: SourceLoc::default(),
                    doc: None,
                };
                let _ = self.symtab.define(self.current_scope, sym);
            }
        }
        // Second pass: resolve all declarations fully
        for decl in &block.decls {
            self.analyze_declaration(decl);
        }
        if let Some(stmts) = &block.body {
            for stmt in stmts {
                self.analyze_statement(stmt);
            }
        }
    }

    fn analyze_declaration(&mut self, decl: &Declaration) {
        match decl {
            Declaration::Const(c) => {
                let typ = self.analyze_expr(&c.expr);
                let val = self.eval_const_expr(&c.expr);
                let sym = Symbol {
                    name: c.name.clone(),
                    kind: SymbolKind::Constant(val),
                    typ,
                    exported: false,
                    module: None,
                    loc: SourceLoc::default(),
                    doc: c.doc.clone(),
                };
                self.define_sym(sym, &c.loc);
            }
            Declaration::Type(t) => {
                let type_id = if let Some(tn) = &t.typ {
                    self.resolve_type_node(tn)
                } else {
                    TY_VOID
                };
                // Type was already pre-registered in first pass; update its resolved type.
                // We don't re-define; just look up and update the placeholder's target.
                if let Some(sym) = self.symtab.lookup(&t.name) {
                    let old_id = sym.typ;
                    // Replace the placeholder type with the real resolved type
                    *self.types.get_mut(old_id) = self.types.get(type_id).clone();
                } else {
                    let sym = Symbol {
                        name: t.name.clone(),
                        kind: SymbolKind::Type,
                        typ: type_id,
                        exported: false,
                        module: None,
                        loc: SourceLoc::default(),
                        doc: t.doc.clone(),
                    };
                    self.define_sym(sym, &t.loc);
                }
            }
            Declaration::Var(v) => {
                let type_id = self.resolve_type_node(&v.typ);
                for (i, name) in v.names.iter().enumerate() {
                    let sym = Symbol {
                        name: name.clone(),
                        kind: SymbolKind::Variable,
                        typ: type_id,
                        exported: false,
                        module: None,
                        loc: SourceLoc::default(),
                        doc: v.doc.clone(),
                    };
                    let loc = v.name_locs.get(i).unwrap_or(&v.loc);
                    self.define_sym(sym, loc);
                }
            }
            Declaration::Procedure(p) => {
                let (params, ret) = self.analyze_proc_heading(&p.heading);
                let sym = Symbol {
                    name: p.heading.name.clone(),
                    kind: SymbolKind::Procedure {
                        params: params.clone(),
                        return_type: ret,
                        is_builtin: false,
                    },
                    typ: TY_VOID,
                    exported: false,
                    module: None,
                    loc: SourceLoc::default(),
                    doc: p.doc.clone(),
                };
                self.define_sym(sym, &p.loc);

                // Analyze procedure body
                let saved_return = self.current_proc_return;
                self.current_proc_return = ret;
                self.enter_scope_at(&p.heading.name, &p.loc);

                // Define parameters as local variables
                for param in &params {
                    let sym = Symbol {
                        name: param.name.clone(),
                        kind: SymbolKind::Variable,
                        typ: param.typ,
                        exported: false,
                        module: None,
                        loc: SourceLoc::default(),
                        doc: None,
                    };
                    let _ = self.symtab.define(self.current_scope, sym);
                }

                self.analyze_block(&p.block);
                self.leave_scope_at();
                self.current_proc_return = saved_return;
            }
            Declaration::Module(m) => {
                self.analyze_program_module(m);
            }
            Declaration::Exception(e) => {
                let type_id = self.types.register(Type::Exception { name: e.name.clone() });
                let sym = Symbol {
                    name: e.name.clone(),
                    kind: SymbolKind::Constant(ConstValue::Integer(type_id as i64)),
                    typ: type_id,
                    exported: false,
                    module: None,
                    loc: SourceLoc::default(),
                    doc: e.doc.clone(),
                };
                self.define_sym(sym, &e.loc);
            }
        }
    }

    fn analyze_proc_heading(&mut self, h: &ProcHeading) -> (Vec<ParamInfo>, Option<TypeId>) {
        let mut params = Vec::new();
        for fp in &h.params {
            let typ = self.resolve_type_node(&fp.typ);
            for name in &fp.names {
                params.push(ParamInfo {
                    name: name.clone(),
                    typ,
                    is_var: fp.is_var,
                });
            }
        }
        let ret = h
            .return_type
            .as_ref()
            .map(|t| self.resolve_type_node(t));
        (params, ret)
    }

    // ── Type resolution ─────────────────────────────────────────────

    fn resolve_type_node(&mut self, tn: &TypeNode) -> TypeId {
        match tn {
            TypeNode::Named(qi) => self.resolve_named_type(qi),
            TypeNode::Array {
                index_types,
                elem_type,
                ..
            } => {
                let elem = self.resolve_type_node(elem_type);
                // Multi-dimensional arrays: ARRAY [1..N], [1..M] OF T
                // becomes Array { idx1, Array { idx2, T } }
                // Process from right to left to build nested array types
                let mut current_elem = elem;
                for idx_node in index_types.iter().rev() {
                    let idx = self.resolve_type_node(idx_node);
                    let (low, high) = self.get_ordinal_range(idx);
                    current_elem = self.types.register(Type::Array {
                        index_type: idx,
                        elem_type: current_elem,
                        low,
                        high,
                    });
                }
                current_elem
            }
            TypeNode::OpenArray { elem_type, .. } => {
                let elem = self.resolve_type_node(elem_type);
                self.types.register(Type::OpenArray { elem_type: elem })
            }
            TypeNode::Record { fields, .. } => {
                let mut record_fields = Vec::new();
                let mut variant_info = None;
                for fl in fields {
                    for f in &fl.fixed {
                        let typ = self.resolve_type_node(&f.typ);
                        for name in &f.names {
                            record_fields.push(RecordField {
                                name: name.clone(),
                                typ,
                                offset: 0,
                            });
                        }
                    }
                    if let Some(vp) = &fl.variant {
                        let tag_type = self.resolve_named_type(&vp.tag_type);

                        // Add tag field to record's fixed fields
                        if let Some(tag_name) = &vp.tag_name {
                            record_fields.push(RecordField {
                                name: tag_name.clone(),
                                typ: tag_type,
                                offset: 0,
                            });
                        }

                        let mut vcases = Vec::new();
                        for v in &vp.variants {
                            let labels: Vec<i64> = v
                                .labels
                                .iter()
                                .filter_map(|l| match l {
                                    CaseLabel::Single(e) => self.eval_const_int(e),
                                    CaseLabel::Range(lo, _hi) => {
                                        self.eval_const_int(lo)
                                    }
                                })
                                .collect();
                            let mut vfields = Vec::new();
                            for vfl in &v.fields {
                                for f in &vfl.fixed {
                                    let typ = self.resolve_type_node(&f.typ);
                                    for name in &f.names {
                                        vfields.push(RecordField {
                                            name: name.clone(),
                                            typ,
                                            offset: 0,
                                        });
                                    }
                                }
                            }
                            vcases.push(VariantCase {
                                labels,
                                fields: vfields,
                            });
                        }

                        // Add a pseudo-field "variant" that covers the union
                        // This allows s.variant.v0.field syntax to work through sema
                        // We register it as a record type with the variant sub-structs
                        let variant_record_type = self.types.register(Type::Record {
                            fields: Vec::new(), // variant fields accessed specially
                            variants: None,
                        });
                        record_fields.push(RecordField {
                            name: "variant".to_string(),
                            typ: variant_record_type,
                            offset: 0,
                        });

                        variant_info = Some(VariantInfo {
                            tag_name: vp.tag_name.clone(),
                            tag_type,
                            variants: vcases,
                        });
                    }
                }
                self.types.register(Type::Record {
                    fields: record_fields,
                    variants: variant_info,
                })
            }
            TypeNode::Pointer { base, .. } => {
                let base_ty = self.resolve_type_node(base);
                self.types.register(Type::Pointer { base: base_ty })
            }
            TypeNode::Set { base, .. } => {
                let base_ty = self.resolve_type_node(base);
                self.types.register(Type::Set { base: base_ty })
            }
            TypeNode::Enumeration { variants, loc } => {
                let name = format!("enum@{}:{}", loc.line, loc.col);
                let type_id = self.types.register(Type::Enumeration {
                    name: name.clone(),
                    variants: variants.clone(),
                });
                // Define enum variants as constants
                for (i, v) in variants.iter().enumerate() {
                    let sym = Symbol {
                        name: v.clone(),
                        kind: SymbolKind::EnumVariant(i as i64),
                        typ: type_id,
                        exported: false,
                        module: None,
                        loc: SourceLoc::default(),
                        doc: None,
                    };
                    let _ = self.symtab.define(self.current_scope, sym);
                }
                type_id
            }
            TypeNode::Subrange { low, high, loc } => {
                let lo = self.eval_const_int(low).unwrap_or(0);
                let hi = self.eval_const_int(high).unwrap_or(0);
                self.types.register(Type::Subrange {
                    base: TY_INTEGER,
                    low: lo,
                    high: hi,
                })
            }
            TypeNode::ProcedureType {
                params,
                return_type,
                ..
            } => {
                let mut ptypes = Vec::new();
                for fp in params {
                    let typ = self.resolve_type_node(&fp.typ);
                    for _ in &fp.names {
                        ptypes.push(ParamType {
                            is_var: fp.is_var,
                            typ,
                        });
                    }
                }
                let ret = return_type.as_ref().map(|t| self.resolve_type_node(t));
                self.types
                    .register(Type::ProcedureType {
                        params: ptypes,
                        return_type: ret,
                    })
            }
            TypeNode::Ref { target, branded, .. } => {
                let target_id = self.resolve_type_node(target);
                self.types.register(Type::Ref {
                    target: target_id,
                    branded: branded.clone(),
                })
            }
            TypeNode::RefAny { .. } => TY_REFANY,
            TypeNode::Object { parent, fields, methods, overrides, .. } => {
                let parent_id = parent.as_ref().map(|qi| self.resolve_named_type(qi));
                let mut rec_fields = Vec::new();
                for (i, f) in fields.iter().enumerate() {
                    let typ = self.resolve_type_node(&f.typ);
                    for name in &f.names {
                        rec_fields.push(RecordField {
                            name: name.clone(),
                            typ,
                            offset: i,
                        });
                    }
                }
                let mut obj_methods = Vec::new();
                for md in methods {
                    let mut params = Vec::new();
                    for fp in &md.params {
                        let typ = self.resolve_type_node(&fp.typ);
                        for _ in &fp.names {
                            params.push(ParamType { is_var: fp.is_var, typ });
                        }
                    }
                    let ret = md.return_type.as_ref().map(|t| self.resolve_type_node(t));
                    obj_methods.push(ObjectMethod {
                        name: md.name.clone(),
                        params,
                        return_type: ret,
                    });
                }
                self.types.register(Type::Object {
                    name: String::new(),
                    parent: parent_id,
                    fields: rec_fields,
                    methods: obj_methods,
                })
            }
        }
    }

    fn resolve_named_type(&mut self, qi: &QualIdent) -> TypeId {
        let resolved = if let Some(module) = &qi.module {
            self.symtab.lookup_qualified_with_scope(module, &qi.name)
                .map(|(ds, sym)| (ds, sym.typ, sym.kind.clone()))
        } else {
            self.symtab.lookup_in_scope_with_id(self.current_scope, &qi.name)
                .map(|(ds, sym)| (ds, sym.typ, sym.kind.clone()))
        };

        if let Some((def_scope, typ, kind)) = resolved {
            self.record_use_ref(&qi.loc, &qi.name, def_scope);
            match &kind {
                SymbolKind::Type => typ,
                SymbolKind::EnumVariant(_) => typ,
                _ => {
                    // Could be using a type name that's also a module, etc.
                    typ
                }
            }
        } else {
            // Check built-in type names
            match qi.name.as_str() {
                "INTEGER" => TY_INTEGER,
                "CARDINAL" => TY_CARDINAL,
                "REAL" => TY_REAL,
                "LONGREAL" => TY_LONGREAL,
                "BOOLEAN" => TY_BOOLEAN,
                "CHAR" => TY_CHAR,
                "BITSET" => TY_BITSET,
                "WORD" => TY_WORD,
                "BYTE" => TY_BYTE,
                "ADDRESS" => TY_ADDRESS,
                "LONGINT" => TY_LONGINT,
                "LONGCARD" => TY_LONGCARD,
                _ => {
                    self.error(&qi.loc, format!("undefined type '{}'", qi.name));
                    TY_VOID
                }
            }
        }
    }

    fn get_ordinal_range(&self, type_id: TypeId) -> (i64, i64) {
        match self.types.get(type_id) {
            Type::Subrange { low, high, .. } => (*low, *high),
            Type::Enumeration { variants, .. } => (0, variants.len() as i64 - 1),
            Type::Boolean => (0, 1),
            Type::Char => (0, 255),
            Type::Integer => (i32::MIN as i64, i32::MAX as i64),
            Type::Cardinal => (0, u32::MAX as i64),
            _ => (0, 0),
        }
    }

    // ── Statement analysis ──────────────────────────────────────────

    fn analyze_statement(&mut self, stmt: &Statement) {
        self.update_last_pos(&stmt.loc);
        match &stmt.kind {
            StatementKind::Empty => {}
            StatementKind::Assign { desig, expr } => {
                let lhs_type = self.analyze_designator(desig);
                let rhs_type = self.analyze_expr(expr);
                if lhs_type != TY_VOID
                    && rhs_type != TY_VOID
                    && !assignment_compatible(&self.types, lhs_type, rhs_type)
                {
                    self.error(
                        &stmt.loc,
                        format!(
                            "incompatible types in assignment: {} := {}",
                            self.types.get(lhs_type),
                            self.types.get(rhs_type)
                        ),
                    );
                }
            }
            StatementKind::ProcCall { desig, args } => {
                self.analyze_designator(desig);
                // Check if it's a builtin
                let name = &desig.ident.name;
                if builtins::is_builtin_proc(name) {
                    self.check_builtin_call(name, args, &stmt.loc);
                } else {
                    for arg in args {
                        self.analyze_expr(arg);
                    }
                }
            }
            StatementKind::If {
                cond,
                then_body,
                elsifs,
                else_body,
            } => {
                let ct = self.analyze_expr(cond);
                if ct != TY_VOID && ct != TY_BOOLEAN {
                    self.error(&stmt.loc, "IF condition must be BOOLEAN");
                }
                for s in then_body {
                    self.analyze_statement(s);
                }
                for (ec, eb) in elsifs {
                    let ect = self.analyze_expr(ec);
                    if ect != TY_VOID && ect != TY_BOOLEAN {
                        self.error(&stmt.loc, "ELSIF condition must be BOOLEAN");
                    }
                    for s in eb {
                        self.analyze_statement(s);
                    }
                }
                if let Some(eb) = else_body {
                    for s in eb {
                        self.analyze_statement(s);
                    }
                }
            }
            StatementKind::Case {
                expr,
                branches,
                else_body,
            } => {
                let et = self.analyze_expr(expr);
                if et != TY_VOID && !self.types.get(et).is_ordinal() {
                    self.error(&stmt.loc, "CASE expression must be ordinal type");
                }
                for branch in branches {
                    for label in &branch.labels {
                        match label {
                            CaseLabel::Single(e) => {
                                self.analyze_expr(e);
                            }
                            CaseLabel::Range(lo, hi) => {
                                self.analyze_expr(lo);
                                self.analyze_expr(hi);
                            }
                        }
                    }
                    for s in &branch.body {
                        self.analyze_statement(s);
                    }
                }
                if let Some(eb) = else_body {
                    for s in eb {
                        self.analyze_statement(s);
                    }
                }
            }
            StatementKind::While { cond, body } => {
                let ct = self.analyze_expr(cond);
                if ct != TY_VOID && ct != TY_BOOLEAN {
                    self.error(&stmt.loc, "WHILE condition must be BOOLEAN");
                }
                for s in body {
                    self.analyze_statement(s);
                }
            }
            StatementKind::Repeat { body, cond } => {
                for s in body {
                    self.analyze_statement(s);
                }
                let ct = self.analyze_expr(cond);
                if ct != TY_VOID && ct != TY_BOOLEAN {
                    self.error(&stmt.loc, "UNTIL condition must be BOOLEAN");
                }
            }
            StatementKind::For {
                var,
                start,
                end,
                step,
                body,
            } => {
                let lookup = self.symtab.lookup_in_scope_with_id(self.current_scope, var)
                    .map(|(ds, sym)| (ds, sym.typ));
                if let Some((def_scope, vt)) = lookup {
                    self.record_use_ref(&stmt.loc, var, def_scope);
                    if !self.types.get(vt).is_ordinal() {
                        self.error(&stmt.loc, "FOR variable must be ordinal type");
                    }
                } else {
                    self.error(&stmt.loc, format!("undefined variable '{}'", var));
                }
                self.analyze_expr(start);
                self.analyze_expr(end);
                if let Some(s) = step {
                    self.analyze_expr(s);
                }
                for s in body {
                    self.analyze_statement(s);
                }
            }
            StatementKind::Loop { body } => {
                self.in_loop += 1;
                for s in body {
                    self.analyze_statement(s);
                }
                self.in_loop -= 1;
            }
            StatementKind::With { desig, body } => {
                let dt = self.analyze_designator(desig);
                if dt != TY_VOID {
                    if !matches!(self.types.get(dt), Type::Record { .. }) {
                        self.error(&stmt.loc, "WITH requires a record variable");
                    }
                }
                for s in body {
                    self.analyze_statement(s);
                }
            }
            StatementKind::Return { expr } => {
                if let Some(e) = expr {
                    let et = self.analyze_expr(e);
                    if let Some(ret_ty) = self.current_proc_return {
                        if et != TY_VOID && !assignment_compatible(&self.types, ret_ty, et) {
                            self.error(&stmt.loc, "RETURN type mismatch");
                        }
                    }
                }
            }
            StatementKind::Exit => {
                if self.in_loop == 0 {
                    self.error(&stmt.loc, "EXIT must be inside a LOOP statement");
                }
            }
            StatementKind::Raise { expr } => {
                if let Some(e) = expr {
                    self.analyze_expr(e);
                }
            }
            StatementKind::Retry => {
                // RETRY is only valid inside an EXCEPT handler, but we don't track that here
            }
            StatementKind::Try { body, excepts, finally_body } => {
                for s in body {
                    self.analyze_statement(s);
                }
                for ec in excepts {
                    for s in &ec.body {
                        self.analyze_statement(s);
                    }
                }
                if let Some(fb) = finally_body {
                    for s in fb {
                        self.analyze_statement(s);
                    }
                }
            }
            StatementKind::Lock { mutex, body } => {
                self.analyze_expr(mutex);
                for s in body {
                    self.analyze_statement(s);
                }
            }
            StatementKind::TypeCase { expr, branches, else_body } => {
                self.analyze_expr(expr);
                for branch in branches {
                    for s in &branch.body {
                        self.analyze_statement(s);
                    }
                }
                if let Some(eb) = else_body {
                    for s in eb {
                        self.analyze_statement(s);
                    }
                }
            }
        }
    }

    // ── Expression analysis ─────────────────────────────────────────

    fn analyze_expr(&mut self, expr: &Expr) -> TypeId {
        match &expr.kind {
            ExprKind::IntLit(_) => TY_INTEGER,
            ExprKind::RealLit(_) => TY_REAL,
            ExprKind::StringLit(s) => {
                self.types.register(Type::StringLit(s.len()))
            }
            ExprKind::CharLit(_) => TY_CHAR,
            ExprKind::BoolLit(_) => TY_BOOLEAN,
            ExprKind::NilLit => TY_NIL,
            ExprKind::Designator(d) => self.analyze_designator(d),
            ExprKind::FuncCall { desig, args } => {
                self.analyze_designator(desig);
                let name = &desig.ident.name;
                if builtins::is_builtin_proc(name) {
                    for arg in args {
                        self.analyze_expr(arg);
                    }
                    return builtins::builtin_return_type(name);
                }
                // Look up procedure and get return type
                let ret = if let Some(sym) = self.symtab.lookup(name).cloned() {
                    match &sym.kind {
                        SymbolKind::Procedure {
                            return_type,
                            params,
                            ..
                        } => {
                            // Check arg count (allow variadic for stdlib stubs)
                            if !params.is_empty() && args.len() != params.len() {
                                self.error(
                                    &expr.loc,
                                    format!(
                                        "expected {} arguments, got {}",
                                        params.len(),
                                        args.len()
                                    ),
                                );
                            }
                            return_type.unwrap_or(TY_VOID)
                        }
                        _ => TY_VOID,
                    }
                } else {
                    TY_VOID
                };

                for arg in args {
                    self.analyze_expr(arg);
                }
                ret
            }
            ExprKind::UnaryOp { op, operand } => {
                let t = self.analyze_expr(operand);
                match op {
                    UnaryOp::Pos | UnaryOp::Neg => {
                        if t != TY_VOID && !self.types.get(t).is_numeric() {
                            self.error(&expr.loc, "unary +/- requires numeric operand");
                        }
                        t
                    }
                }
            }
            ExprKind::Not(operand) => {
                let t = self.analyze_expr(operand);
                if t != TY_VOID && t != TY_BOOLEAN {
                    self.error(&expr.loc, "NOT requires BOOLEAN operand");
                }
                TY_BOOLEAN
            }
            ExprKind::BinaryOp { op, left, right } => {
                let lt = self.analyze_expr(left);
                let rt = self.analyze_expr(right);
                match op {
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul => {
                        if lt != TY_VOID && rt != TY_VOID {
                            if self.types.get(lt).is_set() && self.types.get(rt).is_set() {
                                return lt; // set operations
                            }
                            if !expression_compatible(&self.types, lt, rt) {
                                self.error(&expr.loc, "incompatible types in arithmetic");
                            }
                        }
                        // Return the "wider" type
                        if lt == TY_REAL || lt == TY_LONGREAL || rt == TY_REAL || rt == TY_LONGREAL
                        {
                            if lt == TY_LONGREAL || rt == TY_LONGREAL {
                                TY_LONGREAL
                            } else {
                                TY_REAL
                            }
                        } else {
                            lt
                        }
                    }
                    BinaryOp::RealDiv => {
                        // '/' is overloaded: symmetric difference for sets, real division for numbers
                        if lt != TY_VOID && rt != TY_VOID {
                            if self.types.get(lt).is_set() && self.types.get(rt).is_set() {
                                return lt; // set symmetric difference
                            }
                            if !self.types.get(lt).is_numeric() {
                                self.error(&expr.loc, "'/' requires numeric or SET operands");
                            }
                        }
                        TY_REAL
                    }
                    BinaryOp::IntDiv | BinaryOp::Mod => {
                        if lt != TY_VOID && !self.types.get(lt).is_integer_type() {
                            self.error(&expr.loc, "DIV/MOD requires integer operands");
                        }
                        TY_INTEGER
                    }
                    BinaryOp::And | BinaryOp::Or => {
                        if lt != TY_VOID && lt != TY_BOOLEAN {
                            self.error(&expr.loc, "AND/OR requires BOOLEAN operands");
                        }
                        TY_BOOLEAN
                    }
                    BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Le
                    | BinaryOp::Gt | BinaryOp::Ge => TY_BOOLEAN,
                    BinaryOp::In => {
                        // left must be ordinal, right must be set
                        TY_BOOLEAN
                    }
                }
            }
            ExprKind::SetConstructor { base_type, elements } => {
                for elem in elements {
                    match elem {
                        SetElement::Single(e) => {
                            self.analyze_expr(e);
                        }
                        SetElement::Range(lo, hi) => {
                            self.analyze_expr(lo);
                            self.analyze_expr(hi);
                        }
                    }
                }
                TY_BITSET
            }
        }
    }

    fn analyze_designator(&mut self, desig: &Designator) -> TypeId {
        let sym_type = if let Some(module) = &desig.ident.module {
            // Qualified access: Module.Name
            let lookup = self.symtab.lookup_qualified_with_scope(module, &desig.ident.name)
                .map(|(ds, sym)| (ds, sym.typ));
            if let Some((def_scope, typ)) = lookup {
                self.record_use_ref(&desig.ident.loc, &desig.ident.name, def_scope);
                typ
            } else {
                // Try direct lookup as fallback
                let lookup2 = self.symtab.lookup_in_scope_with_id(self.current_scope, &desig.ident.name)
                    .map(|(ds, sym)| (ds, sym.typ));
                if let Some((def_scope, typ)) = lookup2 {
                    self.record_use_ref(&desig.ident.loc, &desig.ident.name, def_scope);
                    typ
                } else {
                    TY_VOID
                }
            }
        } else {
            let lookup = self.symtab.lookup_in_scope_with_id(self.current_scope, &desig.ident.name)
                .map(|(ds, sym)| (ds, sym.typ));
            if let Some((def_scope, typ)) = lookup {
                self.record_use_ref(&desig.ident.loc, &desig.ident.name, def_scope);
                typ
            } else {
                // Don't error for imported names that might be forward-declared
                TY_VOID
            }
        };

        let mut current_type = sym_type;
        for sel in &desig.selectors {
            match sel {
                Selector::Field(name, loc) => {
                    if current_type != TY_VOID {
                        match self.types.get(current_type) {
                            Type::Record { fields, variants, .. } => {
                                if let Some(f) = fields.iter().find(|f| &f.name == name) {
                                    current_type = f.typ;
                                } else if name == "variant" || name.starts_with("v") {
                                    // Allow variant field access through union/struct syntax
                                    // (variant, v0, v1, etc.) - trust the programmer
                                    // We can't fully type-check variant access at this point
                                    current_type = TY_VOID; // unknown but allowed
                                } else if variants.is_some() {
                                    // In a variant record, variant fields may be accessed directly
                                    // Check variant fields
                                    let mut found = false;
                                    if let Some(vi) = variants {
                                        for vc in &vi.variants {
                                            if let Some(f) = vc.fields.iter().find(|f| &f.name == name) {
                                                current_type = f.typ;
                                                found = true;
                                                break;
                                            }
                                        }
                                    }
                                    if !found {
                                        self.error(loc, format!("no field '{}' in record", name));
                                        current_type = TY_VOID;
                                    }
                                } else {
                                    self.error(loc, format!("no field '{}' in record", name));
                                    current_type = TY_VOID;
                                }
                            }
                            Type::Opaque { .. } => {
                                // Opaque type - allow field access (trust the programmer)
                                current_type = TY_VOID;
                            }
                            Type::Void => {
                                // Already void (e.g. from variant access) - stay void
                            }
                            _ => {
                                self.error(loc, "field access on non-record type");
                                current_type = TY_VOID;
                            }
                        }
                    }
                }
                Selector::Index(indices, loc) => {
                    for idx in indices {
                        self.analyze_expr(idx);
                        // Peel off one array dimension per index
                        if current_type != TY_VOID {
                            match self.types.get(current_type) {
                                Type::Array { elem_type, .. } => current_type = *elem_type,
                                Type::OpenArray { elem_type } => current_type = *elem_type,
                                Type::StringLit(_) => current_type = TY_CHAR,
                                _ => {
                                    self.error(loc, "indexing non-array type");
                                    current_type = TY_VOID;
                                }
                            }
                        }
                    }
                }
                Selector::Deref(loc) => {
                    if current_type != TY_VOID {
                        match self.types.get(current_type) {
                            Type::Pointer { base } => {
                                current_type = *base;
                            }
                            Type::Ref { target, .. } => {
                                current_type = *target;
                            }
                            _ => {
                                self.error(loc, "dereference of non-pointer type");
                                current_type = TY_VOID;
                            }
                        }
                    }
                }
            }
        }
        current_type
    }

    fn check_builtin_call(&mut self, name: &str, args: &[Expr], loc: &SourceLoc) {
        for arg in args {
            self.analyze_expr(arg);
        }
        match name {
            "INC" | "DEC" => {
                if args.is_empty() || args.len() > 2 {
                    self.error(loc, format!("{} expects 1 or 2 arguments", name));
                }
            }
            "INCL" | "EXCL" => {
                if args.len() != 2 {
                    self.error(loc, format!("{} expects 2 arguments", name));
                }
            }
            "NEW" | "DISPOSE" => {
                if args.len() != 1 {
                    self.error(loc, format!("{} expects 1 argument", name));
                }
            }
            "HALT" => {
                if args.len() > 1 {
                    self.error(loc, "HALT expects 0 or 1 arguments");
                }
            }
            "SHIFT" | "ROTATE" | "SHL" | "SHR" | "BAND" | "BOR" | "BXOR" => {
                if args.len() != 2 {
                    self.error(loc, format!("{} expects 2 arguments", name));
                }
            }
            "BNOT" => {
                if args.len() != 1 {
                    self.error(loc, "BNOT expects 1 argument");
                }
            }
            _ => {}
        }
    }

    // ── Constant evaluation ─────────────────────────────────────────

    fn eval_const_expr(&self, expr: &Expr) -> ConstValue {
        match &expr.kind {
            ExprKind::IntLit(v) => ConstValue::Integer(*v),
            ExprKind::RealLit(v) => ConstValue::Real(*v),
            ExprKind::StringLit(s) => ConstValue::String(s.clone()),
            ExprKind::CharLit(c) => ConstValue::Char(*c),
            ExprKind::BoolLit(b) => ConstValue::Boolean(*b),
            ExprKind::NilLit => ConstValue::Nil,
            ExprKind::UnaryOp {
                op: UnaryOp::Neg,
                operand,
            } => match self.eval_const_expr(operand) {
                ConstValue::Integer(v) => ConstValue::Integer(-v),
                ConstValue::Real(v) => ConstValue::Real(-v),
                other => other,
            },
            ExprKind::BinaryOp { op, left, right } => {
                let l = self.eval_const_expr(left);
                let r = self.eval_const_expr(right);
                match (l, r) {
                    (ConstValue::Integer(a), ConstValue::Integer(b)) => match op {
                        BinaryOp::Add => ConstValue::Integer(a + b),
                        BinaryOp::Sub => ConstValue::Integer(a - b),
                        BinaryOp::Mul => ConstValue::Integer(a * b),
                        BinaryOp::IntDiv => {
                            if b != 0 {
                                ConstValue::Integer(a / b)
                            } else {
                                ConstValue::Integer(0)
                            }
                        }
                        BinaryOp::Mod => {
                            if b != 0 {
                                ConstValue::Integer(a % b)
                            } else {
                                ConstValue::Integer(0)
                            }
                        }
                        _ => ConstValue::Integer(0),
                    },
                    _ => ConstValue::Integer(0),
                }
            }
            ExprKind::Designator(d) => {
                if let Some(sym) = self.symtab.lookup(&d.ident.name) {
                    if let SymbolKind::Constant(v) = &sym.kind {
                        return v.clone();
                    }
                    if let SymbolKind::EnumVariant(v) = &sym.kind {
                        return ConstValue::Integer(*v);
                    }
                }
                ConstValue::Integer(0)
            }
            _ => ConstValue::Integer(0),
        }
    }

    fn eval_const_int(&self, expr: &Expr) -> Option<i64> {
        match self.eval_const_expr(expr) {
            ConstValue::Integer(v) => Some(v),
            ConstValue::Char(c) => Some(c as i64),
            _ => None,
        }
    }
}
