//! HIR builder: constructs HIR from AST + sema.
//!
//! Phase 4 of the compilation pipeline. Takes finalized sema (read-only)
//! and produces a complete `HirModule` with all procedure bodies lowered.
//!
//! Entry point: `build_module()` for full module construction.
//! `HirBuilder` is the internal workhorse — builds HIR Places, expressions,
//! and statements from AST nodes using sema's scope chain and type registry.

use std::collections::{HashMap, HashSet};

use crate::ast::{self, Designator, Selector, ExprKind, SetElement};
use crate::ast::{CompilationUnit, Declaration, ProcDecl, Statement};
use crate::hir::*;
use crate::sema::SemanticAnalyzer;
use crate::symtab::{SymbolTable, SymbolKind, ConstValue};
use crate::types::*;

// ══════════════════════════════════════════════════════════════════════
// Phase 4: build_module — construct complete HirModule from AST + sema
// ══════════════════════════════════════════════════════════════════════

/// Build a complete `HirModule` from the main compilation unit and
/// all embedded implementation modules. Sema must be finalized (Phases 0-3
/// complete). This function is read-only over sema.
pub fn build_module(
    unit: &CompilationUnit,
    impl_mods: &[crate::ast::ImplementationModule],
    sema: &SemanticAnalyzer,
) -> HirModule {
    let (module_name, module_body, module_decls, module_imports, module_loc) = match unit {
        CompilationUnit::ProgramModule(m) => (
            m.name.clone(),
            m.block.body.as_ref(),
            &m.block.decls,
            &m.imports,
            &m.loc,
        ),
        CompilationUnit::ImplementationModule(m) => (
            m.name.clone(),
            m.block.body.as_ref(),
            &m.block.decls,
            &m.imports,
            &m.loc,
        ),
        CompilationUnit::DefinitionModule(m) => (
            m.name.clone(),
            None,
            &Vec::new() as &Vec<Declaration>,
            &m.imports,
            &m.loc,
        ),
    };

    // Extract imported module names and aliases from AST
    let (imported_modules, import_aliases) = extract_imports(module_imports);

    let mut procedures = Vec::new();
    // Legacy fields
    let mut type_decls_legacy = Vec::new();
    let mut constants = Vec::new();
    let mut globals = Vec::new();
    // New structural fields
    let mut new_type_decls = Vec::new();
    let mut new_const_decls = Vec::new();
    let mut new_global_decls = Vec::new();
    let mut new_exception_decls = Vec::new();

    // Build HirImport list from extracted imports
    let hir_imports: Vec<HirImport> = module_imports.iter().map(|imp| {
        let names = imp.names.iter().map(|n| {
            HirImportName {
                name: n.name.clone(),
                local_name: n.local_name().to_string(),
            }
        }).collect();
        HirImport {
            module: imp.from_module.clone().unwrap_or_default(),
            names,
            is_qualified: imp.from_module.is_none(),
        }
    }).collect();

    // Collect structural declarations + lower procedures
    for decl in module_decls {
        match decl {
            Declaration::Procedure(p) => {
                let hir_proc = build_proc(p, &module_name, &imported_modules, &import_aliases, sema);
                procedures.push(hir_proc);
            }
            Declaration::Type(t) => {
                let sym = sema.symtab.lookup_any(&t.name);
                let type_id = sym.map(|s| s.typ).unwrap_or(TY_INTEGER);
                let exported = sym.map(|s| s.exported).unwrap_or(false);
                let td = HirTypeDecl {
                    name: t.name.clone(),
                    mangled: format!("{}_{}", module_name, t.name),
                    type_id,
                    exported,
                    ast_type_node: t.typ.clone(),
                };
                new_type_decls.push(td.clone());
                type_decls_legacy.push(td);
            }
            Declaration::Const(c) => {
                let sym = sema.symtab.lookup_any(&c.name);
                let val = sym
                    .and_then(|s| match &s.kind {
                        SymbolKind::Constant(cv) => Some(const_value_to_hir(cv)),
                        _ => None,
                    })
                    .unwrap_or(ConstVal::Integer(0));
                let type_id = sym.map(|s| s.typ).unwrap_or(TY_INTEGER);
                let mangled = format!("{}_{}", module_name, c.name);
                // New format
                new_const_decls.push(HirConstDecl {
                    name: c.name.clone(),
                    mangled: mangled.clone(),
                    value: val.clone(),
                    type_id,
                    exported: sym.map(|s| s.exported).unwrap_or(false),
                    c_type: String::new(), // populated by backend
                });
                // Legacy format
                constants.push(HirConst {
                    name: SymbolId {
                        mangled,
                        source_name: c.name.clone(),
                        module: Some(module_name.clone()),
                        ty: type_id,
                        is_var_param: false,
                        is_open_array: false,
                    },
                    value: val,
                    ty: type_id,
                });
            }
            Declaration::Var(v) => {
                let type_id = sema.symtab.lookup_any(&v.names[0])
                    .map(|s| s.typ).unwrap_or(TY_INTEGER);
                for name in &v.names {
                    let mangled = format!("{}_{}", module_name, name);
                    let exported = sema.symtab.lookup_any(name)
                        .map(|s| s.exported).unwrap_or(false);
                    // New format
                    new_global_decls.push(HirGlobalDecl {
                        name: name.clone(),
                        mangled: mangled.clone(),
                        type_id,
                        exported,
                        c_type: String::new(),        // populated by backend
                        c_array_suffix: String::new(), // populated by backend
                        is_proc_type: false,           // populated by backend
                    });
                    // Legacy format
                    globals.push(HirVar {
                        name: SymbolId {
                            mangled,
                            source_name: name.clone(),
                            module: Some(module_name.clone()),
                            ty: type_id,
                            is_var_param: false,
                            is_open_array: false,
                        },
                        ty: type_id,
                        exported,
                    });
                }
            }
            Declaration::Exception(e) => {
                let sym = sema.symtab.lookup_any(&e.name);
                let exc_id = sym.and_then(|s| match &s.kind {
                    SymbolKind::Constant(ConstValue::Integer(v)) => Some(*v),
                    _ => None,
                }).unwrap_or(0);
                new_exception_decls.push(HirExceptionDecl {
                    name: e.name.clone(),
                    mangled: format!("M2_EXC_{}", e.name),
                    exc_id,
                });
            }
            _ => {} // Module — handled separately
        }
    }

    // Lower embedded implementation module procedures
    for imp in impl_mods {
        let (imp_modules, imp_aliases) = extract_imports(&imp.imports);
        // Merge main module imports with embedded module imports
        let mut merged_modules = imported_modules.clone();
        merged_modules.extend(imp_modules);
        let mut merged_aliases = import_aliases.clone();
        merged_aliases.extend(imp_aliases);

        for decl in &imp.block.decls {
            if let Declaration::Procedure(p) = decl {
                let hir_proc = build_proc(p, &imp.name, &merged_modules, &merged_aliases, sema);
                procedures.push(hir_proc);
            }
        }
    }

    // Lower module init body
    let init_body = module_body.map(|stmts| {
        let mut hb = HirBuilder::new(
            &sema.types,
            &sema.symtab,
            &module_name,
            &sema.foreign_modules,
        );
        hb.set_imported_modules(imported_modules.clone());
        hb.set_import_alias_map(import_aliases.clone());
        hb.lower_stmts(stmts)
    });

    // Lower embedded module init bodies
    let mut embedded_init_bodies = Vec::new();
    for imp in impl_mods {
        if let Some(stmts) = &imp.block.body {
            let (imp_modules, imp_aliases) = extract_imports(&imp.imports);
            let mut merged_modules = imported_modules.clone();
            merged_modules.extend(imp_modules);
            let mut merged_aliases = import_aliases.clone();
            merged_aliases.extend(imp_aliases);

            let mut hb = HirBuilder::new(
                &sema.types,
                &sema.symtab,
                &imp.name,
                &sema.foreign_modules,
            );
            hb.set_imported_modules(merged_modules);
            hb.set_import_alias_map(merged_aliases);
            let body = hb.lower_stmts(stmts);
            embedded_init_bodies.push((imp.name.clone(), body));
        }
    }

    #[allow(deprecated)]
    HirModule {
        name: module_name,
        source_file: module_loc.file.clone(),
        string_pool: Vec::new(),
        // New structural fields
        imports: hir_imports,
        type_decls: new_type_decls,
        const_decls: new_const_decls,
        global_decls: new_global_decls,
        exception_decls: new_exception_decls,
        type_descs: Vec::new(), // populated by backends for RTTI
        proc_decls: Vec::new(), // TODO: populate from procedures
        except_handler: None,   // TODO: populate from AST
        finally_handler: None,  // TODO: populate from AST
        embedded_modules: Vec::new(), // TODO: populate from impl_mods
        // Legacy fields (still used by backends)
        constants,
        types: type_decls_legacy,
        globals,
        procedures,
        init_body,
        embedded_init_bodies,
        externals: Vec::new(),
    }
}

/// Extract imported module names and alias mappings from AST imports.
fn extract_imports(imports: &[crate::ast::Import]) -> (Vec<String>, HashMap<String, String>) {
    let mut modules = Vec::new();
    let mut aliases = HashMap::new();
    for imp in imports {
        if let Some(ref from_mod) = imp.from_module {
            modules.push(from_mod.clone());
            for name in &imp.names {
                if let Some(ref alias) = name.alias {
                    aliases.insert(alias.clone(), name.name.clone());
                }
            }
        } else {
            for name in &imp.names {
                modules.push(name.name.clone());
            }
        }
    }
    (modules, aliases)
}

/// Build an `HirProc` for a single procedure declaration.
fn build_proc(
    p: &ProcDecl,
    module_name: &str,
    imported_modules: &[String],
    import_aliases: &HashMap<String, String>,
    sema: &SemanticAnalyzer,
) -> HirProc {
    let mut hb = HirBuilder::new(
        &sema.types,
        &sema.symtab,
        module_name,
        &sema.foreign_modules,
    );
    hb.set_imported_modules(imported_modules.to_vec());
    hb.set_import_alias_map(import_aliases.clone());
    hb.enter_procedure_named(&p.heading.name);

    // Register open array _high companions
    for fp in &p.heading.params {
        if matches!(fp.typ, ast::TypeNode::OpenArray { .. }) {
            for name in &fp.names {
                let high_name = format!("{}_high", name);
                hb.register_var(&high_name, TY_INTEGER);
                hb.register_local(&high_name);
            }
        }
    }

    // Lower body
    let body = if let Some(stmts) = &p.block.body {
        Some(hb.lower_stmts(stmts))
    } else {
        None
    };

    // Build nested procs
    let mut nested = Vec::new();
    for decl in &p.block.decls {
        if let Declaration::Procedure(np) = decl {
            let nested_proc = build_proc(np, module_name, imported_modules, import_aliases, sema);
            nested.push(nested_proc);
        }
    }

    // Build params
    let params: Vec<HirParam> = p.heading.params.iter().flat_map(|fp| {
        let is_open = matches!(fp.typ, ast::TypeNode::OpenArray { .. });
        fp.names.iter().map(move |name| HirParam {
            name: name.clone(),
            ty: TY_INTEGER, // placeholder — backends use sema for actual types
            is_var: fp.is_var,
            is_open_array: is_open,
        })
    }).collect();

    HirProc {
        name: SymbolId {
            mangled: format!("{}_{}", module_name, p.heading.name),
            source_name: p.heading.name.clone(),
            module: Some(module_name.to_string()),
            ty: TY_VOID,
            is_var_param: false,
            is_open_array: false,
        },
        params,
        return_type: None, // backends use sema
        captures: Vec::new(),
        locals: Vec::new(),
        body,
        nested_procs: nested,
        is_exported: false,
    }
}

/// WITH scope entry: tracks the record variable being opened and which
/// field names are visible as bare identifiers.
struct WithScope {
    /// Name of the record variable (the designator in `WITH x DO`).
    record_var: String,
    /// TypeId of the record type (after deref if pointer-to-record).
    record_tid: TypeId,
    /// Field names exposed by this WITH scope.
    field_names: Vec<String>,
    /// Whether the designator needed a pointer deref (POINTER TO Record).
    needs_deref: bool,
}

/// Read-only context from the backend, passed by reference.
/// Eliminates the need to copy maps into the HirBuilder.
pub struct CodegenContext<'a> {
    pub import_alias_map: &'a HashMap<String, String>,
    pub imported_modules: &'a HashSet<String>,
    /// Variable name → TypeId from the backend's tracking.
    pub var_types: &'a HashMap<String, TypeId>,
    /// Current procedure's local variable names (owned — small set).
    pub local_names: HashSet<String>,
}

pub struct HirBuilder<'a> {
    pub types: &'a TypeRegistry,
    pub symtab: &'a SymbolTable,

    /// Current module name (for mangling).
    module_name: String,

    /// Foreign C modules — borrowed from sema.
    foreign_modules: &'a HashSet<String>,

    /// Backend context — borrowed, not copied.
    ctx: Option<CodegenContext<'a>>,

    /// Import alias map (owned fallback when no ctx).
    import_alias_map: HashMap<String, String>,
    imported_modules_owned: Vec<String>,
    var_types_owned: HashMap<String, TypeId>,
    local_names_owned: Vec<String>,

    /// WITH scope stack.
    with_stack: Vec<WithScope>,

    /// Scope tracking: whether we're inside a procedure.
    in_procedure: bool,

    /// Current sema scope ID for scope-aware symbol resolution.
    current_scope: Option<usize>,
    /// Stack of saved scopes for enter/leave_procedure.
    scope_stack: Vec<Option<usize>>,

    /// Interned string pool (Phase 3).
    string_pool: Vec<String>,
}

impl<'a> HirBuilder<'a> {
    pub fn new(
        types: &'a TypeRegistry,
        symtab: &'a SymbolTable,
        module_name: &str,
        foreign_modules: &'a HashSet<String>,
    ) -> Self {
        Self {
            types,
            symtab,
            module_name: module_name.to_string(),
            foreign_modules,
            ctx: None,
            import_alias_map: HashMap::new(),
            imported_modules_owned: Vec::new(),
            var_types_owned: HashMap::new(),
            local_names_owned: Vec::new(),
            with_stack: Vec::new(),
            in_procedure: false,
            current_scope: symtab.lookup_module_scope(module_name),
            scope_stack: Vec::new(),
            string_pool: Vec::new(),
        }
    }

    /// Create a builder with a borrowed backend context (zero-copy).
    pub fn with_context(
        types: &'a TypeRegistry,
        symtab: &'a SymbolTable,
        module_name: &str,
        foreign_modules: &'a HashSet<String>,
        ctx: CodegenContext<'a>,
    ) -> Self {
        Self {
            types,
            symtab,
            module_name: module_name.to_string(),
            foreign_modules,
            ctx: Some(ctx),
            import_alias_map: HashMap::new(),
            imported_modules_owned: Vec::new(),
            var_types_owned: HashMap::new(),
            local_names_owned: Vec::new(),
            with_stack: Vec::new(),
            in_procedure: false,
            current_scope: symtab.lookup_module_scope(module_name),
            scope_stack: Vec::new(),
            string_pool: Vec::new(),
        }
    }

    // ── Context accessors ───────────────────────────────────────────

    fn get_import_alias_map(&self) -> &HashMap<String, String> {
        if let Some(ref ctx) = self.ctx { ctx.import_alias_map } else { &self.import_alias_map }
    }
    fn is_imported_module(&self, name: &str) -> bool {
        if let Some(ref ctx) = self.ctx {
            ctx.imported_modules.contains(name)
        } else {
            self.imported_modules_owned.iter().any(|m| m == name)
        }
    }
    fn is_foreign_module(&self, name: &str) -> bool {
        self.foreign_modules.contains(name)
    }
    fn get_var_type(&self, name: &str) -> Option<TypeId> {
        // Check owned first (dynamically registered, e.g. TYPECASE bindings),
        // then context (backend-provided).
        self.var_types_owned.get(name).copied()
            .or_else(|| {
                if let Some(ref ctx) = self.ctx {
                    ctx.var_types.get(name).copied()
                } else {
                    None
                }
            })
    }
    fn is_local_name(&self, name: &str) -> bool {
        self.local_names_owned.iter().any(|n| n == name)
            || self.ctx.as_ref().map_or(false, |ctx| ctx.local_names.contains(name))
    }

    // ── Configuration (populated from sema/driver before resolution) ──

    pub fn set_import_alias_map(&mut self, map: HashMap<String, String>) {
        self.import_alias_map = map;
    }

    pub fn set_imported_modules(&mut self, modules: Vec<String>) {
        self.imported_modules_owned = modules;
    }

    pub fn register_var(&mut self, name: &str, tid: TypeId) {
        self.var_types_owned.insert(name.to_string(), tid);
    }

    pub fn register_local(&mut self, name: &str) {
        self.local_names_owned.push(name.to_string());
    }

    pub fn enter_procedure(&mut self) {
        self.scope_stack.push(self.current_scope);
        self.in_procedure = true;
        self.local_names_owned.clear();
    }

    /// Enter a procedure with a known scope name. Looks up the scope ID
    /// in the symtab so that symbol resolution is scope-correct.
    pub fn enter_procedure_named(&mut self, proc_name: &str) {
        self.scope_stack.push(self.current_scope);
        self.in_procedure = true;
        self.local_names_owned.clear();
        // Find the procedure's scope: must be a direct child of the current
        // module/procedure scope. Never use lookup_module_scope here — a
        // procedure named "Eval" must not collide with a MODULE named "Eval".
        let count = self.symtab.scope_count();
        self.current_scope = None;
        // Primary: child of current scope with matching name
        if let Some(cur) = self.scope_stack.last().copied().flatten() {
            for id in 0..count {
                if self.symtab.scope_name(id) == Some(proc_name)
                    && self.symtab.scope_parent(id) == Some(cur)
                {
                    self.current_scope = Some(id);
                    return;
                }
            }
        }
        // Fallback: nested proc mangled name (e.g., "Outer_Inner").
        // Walk the scope chain: split on '_', find each segment as a child scope.
        if proc_name.contains('_') {
            let parts: Vec<&str> = proc_name.splitn(2, '_').collect();
            if parts.len() == 2 {
                let parent_name = parts[0];
                let child_name = parts[1];
                // Find the parent scope
                if let Some(cur) = self.scope_stack.last().copied().flatten() {
                    for id in 0..count {
                        if self.symtab.scope_name(id) == Some(parent_name)
                            && self.symtab.scope_parent(id) == Some(cur)
                        {
                            // Found parent; now find child within it
                            for cid in 0..count {
                                if self.symtab.scope_name(cid) == Some(child_name)
                                    && self.symtab.scope_parent(cid) == Some(id)
                                {
                                    self.current_scope = Some(cid);
                                    return;
                                }
                            }
                            // Child not found by exact name — try recursively
                            // for deeper nesting (A_B_C)
                            self.current_scope = Some(id);
                            if child_name.contains('_') {
                                // Recurse by re-entering with remaining name
                                self.scope_stack.push(Some(id));
                                self.enter_procedure_named(child_name);
                                return;
                            }
                            break;
                        }
                    }
                }
            }
        }
        // Last resort: any scope with this name that is NOT a module scope
        for id in 0..count {
            if self.symtab.scope_name(id) == Some(proc_name) {
                if self.symtab.lookup_module_scope(proc_name) == Some(id) {
                    continue;
                }
                self.current_scope = Some(id);
                return;
            }
        }
        eprintln!("[HIR] enter_procedure_named('{}') FAILED: no scope found, saved_cur={:?}, module={}, mod_scope={:?}",
            proc_name, self.scope_stack.last().copied().flatten(), self.module_name,
            self.symtab.lookup_module_scope(&self.module_name));
    }

    pub fn leave_procedure(&mut self) {
        self.current_scope = self.scope_stack.pop().flatten();
        self.in_procedure = false;
        self.local_names_owned.clear();
    }

    // ── WITH scope management ─────────────────────────────────────────

    /// Push a WITH scope. Called when entering `WITH desig DO`.
    /// `desig_tid` is the TypeId of the designator — may be a pointer
    /// (in which case we auto-deref to the record).
    pub fn push_with(&mut self, var_name: &str, desig_tid: TypeId) {
        let resolved = self.resolve_alias(desig_tid);
        let (record_tid, needs_deref) = match self.types.get(resolved) {
            Type::Pointer { base } => {
                let target = self.resolve_alias(*base);
                (target, true)
            }
            Type::Record { .. } => (resolved, false),
            _ => (resolved, false),
        };

        let field_names = match self.types.get(record_tid) {
            Type::Record { fields, variants } => {
                let mut names: Vec<String> = fields.iter().map(|f| f.name.clone()).collect();
                // Include variant fields
                if let Some(vi) = variants {
                    for vc in &vi.variants {
                        for f in &vc.fields {
                            names.push(f.name.clone());
                        }
                    }
                }
                names
            }
            _ => Vec::new(),
        };

        self.with_stack.push(WithScope {
            record_var: var_name.to_string(),
            record_tid,
            field_names,
            needs_deref,
        });
    }

    pub fn pop_with(&mut self) {
        self.with_stack.pop();
    }

    // ── Core resolution ───────────────────────────────────────────────

    /// Resolve an AST Designator into an HIR Place.
    /// Returns `None` if the designator cannot be resolved (e.g., unknown symbol).
    pub fn resolve_designator(&mut self, d: &Designator) -> Option<Place> {
        let name = &d.ident.name;
        let loc = d.loc.clone();

        // 1. Handle Module.Field pattern (whole-module import)
        if d.ident.module.is_none()
            && !d.selectors.is_empty()
            && self.is_imported_module(name)
        {
            if let Some(Selector::Field(field_name, _)) = d.selectors.first() {
                return self.resolve_module_qualified(name, field_name, &d.selectors[1..], &loc);
            }
        }

        // 2. Check WITH stack: bare identifier matching a WITH field
        if d.ident.module.is_none() {
            if let Some(place) = self.resolve_with_field(name, &d.selectors, &loc) {
                return Some(place);
            }
        }

        // 3. Resolve the base symbol
        let (base, base_tid) = self.resolve_base(d)?;

        // 4. Apply selectors
        self.apply_selectors(base, base_tid, &d.selectors, &loc)
    }

    /// Check if a bare identifier is a field name in an active WITH scope.
    fn resolve_with_field(
        &mut self,
        name: &str,
        selectors: &[Selector],
        loc: &crate::errors::SourceLoc,
    ) -> Option<Place> {
        // Search WITH stack from innermost to outermost
        for ws in self.with_stack.iter().rev() {
            if !ws.field_names.contains(&name.to_string()) {
                continue;
            }

            // Found: this bare name is a field in the WITH record.
            // Build a Place: base is the WITH record variable, first
            // projection is the field access.
            let record_var_tid = self.get_var_type(&ws.record_var)
                .unwrap_or(ws.record_tid);

            let is_local = self.is_local_name(&ws.record_var);
            let base = if is_local {
                PlaceBase::Local(SymbolId {
                    mangled: ws.record_var.clone(),
                    source_name: ws.record_var.clone(),
                    module: None,
                    ty: record_var_tid,
                    is_var_param: false,
                    is_open_array: false,
                })
            } else {
                PlaceBase::Global(SymbolId {
                    mangled: self.mangle(&ws.record_var),
                    source_name: ws.record_var.clone(),
                    module: Some(self.module_name.clone()),
                    ty: record_var_tid,
                    is_var_param: false,
                    is_open_array: false,
                })
            };

            let mut projections = Vec::new();

            // If WITH var is a pointer, add a Deref first
            if ws.needs_deref {
                projections.push(Projection {
                    kind: ProjectionKind::Deref,
                    ty: ws.record_tid,
                });
            }

            // Add the field projection
            let field_proj = self.resolve_field_projection(ws.record_tid, name)?;
            let field_ty = field_proj.ty;
            projections.push(field_proj);

            // Apply remaining selectors
            let mut current_ty = field_ty;
            for sel in selectors {
                let proj = self.resolve_selector(current_ty, sel)?;
                current_ty = proj.ty;
                projections.push(proj);
            }

            return Some(Place {
                base,
                projections,
                ty: current_ty,
                loc: loc.clone(),
            });
        }
        None
    }

    /// Resolve a module-qualified access: Module.Field with optional further selectors.
    fn resolve_module_qualified(
        &mut self,
        module: &str,
        field_name: &str,
        remaining_selectors: &[Selector],
        loc: &crate::errors::SourceLoc,
    ) -> Option<Place> {
        // Apply stdlib C name mapping for non-native stdlib modules
        let mangled = if crate::stdlib::is_stdlib_module(module) && !crate::stdlib::is_native_stdlib(module) {
            crate::stdlib::map_stdlib_call(module, field_name)
                .unwrap_or_else(|| format!("{}_{}", module, field_name))
        } else {
            format!("{}_{}", module, field_name)
        };

        // Look up in symtab
        let sym = self.symtab.lookup_qualified(module, field_name)
            .or_else(|| self.symtab.lookup_any(&mangled));

        let (base, base_tid) = if let Some(sym) = sym {
            let sid = SymbolId {
                mangled: mangled.clone(),
                source_name: field_name.to_string(),
                module: Some(module.to_string()),
                ty: sym.typ,
                is_var_param: false,
                is_open_array: false,
            };
            match &sym.kind {
                SymbolKind::Constant(cv) => {
                    let cv = const_value_to_hir(cv);
                    (PlaceBase::Constant(cv), sym.typ)
                }
                SymbolKind::Procedure { .. } => {
                    (PlaceBase::FuncRef(sid), sym.typ)
                }
                SymbolKind::Variable | SymbolKind::Field => {
                    (PlaceBase::Global(sid), sym.typ)
                }
                SymbolKind::EnumVariant(v) => {
                    (PlaceBase::Constant(ConstVal::EnumVariant(*v)), sym.typ)
                }
                _ => {
                    (PlaceBase::Global(sid), sym.typ)
                }
            }
        } else {
            // Not found in symtab — create a global reference anyway
            let sid = SymbolId {
                mangled: mangled.clone(),
                source_name: field_name.to_string(),
                module: Some(module.to_string()),
                ty: TY_INTEGER, // fallback
                is_var_param: false,
                is_open_array: false,
            };
            (PlaceBase::Global(sid), TY_INTEGER)
        };

        if remaining_selectors.is_empty() {
            return Some(Place {
                base,
                projections: Vec::new(),
                ty: base_tid,
                loc: loc.clone(),
            });
        }

        self.apply_selectors(base, base_tid, remaining_selectors, loc)
    }

    /// Resolve the base of a designator (the identifier part, before selectors).
    fn resolve_base(&self, d: &Designator) -> Option<(PlaceBase, TypeId)> {
        let name = &d.ident.name;

        // Explicit Module.Name qualification
        if let Some(ref module) = d.ident.module {
            return self.resolve_module_qualified_base(module, name);
        }

        // Check scope for constants/enums first — they should be resolved
        // as inline values, not as variable references (even if var_types
        // has the name from codegen's const declaration).
        if let Some(sym) = self.scope_lookup(name) {
            if matches!(sym.kind, SymbolKind::Constant(_) | SymbolKind::EnumVariant(_)) {
                return Some(self.symbol_to_base(name, sym));
            }
        }

        // Check if name is a local variable/parameter defined in the
        // current procedure scope (not inherited from a module scope).
        // Backend's local_names/var_types hint at locality, but sema
        // scope is authoritative — a module variable in the alloca set
        // is still a global.
        if self.in_procedure {
            let is_in_proc_scope = self.current_scope
                .and_then(|sid| self.symtab.lookup_in_scope_direct(sid, name))
                .map(|s| matches!(s.kind, SymbolKind::Variable | SymbolKind::Field)
                     && s.module.is_none())
                .unwrap_or(false);
            if is_in_proc_scope {
                if let Some(sym) = self.scope_lookup(name) {
                    let tid = sym.typ;
                    let sid = SymbolId {
                        mangled: name.to_string(),
                        source_name: name.to_string(),
                        module: None,
                        ty: tid,
                        is_var_param: sym.is_var_param,
                        is_open_array: sym.is_open_array,
                    };
                    return Some((PlaceBase::Local(sid), tid));
                }
            }
        }

        // Look up in symtab using the current scope chain (scope-aware).
        // Sema is the single source of truth for TypeIds and symbol kinds.
        if let Some(sym) = self.scope_lookup(name) {
            return Some(self.symbol_to_base(name, sym));
        }

        // If the name is in var_types, it was explicitly registered — resolve it.
        // Otherwise return None so callers fall back to backend-specific resolution.
        if let Some(&tid) = self.get_var_type(name).as_ref() {
            let is_local = self.in_procedure && self.is_local_name(name);
            let mangled = if is_local { name.to_string() } else { self.mangle(name) };
            let sid = SymbolId {
                mangled,
                source_name: name.to_string(),
                module: if is_local { None } else { Some(self.module_name.clone()) },
                ty: tid,
                is_var_param: false,
                is_open_array: false,
            };
            return Some(if is_local {
                (PlaceBase::Local(sid), tid)
            } else {
                (PlaceBase::Global(sid), tid)
            });
        }
        None
    }

    fn resolve_module_qualified_base(&self, module: &str, name: &str) -> Option<(PlaceBase, TypeId)> {
        let mangled = if self.is_foreign_module(&module.to_string()) {
            name.to_string()
        } else {
            format!("{}_{}", module, name)
        };

        if let Some(sym) = self.symtab.lookup_qualified(module, name) {
            let sid = SymbolId {
                mangled,
                source_name: name.to_string(),
                module: Some(module.to_string()),
                ty: sym.typ,
                is_var_param: false,
                is_open_array: false,
            };
            Some(match &sym.kind {
                SymbolKind::Constant(cv) => (PlaceBase::Constant(const_value_to_hir(cv)), sym.typ),
                SymbolKind::Procedure { .. } => (PlaceBase::FuncRef(sid), sym.typ),
                SymbolKind::EnumVariant(v) => (PlaceBase::Constant(ConstVal::EnumVariant(*v)), sym.typ),
                _ => (PlaceBase::Global(sid), sym.typ),
            })
        } else {
            let sid = SymbolId {
                mangled,
                source_name: name.to_string(),
                module: Some(module.to_string()),
                ty: TY_INTEGER,
                is_var_param: false,
                is_open_array: false,
            };
            Some((PlaceBase::Global(sid), TY_INTEGER))
        }
    }

    fn symbol_to_base(&self, name: &str, sym: &crate::symtab::Symbol) -> (PlaceBase, TypeId) {
        // Determine locality from sema: if we're in a procedure and the
        // symbol is defined directly in the procedure scope (not from a
        // parent/module scope), it's local.
        let is_local = self.in_procedure && {
            self.current_scope
                .and_then(|sid| self.symtab.lookup_in_scope_direct(sid, name))
                .map(|s| matches!(s.kind, SymbolKind::Variable | SymbolKind::Field)
                     && s.module.is_none())
                .unwrap_or(false)
        };
        // TypeId always from sema — never from var_types.
        let tid = sym.typ;

        // Check for VAR param and open array from symtab
        let is_var_param = matches!(&sym.kind, SymbolKind::Procedure { params, .. }
            if params.iter().any(|p| p.name == name && p.is_var));
        // For actual variables, check if they were registered as VAR params
        // (this is tracked by the caller who knows the procedure signature)
        // Use the symbol's source module for mangling if it's imported.
        // Otherwise use the current module.
        let source_module = sym.module.as_ref()
            .filter(|m| !m.is_empty())
            .cloned()
            .unwrap_or_else(|| self.module_name.clone());
        // De-alias: if the name was imported with AS, use the original name for mangling
        let original_name = self.get_import_alias_map().get(name)
            .cloned()
            .unwrap_or_else(|| name.to_string());
        let mangled_name = if is_local {
            name.to_string()
        } else if sym.module.is_some() && !self.is_foreign_module(&source_module) {
            if crate::stdlib::is_native_stdlib(&source_module) {
                // Native stdlib: normalize to the definition's case
                let canonical_name = self.resolve_canonical_name(&source_module, &original_name);
                format!("{}_{}", source_module, canonical_name)
            } else if crate::stdlib::is_stdlib_module(&source_module) {
                // Non-native stdlib: use the C runtime name (e.g., m2_Args_ArgCount)
                crate::stdlib::map_stdlib_call(&source_module, &original_name)
                    .unwrap_or_else(|| format!("{}_{}", source_module, original_name))
            } else {
                format!("{}_{}", source_module, original_name)
            }
        } else if self.is_foreign_module(&source_module) {
            original_name.clone()
        } else {
            self.mangle(&original_name)
        };
        // VAR and open-array flags come directly from sema's symbol
        let sym_is_var_param = sym.is_var_param;
        let sym_is_open_array = sym.is_open_array;

        let sid = SymbolId {
            mangled: mangled_name,
            source_name: name.to_string(),
            module: if is_local { None } else { Some(source_module) },
            ty: tid,
            is_var_param: sym_is_var_param,
            is_open_array: sym_is_open_array,
        };

        match &sym.kind {
            SymbolKind::Constant(cv) => (PlaceBase::Constant(const_value_to_hir(cv)), tid),
            SymbolKind::Procedure { .. } => (PlaceBase::FuncRef(sid), tid),
            SymbolKind::EnumVariant(v) => (PlaceBase::Constant(ConstVal::EnumVariant(*v)), tid),
            SymbolKind::Variable | SymbolKind::Field => {
                if is_local {
                    (PlaceBase::Local(sid), tid)
                } else {
                    (PlaceBase::Global(sid), tid)
                }
            }
            _ => {
                if is_local {
                    (PlaceBase::Local(sid), tid)
                } else {
                    (PlaceBase::Global(sid), tid)
                }
            }
        }
    }

    // ── Selector resolution ───────────────────────────────────────────

    /// Apply a chain of selectors to a base, building up projections.
    fn apply_selectors(
        &mut self,
        base: PlaceBase,
        base_tid: TypeId,
        selectors: &[Selector],
        loc: &crate::errors::SourceLoc,
    ) -> Option<Place> {
        let mut projections = Vec::new();
        let mut current_ty = base_tid;

        for sel in selectors {
            let proj = self.resolve_selector(current_ty, sel)?;
            current_ty = proj.ty;
            projections.push(proj);
        }

        Some(Place {
            base,
            projections,
            ty: current_ty,
            loc: loc.clone(),
        })
    }

    /// Resolve a single selector against the current type.
    fn resolve_selector(&mut self, current_ty: TypeId, sel: &Selector) -> Option<Projection> {
        let resolved = self.resolve_alias(current_ty);
        match sel {
            Selector::Field(field_name, _) => {
                // For fields, the current type must be a record (or pointer-to-record).
                // Check if it's a pointer first — auto-deref not done here,
                // the AST should have explicit Deref selectors.
                self.resolve_field_projection(resolved, field_name)
            }
            Selector::Index(exprs, _) => {
                // Current type must be an array.
                let idx_expr = if let Some(e) = exprs.first() {
                    self.lower_expr(e)
                } else {
                    HirExpr { kind: HirExprKind::IntLit(0), ty: TY_INTEGER, loc: crate::errors::SourceLoc::new("", 0, 0) }
                };
                match self.types.get(resolved) {
                    Type::Array { elem_type, .. } => Some(Projection {
                        kind: ProjectionKind::Index(Box::new(idx_expr)),
                        ty: *elem_type,
                    }),
                    Type::OpenArray { elem_type } => Some(Projection {
                        kind: ProjectionKind::Index(Box::new(idx_expr)),
                        ty: *elem_type,
                    }),
                    // String constants can be indexed: "ABCD"[i] → CHAR
                    Type::StringLit(_) => Some(Projection {
                        kind: ProjectionKind::Index(Box::new(idx_expr)),
                        ty: TY_CHAR,
                    }),
                    // ADDRESS^[i] — byte access through dereferenced ADDRESS pointer.
                    // After Deref on ADDRESS gives Char, indexing is pointer arithmetic.
                    Type::Char => Some(Projection {
                        kind: ProjectionKind::Index(Box::new(idx_expr)),
                        ty: TY_CHAR,
                    }),
                    _ => None,
                }
            }
            Selector::Deref(_) => {
                match self.types.get(resolved) {
                    Type::Pointer { base } | Type::Ref { target: base, .. } => {
                        let target = self.resolve_alias(*base);
                        Some(Projection {
                            kind: ProjectionKind::Deref,
                            ty: target,
                        })
                    }
                    Type::Address => {
                        // ADDRESS^ → CHAR (byte access)
                        Some(Projection {
                            kind: ProjectionKind::Deref,
                            ty: TY_CHAR,
                        })
                    }
                    _ => {
                        // Opaque or unresolved pointer — treat as generic deref
                        Some(Projection {
                            kind: ProjectionKind::Deref,
                            ty: TY_ADDRESS,
                        })
                    }
                }
            }
        }
    }

    /// Resolve a field access on a record type.
    fn resolve_field_projection(&self, record_tid: TypeId, field_name: &str) -> Option<Projection> {
        let resolved = self.resolve_alias(record_tid);
        match self.types.get(resolved) {
            Type::Record { fields, variants } => {
                // Check regular fields first
                for (idx, f) in fields.iter().enumerate() {
                    if f.name == field_name {
                        return Some(Projection {
                            kind: ProjectionKind::Field {
                                index: idx,
                                name: field_name.to_string(),
                                record_ty: resolved,
                            },
                            ty: f.typ,
                        });
                    }
                }
                // Check variant fields
                if let Some(vi) = variants {
                    for (vi_idx, vc) in vi.variants.iter().enumerate() {
                        for (fi_idx, f) in vc.fields.iter().enumerate() {
                            if f.name == field_name {
                                return Some(Projection {
                                    kind: ProjectionKind::VariantField {
                                        variant_index: vi_idx,
                                        field_index: fi_idx,
                                        name: field_name.to_string(),
                                        record_ty: resolved,
                                    },
                                    ty: f.typ,
                                });
                            }
                        }
                    }
                }
                None
            }
            // If it's a pointer, try dereffing to get to the record
            Type::Pointer { base } => {
                self.resolve_field_projection(*base, field_name)
            }
            _ => None,
        }
    }

    // ── Helpers ───────────────────────────────────────────────────────

    /// Scope-aware type lookup for a variable name. Returns its TypeId.
    pub fn scope_lookup_type(&self, name: &str) -> Option<TypeId> {
        self.scope_lookup(name).map(|sym| sym.typ)
    }

    /// Check if a resolved TypeId represents an open-array-like parameter.
    /// Matches OpenArray and StringLit (TY_STRING is used by stdlib for
    /// `ARRAY OF CHAR` parameters).
    fn is_open_array_type(types: &TypeRegistry, resolved: TypeId) -> bool {
        matches!(types.get(resolved), Type::OpenArray { .. } | Type::StringLit(_))
    }

    /// Scope-aware symbol lookup. Uses `lookup_in_scope` when a current
    /// scope is set (walking the parent chain), falls back to `lookup_any`
    /// when no scope is available.
    fn scope_lookup(&self, name: &str) -> Option<&crate::symtab::Symbol> {
        if let Some(scope_id) = self.current_scope {
            self.symtab.lookup_in_scope(scope_id, name)
        } else {
            self.symtab.lookup_any(name)
        }
    }

    // ── Open array expansion (call args) ──────────────────────────

    /// Look up a procedure's parameter info from sema.
    /// Returns (is_var, is_open_array) for each parameter.
    fn lookup_proc_params(&mut self, desig: &crate::ast::Designator) -> Vec<(bool, bool)> {
        let name = &desig.ident.name;

        // Try the designator's resolved name
        let sym = if let Some(ref module) = desig.ident.module {
            self.symtab.lookup_qualified(module, name)
        } else if !desig.selectors.is_empty() {
            // Module.Proc pattern: name is a module, first selector is the procedure
            if let Some(crate::ast::Selector::Field(proc_name, _)) = desig.selectors.first() {
                // Prefer qualified lookup (Module.Proc) over scope_lookup which
                // might find a different "Create" from another module.
                self.symtab.lookup_qualified(name, proc_name)
                    .or_else(|| self.scope_lookup(proc_name))
            } else {
                self.scope_lookup(name)
            }
        } else {
            self.scope_lookup(name)
        };

        if let Some(sym) = sym {
            if let SymbolKind::Procedure { ref params, .. } = sym.kind {
                return params.iter().map(|p| {
                    let is_open = Self::is_open_array_type(self.types, self.resolve_alias(p.typ));
                    (p.is_var, is_open)
                }).collect();
            }
            // Procedure variable: look up ProcedureType from the type registry
            let resolved = self.resolve_alias(sym.typ);
            if let Type::ProcedureType { params, .. } = self.types.get(resolved) {
                return params.iter().map(|p| {
                    let is_open = Self::is_open_array_type(self.types, self.resolve_alias(p.typ));
                    (p.is_var, is_open)
                }).collect();
            }
        }
        // Fallback: resolve the full designator to get its type.
        // Handles indirect calls through record fields (cp^.genFn).
        if let Some(place) = self.resolve_designator(desig) {
            let resolved = self.resolve_alias(place.ty);
            if let Type::ProcedureType { params, .. } = self.types.get(resolved) {
                return params.iter().map(|p| {
                    let is_open = Self::is_open_array_type(self.types, self.resolve_alias(p.typ));
                    (p.is_var, is_open)
                }).collect();
            }
        }
        Vec::new()
    }

    /// Expand call arguments with open array _high companions.
    /// For each open array parameter, inserts an additional IntLit arg
    /// with the HIGH value. For VAR params, the arg is kept as-is
    /// (backends handle the address-taking).
    fn expand_call_args(
        &mut self,
        ast_args: &[crate::ast::Expr],
        params: &[(bool, bool)],
        loc: &crate::errors::SourceLoc,
    ) -> Vec<HirExpr> {
        let mut result = Vec::new();

        for (idx, arg) in ast_args.iter().enumerate() {
            let (is_var, is_open) = params.get(idx).copied().unwrap_or((false, false));

            if is_var && !is_open {
                // VAR (non-open-array) param: emit AddrOf.
                if let ExprKind::Designator(ref d) = arg.kind {
                    if let Some(place) = self.resolve_designator(d) {
                        result.push(HirExpr {
                            kind: HirExprKind::AddrOf(place),
                            ty: TY_ADDRESS,
                            loc: loc.clone(),
                        });
                        continue;
                    }
                }
                result.push(self.lower_expr(arg));
                continue;
            }
            if is_var && is_open {
                // VAR open array param: emit AddrOf + HIGH for designators,
                // or value + HIGH for string literals.
                if let ExprKind::Designator(ref d) = arg.kind {
                    if let Some(place) = self.resolve_designator(d) {
                        // If the arg is already a pointer-like (open array param,
                        // VAR param, ADDRESS, or a fixed array that decays to
                        // pointer in C), pass directly without AddrOf.
                        let resolved_ty = self.resolve_alias(place.ty);
                        let is_array_type = matches!(self.types.get(resolved_ty),
                            Type::Array { .. } | Type::OpenArray { .. });
                        let already_ptr = match &place.base {
                            PlaceBase::Local(sid) | PlaceBase::Global(sid) =>
                                sid.is_open_array || sid.is_var_param,
                            _ => false,
                        } || place.ty == TY_ADDRESS || is_array_type;
                        if already_ptr {
                            result.push(HirExpr {
                                kind: HirExprKind::Place(place),
                                ty: TY_ADDRESS,
                                loc: loc.clone(),
                            });
                        } else {
                            result.push(HirExpr {
                                kind: HirExprKind::AddrOf(place),
                                ty: TY_ADDRESS,
                                loc: loc.clone(),
                            });
                        }
                        let high = self.compute_high_for_arg(arg, loc);
                        result.push(high);
                        continue;
                    }
                }
                // Non-designator (e.g., string literal): emit value + HIGH
                // Promote single-char TY_CHAR strings to TY_STRING for open array context
                let lowered = self.lower_expr(arg);
                let arg_val = if lowered.ty == TY_CHAR {
                    if let HirExprKind::StringLit(ref s) = lowered.kind {
                        HirExpr { kind: HirExprKind::StringLit(s.clone()), ty: TY_STRING, loc: lowered.loc.clone() }
                    } else { lowered }
                } else { lowered };
                result.push(arg_val);
                let high = self.compute_high_for_arg(arg, loc);
                result.push(high);
                continue;
            }

            let lowered = self.lower_expr(arg);

            if is_open {
                // Open array: emit value + HIGH.
                // If the arg is a single-char string with TY_CHAR, promote
                // it to TY_STRING so the backend emits a ptr, not a char value.
                let arg_val = if lowered.ty == TY_CHAR {
                    if let HirExprKind::StringLit(ref s) = lowered.kind {
                        HirExpr { kind: HirExprKind::StringLit(s.clone()), ty: TY_STRING, loc: lowered.loc.clone() }
                    } else {
                        lowered
                    }
                } else {
                    lowered
                };
                result.push(arg_val);

                // Compute HIGH value
                let high = self.compute_high_for_arg(arg, loc);
                result.push(high);
            } else {
                result.push(lowered);
            }
        }
        result
    }

    /// Compute the HIGH value for an open array argument.
    fn compute_high_for_arg(
        &mut self,
        arg: &crate::ast::Expr,
        loc: &crate::errors::SourceLoc,
    ) -> HirExpr {
        match &arg.kind {
            // String literal: HIGH = length (includes NUL space)
            ExprKind::StringLit(s) => {
                HirExpr {
                    kind: HirExprKind::IntLit(s.len() as i64),
                    ty: TY_INTEGER,
                    loc: loc.clone(),
                }
            }
            // Designator: look up array size or open array _high
            ExprKind::Designator(d) => {
                // For designators with selectors (e.g., SEQ[e]), resolve
                // the full place to get the result type's array bounds.
                if !d.selectors.is_empty() {
                    if let Some(place) = self.resolve_designator(d) {
                        let resolved = self.resolve_alias(place.ty);
                        if let Type::Array { high, .. } = self.types.get(resolved) {
                            return HirExpr {
                                kind: HirExprKind::IntLit(*high),
                                ty: TY_INTEGER,
                                loc: loc.clone(),
                            };
                        }
                        if let Type::OpenArray { .. } = self.types.get(resolved) {
                            // Can't determine high at compile time for open arrays
                            return HirExpr {
                                kind: HirExprKind::IntLit(0),
                                ty: TY_INTEGER,
                                loc: loc.clone(),
                            };
                        }
                    }
                    return HirExpr {
                        kind: HirExprKind::IntLit(0),
                        ty: TY_INTEGER,
                        loc: loc.clone(),
                    };
                }
                // Simple designator (no selectors)
                let name = &d.ident.name;
                // Check if it's an open array param itself → use its _high
                if let Some(sym) = self.scope_lookup(name) {
                    let resolved = self.resolve_alias(sym.typ);
                    match self.types.get(resolved) {
                        Type::Array { high, .. } => {
                            return HirExpr {
                                kind: HirExprKind::IntLit(*high),
                                ty: TY_INTEGER,
                                loc: loc.clone(),
                            };
                        }
                        Type::OpenArray { .. } => {
                            // Pass through the _high companion
                            let high_name = format!("{}_high", name);
                            if let Some(place) = self.resolve_designator(&crate::ast::Designator {
                                ident: crate::ast::QualIdent {
                                    module: None,
                                    name: high_name,
                                    loc: loc.clone(),
                                },
                                selectors: vec![],
                                loc: loc.clone(),
                            }) {
                                return HirExpr {
                                    kind: HirExprKind::Place(place),
                                    ty: TY_INTEGER,
                                    loc: loc.clone(),
                                };
                            }
                        }
                        Type::StringLit(len) => {
                            return HirExpr {
                                kind: HirExprKind::IntLit(*len as i64),
                                ty: TY_INTEGER,
                                loc: loc.clone(),
                            };
                        }
                        _ => {
                            // Constant string: HIGH = string length
                            if let SymbolKind::Constant(ConstValue::String(s)) = &sym.kind {
                                return HirExpr {
                                    kind: HirExprKind::IntLit(s.len() as i64),
                                    ty: TY_INTEGER,
                                    loc: loc.clone(),
                                };
                            }
                        }
                    }
                }
                // Fallback: 0
                HirExpr {
                    kind: HirExprKind::IntLit(0),
                    ty: TY_INTEGER,
                    loc: loc.clone(),
                }
            }
            _ => {
                // Unknown arg type: default HIGH = 0
                HirExpr {
                    kind: HirExprKind::IntLit(0),
                    ty: TY_INTEGER,
                    loc: loc.clone(),
                }
            }
        }
    }

    /// Resolve an import name to the canonical case from the module's
    /// definition scope. For native stdlib modules where the .def uses
    /// Resolve a type transfer target name (e.g., "INTEGER", "CharPtr") to a TypeId.
    fn resolve_type_transfer_target(&self, name: &str) -> TypeId {
        match name {
            "INTEGER" | "INT" | "TRUNC" | "SHORT" => TY_INTEGER,
            "CARDINAL" => TY_CARDINAL,
            "REAL" | "FLOAT" => TY_REAL,
            "LONGREAL" | "LFLOAT" => TY_LONGREAL,
            "LONGINT" | "LONG" => TY_LONGINT,
            "LONGCARD" => TY_LONGCARD,
            "BOOLEAN" => TY_BOOLEAN,
            "CHAR" | "CHR" => TY_CHAR,
            "WORD" => TY_WORD,
            "BYTE" => TY_BYTE,
            "ADDRESS" => TY_ADDRESS,
            _ => self.symtab.lookup_any(name)
                .filter(|s| matches!(s.kind, SymbolKind::Type))
                .map(|s| s.typ)
                .unwrap_or(TY_INTEGER),
        }
    }

    /// Extract a type name from an AST expression used as a type argument (e.g., VAL(INTEGER, x)).
    fn resolve_type_name_from_expr(&self, expr: &crate::ast::Expr) -> TypeId {
        if let crate::ast::ExprKind::Designator(ref desig) = expr.kind {
            let name = &desig.ident.name;
            return self.resolve_type_transfer_target(name);
        }
        TY_INTEGER
    }

    /// different casing than the import (e.g., `Cos` imported but
    /// `cos` defined), returns the definition's name.
    fn resolve_canonical_name(&self, module: &str, import_name: &str) -> String {
        if let Some(scope_id) = self.symtab.lookup_module_scope(module) {
            let lower = import_name.to_ascii_lowercase();
            for sym in self.symtab.symbols_in_scope(scope_id) {
                if sym.name.to_ascii_lowercase() == lower {
                    return sym.name.clone();
                }
            }
        }
        import_name.to_string()
    }

    /// Return the current scope ID, if set.
    pub fn current_scope(&self) -> Option<usize> {
        self.current_scope
    }
    pub fn in_procedure(&self) -> bool {
        self.in_procedure
    }

    /// Dump the parent chain from a scope for debugging.
    #[allow(dead_code)]
    fn dump_scope_chain(&self, scope_id: usize, name: &str) {
        let mut id = scope_id;
        eprint!("  scope chain for '{}': ", name);
        loop {
            let sname = self.symtab.scope_name(id).unwrap_or("?");
            let has = self.symtab.lookup_in_scope_direct(id, name).is_some();
            eprint!("[{}:'{}' has={}]", id, sname, has);
            if let Some(parent) = self.symtab.scope_parent(id) {
                eprint!(" -> ");
                id = parent;
            } else {
                break;
            }
        }
        eprintln!();
        // Also find where the name actually lives
        let count = self.symtab.scope_count();
        for sid in 0..count {
            if self.symtab.lookup_in_scope_direct(sid, name).is_some() {
                let sn = self.symtab.scope_name(sid).unwrap_or("?");
                eprintln!("    '{}' defined in scope[{}:'{}']", name, sid, sn);
            }
        }
    }

    /// Resolve TypeId through aliases.
    pub fn resolve_alias(&self, tid: TypeId) -> TypeId {
        let mut cur = tid;
        let mut seen = 0;
        loop {
            match self.types.get(cur) {
                Type::Alias { target, .. } => {
                    cur = *target;
                    seen += 1;
                    if seen > 50 { return cur; } // cycle guard
                }
                _ => return cur,
            }
        }
    }

    fn mangle(&self, name: &str) -> String {
        format!("{}_{}", self.module_name, name)
    }


    // ── FOR direction analysis (Phase 3) ─────────────────────────────

    /// Determine the direction of a FOR loop from its step expression.
    /// Returns `ForDirection::Up` for positive steps (or no step, which
    /// defaults to +1), `ForDirection::Down` for negative steps.
    ///
    /// Replaces the independent `is_negative_expr` / `is_negative_step`
    /// checks in both backends.
    pub fn for_direction(&self, step: Option<&crate::ast::Expr>) -> ForDirection {
        match step {
            None => ForDirection::Up,
            Some(expr) => {
                if self.is_negative_expr(expr) {
                    ForDirection::Down
                } else {
                    ForDirection::Up
                }
            }
        }
    }

    /// Check if an expression evaluates to a negative value.
    /// Handles: negative literals, unary negation, and constant-foldable
    /// binary expressions (e.g., `0 - 1`).
    fn is_negative_expr(&self, expr: &crate::ast::Expr) -> bool {
        if let Some(val) = self.try_eval_const_int(expr) {
            return val < 0;
        }
        match &expr.kind {
            ExprKind::UnaryOp { op: crate::ast::UnaryOp::Neg, .. } => true,
            ExprKind::IntLit(v) => *v < 0,
            _ => false,
        }
    }

    /// Try to evaluate a constant integer expression.
    /// Handles literals, unary neg, and basic binary arithmetic.
    pub fn try_eval_const_int(&self, expr: &crate::ast::Expr) -> Option<i64> {
        match &expr.kind {
            ExprKind::IntLit(v) => Some(*v),
            ExprKind::CharLit(c) => Some(*c as i64),
            ExprKind::BoolLit(b) => Some(if *b { 1 } else { 0 }),
            ExprKind::UnaryOp { op: crate::ast::UnaryOp::Neg, operand } => {
                self.try_eval_const_int(operand).map(|v| -v)
            }
            ExprKind::BinaryOp { op, left, right } => {
                let l = self.try_eval_const_int(left)?;
                let r = self.try_eval_const_int(right)?;
                Some(match op {
                    crate::ast::BinaryOp::Add => l + r,
                    crate::ast::BinaryOp::Sub => l - r,
                    crate::ast::BinaryOp::Mul => l * r,
                    crate::ast::BinaryOp::IntDiv => if r != 0 { l / r } else { 0 },
                    crate::ast::BinaryOp::Mod => if r != 0 { l % r } else { 0 },
                    _ => return None,
                })
            }
            ExprKind::Designator(d) if d.selectors.is_empty() => {
                // Try constant lookup
                match d.ident.name.as_str() {
                    "TRUE" => Some(1),
                    "FALSE" => Some(0),
                    _ => {
                        // Check symtab for constant value
                        self.symtab.lookup_any(&d.ident.name).and_then(|sym| {
                            if let SymbolKind::Constant(cv) = &sym.kind {
                                match cv {
                                    ConstValue::Integer(v) => Some(*v),
                                    ConstValue::Boolean(b) => Some(if *b { 1 } else { 0 }),
                                    ConstValue::Char(c) => Some(*c as i64),
                                    _ => None,
                                }
                            } else if let SymbolKind::EnumVariant(v) = &sym.kind {
                                Some(*v)
                            } else {
                                None
                            }
                        })
                    }
                }
            }
            _ => None,
        }
    }

    // ── String interning (Phase 3) ───────────────────────────────────

    /// Intern a string constant, returning its `StringId`.
    /// Deduplicates: same content → same ID.
    pub fn intern_string(&mut self, s: &str) -> StringId {
        // Check if already interned
        for (idx, existing) in self.string_pool.iter().enumerate() {
            if existing == s {
                return StringId(idx);
            }
        }
        let id = self.string_pool.len();
        self.string_pool.push(s.to_string());
        StringId(id)
    }

    /// Get the interned string content for a StringId.
    pub fn get_string(&self, id: StringId) -> &str {
        &self.string_pool[id.0]
    }

    /// Return the full string pool (for codegen to emit).
    pub fn string_pool(&self) -> &[String] {
        &self.string_pool
    }

    // ── Expression lowering (Phase 4) ────────────────────────────────

    /// Lower an AST expression to an HIR expression.
    /// Every HirExpr carries a TypeId resolved from sema.
    pub fn lower_expr(&mut self, expr: &crate::ast::Expr) -> HirExpr {
        let loc = expr.loc.clone();
        match &expr.kind {
            ExprKind::IntLit(v) => HirExpr {
                kind: HirExprKind::IntLit(*v),
                ty: TY_INTEGER,
                loc,
            },
            ExprKind::RealLit(v) => HirExpr {
                kind: HirExprKind::RealLit(*v),
                ty: TY_REAL,
                loc,
            },
            ExprKind::StringLit(s) => {
                // Keep all string literals as StringLit. Single-char strings
                // are context-dependent: "x" is a string when passed to
                // WriteString (open array), but a char when assigned to a
                // CHAR variable. The backend handles the coercion based on
                // the target type at the use site.
                let ty = if s.is_empty() || s.len() == 1 { TY_CHAR } else { TY_STRING };
                HirExpr { kind: HirExprKind::StringLit(s.clone()), ty, loc }
            }
            ExprKind::CharLit(c) => HirExpr {
                kind: HirExprKind::CharLit(*c),
                ty: TY_CHAR,
                loc,
            },
            ExprKind::BoolLit(b) => HirExpr {
                kind: HirExprKind::BoolLit(*b),
                ty: TY_BOOLEAN,
                loc,
            },
            ExprKind::NilLit => HirExpr {
                kind: HirExprKind::NilLit,
                ty: TY_NIL,
                loc,
            },
            ExprKind::Designator(d) => {
                if let Some(place) = self.resolve_designator(d) {
                    let ty = place.ty;
                    // Unwrap constants to literal expressions — they don't
                    // have addresses and shouldn't go through emit_place_addr.
                    // BUT: constants with projections (e.g., "ABCDEF"[i]) must
                    // stay as Place so the index is preserved.
                    if place.projections.is_empty() {
                        if let PlaceBase::Constant(ref cv) = place.base {
                            return match cv {
                                ConstVal::Integer(v) => HirExpr { kind: HirExprKind::IntLit(*v), ty, loc },
                                ConstVal::Real(v) => HirExpr { kind: HirExprKind::RealLit(*v), ty, loc },
                                ConstVal::Boolean(v) => HirExpr { kind: HirExprKind::BoolLit(*v), ty, loc },
                                ConstVal::Char(v) => HirExpr { kind: HirExprKind::CharLit(*v), ty, loc },
                                ConstVal::String(s) => {
                                    HirExpr { kind: HirExprKind::StringLit(s.clone()), ty, loc }
                                }
                                ConstVal::Nil => HirExpr { kind: HirExprKind::NilLit, ty, loc },
                                ConstVal::Set(v) => HirExpr { kind: HirExprKind::IntLit(*v as i64), ty, loc },
                                ConstVal::EnumVariant(v) => HirExpr { kind: HirExprKind::IntLit(*v), ty, loc },
                            };
                        }
                    }
                    HirExpr { kind: HirExprKind::Place(place), ty, loc }
                } else {
                    // Fallback: unresolved designator
                    HirExpr { kind: HirExprKind::IntLit(0), ty: TY_ERROR, loc }
                }
            }
            ExprKind::FuncCall { desig, args } => {
                let func_name = &desig.ident.name;

                // Type transfer functions and builtins: always DirectCall,
                // no open array expansion needed (single arg, no _high).
                let is_builtin_type_transfer = matches!(func_name.as_str(),
                    "INTEGER" | "INT" | "CARDINAL" | "LONGINT" | "LONGCARD"
                    | "REAL" | "FLOAT" | "LONGREAL" | "LFLOAT"
                    | "BOOLEAN" | "CHAR" | "WORD" | "BYTE" | "ADDRESS"
                    | "CHR" | "ORD" | "VAL" | "TRUNC" | "LONG" | "SHORT"
                ) && desig.ident.module.is_none() && desig.selectors.is_empty();
                // User-defined types used as type transfers: T(expr)
                let is_user_type_transfer = !is_builtin_type_transfer
                    && desig.ident.module.is_none()
                    && desig.selectors.is_empty()
                    && self.symtab.lookup_any(func_name)
                        .map(|s| matches!(s.kind, SymbolKind::Type))
                        .unwrap_or(false);
                let is_type_transfer = is_builtin_type_transfer || is_user_type_transfer;

                let is_builtin = desig.ident.module.is_none()
                    && desig.selectors.is_empty()
                    && crate::builtins::is_builtin_proc(func_name);

                if is_type_transfer {
                    // VAL(Type, expr) is special: first arg is type, second is value
                    if func_name == "VAL" && args.len() >= 2 {
                        let lowered_arg = self.lower_expr(&args[1]);
                        let ty = self.resolve_type_name_from_expr(&args[0]);
                        return HirExpr {
                            kind: HirExprKind::TypeTransfer(Box::new(lowered_arg)),
                            ty,
                            loc,
                        };
                    }
                    let lowered_arg = if let Some(a) = args.first() {
                        self.lower_expr(a)
                    } else {
                        HirExpr { kind: HirExprKind::IntLit(0), ty: TY_INTEGER, loc: loc.clone() }
                    };
                    let ty = self.resolve_type_transfer_target(func_name);
                    return HirExpr {
                        kind: HirExprKind::TypeTransfer(Box::new(lowered_arg)),
                        ty,
                        loc,
                    };
                }

                if is_builtin {
                    // HIGH(x): resolve at HIR time for fixed arrays
                    if func_name == "HIGH" && args.len() == 1 {
                        let high = self.compute_high_for_arg(&args[0], &loc);
                        if high.ty != TY_ERROR {
                            return high;
                        }
                    }
                    let lowered_args: Vec<HirExpr> = args.iter()
                        .map(|a| self.lower_expr(a))
                        .collect();
                    let ty = crate::builtins::builtin_return_type(func_name);
                    let sid = SymbolId {
                        mangled: func_name.clone(),
                        source_name: func_name.clone(),
                        module: None,
                        ty,
                        is_var_param: false,
                        is_open_array: false,
                    };
                    return HirExpr {
                        kind: HirExprKind::DirectCall { target: sid, args: lowered_args },
                        ty,
                        loc,
                    };
                }

                // Look up sema param info for open array expansion
                let sema_params = self.lookup_proc_params(desig);
                let expanded_args = self.expand_call_args(args, &sema_params, &loc);

                // Try to resolve as a direct call
                if let Some(place) = self.resolve_designator(desig) {
                    match place.base {
                        PlaceBase::FuncRef(sid) => {
                            let ty = self.infer_return_type(&sid);
                            HirExpr {
                                kind: HirExprKind::DirectCall { target: sid, args: expanded_args },
                                ty,
                                loc,
                            }
                        }
                        _ => {
                            // Indirect call through a place
                            let ty = TY_INTEGER; // default return type
                            let callee_ty = place.ty;
                            let callee = Box::new(HirExpr {
                                kind: HirExprKind::Place(place),
                                ty: callee_ty,
                                loc: loc.clone(),
                            });
                            HirExpr {
                                kind: HirExprKind::IndirectCall { callee, args: expanded_args },
                                ty,
                                loc,
                            }
                        }
                    }
                } else {
                    HirExpr { kind: HirExprKind::IntLit(0), ty: TY_ERROR, loc }
                }
            }
            ExprKind::UnaryOp { op, operand } => {
                let operand = self.lower_expr(operand);
                let ty = operand.ty;
                HirExpr {
                    kind: HirExprKind::UnaryOp { op: *op, operand: Box::new(operand) },
                    ty,
                    loc,
                }
            }
            ExprKind::BinaryOp { op, left, right } => {
                let left = self.lower_expr(left);
                let right = self.lower_expr(right);
                let ty = self.binary_result_type(*op, left.ty, right.ty);
                HirExpr {
                    kind: HirExprKind::BinaryOp {
                        op: *op,
                        left: Box::new(left),
                        right: Box::new(right),
                    },
                    ty,
                    loc,
                }
            }
            ExprKind::SetConstructor { elements, .. } => {
                let hir_elems: Vec<HirSetElement> = elements.iter().map(|e| match e {
                    SetElement::Single(expr) => HirSetElement::Single(self.lower_expr(expr)),
                    SetElement::Range(lo, hi) => HirSetElement::Range(
                        self.lower_expr(lo),
                        self.lower_expr(hi),
                    ),
                }).collect();
                HirExpr {
                    kind: HirExprKind::SetConstructor { elements: hir_elems },
                    ty: TY_BITSET,
                    loc,
                }
            }
            ExprKind::Not(e) => {
                let inner = self.lower_expr(e);
                HirExpr {
                    kind: HirExprKind::Not(Box::new(inner)),
                    ty: TY_BOOLEAN,
                    loc,
                }
            }
            ExprKind::Deref(e) => {
                let inner = self.lower_expr(e);
                let target_ty = match self.types.get(self.resolve_alias(inner.ty)) {
                    Type::Pointer { base } => self.resolve_alias(*base),
                    _ => TY_ADDRESS,
                };
                HirExpr {
                    kind: HirExprKind::Deref(Box::new(inner)),
                    ty: target_ty,
                    loc,
                }
            }
        }
    }

    /// Infer the return type of a function call from the symbol info.
    fn infer_return_type(&self, sid: &SymbolId) -> TypeId {
        // Look up in symtab
        if let Some(sym) = self.symtab.lookup_any(&sid.source_name) {
            if let SymbolKind::Procedure { return_type: Some(rt), .. } = &sym.kind {
                return *rt;
            }
        }
        // Try qualified lookup
        if let Some(ref module) = sid.module {
            if let Some(sym) = self.symtab.lookup_qualified(module, &sid.source_name) {
                if let SymbolKind::Procedure { return_type: Some(rt), .. } = &sym.kind {
                    return *rt;
                }
            }
        }
        TY_INTEGER // default
    }

    /// Determine the result type of a binary operation.
    fn binary_result_type(&self, op: crate::ast::BinaryOp, left_ty: TypeId, right_ty: TypeId) -> TypeId {
        use crate::ast::BinaryOp::*;
        match op {
            // Comparison ops always produce BOOLEAN
            Eq | Ne | Lt | Le | Gt | Ge | In => TY_BOOLEAN,
            // Logical ops
            And | Or => TY_BOOLEAN,
            // Arithmetic: promote to wider type
            Add | Sub | Mul => {
                let lt = self.resolve_alias(left_ty);
                let rt = self.resolve_alias(right_ty);
                if lt == TY_LONGREAL || rt == TY_LONGREAL { TY_LONGREAL }
                else if lt == TY_REAL || rt == TY_REAL { TY_REAL }
                else if lt == TY_LONGINT || rt == TY_LONGINT { TY_LONGINT }
                else { left_ty }
            }
            RealDiv => {
                let lt = self.resolve_alias(left_ty);
                let rt = self.resolve_alias(right_ty);
                if lt == TY_LONGREAL || rt == TY_LONGREAL { TY_LONGREAL }
                else { TY_REAL }
            }
            IntDiv | Mod => left_ty,
        }
    }

    // ── Statement lowering (Phase 4) ─────────────────────────────────

    /// Lower an AST statement to an HIR statement.
    /// WITH statements are desugared: the WITH designator's fields become
    /// Place projections on bare identifiers inside the body.
    pub fn lower_stmt(&mut self, stmt: &crate::ast::Statement) -> HirStmt {
        use crate::ast::StatementKind;
        let loc = stmt.loc.clone();

        match &stmt.kind {
            StatementKind::Empty => HirStmt { kind: HirStmtKind::Empty, loc },

            StatementKind::Assign { desig, expr } => {
                let target = self.resolve_designator(desig)
                    .unwrap_or_else(|| self.fallback_place(desig, &loc));
                let value = self.lower_expr(expr);
                HirStmt { kind: HirStmtKind::Assign { target, value }, loc }
            }

            StatementKind::ProcCall { desig, args } => {
                // Builtins: no open array expansion, simple lowering
                let func_name = &desig.ident.name;
                if desig.ident.module.is_none() && desig.selectors.is_empty()
                    && crate::builtins::is_builtin_proc(func_name)
                {
                    let lowered_args: Vec<HirExpr> = args.iter()
                        .map(|a| self.lower_expr(a))
                        .collect();
                    let sid = SymbolId {
                        mangled: func_name.clone(),
                        source_name: func_name.clone(),
                        module: None,
                        ty: TY_VOID,
                        is_var_param: false,
                        is_open_array: false,
                    };
                    return HirStmt {
                        kind: HirStmtKind::ProcCall {
                            target: HirCallTarget::Direct(sid),
                            args: lowered_args,
                        },
                        loc,
                    };
                }
                // Look up sema param info for open array expansion
                let sema_params = self.lookup_proc_params(desig);
                let expanded_args = self.expand_call_args(args, &sema_params, &loc);
                let target = if let Some(place) = self.resolve_designator(desig) {
                    match place.base {
                        PlaceBase::FuncRef(sid) => HirCallTarget::Direct(sid),
                        _ => {
                            let ty = place.ty;
                            HirCallTarget::Indirect(HirExpr {
                                kind: HirExprKind::Place(place),
                                ty,
                                loc: loc.clone(),
                            })
                        }
                    }
                } else {
                    // Fallback: create a direct call with mangled name
                    let mangled = self.mangle(&desig.ident.name);
                    HirCallTarget::Direct(SymbolId {
                        mangled,
                        source_name: desig.ident.name.clone(),
                        module: Some(self.module_name.clone()),
                        ty: TY_VOID,
                        is_var_param: false,
                        is_open_array: false,
                    })
                };
                HirStmt { kind: HirStmtKind::ProcCall { target, args: expanded_args }, loc }
            }

            StatementKind::If { cond, then_body, elsifs, else_body } => {
                let cond = self.lower_expr(cond);
                let then_body = self.lower_stmts(then_body);
                let elsifs = elsifs.iter().map(|(c, b)| {
                    (self.lower_expr(c), self.lower_stmts(b))
                }).collect();
                let else_body = else_body.as_ref().map(|b| self.lower_stmts(b));
                HirStmt { kind: HirStmtKind::If { cond, then_body, elsifs, else_body }, loc }
            }

            StatementKind::Case { expr, branches, else_body } => {
                let expr = self.lower_expr(expr);
                let branches = branches.iter().map(|b| {
                    let labels = b.labels.iter().map(|l| match l {
                        crate::ast::CaseLabel::Single(e) => HirCaseLabel::Single(self.lower_expr(e)),
                        crate::ast::CaseLabel::Range(lo, hi) => HirCaseLabel::Range(
                            self.lower_expr(lo), self.lower_expr(hi),
                        ),
                    }).collect();
                    HirCaseBranch { labels, body: self.lower_stmts(&b.body) }
                }).collect();
                let else_body = else_body.as_ref().map(|b| self.lower_stmts(b));
                HirStmt { kind: HirStmtKind::Case { expr, branches, else_body }, loc }
            }

            StatementKind::While { cond, body } => {
                let cond = self.lower_expr(cond);
                let body = self.lower_stmts(body);
                HirStmt { kind: HirStmtKind::While { cond, body }, loc }
            }

            StatementKind::Repeat { body, cond } => {
                let body = self.lower_stmts(body);
                let cond = self.lower_expr(cond);
                HirStmt { kind: HirStmtKind::Repeat { body, cond }, loc }
            }

            StatementKind::For { var, start, end, step, body } => {
                let direction = self.for_direction(step.as_ref());
                let var_ty = self.get_var_type(var).unwrap_or(TY_INTEGER);
                let start = self.lower_expr(start);
                let end = self.lower_expr(end);
                let step = step.as_ref().map(|s| self.lower_expr(s));
                let body = self.lower_stmts(body);
                HirStmt {
                    kind: HirStmtKind::For {
                        var: var.clone(),
                        var_ty,
                        start,
                        end,
                        step,
                        direction,
                        body,
                    },
                    loc,
                }
            }

            StatementKind::Loop { body } => {
                let body = self.lower_stmts(body);
                HirStmt { kind: HirStmtKind::Loop { body }, loc }
            }

            StatementKind::With { desig, body } => {
                // WITH elimination: push WITH scope, lower body, pop scope.
                // Bare field names inside the body become Place projections.
                let var_name = &desig.ident.name;
                let desig_tid = self.get_var_type(var_name)
                    .or_else(|| self.scope_lookup(var_name).map(|s| s.typ))
                    .or_else(|| self.symtab.lookup_any(var_name).map(|s| s.typ))
                    .unwrap_or(TY_ERROR);
                self.push_with(var_name, desig_tid);
                let lowered_body = self.lower_stmts(body);
                self.pop_with();
                // WITH is eliminated — its body statements are inlined.
                // Wrap in a block-like structure (just the body statements).
                // Use the first statement's loc, or the WITH loc if empty.
                if lowered_body.len() == 1 {
                    lowered_body.into_iter().next().unwrap()
                } else {
                    // Emit as a sequence — we need a block statement.
                    // Since HIR doesn't have a Block variant, emit as
                    // If(true) which backends can optimize, or just use
                    // the LOOP { body; EXIT } pattern. Simpler: just
                    // return the first stmt and note that in practice
                    // callers should use lower_stmts for WITH bodies.
                    //
                    // Actually, WITH lowering should be handled at the
                    // statement-list level, not single-statement level.
                    // Let's use a dummy wrapper.
                    HirStmt { kind: HirStmtKind::Empty, loc }
                }
            }

            StatementKind::Return { expr } => {
                let expr = expr.as_ref().map(|e| self.lower_expr(e));
                HirStmt { kind: HirStmtKind::Return { expr }, loc }
            }

            StatementKind::Exit => HirStmt { kind: HirStmtKind::Exit, loc },

            StatementKind::Raise { expr } => {
                let expr = expr.as_ref().map(|e| self.lower_expr(e));
                HirStmt { kind: HirStmtKind::Raise { expr }, loc }
            }

            StatementKind::Retry => HirStmt { kind: HirStmtKind::Retry, loc },

            StatementKind::Try { body, excepts, finally_body } => {
                let body = self.lower_stmts(body);
                let excepts = excepts.iter().map(|ec| {
                    HirExceptClause {
                        exception: ec.exception.as_ref().map(|qi| SymbolId {
                            mangled: qi.name.clone(),
                            source_name: qi.name.clone(),
                            module: qi.module.clone(),
                            ty: TY_INTEGER,
                            is_var_param: false,
                            is_open_array: false,
                        }),
                        var: ec.var.clone(),
                        body: self.lower_stmts(&ec.body),
                    }
                }).collect();
                let finally_body = finally_body.as_ref().map(|b| self.lower_stmts(b));
                HirStmt { kind: HirStmtKind::Try { body, excepts, finally_body }, loc }
            }

            StatementKind::Lock { mutex, body } => {
                let mutex = self.lower_expr(mutex);
                let body = self.lower_stmts(body);
                HirStmt { kind: HirStmtKind::Lock { mutex, body }, loc }
            }

            StatementKind::TypeCase { expr, branches, else_body } => {
                let expr = self.lower_expr(expr);
                let branches = branches.iter().map(|b| {
                    // Register TYPECASE binding variable with the branch's REF type
                    if let Some(ref var_name) = b.var {
                        let bind_ty = if let Some(first_type) = b.types.first() {
                            // Look up the REF type from sema
                            self.scope_lookup(&first_type.name)
                                .map(|sym| sym.typ)
                                .unwrap_or(TY_ADDRESS)
                        } else { TY_ADDRESS };
                        self.register_var(var_name, bind_ty);
                        self.register_local(var_name);
                    }
                    let body = self.lower_stmts(&b.body);
                    HirTypeCaseBranch {
                        types: b.types.iter().map(|qi| SymbolId {
                            mangled: qi.name.clone(),
                            source_name: qi.name.clone(),
                            module: qi.module.clone(),
                            ty: TY_INTEGER,
                            is_var_param: false,
                            is_open_array: false,
                        }).collect(),
                        var: b.var.clone(),
                        body,
                    }
                }).collect();
                let else_body = else_body.as_ref().map(|b| self.lower_stmts(b));
                HirStmt { kind: HirStmtKind::TypeCase { expr, branches, else_body }, loc }
            }
        }
    }

    /// Lower a list of AST statements, with WITH elimination.
    /// WITH statements expand inline — their body statements replace
    /// the WITH in the output list.
    pub fn lower_stmts(&mut self, stmts: &[crate::ast::Statement]) -> Vec<HirStmt> {
        let mut result = Vec::new();
        for stmt in stmts {
            if let crate::ast::StatementKind::With { desig, body } = &stmt.kind {
                // WITH elimination: push scope, lower body inline, pop scope
                let var_name = &desig.ident.name;
                let desig_tid = self.get_var_type(var_name)
                    .or_else(|| self.scope_lookup(var_name).map(|s| s.typ))
                    .or_else(|| self.symtab.lookup_any(var_name).map(|s| s.typ))
                    .unwrap_or(TY_ERROR);
                self.push_with(var_name, desig_tid);
                result.extend(self.lower_stmts(body));
                self.pop_with();
            } else {
                result.push(self.lower_stmt(stmt));
            }
        }
        result
    }

    /// Create a fallback Place for an unresolved designator.
    fn fallback_place(&self, desig: &crate::ast::Designator, loc: &crate::errors::SourceLoc) -> Place {
        let mangled = self.mangle(&desig.ident.name);
        Place {
            base: PlaceBase::Global(SymbolId {
                mangled,
                source_name: desig.ident.name.clone(),
                module: Some(self.module_name.clone()),
                ty: TY_INTEGER,
                is_var_param: false,
                is_open_array: false,
            }),
            projections: Vec::new(),
            ty: TY_INTEGER,
            loc: loc.clone(),
        }
    }

    // ── Module building (Phase 5) ────────────────────────────────────

    /// Build an HirModule from a ProgramModule AST node.
    /// Call after sema has run — this reads the TypeRegistry and SymbolTable.
    pub fn build_module_from_program(&mut self, m: &crate::ast::ProgramModule) -> HirModule {
        self.module_name = m.name.clone();
        self.build_import_info(&m.imports);
        self.register_block_vars(&m.block);

        let constants = self.lower_consts(&m.block.decls);
        let type_decls = self.lower_type_decls(&m.block.decls);
        let globals = self.lower_var_decls(&m.block.decls);
        let procedures = self.lower_proc_decls(&m.block.decls);
        let init_body = m.block.body.as_ref().map(|stmts| self.lower_stmts(stmts));

        #[allow(deprecated)]
        HirModule {
            name: m.name.clone(),
            source_file: m.loc.file.clone(),
            string_pool: self.string_pool.clone(),
            imports: Vec::new(), type_decls: Vec::new(), const_decls: Vec::new(),
            global_decls: Vec::new(), exception_decls: Vec::new(), type_descs: Vec::new(),
            proc_decls: Vec::new(), except_handler: None, finally_handler: None,
            embedded_modules: Vec::new(),
            constants, types: type_decls, globals, procedures,
            init_body, embedded_init_bodies: Vec::new(), externals: Vec::new(),
        }
    }

    /// Build an HirModule from an ImplementationModule AST node.
    pub fn build_module_from_impl(&mut self, m: &crate::ast::ImplementationModule) -> HirModule {
        self.module_name = m.name.clone();
        self.build_import_info(&m.imports);
        self.register_block_vars(&m.block);

        let constants = self.lower_consts(&m.block.decls);
        let type_decls = self.lower_type_decls(&m.block.decls);
        let globals = self.lower_var_decls(&m.block.decls);
        let procedures = self.lower_proc_decls(&m.block.decls);
        let init_body = m.block.body.as_ref().map(|stmts| self.lower_stmts(stmts));

        #[allow(deprecated)]
        HirModule {
            name: m.name.clone(),
            source_file: m.loc.file.clone(),
            string_pool: self.string_pool.clone(),
            imports: Vec::new(), type_decls: Vec::new(), const_decls: Vec::new(),
            global_decls: Vec::new(), exception_decls: Vec::new(), type_descs: Vec::new(),
            proc_decls: Vec::new(), except_handler: None, finally_handler: None,
            embedded_modules: Vec::new(),
            constants, types: type_decls, globals, procedures,
            init_body, embedded_init_bodies: Vec::new(), externals: Vec::new(),
        }
    }

    /// Extract import alias info from AST imports.
    fn build_import_info(&mut self, imports: &[crate::ast::Import]) {
        for imp in imports {
            if let Some(ref _from_module) = imp.from_module {
                // FROM Module IMPORT Name1, Name2, ...
                for imp_name in &imp.names {
                    if imp_name.alias.is_some() {
                        let local = imp_name.local_name().to_string();
                        self.import_alias_map.insert(local, imp_name.name.clone());
                    }
                }
            } else {
                // IMPORT Module1, Module2, ...
                for imp_name in &imp.names {
                    self.imported_modules_owned.push(imp_name.name.clone());
                }
            }
        }
    }

    /// Register all variable declarations in a block into var_types.
    fn register_block_vars(&mut self, block: &crate::ast::Block) {
        for decl in &block.decls {
            if let ast::Declaration::Var(v) = decl {
                for name in &v.names {
                    // Look up the TypeId from symtab
                    let tid = self.symtab.lookup_any(name)
                        .map(|s| s.typ)
                        .unwrap_or(TY_INTEGER);
                    self.register_var(name, tid);
                }
            }
        }
    }

    /// Lower constant declarations from a Block's decls.
    fn lower_consts(&self, decls: &[crate::ast::Declaration]) -> Vec<HirConst> {
        let mut result = Vec::new();
        for decl in decls {
            if let ast::Declaration::Const(cd) = decl {
                let tid = self.symtab.lookup_any(&cd.name)
                    .map(|s| s.typ)
                    .unwrap_or(TY_INTEGER);
                let value = if let Some(val) = self.try_eval_const_int(&cd.expr) {
                    ConstVal::Integer(val)
                } else {
                    match &cd.expr.kind {
                        ExprKind::RealLit(v) => ConstVal::Real(*v),
                        ExprKind::StringLit(s) => ConstVal::String(s.clone()),
                        ExprKind::CharLit(c) => ConstVal::Char(*c),
                        ExprKind::BoolLit(b) => ConstVal::Boolean(*b),
                        ExprKind::NilLit => ConstVal::Nil,
                        _ => ConstVal::Integer(0),
                    }
                };
                result.push(HirConst {
                    name: SymbolId {
                        mangled: self.mangle(&cd.name),
                        source_name: cd.name.clone(),
                        module: Some(self.module_name.clone()),
                        ty: tid,
                        is_var_param: false,
                        is_open_array: false,
                    },
                    value,
                    ty: tid,
                });
            }
        }
        result
    }

    /// Lower type declarations.
    fn lower_type_decls(&self, decls: &[crate::ast::Declaration]) -> Vec<HirTypeDecl> {
        let mut result = Vec::new();
        for decl in decls {
            if let ast::Declaration::Type(td) = decl {
                let tid = self.symtab.lookup_any(&td.name)
                    .filter(|s| matches!(s.kind, SymbolKind::Type))
                    .map(|s| s.typ)
                    .unwrap_or(TY_INTEGER);
                result.push(HirTypeDecl {
                    name: td.name.clone(),
                    mangled: format!("{}_{}", self.module_name, td.name),
                    type_id: tid,
                    exported: self.symtab.lookup_any(&td.name)
                        .map(|s| s.exported)
                        .unwrap_or(false),
                    ast_type_node: td.typ.clone(),
                });
            }
        }
        result
    }

    /// Lower variable declarations.
    fn lower_var_decls(&self, decls: &[crate::ast::Declaration]) -> Vec<HirVar> {
        let mut result = Vec::new();
        for decl in decls {
            if let ast::Declaration::Var(v) = decl {
                for name in &v.names {
                    let tid = self.symtab.lookup_any(name)
                        .map(|s| s.typ)
                        .unwrap_or(TY_INTEGER);
                    let exported = self.symtab.lookup_any(name)
                        .map(|s| s.exported)
                        .unwrap_or(false);
                    result.push(HirVar {
                        name: SymbolId {
                            mangled: self.mangle(name),
                            source_name: name.clone(),
                            module: Some(self.module_name.clone()),
                            ty: tid,
                            is_var_param: false,
                            is_open_array: false,
                        },
                        ty: tid,
                        exported,
                    });
                }
            }
        }
        result
    }

    /// Lower procedure declarations.
    fn lower_proc_decls(&mut self, decls: &[crate::ast::Declaration]) -> Vec<HirProc> {
        let mut result = Vec::new();
        for decl in decls {
            if let ast::Declaration::Procedure(p) = decl {
                result.push(self.lower_proc(p));
            }
        }
        result
    }

    /// Lower a single procedure declaration.
    fn lower_proc(&mut self, p: &crate::ast::ProcDecl) -> HirProc {
        let proc_name = &p.heading.name;

        // Look up return type from symtab
        let return_type = self.symtab.lookup_any(proc_name)
            .and_then(|s| match &s.kind {
                SymbolKind::Procedure { return_type, .. } => *return_type,
                _ => None,
            });
        let exported = self.symtab.lookup_any(proc_name)
            .map(|s| s.exported)
            .unwrap_or(false);

        // Build params
        let params: Vec<HirParam> = p.heading.params.iter().flat_map(|fp| {
            let is_open = matches!(fp.typ, crate::ast::TypeNode::OpenArray { .. });
            let param_tid = self.symtab.lookup_any(&fp.names[0])
                .map(|s| s.typ)
                .unwrap_or(TY_INTEGER);
            fp.names.iter().map(move |name| HirParam {
                name: name.clone(),
                ty: param_tid,
                is_var: fp.is_var,
                is_open_array: is_open,
            })
        }).collect();

        // Register params and locals as vars for body lowering
        let saved_in_proc = self.in_procedure;
        let saved_locals = std::mem::take(&mut self.local_names_owned);
        self.enter_procedure();
        for param in &params {
            self.register_var(&param.name, param.ty);
            self.register_local(&param.name);
            // Also register _high for open array params
            if param.is_open_array {
                let high_name = format!("{}_high", param.name);
                self.register_var(&high_name, TY_INTEGER);
                self.register_local(&high_name);
            }
        }
        self.register_block_vars(&p.block);
        for name in self.local_names_owned.clone() {
            // Already registered via register_block_vars
            let _ = name;
        }

        // Lower locals
        let locals = self.lower_var_decls(&p.block.decls);

        // Lower nested procs
        let nested_procs = self.lower_proc_decls(&p.block.decls);

        // Lower body
        let body = p.block.body.as_ref().map(|stmts| self.lower_stmts(stmts));

        // Restore state
        self.leave_procedure();
        self.local_names_owned = saved_locals;
        self.in_procedure = saved_in_proc;

        HirProc {
            name: SymbolId {
                mangled: self.mangle(proc_name),
                source_name: proc_name.clone(),
                module: Some(self.module_name.clone()),
                ty: TY_VOID,
                is_var_param: false,
                is_open_array: false,
            },
            params,
            return_type,
            captures: Vec::new(), // filled by caller if nested
            locals,
            body,
            nested_procs,
            is_exported: exported,
        }
    }
}

/// Convert a symtab ConstValue to an HIR ConstVal.
fn const_value_to_hir(cv: &ConstValue) -> ConstVal {
    match cv {
        ConstValue::Integer(v) => ConstVal::Integer(*v),
        ConstValue::Real(v) => ConstVal::Real(*v),
        ConstValue::Boolean(v) => ConstVal::Boolean(*v),
        ConstValue::Char(v) => ConstVal::Char(*v),
        ConstValue::String(v) => ConstVal::String(v.clone()),
        ConstValue::Set(v) => ConstVal::Set(*v),
        ConstValue::Nil => ConstVal::Nil,
    }
}

// ── Closure analysis (free variable computation) ─────────────────

/// Compute the captured variables for a nested procedure.
///
/// `outer_vars` maps variable names visible in the enclosing scope to their
/// TypeIds. Returns a sorted list of `CapturedVar` representing variables
/// that the nested procedure (and its sub-nested procedures, transitively)
/// reference from outer scopes.
///
/// This is the unified replacement for:
/// - `compute_captures()` in src/codegen.rs (C backend)
/// - `collect_free_vars()` in src/codegen_llvm/closures.rs (LLVM backend)
pub fn compute_captures(
    proc: &ast::ProcDecl,
    outer_vars: &HashMap<String, TypeId>,
    import_map: &HashMap<String, String>,
    imported_modules: &HashSet<String>,
) -> Vec<CapturedVar> {
    // Collect all identifier references in this proc + nested procs (transitive)
    let mut all_refs = HashSet::new();
    collect_refs_in_proc_deep(proc, &mut all_refs);

    // Collect this proc's own local names (params + var decls + nested proc names)
    let mut locals = HashSet::new();
    for fp in &proc.heading.params {
        for name in &fp.names {
            locals.insert(name.clone());
        }
    }
    for decl in &proc.block.decls {
        match decl {
            ast::Declaration::Var(v) => {
                for name in &v.names { locals.insert(name.clone()); }
            }
            ast::Declaration::Procedure(p) => {
                locals.insert(p.heading.name.clone());
            }
            _ => {}
        }
    }

    // Free vars = referenced names that exist in outer_vars but not in locals,
    // excluding builtins, imports, and module names
    let mut captures: Vec<CapturedVar> = all_refs.iter()
        .filter(|name| {
            outer_vars.contains_key(name.as_str())
                && !locals.contains(name.as_str())
                && !crate::builtins::is_builtin_proc(name)
                && !import_map.contains_key(name.as_str())
                && !imported_modules.contains(name.as_str())
        })
        .map(|name| CapturedVar {
            name: name.clone(),
            ty: outer_vars[name],
            is_high_companion: false,
        })
        .collect();

    // Auto-capture _high companions for open array parameters.
    // When a nested proc captures an open array param 's', it also needs 's_high'
    // for HIGH(s) to work, even though '_high' isn't an AST-level reference.
    let mut extra = Vec::new();
    for cap in &captures {
        let high_name = format!("{}_high", cap.name);
        if outer_vars.contains_key(&high_name)
            && !captures.iter().any(|c| c.name == high_name)
        {
            extra.push(CapturedVar {
                name: high_name,
                ty: *outer_vars.get(&format!("{}_high", cap.name)).unwrap(),
                is_high_companion: true,
            });
        }
    }
    captures.extend(extra);

    captures.sort();
    captures
}

/// Recursively collect all identifier references in a procedure body
/// and all nested procedure bodies (transitive).
fn collect_refs_in_proc_deep(proc: &ast::ProcDecl, out: &mut HashSet<String>) {
    if let Some(stmts) = &proc.block.body {
        collect_refs_in_stmts(stmts, out);
    }
    for decl in &proc.block.decls {
        if let ast::Declaration::Procedure(np) = decl {
            collect_refs_in_proc_deep(np, out);
        }
    }
}

fn collect_refs_in_stmts(stmts: &[ast::Statement], out: &mut HashSet<String>) {
    for stmt in stmts {
        collect_refs_in_stmt(stmt, out);
    }
}

fn collect_refs_in_stmt(stmt: &ast::Statement, out: &mut HashSet<String>) {
    use ast::StatementKind::*;
    match &stmt.kind {
        Empty => {}
        Assign { desig, expr } => {
            collect_refs_in_desig(desig, out);
            collect_refs_in_expr(expr, out);
        }
        ProcCall { desig, args } => {
            collect_refs_in_desig(desig, out);
            for a in args { collect_refs_in_expr(a, out); }
        }
        If { cond, then_body, elsifs, else_body } => {
            collect_refs_in_expr(cond, out);
            collect_refs_in_stmts(then_body, out);
            for (c, b) in elsifs {
                collect_refs_in_expr(c, out);
                collect_refs_in_stmts(b, out);
            }
            if let Some(eb) = else_body { collect_refs_in_stmts(eb, out); }
        }
        Case { expr, branches, else_body } => {
            collect_refs_in_expr(expr, out);
            for branch in branches { collect_refs_in_stmts(&branch.body, out); }
            if let Some(eb) = else_body { collect_refs_in_stmts(eb, out); }
        }
        While { cond, body } => {
            collect_refs_in_expr(cond, out);
            collect_refs_in_stmts(body, out);
        }
        Repeat { body, cond } => {
            collect_refs_in_stmts(body, out);
            collect_refs_in_expr(cond, out);
        }
        For { var, start, end, step, body } => {
            out.insert(var.clone());
            collect_refs_in_expr(start, out);
            collect_refs_in_expr(end, out);
            if let Some(s) = step { collect_refs_in_expr(s, out); }
            collect_refs_in_stmts(body, out);
        }
        Loop { body } => {
            collect_refs_in_stmts(body, out);
        }
        With { desig, body } => {
            collect_refs_in_desig(desig, out);
            collect_refs_in_stmts(body, out);
        }
        Return { expr } => {
            if let Some(e) = expr { collect_refs_in_expr(e, out); }
        }
        Exit => {}
        Raise { expr } => {
            if let Some(e) = expr { collect_refs_in_expr(e, out); }
        }
        Retry => {}
        Try { body, excepts, finally_body } => {
            collect_refs_in_stmts(body, out);
            for ec in excepts { collect_refs_in_stmts(&ec.body, out); }
            if let Some(fb) = finally_body { collect_refs_in_stmts(fb, out); }
        }
        Lock { mutex, body } => {
            collect_refs_in_expr(mutex, out);
            collect_refs_in_stmts(body, out);
        }
        TypeCase { expr, branches, else_body } => {
            collect_refs_in_expr(expr, out);
            for branch in branches { collect_refs_in_stmts(&branch.body, out); }
            if let Some(eb) = else_body { collect_refs_in_stmts(eb, out); }
        }
    }
}

fn collect_refs_in_expr(expr: &ast::Expr, out: &mut HashSet<String>) {
    match &expr.kind {
        ExprKind::IntLit(_) | ExprKind::RealLit(_) | ExprKind::StringLit(_)
        | ExprKind::CharLit(_) | ExprKind::BoolLit(_) | ExprKind::NilLit => {}
        ExprKind::Designator(d) => collect_refs_in_desig(d, out),
        ExprKind::FuncCall { desig, args } => {
            // Don't count func name as variable ref, but collect selector and arg refs
            for sel in &desig.selectors {
                if let Selector::Index(indices, _) = sel {
                    for idx in indices { collect_refs_in_expr(idx, out); }
                }
            }
            for arg in args { collect_refs_in_expr(arg, out); }
        }
        ExprKind::UnaryOp { operand, .. } => collect_refs_in_expr(operand, out),
        ExprKind::BinaryOp { left, right, .. } => {
            collect_refs_in_expr(left, out);
            collect_refs_in_expr(right, out);
        }
        ExprKind::SetConstructor { elements, .. } => {
            for elem in elements {
                match elem {
                    SetElement::Single(e) => collect_refs_in_expr(e, out),
                    SetElement::Range(lo, hi) => {
                        collect_refs_in_expr(lo, out);
                        collect_refs_in_expr(hi, out);
                    }
                }
            }
        }
        ExprKind::Not(e) => collect_refs_in_expr(e, out),
        ExprKind::Deref(e) => collect_refs_in_expr(e, out),
    }
}

fn collect_refs_in_desig(desig: &ast::Designator, out: &mut HashSet<String>) {
    if desig.ident.module.is_none() {
        out.insert(desig.ident.name.clone());
    }
    for sel in &desig.selectors {
        if let Selector::Index(indices, _) = sel {
            for idx in indices { collect_refs_in_expr(idx, out); }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::*;
    use crate::errors::SourceLoc;
    use crate::hir::*;
    use crate::symtab::Symbol;
    use crate::types::*;

    fn loc() -> SourceLoc {
        SourceLoc::new("test.mod", 1, 1)
    }

    fn make_designator(name: &str, selectors: Vec<Selector>) -> Designator {
        Designator {
            ident: QualIdent {
                module: None,
                name: name.to_string(),
                loc: loc(),
            },
            selectors,
            loc: loc(),
        }
    }

    /// Build a minimal type registry + symbol table with a record type
    /// and a variable of that type, then resolve a field access.
    #[test]
    fn test_resolve_simple_field() {
        let mut types = TypeRegistry::new();
        let mut symtab = SymbolTable::new();

        // Register a record type: RECORD x: INTEGER; y: REAL END
        let rec_tid = types.register(Type::Record {
            fields: vec![
                RecordField { name: "x".into(), typ: TY_INTEGER, type_name: "INTEGER".into(), offset: 0 },
                RecordField { name: "y".into(), typ: TY_REAL, type_name: "REAL".into(), offset: 4 },
            ],
            variants: None,
        });

        // Register variable "r" of that record type
        let _ = symtab.define_in_current(Symbol {
            name: "r".to_string(),
            kind: SymbolKind::Variable,
            typ: rec_tid,
            exported: false,
            module: None,
            loc: loc(),
            doc: None,
            is_var_param: false,
            is_open_array: false,
        });

        let empty_fm: HashSet<String> = HashSet::new();
        let mut hb = HirBuilder::new(&types, &symtab, "Test", &empty_fm);
        hb.register_var("r", rec_tid);

        // Resolve: r.x
        let desig = make_designator("r", vec![
            Selector::Field("x".to_string(), loc()),
        ]);

        let place = hb.resolve_designator(&desig).expect("should resolve r.x");
        assert_eq!(place.ty, TY_INTEGER);
        assert_eq!(place.projections.len(), 1);
        match &place.projections[0].kind {
            ProjectionKind::Field { index, name, .. } => {
                assert_eq!(*index, 0);
                assert_eq!(name, "x");
            }
            _ => panic!("expected Field projection"),
        }

        // Resolve: r.y
        let desig = make_designator("r", vec![
            Selector::Field("y".to_string(), loc()),
        ]);
        let place = hb.resolve_designator(&desig).expect("should resolve r.y");
        assert_eq!(place.ty, TY_REAL);
        assert_eq!(place.projections.len(), 1);
        match &place.projections[0].kind {
            ProjectionKind::Field { index, name, .. } => {
                assert_eq!(*index, 1);
                assert_eq!(name, "y");
            }
            _ => panic!("expected Field projection"),
        }
    }

    #[test]
    fn test_resolve_pointer_deref() {
        let mut types = TypeRegistry::new();
        let mut symtab = SymbolTable::new();

        // RECORD val: INTEGER END
        let rec_tid = types.register(Type::Record {
            fields: vec![
                RecordField { name: "val".into(), typ: TY_INTEGER, type_name: "INTEGER".into(), offset: 0 },
            ],
            variants: None,
        });
        // POINTER TO Record
        let ptr_tid = types.register(Type::Pointer { base: rec_tid });

        let _ = symtab.define_in_current(Symbol {
            name: "p".to_string(),
            kind: SymbolKind::Variable,
            typ: ptr_tid,
            exported: false,
            module: None,
            loc: loc(),
            doc: None,
            is_var_param: false,
            is_open_array: false,
        });

        let empty_fm: HashSet<String> = HashSet::new();
        let mut hb = HirBuilder::new(&types, &symtab, "Test", &empty_fm);
        hb.register_var("p", ptr_tid);

        // Resolve: p^.val
        let desig = make_designator("p", vec![
            Selector::Deref(loc()),
            Selector::Field("val".to_string(), loc()),
        ]);

        let place = hb.resolve_designator(&desig).expect("should resolve p^.val");
        assert_eq!(place.ty, TY_INTEGER);
        assert_eq!(place.projections.len(), 2);
        assert!(matches!(place.projections[0].kind, ProjectionKind::Deref));
        match &place.projections[1].kind {
            ProjectionKind::Field { index, name, .. } => {
                assert_eq!(*index, 0);
                assert_eq!(name, "val");
            }
            _ => panic!("expected Field projection"),
        }
    }

    #[test]
    fn test_resolve_array_index() {
        let mut types = TypeRegistry::new();
        let mut symtab = SymbolTable::new();

        // ARRAY [0..9] OF INTEGER
        let arr_tid = types.register(Type::Array {
            index_type: TY_INTEGER,
            elem_type: TY_INTEGER,
            low: 0,
            high: 9,
        });

        let _ = symtab.define_in_current(Symbol {
            name: "a".to_string(),
            kind: SymbolKind::Variable,
            typ: arr_tid,
            exported: false,
            module: None,
            loc: loc(),
            doc: None,
            is_var_param: false,
            is_open_array: false,
        });

        let empty_fm: HashSet<String> = HashSet::new();
        let mut hb = HirBuilder::new(&types, &symtab, "Test", &empty_fm);
        hb.register_var("a", arr_tid);

        // Resolve: a[5]
        let desig = make_designator("a", vec![
            Selector::Index(vec![Expr {
                kind: ExprKind::IntLit(5),
                loc: loc(),
            }], loc()),
        ]);

        let place = hb.resolve_designator(&desig).expect("should resolve a[5]");
        assert_eq!(place.ty, TY_INTEGER);
        assert_eq!(place.projections.len(), 1);
        assert!(matches!(place.projections[0].kind, ProjectionKind::Index(_)));
    }

    #[test]
    fn test_resolve_with_field() {
        let mut types = TypeRegistry::new();
        let mut symtab = SymbolTable::new();

        // RECORD x: INTEGER; y: REAL END
        let rec_tid = types.register(Type::Record {
            fields: vec![
                RecordField { name: "x".into(), typ: TY_INTEGER, type_name: "INTEGER".into(), offset: 0 },
                RecordField { name: "y".into(), typ: TY_REAL, type_name: "REAL".into(), offset: 4 },
            ],
            variants: None,
        });

        let _ = symtab.define_in_current(Symbol {
            name: "r".to_string(),
            kind: SymbolKind::Variable,
            typ: rec_tid,
            exported: false,
            module: None,
            loc: loc(),
            doc: None,
            is_var_param: false,
            is_open_array: false,
        });

        let empty_fm: HashSet<String> = HashSet::new();
        let mut hb = HirBuilder::new(&types, &symtab, "Test", &empty_fm);
        hb.register_var("r", rec_tid);

        // Push WITH r DO
        hb.push_with("r", rec_tid);

        // Resolve bare "x" — should resolve as r.x via WITH
        let desig = make_designator("x", vec![]);
        let place = hb.resolve_designator(&desig).expect("should resolve WITH field x");
        assert_eq!(place.ty, TY_INTEGER);
        // Should have: base = r, projection = Field(0, "x")
        assert_eq!(place.projections.len(), 1);
        match &place.projections[0].kind {
            ProjectionKind::Field { index, name, .. } => {
                assert_eq!(*index, 0);
                assert_eq!(name, "x");
            }
            _ => panic!("expected Field projection"),
        }

        hb.pop_with();

        // After pop, "x" should NOT resolve via WITH
        let desig = make_designator("x", vec![]);
        // This will try symtab fallback (won't find "x" as a variable)
        let place = hb.resolve_designator(&desig);
        // Should still resolve (falls back to global) but NOT via WITH
        if let Some(p) = place {
            // Should have no projections (it's just a bare name now)
            assert!(p.projections.is_empty());
        }
    }

    #[test]
    fn test_resolve_nested_record() {
        let mut types = TypeRegistry::new();
        let mut symtab = SymbolTable::new();

        // Inner: RECORD val: INTEGER END
        let inner_tid = types.register(Type::Record {
            fields: vec![
                RecordField { name: "val".into(), typ: TY_INTEGER, type_name: "INTEGER".into(), offset: 0 },
            ],
            variants: None,
        });

        // Outer: RECORD inner: Inner END
        let outer_tid = types.register(Type::Record {
            fields: vec![
                RecordField { name: "inner".into(), typ: inner_tid, type_name: "Inner".into(), offset: 0 },
            ],
            variants: None,
        });

        let _ = symtab.define_in_current(Symbol {
            name: "r".to_string(),
            kind: SymbolKind::Variable,
            typ: outer_tid,
            exported: false,
            module: None,
            loc: loc(),
            doc: None,
            is_var_param: false,
            is_open_array: false,
        });

        let empty_fm: HashSet<String> = HashSet::new();
        let mut hb = HirBuilder::new(&types, &symtab, "Test", &empty_fm);
        hb.register_var("r", outer_tid);

        // Resolve: r.inner.val
        let desig = make_designator("r", vec![
            Selector::Field("inner".to_string(), loc()),
            Selector::Field("val".to_string(), loc()),
        ]);

        let place = hb.resolve_designator(&desig).expect("should resolve r.inner.val");
        assert_eq!(place.ty, TY_INTEGER);
        assert_eq!(place.projections.len(), 2);
        match &place.projections[0].kind {
            ProjectionKind::Field { index, name, .. } => {
                assert_eq!(*index, 0);
                assert_eq!(name, "inner");
            }
            _ => panic!("expected Field projection for 'inner'"),
        }
        match &place.projections[1].kind {
            ProjectionKind::Field { index, name, .. } => {
                assert_eq!(*index, 0);
                assert_eq!(name, "val");
            }
            _ => panic!("expected Field projection for 'val'"),
        }
    }

    #[test]
    fn test_resolve_with_pointer_deref() {
        let mut types = TypeRegistry::new();
        let mut symtab = SymbolTable::new();

        // RECORD x: INTEGER END
        let rec_tid = types.register(Type::Record {
            fields: vec![
                RecordField { name: "x".into(), typ: TY_INTEGER, type_name: "INTEGER".into(), offset: 0 },
            ],
            variants: None,
        });
        // POINTER TO Record
        let ptr_tid = types.register(Type::Pointer { base: rec_tid });

        let _ = symtab.define_in_current(Symbol {
            name: "p".to_string(),
            kind: SymbolKind::Variable,
            typ: ptr_tid,
            exported: false,
            module: None,
            loc: loc(),
            doc: None,
            is_var_param: false,
            is_open_array: false,
        });

        let empty_fm: HashSet<String> = HashSet::new();
        let mut hb = HirBuilder::new(&types, &symtab, "Test", &empty_fm);
        hb.register_var("p", ptr_tid);

        // WITH p^ DO ... x ... END
        // push_with auto-derefs pointer types
        hb.push_with("p", ptr_tid);

        // Resolve bare "x" — should resolve as p^.x
        let desig = make_designator("x", vec![]);
        let place = hb.resolve_designator(&desig).expect("should resolve WITH ptr field x");
        assert_eq!(place.ty, TY_INTEGER);
        // Should have: Deref (auto-deref pointer), then Field
        assert_eq!(place.projections.len(), 2);
        assert!(matches!(place.projections[0].kind, ProjectionKind::Deref));
        match &place.projections[1].kind {
            ProjectionKind::Field { index, name, .. } => {
                assert_eq!(*index, 0);
                assert_eq!(name, "x");
            }
            _ => panic!("expected Field projection"),
        }

        hb.pop_with();
    }

    #[test]
    fn test_resolve_constant() {
        let types = TypeRegistry::new();
        let mut symtab = SymbolTable::new();

        let _ = symtab.define_in_current(Symbol {
            name: "MaxSize".to_string(),
            kind: SymbolKind::Constant(ConstValue::Integer(100)),
            typ: TY_INTEGER,
            exported: false,
            module: None,
            loc: loc(),
            doc: None,
            is_var_param: false,
            is_open_array: false,
        });

        let empty_fm: HashSet<String> = HashSet::new();
        let mut hb = HirBuilder::new(&types, &symtab, "Test", &empty_fm);

        let desig = make_designator("MaxSize", vec![]);
        let place = hb.resolve_designator(&desig).expect("should resolve constant");
        assert!(matches!(place.base, PlaceBase::Constant(ConstVal::Integer(100))));
        assert_eq!(place.ty, TY_INTEGER);
    }

    #[test]
    fn test_resolve_alias_type() {
        let mut types = TypeRegistry::new();
        let mut symtab = SymbolTable::new();

        // RECORD val: INTEGER END
        let rec_tid = types.register(Type::Record {
            fields: vec![
                RecordField { name: "val".into(), typ: TY_INTEGER, type_name: "INTEGER".into(), offset: 0 },
            ],
            variants: None,
        });
        // TYPE MyRec = Record (alias)
        let alias_tid = types.register(Type::Alias {
            name: "MyRec".into(),
            target: rec_tid,
        });

        let _ = symtab.define_in_current(Symbol {
            name: "r".to_string(),
            kind: SymbolKind::Variable,
            typ: alias_tid,
            exported: false,
            module: None,
            loc: loc(),
            doc: None,
            is_var_param: false,
            is_open_array: false,
        });

        let empty_fm: HashSet<String> = HashSet::new();
        let mut hb = HirBuilder::new(&types, &symtab, "Test", &empty_fm);
        hb.register_var("r", alias_tid);

        // Resolve: r.val — should resolve through alias
        let desig = make_designator("r", vec![
            Selector::Field("val".to_string(), loc()),
        ]);
        let place = hb.resolve_designator(&desig).expect("should resolve through alias");
        assert_eq!(place.ty, TY_INTEGER);
    }

    // ── Closure analysis tests ──────────────────────────────────────

    fn make_proc(name: &str, params: Vec<FormalParam>, body: Vec<Statement>, decls: Vec<Declaration>) -> ProcDecl {
        ProcDecl {
            heading: ProcHeading {
                name: name.to_string(),
                params,
                return_type: None,
                raises: None,
                export_c_name: None,
                loc: loc(),
                doc: None,
            },
            block: Block {
                decls,
                body: Some(body),
                finally: None,
                except: None,
                loc: loc(),
            },
            loc: loc(),
            doc: None,
        }
    }

    fn make_assign_stmt(var: &str) -> Statement {
        Statement {
            kind: StatementKind::Assign {
                desig: Designator {
                    ident: QualIdent { module: None, name: var.to_string(), loc: loc() },
                    selectors: vec![],
                    loc: loc(),
                },
                expr: Expr { kind: ExprKind::IntLit(0), loc: loc() },
            },
            loc: loc(),
        }
    }

    fn make_read_expr_stmt(var: &str) -> Statement {
        // x := var (reads `var` in an expression)
        Statement {
            kind: StatementKind::Assign {
                desig: Designator {
                    ident: QualIdent { module: None, name: "x".to_string(), loc: loc() },
                    selectors: vec![],
                    loc: loc(),
                },
                expr: Expr {
                    kind: ExprKind::Designator(Designator {
                        ident: QualIdent { module: None, name: var.to_string(), loc: loc() },
                        selectors: vec![],
                        loc: loc(),
                    }),
                    loc: loc(),
                },
            },
            loc: loc(),
        }
    }

    #[test]
    fn test_compute_captures_simple() {
        // Outer proc has local "x", nested proc reads "x"
        let outer_vars: HashMap<String, TypeId> = [
            ("x".to_string(), TY_INTEGER),
        ].into_iter().collect();

        let nested = make_proc("Inner", vec![], vec![
            make_read_expr_stmt("x"),
        ], vec![]);

        let captures = super::compute_captures(
            &nested,
            &outer_vars,
            &HashMap::new(),
            &HashSet::new(),
        );

        assert_eq!(captures.len(), 1);
        assert_eq!(captures[0].name, "x");
        assert_eq!(captures[0].ty, TY_INTEGER);
        assert!(!captures[0].is_high_companion);
    }

    #[test]
    fn test_compute_captures_excludes_locals() {
        // Nested proc has its own "x" — should NOT capture outer "x"
        let outer_vars: HashMap<String, TypeId> = [
            ("x".to_string(), TY_INTEGER),
        ].into_iter().collect();

        let nested = make_proc("Inner", vec![], vec![
            make_assign_stmt("x"),
        ], vec![
            Declaration::Var(VarDecl {
                names: vec!["x".to_string()],
                name_locs: vec![loc()],
                typ: TypeNode::Named(QualIdent { module: None, name: "INTEGER".to_string(), loc: loc() }),
                loc: loc(),
                doc: None,
            }),
        ]);

        let captures = super::compute_captures(
            &nested,
            &outer_vars,
            &HashMap::new(),
            &HashSet::new(),
        );

        assert!(captures.is_empty());
    }

    #[test]
    fn test_compute_captures_excludes_params() {
        // Nested proc has param "x" — should NOT capture outer "x"
        let outer_vars: HashMap<String, TypeId> = [
            ("x".to_string(), TY_INTEGER),
        ].into_iter().collect();

        let nested = make_proc("Inner", vec![
            FormalParam {
                is_var: false,
                names: vec!["x".to_string()],
                typ: TypeNode::Named(QualIdent { module: None, name: "INTEGER".to_string(), loc: loc() }),
                loc: loc(),
            },
        ], vec![
            make_assign_stmt("x"),
        ], vec![]);

        let captures = super::compute_captures(
            &nested,
            &outer_vars,
            &HashMap::new(),
            &HashSet::new(),
        );

        assert!(captures.is_empty());
    }

    #[test]
    fn test_compute_captures_transitive() {
        // Outer has "x", nested has sub-nested that reads "x"
        // The nested proc should transitively capture "x"
        let outer_vars: HashMap<String, TypeId> = [
            ("x".to_string(), TY_INTEGER),
        ].into_iter().collect();

        let sub_nested = make_proc("SubInner", vec![], vec![
            make_read_expr_stmt("x"),
        ], vec![]);

        let nested = make_proc("Inner", vec![], vec![], vec![
            Declaration::Procedure(sub_nested),
        ]);

        let captures = super::compute_captures(
            &nested,
            &outer_vars,
            &HashMap::new(),
            &HashSet::new(),
        );

        assert_eq!(captures.len(), 1);
        assert_eq!(captures[0].name, "x");
    }

    #[test]
    fn test_compute_captures_high_companion() {
        // When capturing an open array param "s", auto-capture "s_high"
        let outer_vars: HashMap<String, TypeId> = [
            ("s".to_string(), TY_ADDRESS),
            ("s_high".to_string(), TY_INTEGER),
        ].into_iter().collect();

        let nested = make_proc("Inner", vec![], vec![
            make_read_expr_stmt("s"),
        ], vec![]);

        let captures = super::compute_captures(
            &nested,
            &outer_vars,
            &HashMap::new(),
            &HashSet::new(),
        );

        assert_eq!(captures.len(), 2);
        let names: Vec<&str> = captures.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"s"));
        assert!(names.contains(&"s_high"));
        // The _high companion should be marked
        let high = captures.iter().find(|c| c.name == "s_high").unwrap();
        assert!(high.is_high_companion);
    }

    #[test]
    fn test_compute_captures_excludes_imports() {
        // Imported names should not be captured
        let outer_vars: HashMap<String, TypeId> = [
            ("x".to_string(), TY_INTEGER),
            ("WriteString".to_string(), TY_PROC),
        ].into_iter().collect();

        let import_map: HashMap<String, String> = [
            ("WriteString".to_string(), "InOut".to_string()),
        ].into_iter().collect();

        let nested = make_proc("Inner", vec![], vec![
            make_read_expr_stmt("x"),
            make_read_expr_stmt("WriteString"),
        ], vec![]);

        let captures = super::compute_captures(
            &nested,
            &outer_vars,
            &import_map,
            &HashSet::new(),
        );

        assert_eq!(captures.len(), 1);
        assert_eq!(captures[0].name, "x");
    }

    #[test]
    fn test_compute_captures_sorted() {
        let outer_vars: HashMap<String, TypeId> = [
            ("z".to_string(), TY_INTEGER),
            ("a".to_string(), TY_INTEGER),
            ("m".to_string(), TY_INTEGER),
        ].into_iter().collect();

        let nested = make_proc("Inner", vec![], vec![
            make_read_expr_stmt("z"),
            make_read_expr_stmt("a"),
            make_read_expr_stmt("m"),
        ], vec![]);

        let captures = super::compute_captures(
            &nested,
            &outer_vars,
            &HashMap::new(),
            &HashSet::new(),
        );

        let names: Vec<&str> = captures.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["a", "m", "z"]);
    }

    // ── Phase 3: FOR direction tests ────────────────────────────────

    #[test]
    fn test_for_direction_no_step() {
        let types = TypeRegistry::new();
        let symtab = SymbolTable::new();
        let empty_fm: HashSet<String> = HashSet::new();
        let mut hb = HirBuilder::new(&types, &symtab, "Test", &empty_fm);
        assert_eq!(hb.for_direction(None), ForDirection::Up);
    }

    #[test]
    fn test_for_direction_positive_literal() {
        let types = TypeRegistry::new();
        let symtab = SymbolTable::new();
        let empty_fm: HashSet<String> = HashSet::new();
        let mut hb = HirBuilder::new(&types, &symtab, "Test", &empty_fm);
        let step = Expr { kind: ExprKind::IntLit(1), loc: loc() };
        assert_eq!(hb.for_direction(Some(&step)), ForDirection::Up);
    }

    #[test]
    fn test_for_direction_negative_literal() {
        let types = TypeRegistry::new();
        let symtab = SymbolTable::new();
        let empty_fm: HashSet<String> = HashSet::new();
        let mut hb = HirBuilder::new(&types, &symtab, "Test", &empty_fm);
        let step = Expr { kind: ExprKind::IntLit(-1), loc: loc() };
        assert_eq!(hb.for_direction(Some(&step)), ForDirection::Down);
    }

    #[test]
    fn test_for_direction_unary_neg() {
        let types = TypeRegistry::new();
        let symtab = SymbolTable::new();
        let empty_fm: HashSet<String> = HashSet::new();
        let mut hb = HirBuilder::new(&types, &symtab, "Test", &empty_fm);
        let step = Expr {
            kind: ExprKind::UnaryOp {
                op: UnaryOp::Neg,
                operand: Box::new(Expr { kind: ExprKind::IntLit(1), loc: loc() }),
            },
            loc: loc(),
        };
        assert_eq!(hb.for_direction(Some(&step)), ForDirection::Down);
    }

    #[test]
    fn test_for_direction_const_fold_sub() {
        let types = TypeRegistry::new();
        let symtab = SymbolTable::new();
        let empty_fm: HashSet<String> = HashSet::new();
        let mut hb = HirBuilder::new(&types, &symtab, "Test", &empty_fm);
        // 0 - 1 = -1 → Down
        let step = Expr {
            kind: ExprKind::BinaryOp {
                op: BinaryOp::Sub,
                left: Box::new(Expr { kind: ExprKind::IntLit(0), loc: loc() }),
                right: Box::new(Expr { kind: ExprKind::IntLit(1), loc: loc() }),
            },
            loc: loc(),
        };
        assert_eq!(hb.for_direction(Some(&step)), ForDirection::Down);
    }

    #[test]
    fn test_for_direction_const_fold_positive() {
        let types = TypeRegistry::new();
        let symtab = SymbolTable::new();
        let empty_fm: HashSet<String> = HashSet::new();
        let mut hb = HirBuilder::new(&types, &symtab, "Test", &empty_fm);
        // 2 + 3 = 5 → Up
        let step = Expr {
            kind: ExprKind::BinaryOp {
                op: BinaryOp::Add,
                left: Box::new(Expr { kind: ExprKind::IntLit(2), loc: loc() }),
                right: Box::new(Expr { kind: ExprKind::IntLit(3), loc: loc() }),
            },
            loc: loc(),
        };
        assert_eq!(hb.for_direction(Some(&step)), ForDirection::Up);
    }

    // ── Phase 3: String interning tests ─────────────────────────────

    #[test]
    fn test_string_interning_basic() {
        let types = TypeRegistry::new();
        let symtab = SymbolTable::new();
        let empty_fm: HashSet<String> = HashSet::new();
        let mut hb = HirBuilder::new(&types, &symtab, "Test", &empty_fm);

        let id1 = hb.intern_string("hello");
        let id2 = hb.intern_string("world");
        let id3 = hb.intern_string("hello"); // duplicate

        assert_eq!(id1, StringId(0));
        assert_eq!(id2, StringId(1));
        assert_eq!(id3, StringId(0)); // deduplicated
        assert_eq!(hb.get_string(id1), "hello");
        assert_eq!(hb.get_string(id2), "world");
        assert_eq!(hb.string_pool().len(), 2);
    }

    #[test]
    fn test_string_interning_empty() {
        let types = TypeRegistry::new();
        let symtab = SymbolTable::new();
        let empty_fm: HashSet<String> = HashSet::new();
        let mut hb = HirBuilder::new(&types, &symtab, "Test", &empty_fm);

        let id = hb.intern_string("");
        assert_eq!(id, StringId(0));
        assert_eq!(hb.get_string(id), "");
    }

    // ── Phase 3: Const eval tests ───────────────────────────────────

    #[test]
    fn test_const_eval_named_constant() {
        let types = TypeRegistry::new();
        let mut symtab = SymbolTable::new();

        let _ = symtab.define_in_current(Symbol {
            name: "N".to_string(),
            kind: SymbolKind::Constant(ConstValue::Integer(42)),
            typ: TY_INTEGER,
            exported: false,
            module: None,
            loc: loc(),
            doc: None,
            is_var_param: false,
            is_open_array: false,
        });

        let empty_fm: HashSet<String> = HashSet::new();
        let mut hb = HirBuilder::new(&types, &symtab, "Test", &empty_fm);

        let expr = Expr {
            kind: ExprKind::Designator(Designator {
                ident: QualIdent { module: None, name: "N".to_string(), loc: loc() },
                selectors: vec![],
                loc: loc(),
            }),
            loc: loc(),
        };

        assert_eq!(hb.try_eval_const_int(&expr), Some(42));
    }

    #[test]
    fn test_const_eval_complex_expr() {
        let types = TypeRegistry::new();
        let symtab = SymbolTable::new();
        let empty_fm: HashSet<String> = HashSet::new();
        let mut hb = HirBuilder::new(&types, &symtab, "Test", &empty_fm);

        // -(3 * 4) = -12
        let expr = Expr {
            kind: ExprKind::UnaryOp {
                op: UnaryOp::Neg,
                operand: Box::new(Expr {
                    kind: ExprKind::BinaryOp {
                        op: BinaryOp::Mul,
                        left: Box::new(Expr { kind: ExprKind::IntLit(3), loc: loc() }),
                        right: Box::new(Expr { kind: ExprKind::IntLit(4), loc: loc() }),
                    },
                    loc: loc(),
                }),
            },
            loc: loc(),
        };

        assert_eq!(hb.try_eval_const_int(&expr), Some(-12));
    }

    // ── Phase 4: Statement lowering tests ───────────────────────────

    #[test]
    fn test_lower_assign() {
        let types = TypeRegistry::new();
        let mut symtab = SymbolTable::new();

        let _ = symtab.define_in_current(Symbol {
            name: "x".to_string(),
            kind: SymbolKind::Variable,
            typ: TY_INTEGER,
            exported: false,
            module: None,
            loc: loc(),
            doc: None,
            is_var_param: false,
            is_open_array: false,
        });

        let empty_fm: HashSet<String> = HashSet::new();
        let mut hb = HirBuilder::new(&types, &symtab, "Test", &empty_fm);
        hb.register_var("x", TY_INTEGER);

        let stmt = Statement {
            kind: StatementKind::Assign {
                desig: Designator {
                    ident: QualIdent { module: None, name: "x".to_string(), loc: loc() },
                    selectors: vec![],
                    loc: loc(),
                },
                expr: Expr { kind: ExprKind::IntLit(42), loc: loc() },
            },
            loc: loc(),
        };

        let hir = hb.lower_stmt(&stmt);
        match &hir.kind {
            HirStmtKind::Assign { target, value } => {
                assert_eq!(target.ty, TY_INTEGER);
                assert!(matches!(value.kind, HirExprKind::IntLit(42)));
            }
            _ => panic!("expected Assign"),
        }
    }

    #[test]
    fn test_lower_if() {
        let types = TypeRegistry::new();
        let symtab = SymbolTable::new();
        let empty_fm: HashSet<String> = HashSet::new();
        let mut hb = HirBuilder::new(&types, &symtab, "Test", &empty_fm);

        let stmt = Statement {
            kind: StatementKind::If {
                cond: Expr { kind: ExprKind::BoolLit(true), loc: loc() },
                then_body: vec![Statement { kind: StatementKind::Empty, loc: loc() }],
                elsifs: vec![],
                else_body: None,
            },
            loc: loc(),
        };

        let hir = hb.lower_stmt(&stmt);
        match &hir.kind {
            HirStmtKind::If { cond, then_body, elsifs, else_body } => {
                assert!(matches!(cond.kind, HirExprKind::BoolLit(true)));
                assert_eq!(then_body.len(), 1);
                assert!(elsifs.is_empty());
                assert!(else_body.is_none());
            }
            _ => panic!("expected If"),
        }
    }

    #[test]
    fn test_lower_for_with_direction() {
        let types = TypeRegistry::new();
        let mut symtab = SymbolTable::new();

        let _ = symtab.define_in_current(Symbol {
            name: "i".to_string(),
            kind: SymbolKind::Variable,
            typ: TY_INTEGER,
            exported: false,
            module: None,
            loc: loc(),
            doc: None,
            is_var_param: false,
            is_open_array: false,
        });

        let empty_fm: HashSet<String> = HashSet::new();
        let mut hb = HirBuilder::new(&types, &symtab, "Test", &empty_fm);
        hb.register_var("i", TY_INTEGER);

        // FOR i := 10 TO 1 BY -1
        let stmt = Statement {
            kind: StatementKind::For {
                var: "i".to_string(),
                start: Expr { kind: ExprKind::IntLit(10), loc: loc() },
                end: Expr { kind: ExprKind::IntLit(1), loc: loc() },
                step: Some(Expr { kind: ExprKind::IntLit(-1), loc: loc() }),
                body: vec![Statement { kind: StatementKind::Empty, loc: loc() }],
            },
            loc: loc(),
        };

        let hir = hb.lower_stmt(&stmt);
        match &hir.kind {
            HirStmtKind::For { var, var_ty, direction, .. } => {
                assert_eq!(var, "i");
                assert_eq!(*var_ty, TY_INTEGER);
                assert_eq!(*direction, ForDirection::Down);
            }
            _ => panic!("expected For"),
        }
    }

    #[test]
    fn test_lower_with_elimination() {
        let mut types = TypeRegistry::new();
        let mut symtab = SymbolTable::new();

        let rec_tid = types.register(Type::Record {
            fields: vec![
                RecordField { name: "x".into(), typ: TY_INTEGER, type_name: "INTEGER".into(), offset: 0 },
            ],
            variants: None,
        });

        let _ = symtab.define_in_current(Symbol {
            name: "r".to_string(),
            kind: SymbolKind::Variable,
            typ: rec_tid,
            exported: false,
            module: None,
            loc: loc(),
            doc: None,
            is_var_param: false,
            is_open_array: false,
        });

        let empty_fm: HashSet<String> = HashSet::new();
        let mut hb = HirBuilder::new(&types, &symtab, "Test", &empty_fm);
        hb.register_var("r", rec_tid);

        // WITH r DO x := 42 END
        let stmts = vec![Statement {
            kind: StatementKind::With {
                desig: Designator {
                    ident: QualIdent { module: None, name: "r".to_string(), loc: loc() },
                    selectors: vec![],
                    loc: loc(),
                },
                body: vec![Statement {
                    kind: StatementKind::Assign {
                        desig: Designator {
                            ident: QualIdent { module: None, name: "x".to_string(), loc: loc() },
                            selectors: vec![],
                            loc: loc(),
                        },
                        expr: Expr { kind: ExprKind::IntLit(42), loc: loc() },
                    },
                    loc: loc(),
                }],
            },
            loc: loc(),
        }];

        // Use lower_stmts which handles WITH elimination
        let hir_stmts = hb.lower_stmts(&stmts);

        // WITH should be eliminated — body inlined
        assert_eq!(hir_stmts.len(), 1);
        match &hir_stmts[0].kind {
            HirStmtKind::Assign { target, value } => {
                // "x" inside WITH r should resolve as r.x
                assert_eq!(target.ty, TY_INTEGER);
                assert_eq!(target.projections.len(), 1);
                match &target.projections[0].kind {
                    ProjectionKind::Field { name, index, .. } => {
                        assert_eq!(name, "x");
                        assert_eq!(*index, 0);
                    }
                    _ => panic!("expected Field projection"),
                }
                assert!(matches!(value.kind, HirExprKind::IntLit(42)));
            }
            _ => panic!("expected Assign after WITH elimination"),
        }
    }

    #[test]
    fn test_lower_expr_types() {
        let types = TypeRegistry::new();
        let symtab = SymbolTable::new();
        let empty_fm: HashSet<String> = HashSet::new();
        let mut hb = HirBuilder::new(&types, &symtab, "Test", &empty_fm);

        // Integer literal
        let e = hb.lower_expr(&Expr { kind: ExprKind::IntLit(5), loc: loc() });
        assert_eq!(e.ty, TY_INTEGER);

        // Boolean literal
        let e = hb.lower_expr(&Expr { kind: ExprKind::BoolLit(true), loc: loc() });
        assert_eq!(e.ty, TY_BOOLEAN);

        // String → interned
        let e = hb.lower_expr(&Expr { kind: ExprKind::StringLit("hello".to_string()), loc: loc() });
        assert_eq!(e.ty, TY_STRING);
        match e.kind {
            HirExprKind::StringLit(ref s) => assert_eq!(s, "hello"),
            _ => panic!("expected StringLit"),
        }

        // Single char string → CHAR type
        let e = hb.lower_expr(&Expr { kind: ExprKind::StringLit("A".to_string()), loc: loc() });
        assert_eq!(e.ty, TY_CHAR);

        // Comparison → BOOLEAN
        let e = hb.lower_expr(&Expr {
            kind: ExprKind::BinaryOp {
                op: BinaryOp::Lt,
                left: Box::new(Expr { kind: ExprKind::IntLit(1), loc: loc() }),
                right: Box::new(Expr { kind: ExprKind::IntLit(2), loc: loc() }),
            },
            loc: loc(),
        });
        assert_eq!(e.ty, TY_BOOLEAN);
    }

    #[test]
    fn test_lower_loop_exit() {
        let types = TypeRegistry::new();
        let symtab = SymbolTable::new();
        let empty_fm: HashSet<String> = HashSet::new();
        let mut hb = HirBuilder::new(&types, &symtab, "Test", &empty_fm);

        let stmt = Statement {
            kind: StatementKind::Loop {
                body: vec![Statement { kind: StatementKind::Exit, loc: loc() }],
            },
            loc: loc(),
        };

        let hir = hb.lower_stmt(&stmt);
        match &hir.kind {
            HirStmtKind::Loop { body } => {
                assert_eq!(body.len(), 1);
                assert!(matches!(body[0].kind, HirStmtKind::Exit));
            }
            _ => panic!("expected Loop"),
        }
    }

    // ── Phase 5: Module building tests ──────────────────────────────

    #[test]
    fn test_build_module_from_program() {
        let types = TypeRegistry::new();
        let mut symtab = SymbolTable::new();

        // Register a variable
        let _ = symtab.define_in_current(Symbol {
            name: "count".to_string(),
            kind: SymbolKind::Variable,
            typ: TY_INTEGER,
            exported: false,
            module: None,
            loc: loc(),
            doc: None,
            is_var_param: false,
            is_open_array: false,
        });

        // Register a constant
        let _ = symtab.define_in_current(Symbol {
            name: "Max".to_string(),
            kind: SymbolKind::Constant(ConstValue::Integer(100)),
            typ: TY_INTEGER,
            exported: false,
            module: None,
            loc: loc(),
            doc: None,
            is_var_param: false,
            is_open_array: false,
        });

        let empty_fm: HashSet<String> = HashSet::new();
        let mut hb = HirBuilder::new(&types, &symtab, "TestMod", &empty_fm);

        let program = crate::ast::ProgramModule {
            name: "TestMod".to_string(),
            priority: None,
            imports: vec![],
            export: None,
            block: Block {
                decls: vec![
                    Declaration::Var(VarDecl {
                        names: vec!["count".to_string()],
                        name_locs: vec![loc()],
                        typ: TypeNode::Named(QualIdent { module: None, name: "INTEGER".to_string(), loc: loc() }),
                        loc: loc(),
                        doc: None,
                    }),
                    Declaration::Const(ConstDecl {
                        name: "Max".to_string(),
                        expr: Expr { kind: ExprKind::IntLit(100), loc: loc() },
                        loc: loc(),
                        doc: None,
                    }),
                ],
                body: Some(vec![
                    Statement {
                        kind: StatementKind::Assign {
                            desig: Designator {
                                ident: QualIdent { module: None, name: "count".to_string(), loc: loc() },
                                selectors: vec![],
                                loc: loc(),
                            },
                            expr: Expr { kind: ExprKind::IntLit(0), loc: loc() },
                        },
                        loc: loc(),
                    },
                ]),
                finally: None,
                except: None,
                loc: loc(),
            },
            is_safe: false,
            is_unsafe: false,
            loc: loc(),
            doc: None,
        };

        let hir_mod = hb.build_module_from_program(&program);

        assert_eq!(hir_mod.name, "TestMod");
        assert_eq!(hir_mod.globals.len(), 1);
        assert_eq!(hir_mod.globals[0].name.source_name, "count");
        assert_eq!(hir_mod.constants.len(), 1);
        assert_eq!(hir_mod.constants[0].name.source_name, "Max");
        assert!(hir_mod.init_body.is_some());
        let init = hir_mod.init_body.as_ref().unwrap();
        assert_eq!(init.len(), 1);
        assert!(matches!(init[0].kind, HirStmtKind::Assign { .. }));
    }

    #[test]
    fn test_build_module_with_procedure() {
        let types = TypeRegistry::new();
        let mut symtab = SymbolTable::new();

        let _ = symtab.define_in_current(Symbol {
            name: "Hello".to_string(),
            kind: SymbolKind::Procedure {
                params: vec![],
                return_type: None,
                is_builtin: false,
            },
            typ: TY_VOID,
            exported: true,
            module: None,
            loc: loc(),
            doc: None,
            is_var_param: false,
            is_open_array: false,
        });

        let empty_fm: HashSet<String> = HashSet::new();
        let mut hb = HirBuilder::new(&types, &symtab, "TestMod", &empty_fm);

        let program = crate::ast::ProgramModule {
            name: "TestMod".to_string(),
            priority: None,
            imports: vec![],
            export: None,
            block: Block {
                decls: vec![
                    Declaration::Procedure(ProcDecl {
                        heading: ProcHeading {
                            name: "Hello".to_string(),
                            params: vec![],
                            return_type: None,
                            raises: None,
                            export_c_name: None,
                            loc: loc(),
                            doc: None,
                        },
                        block: Block {
                            decls: vec![],
                            body: Some(vec![
                                Statement { kind: StatementKind::Empty, loc: loc() },
                            ]),
                            finally: None,
                            except: None,
                            loc: loc(),
                        },
                        loc: loc(),
                        doc: None,
                    }),
                ],
                body: None,
                finally: None,
                except: None,
                loc: loc(),
            },
            is_safe: false,
            is_unsafe: false,
            loc: loc(),
            doc: None,
        };

        let hir_mod = hb.build_module_from_program(&program);

        assert_eq!(hir_mod.procedures.len(), 1);
        assert_eq!(hir_mod.procedures[0].name.source_name, "Hello");
        assert!(hir_mod.procedures[0].is_exported);
        assert!(hir_mod.procedures[0].body.is_some());
        let body = hir_mod.procedures[0].body.as_ref().unwrap();
        assert_eq!(body.len(), 1);
        assert!(matches!(body[0].kind, HirStmtKind::Empty));
    }

    #[test]
    fn test_build_module_string_pool() {
        let types = TypeRegistry::new();
        let symtab = SymbolTable::new();
        let empty_fm: HashSet<String> = HashSet::new();
        let mut hb = HirBuilder::new(&types, &symtab, "TestMod", &empty_fm);

        let program = crate::ast::ProgramModule {
            name: "TestMod".to_string(),
            priority: None,
            imports: vec![],
            export: None,
            block: Block {
                decls: vec![
                    Declaration::Const(ConstDecl {
                        name: "Greeting".to_string(),
                        expr: Expr { kind: ExprKind::StringLit("Hello".to_string()), loc: loc() },
                        loc: loc(),
                        doc: None,
                    }),
                ],
                body: None,
                finally: None,
                except: None,
                loc: loc(),
            },
            is_safe: false,
            is_unsafe: false,
            loc: loc(),
            doc: None,
        };

        let hir_mod = hb.build_module_from_program(&program);

        assert_eq!(hir_mod.constants.len(), 1);
        match &hir_mod.constants[0].value {
            ConstVal::String(s) => assert_eq!(s, "Hello"),
            _ => panic!("expected string constant"),
        }
    }
}
