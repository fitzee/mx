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
    let (module_name, module_body, module_except, module_finally, module_decls, module_imports, module_loc) = match unit {
        CompilationUnit::ProgramModule(m) => (
            m.name.clone(),
            m.block.body.as_ref(),
            m.block.except.as_deref(),
            m.block.finally.as_deref(),
            &m.block.decls,
            &m.imports,
            &m.loc,
        ),
        CompilationUnit::ImplementationModule(m) => (
            m.name.clone(),
            m.block.body.as_ref(),
            m.block.except.as_deref(),
            m.block.finally.as_deref(),
            &m.block.decls,
            &m.imports,
            &m.loc,
        ),
        CompilationUnit::DefinitionModule(m) => (
            m.name.clone(),
            None,
            None,
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
    let mut local_module_inits = Vec::new();
    let mut new_exception_decls = Vec::new();
    let mut new_proc_decls: Vec<HirProcDecl> = Vec::new();

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
                let mut pd = build_proc_decl(&p.heading, &p.block.decls, &module_name, sema, false, None);
                // Lower the procedure body
                if let Some(stmts) = &p.block.body {
                    let mut hb = HirBuilder::new(
                        &sema.types, &sema.symtab, &module_name, &sema.foreign_modules,
                    );
                    hb.set_imported_modules(imported_modules.clone());
                    hb.set_import_alias_map(import_aliases.clone());
                    hb.enter_procedure_named(&p.heading.name);
                    for fp in &p.heading.params {
                        if matches!(fp.typ, ast::TypeNode::OpenArray { .. }) {
                            for name in &fp.names {
                                let high_name = format!("{}_high", name);
                                hb.register_var(&high_name, TY_INTEGER);
                                hb.register_local(&high_name);
                            }
                        }
                    }
                    pd.body = Some(hb.lower_stmts(stmts));
                }
                if let Some(stmts) = &p.block.except {
                    let mut hb = HirBuilder::new(
                        &sema.types, &sema.symtab, &module_name, &sema.foreign_modules,
                    );
                    hb.set_imported_modules(imported_modules.clone());
                    hb.set_import_alias_map(import_aliases.clone());
                    hb.enter_procedure_named(&p.heading.name);
                    pd.except_handler = Some(hb.lower_stmts(stmts));
                }
                // Build nested proc decls with bodies
                fn build_nested_recursive(
                    parent_decl: &mut crate::hir::HirProcDecl,
                    parent_ast_decls: &[Declaration],
                    scope_chain: &[String],
                    module_name: &str,
                    imported_modules: &[String],
                    import_aliases: &HashMap<String, String>,
                    sema: &SemanticAnalyzer,
                ) {
                    for nd in parent_ast_decls {
                        if let Declaration::Procedure(np) = nd {
                            let parent_name = scope_chain.last().map(|s| s.as_str());
                            let mut npd = build_proc_decl(&np.heading, &np.block.decls, module_name, sema, true, parent_name);
                            // Fix mangled name to include full parent chain
                            let full_mangled = format!("{}_{}", scope_chain.iter()
                                .map(|s| s.as_str()).collect::<Vec<_>>().join("_"),
                                np.heading.name);
                            npd.sig.mangled = format!("{}_{}", module_name, full_mangled);
                            if let Some(stmts) = &np.block.body {
                                let mut nhb = HirBuilder::new(
                                    &sema.types, &sema.symtab, module_name, &sema.foreign_modules,
                                );
                                nhb.set_imported_modules(imported_modules.to_vec());
                                nhb.set_import_alias_map(import_aliases.clone());
                                for scope_name in scope_chain {
                                    nhb.enter_procedure_named(scope_name);
                                }
                                nhb.enter_procedure_named(&np.heading.name);
                                for fp in &np.heading.params {
                                    if matches!(fp.typ, ast::TypeNode::OpenArray { .. }) {
                                        for name in &fp.names {
                                            let high_name = format!("{}_high", name);
                                            nhb.register_var(&high_name, TY_INTEGER);
                                            nhb.register_local(&high_name);
                                        }
                                    }
                                }
                                npd.body = Some(nhb.lower_stmts(stmts));
                            }
                            // Compute closure captures
                            if let Some(ref body) = npd.body {
                                let param_names: Vec<String> = npd.sig.params.iter().map(|p| p.name.clone()).collect();
                                let local_names: HashSet<String> = npd.locals.iter()
                                    .filter_map(|l| if let crate::hir::HirLocalDecl::Var { name, .. } = l { Some(name.clone()) } else { None })
                                    .collect();
                                let mut refs = Vec::new();
                                collect_hir_var_refs(body, &mut refs);
                                let mut seen = HashSet::new();
                                for ref_name in &refs {
                                    if param_names.contains(ref_name) { continue; }
                                    if local_names.contains(ref_name) { continue; }
                                    if seen.contains(ref_name) { continue; }
                                    let tid = sema.symtab.lookup_any(ref_name)
                                        .map(|s| s.typ).unwrap_or(TY_INTEGER);
                                    npd.closure_captures.push(crate::hir::CapturedVar {
                                        name: ref_name.clone(), ty: tid, is_high_companion: false,
                                    });
                                    seen.insert(ref_name.clone());
                                }
                            }
                            // Recurse for deeper nesting
                            let mut chain = scope_chain.to_vec();
                            chain.push(np.heading.name.clone());
                            build_nested_recursive(&mut npd, &np.block.decls, &chain,
                                module_name, imported_modules, import_aliases, sema);
                            // Propagate grandchild captures: if a child captures vars
                            // from outer scopes, the parent needs them too for forwarding.
                            let parent_params: HashSet<String> = parent_decl.sig.params.iter()
                                .map(|p| p.name.clone()).collect();
                            let parent_locals: HashSet<String> = parent_decl.locals.iter()
                                .filter_map(|l| if let crate::hir::HirLocalDecl::Var { name, .. } = l { Some(name.clone()) } else { None })
                                .collect();
                            for child_cap in &npd.closure_captures {
                                if parent_params.contains(&child_cap.name) { continue; }
                                if parent_locals.contains(&child_cap.name) { continue; }
                                if parent_decl.closure_captures.iter().any(|c| c.name == child_cap.name) { continue; }
                                parent_decl.closure_captures.push(child_cap.clone());
                            }
                            parent_decl.nested_procs.push(npd);
                        }
                    }
                }
                let scope_chain = vec![p.heading.name.clone()];
                build_nested_recursive(&mut pd, &p.block.decls, &scope_chain,
                    &module_name, &imported_modules, &import_aliases, sema);
                new_proc_decls.push(pd);
            }
            Declaration::Type(t) => {
                // Use scoped lookup: try module scope, then scope 0 (program module top-level)
                let module_scope = sema.symtab.lookup_module_scope(&module_name);
                let sym = module_scope
                    .and_then(|sid| sema.symtab.lookup_in_scope_direct(sid, &t.name))
                    .or_else(|| sema.symtab.lookup_in_scope_direct(0, &t.name))
                    .or_else(|| sema.symtab.lookup_any(&t.name));
                let type_id = sym.map(|s| s.typ).unwrap_or(TY_INTEGER);
                let exported = sym.map(|s| s.exported).unwrap_or(false);
                let td = HirTypeDecl {
                    name: t.name.clone(),
                    mangled: format!("{}_{}", module_name, t.name),
                    type_id,
                    exported,
                };
                new_type_decls.push(td.clone());
                type_decls_legacy.push(td);
            }
            Declaration::Const(c) => {
                let module_scope = sema.symtab.lookup_module_scope(&module_name);
                let sym = module_scope
                    .and_then(|sid| sema.symtab.lookup_in_scope_direct(sid, &c.name))
                    .or_else(|| sema.symtab.lookup_in_scope_direct(0, &c.name))
                    .or_else(|| sema.symtab.lookup_any(&c.name));
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
                    c_type: const_val_c_type(&val),
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
                let module_scope = sema.symtab.lookup_module_scope(&module_name);
                let var_lookup = |n: &str| -> Option<&crate::symtab::Symbol> {
                    module_scope.and_then(|sid| sema.symtab.lookup_in_scope_direct(sid, n))
                        .or_else(|| sema.symtab.lookup_in_scope_direct(0, n))
                        .or_else(|| sema.symtab.lookup_any(n))
                };
                let type_id = var_lookup(&v.names[0])
                    .map(|s| s.typ).unwrap_or(TY_INTEGER);
                for name in &v.names {
                    let mangled = format!("{}_{}", module_name, name);
                    let exported = var_lookup(name)
                        .map(|s| s.exported).unwrap_or(false);
                    // New format
                    new_global_decls.push(HirGlobalDecl {
                        name: name.clone(),
                        mangled: mangled.clone(),
                        type_id,
                        exported,
                        c_type: String::new(),
                        c_array_suffix: String::new(),
                        is_proc_type: false,
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
            Declaration::Module(local_mod) => {
                // Hoist local module declarations into parent HirModule.
                // Procedures get hoisted (C can't nest function defs).
                // Types, consts, vars are emitted inline.
                for d in &local_mod.block.decls {
                    match d {
                        Declaration::Procedure(p) => {
                            let hir_proc = build_proc(p, &module_name, &imported_modules, &import_aliases, sema);
                            procedures.push(hir_proc);
                            let mut pd = build_proc_decl(&p.heading, &p.block.decls, &module_name, sema, false, None);
                            if let Some(stmts) = &p.block.body {
                                let mut hb = HirBuilder::new(
                                    &sema.types, &sema.symtab, &module_name, &sema.foreign_modules,
                                );
                                hb.set_imported_modules(imported_modules.clone());
                                hb.set_import_alias_map(import_aliases.clone());
                                hb.enter_procedure_named(&p.heading.name);
                                for fp in &p.heading.params {
                                    if matches!(fp.typ, ast::TypeNode::OpenArray { .. }) {
                                        for name in &fp.names {
                                            let high_name = format!("{}_high", name);
                                            hb.register_var(&high_name, TY_INTEGER);
                                            hb.register_local(&high_name);
                                        }
                                    }
                                }
                                pd.body = Some(hb.lower_stmts(stmts));
                            }
                            if let Some(stmts) = &p.block.except {
                                let mut hb = HirBuilder::new(
                                    &sema.types, &sema.symtab, &module_name, &sema.foreign_modules,
                                );
                                hb.set_imported_modules(imported_modules.clone());
                                hb.set_import_alias_map(import_aliases.clone());
                                hb.enter_procedure_named(&p.heading.name);
                                pd.except_handler = Some(hb.lower_stmts(stmts));
                            }
                            for nd in &p.block.decls {
                                if let Declaration::Procedure(np) = nd {
                                    let mut npd = build_proc_decl(&np.heading, &np.block.decls, &module_name, sema, true, Some(&p.heading.name));
                                    if let Some(stmts) = &np.block.body {
                                        let mut nhb = HirBuilder::new(
                                            &sema.types, &sema.symtab, &module_name, &sema.foreign_modules,
                                        );
                                        nhb.set_imported_modules(imported_modules.clone());
                                        nhb.set_import_alias_map(import_aliases.clone());
                                        nhb.enter_procedure_named(&p.heading.name);
                                        nhb.enter_procedure_named(&np.heading.name);
                                        for fp in &np.heading.params {
                                            if matches!(fp.typ, ast::TypeNode::OpenArray { .. }) {
                                                for name in &fp.names {
                                                    let high_name = format!("{}_high", name);
                                                    nhb.register_var(&high_name, TY_INTEGER);
                                                    nhb.register_local(&high_name);
                                                }
                                            }
                                        }
                                        npd.body = Some(nhb.lower_stmts(stmts));
                                    }
                                    pd.nested_procs.push(npd);
                                }
                            }
                            new_proc_decls.push(pd);
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
                            };
                            new_type_decls.push(td);
                        }
                        Declaration::Const(c) => {
                            let sym = sema.symtab.lookup_any(&c.name);
                            let val = sym.and_then(|s| match &s.kind {
                                SymbolKind::Constant(cv) => Some(const_value_to_hir(cv)),
                                _ => None,
                            }).unwrap_or(ConstVal::Integer(0));
                            let type_id = sym.map(|s| s.typ).unwrap_or(TY_INTEGER);
                            new_const_decls.push(HirConstDecl {
                                name: c.name.clone(),
                                mangled: format!("{}_{}", module_name, c.name),
                                value: val.clone(),
                                type_id,
                                exported: sym.map(|s| s.exported).unwrap_or(false),
                                c_type: const_val_c_type(&val),
                            });
                        }
                        Declaration::Var(v) => {
                            let type_id = sema.symtab.lookup_any(&v.names[0])
                                .map(|s| s.typ).unwrap_or(TY_INTEGER);
                            for name in &v.names {
                                let exported = sema.symtab.lookup_any(name)
                                    .map(|s| s.exported).unwrap_or(false);
                                new_global_decls.push(HirGlobalDecl {
                                    name: name.clone(),
                                    mangled: format!("{}_{}", module_name, name),
                                    type_id,
                                    exported,
                                    c_type: String::new(),
                                    c_array_suffix: String::new(),
                                    is_proc_type: false,
                                });
                            }
                        }
                        _ => {}
                    }
                }
                // Lower local module init body
                if let Some(body) = &local_mod.block.body {
                    let mut hb = HirBuilder::new(&sema.types, &sema.symtab, &module_name, &sema.foreign_modules);
                    hb.set_import_alias_map(import_aliases.clone());
                    hb.set_imported_modules(imported_modules.clone());
                    let hir_stmts = hb.lower_stmts(body);
                    local_module_inits.push((local_mod.name.clone(), hir_stmts));
                }
            }
        }
    }

    // Lower embedded implementation module procedures
    for imp in impl_mods {
        let (imp_modules, imp_aliases) = extract_imports(&imp.imports);
        // Use only the embedded module's own imports (not main module imports)
        let mut merged_modules = imp_modules;
        let merged_aliases = imp_aliases;
        // Also include imports from the corresponding definition module (via sema scope)
        if let Some(scope_id) = sema.symtab.lookup_module_scope(&imp.name) {
            for sym in sema.symtab.symbols_in_scope(scope_id) {
                if let Some(ref src_mod) = sym.module {
                    if src_mod != &imp.name && !merged_modules.contains(src_mod) {
                        merged_modules.push(src_mod.clone());
                    }
                }
                // Whole-module imports (IMPORT Json) create Module symbols
                if let SymbolKind::Module { .. } = &sym.kind {
                    if !merged_modules.contains(&sym.name) {
                        merged_modules.push(sym.name.clone());
                    }
                }
            }
        }

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

    // Lower module except handler
    let except_handler = module_except.map(|stmts| {
        let mut hb = HirBuilder::new(
            &sema.types, &sema.symtab, &module_name, &sema.foreign_modules,
        );
        hb.set_imported_modules(imported_modules.clone());
        hb.set_import_alias_map(import_aliases.clone());
        hb.lower_stmts(stmts)
    });

    // Lower module finally handler
    let finally_handler = module_finally.map(|stmts| {
        let mut hb = HirBuilder::new(
            &sema.types, &sema.symtab, &module_name, &sema.foreign_modules,
        );
        hb.set_imported_modules(imported_modules.clone());
        hb.set_import_alias_map(import_aliases.clone());
        hb.lower_stmts(stmts)
    });

    // Build HirEmbeddedModule for each implementation module
    let mut new_embedded_modules = Vec::new();
    for imp in impl_mods {
        let (imp_modules, imp_aliases) = extract_imports(&imp.imports);
        let mut merged_modules = imported_modules.clone();
        merged_modules.extend(imp_modules.clone());
        let mut merged_aliases = import_aliases.clone();
        merged_aliases.extend(imp_aliases.clone());

        let mut emp_const_decls = Vec::new();
        let mut emp_type_decls = Vec::new();
        let mut emp_global_decls = Vec::new();
        let emp_exception_decls = Vec::new();
        let mut emp_proc_decls = Vec::new();

        // Use scoped lookup for this embedded module to avoid TypeId conflicts
        let emp_scope = sema.symtab.lookup_module_scope(&imp.name);
        let emp_lookup = |name: &str| -> Option<&crate::symtab::Symbol> {
            if let Some(scope_id) = emp_scope {
                sema.symtab.lookup_in_scope(scope_id, name)
            } else {
                sema.symtab.lookup_any(name)
            }
        };

        for decl in &imp.block.decls {
            match decl {
                Declaration::Const(c) => {
                    let sym = emp_lookup(&c.name);
                    let val = sym
                        .and_then(|s| match &s.kind {
                            SymbolKind::Constant(cv) => Some(const_value_to_hir(cv)),
                            _ => None,
                        })
                        .unwrap_or(ConstVal::Integer(0));
                    let type_id = sym.map(|s| s.typ).unwrap_or(TY_INTEGER);
                    let exported = sym.map(|s| s.exported).unwrap_or(false);
                    emp_const_decls.push(HirConstDecl {
                        name: c.name.clone(),
                        mangled: format!("{}_{}", imp.name, c.name),
                        value: val.clone(),
                        type_id,
                        exported,
                        c_type: const_val_c_type(&val),
                    });
                }
                Declaration::Type(t) => {
                    let sym = emp_lookup(&t.name);
                    let type_id = sym.map(|s| s.typ).unwrap_or(TY_VOID);
                    emp_type_decls.push(HirTypeDecl {
                        name: t.name.clone(),
                        mangled: format!("{}_{}", imp.name, t.name),
                        type_id,
                        exported: sym.map(|s| s.exported).unwrap_or(false),
                        });
                }
                Declaration::Var(v) => {
                    let type_id = emp_lookup(v.names.first().map(|n| n.as_str()).unwrap_or(""))
                        .map(|s| s.typ).unwrap_or(TY_VOID);
                    for name in &v.names {
                        emp_global_decls.push(HirGlobalDecl {
                            name: name.clone(),
                            mangled: format!("{}_{}", imp.name, name),
                            type_id,
                            exported: false,
                            c_type: String::new(),
                            c_array_suffix: String::new(),
                            is_proc_type: false,
                            });
                    }
                }
                Declaration::Procedure(p) => {
                    let mut pd = build_proc_decl(&p.heading, &p.block.decls, &imp.name, sema, false, None);
                    // Lower the procedure body for LLVM backend consumption
                    if let Some(stmts) = &p.block.body {
                        let mut hb = HirBuilder::new(
                            &sema.types, &sema.symtab, &imp.name, &sema.foreign_modules,
                        );
                        hb.set_imported_modules(merged_modules.clone());
                        hb.set_import_alias_map(merged_aliases.clone());
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
                        pd.body = Some(hb.lower_stmts(stmts));
                    }
                    // Lower except handler if present
                    if let Some(stmts) = &p.block.except {
                        let mut hb = HirBuilder::new(
                            &sema.types, &sema.symtab, &imp.name, &sema.foreign_modules,
                        );
                        hb.set_imported_modules(merged_modules.clone());
                        hb.set_import_alias_map(merged_aliases.clone());
                        hb.enter_procedure_named(&p.heading.name);
                        pd.except_handler = Some(hb.lower_stmts(stmts));
                    }
                    // Build nested proc decls with bodies
                    for nd in &p.block.decls {
                        if let Declaration::Procedure(np) = nd {
                            let mut npd = build_proc_decl(&np.heading, &np.block.decls, &imp.name, sema, true, Some(&p.heading.name));
                            if let Some(stmts) = &np.block.body {
                                let mut nhb = HirBuilder::new(
                                    &sema.types, &sema.symtab, &imp.name, &sema.foreign_modules,
                                );
                                nhb.set_imported_modules(merged_modules.clone());
                                nhb.set_import_alias_map(merged_aliases.clone());
                                nhb.enter_procedure_named(&p.heading.name);
                                nhb.enter_procedure_named(&np.heading.name);
                                for fp in &np.heading.params {
                                    if matches!(fp.typ, ast::TypeNode::OpenArray { .. }) {
                                        for name in &fp.names {
                                            let high_name = format!("{}_high", name);
                                            nhb.register_var(&high_name, TY_INTEGER);
                                            nhb.register_local(&high_name);
                                        }
                                    }
                                }
                                npd.body = Some(nhb.lower_stmts(stmts));
                            }
                            pd.nested_procs.push(npd);
                        }
                    }
                    emp_proc_decls.push(pd);
                }
                _ => {}
            }
        }

        // Also collect exception decls from AST
        if let Some(def_mod) = sema.symtab.lookup_any(&imp.name).and_then(|_| None::<()>) {
            // Exceptions come from definition module — handled separately
            let _ = def_mod;
        }

        let emp_init_body = imp.block.body.as_ref().map(|stmts| {
            let mut hb = HirBuilder::new(
                &sema.types, &sema.symtab, &imp.name, &sema.foreign_modules,
            );
            hb.set_imported_modules(merged_modules.clone());
            hb.set_import_alias_map(merged_aliases.clone());
            hb.lower_stmts(stmts)
        });

        let is_foreign = sema.foreign_modules.contains(&imp.name);

        new_embedded_modules.push(HirEmbeddedModule {
            name: imp.name.clone(),
            is_foreign,
            imports: imp.imports.iter().map(|ii| {
                HirImport {
                    module: ii.from_module.clone().unwrap_or_default(),
                    names: ii.names.iter().map(|n| HirImportName {
                        name: n.name.clone(),
                        local_name: n.local_name().to_string(),
                    }).collect(),
                    is_qualified: ii.from_module.is_none(),
                }
            }).collect(),
            type_decls: emp_type_decls,
            const_decls: emp_const_decls,
            global_decls: emp_global_decls,
            exception_decls: emp_exception_decls,
            procedures: emp_proc_decls,
            init_body: emp_init_body,
            init_cfg: None,
        });
    }

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
        proc_decls: new_proc_decls,
        except_handler,
        finally_handler,
        init_cfg: None,
        local_module_cfgs: Vec::new(),
        finally_cfg: None,
        embedded_modules: new_embedded_modules,
        // Legacy fields (still used by backends)
        constants,
        types: type_decls_legacy,
        globals,
        procedures,
        init_body,
        embedded_init_bodies,
        local_module_inits,
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


    // Populate local declarations using the proc's scope (correctly set by enter_procedure_named)
    let proc_locals = {
        let mut locals = Vec::new();
        for d in &p.block.decls {
            match d {
                Declaration::Var(v) => {
                    let sym = hb.current_scope
                        .and_then(|sid| sema.symtab.lookup_in_scope_direct(sid, &v.names[0]))
                        .filter(|s| matches!(s.kind, SymbolKind::Variable | SymbolKind::Field));
                    if let Some(s) = sym {
                        // Dump for debugging
                        let resolved = {
                            let mut id = s.typ;
                            for _ in 0..50 { match sema.types.get(id) { crate::types::Type::Alias { target, .. } => id = *target, _ => break } }
                            id
                        };
                        for name in &v.names {
                            locals.push(HirLocalDecl::Var { name: name.clone(), type_id: s.typ });
                        }
                    }
                }
                Declaration::Type(t) => {
                    let sym = hb.current_scope
                        .and_then(|sid| sema.symtab.lookup_in_scope_direct(sid, &t.name));
                    if let Some(s) = sym {
                        locals.push(HirLocalDecl::Type { name: t.name.clone(), type_id: s.typ });
                    }
                }
                Declaration::Const(c) => {
                    let sym = hb.current_scope
                        .and_then(|sid| sema.symtab.lookup_in_scope_direct(sid, &c.name));
                    if let Some(s) = sym {
                        if let SymbolKind::Constant(cv) = &s.kind {
                            let val = const_value_to_hir(cv);
                            locals.push(HirLocalDecl::Const(HirConstDecl {
                                name: c.name.clone(),
                                mangled: c.name.clone(),
                                value: val.clone(),
                                type_id: s.typ,
                                exported: false,
                                c_type: const_val_c_type(&val),
                            }));
                        }
                    }
                }
                Declaration::Exception(e) => {
                    locals.push(HirLocalDecl::Exception {
                        name: e.name.clone(),
                        mangled: format!("M2_EXC_{}", e.name),
                        exc_id: 0,
                    });
                }
                _ => {}
            }
        }
        locals
    };

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

    // Build nested procs — enter parent proc scope first so nested scope lookup works
    let mut nested = Vec::new();
    for decl in &p.block.decls {
        if let Declaration::Procedure(np) = decl {
            let mut nhb = HirBuilder::new(
                &sema.types, &sema.symtab, module_name, &sema.foreign_modules,
            );
            nhb.set_imported_modules(imported_modules.to_vec());
            nhb.set_import_alias_map(import_aliases.clone());
            // Enter the parent proc scope so the nested proc's scope can be found as a child
            nhb.enter_procedure_named(&p.heading.name);
            nhb.enter_procedure_named(&np.heading.name);
            // Register nested proc's open array _high companions
            for fp in &np.heading.params {
                if matches!(fp.typ, ast::TypeNode::OpenArray { .. }) {
                    for name in &fp.names {
                        let high_name = format!("{}_high", name);
                        nhb.register_var(&high_name, TY_INTEGER);
                        nhb.register_local(&high_name);
                    }
                }
            }
            let nbody = if let Some(stmts) = &np.block.body {
                Some(nhb.lower_stmts(stmts))
            } else {
                None
            };
            let nparams: Vec<HirParam> = np.heading.params.iter().flat_map(|fp| {
                let is_open = matches!(fp.typ, ast::TypeNode::OpenArray { .. });
                fp.names.iter().map(move |name| HirParam {
                    name: name.clone(),
                    ty: TY_INTEGER,
                    is_var: fp.is_var,
                    is_open_array: is_open,
                })
            }).collect();
            nested.push(HirProc {
                name: SymbolId {
                    mangled: format!("{}_{}", module_name, np.heading.name),
                    source_name: np.heading.name.clone(),
                    module: Some(module_name.to_string()),
                    ty: TY_VOID,
                    is_var_param: false,
                    is_open_array: false,
                },
                params: nparams,
                return_type: None,
                captures: Vec::new(),
                locals: Vec::new(),
                body: nbody,
                nested_procs: Vec::new(),
                is_exported: false,
            });
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
        locals: proc_locals,
        body,
        nested_procs: nested,
        is_exported: false,
    }
}

/// Build a HirProcDecl (sig + locals) from an AST ProcHeading + block declarations.
pub fn build_proc_decl(
    h: &ast::ProcHeading,
    block_decls: &[ast::Declaration],
    module_name: &str,
    sema: &SemanticAnalyzer,
    is_nested: bool,
    parent_proc: Option<&str>,
) -> HirProcDecl {
    // Use scoped lookup for the procedure's module
    let sym = sema.symtab.lookup_module_scope(module_name)
        .and_then(|scope| sema.symtab.lookup_in_scope(scope, &h.name))
        .or_else(|| sema.symtab.lookup_any(&h.name));
    let exported = sym.map(|s| s.exported).unwrap_or(false);
    let return_type_id = h.return_type.as_ref().and_then(|_| {
        sym.and_then(|s| match &s.kind {
            crate::symtab::SymbolKind::Procedure { return_type, .. } => *return_type,
            _ => {
                // Fallback: check if s.typ is a ProcedureType
                match sema.types.get(s.typ) {
                    crate::types::Type::ProcedureType { return_type, .. } => *return_type,
                    _ => None,
                }
            }
        })
    });
    // Get param TypeIds from the procedure's symbol
    let proc_param_types: Vec<crate::symtab::ParamInfo> = sym
        .and_then(|s| match &s.kind {
            crate::symtab::SymbolKind::Procedure { params, .. } => Some(params.clone()),
            _ => None,
        })
        .unwrap_or_default();
    let mut params = Vec::new();
    let mut pi_idx = 0usize;
    for fp in &h.params {
        let is_open = matches!(fp.typ, ast::TypeNode::OpenArray { .. });
        let is_proc = matches!(fp.typ, ast::TypeNode::ProcedureType { .. })
            || matches!(&fp.typ, ast::TypeNode::Named(qi) if qi.module.is_none() && qi.name == "PROC");
        let is_char = matches!(&fp.typ, ast::TypeNode::Named(qi) if qi.name == "CHAR");
        for name in &fp.names {
            let type_id = proc_param_types.get(pi_idx)
                .map(|pi| pi.typ)
                .unwrap_or(TY_INTEGER);
            pi_idx += 1;
            params.push(HirParamDecl {
                name: name.clone(),
                type_id,
                is_var: fp.is_var,
                is_open_array: is_open,
                is_proc_type: is_proc,
                is_char,
                needs_high: is_open,
            });
        }
    }

    HirProcDecl {
        sig: HirProcSig {
            name: h.name.clone(),
            mangled: format!("{}_{}", module_name, h.name),
            module: module_name.to_string(),
            params,
            return_type: return_type_id,
            exported,
            is_foreign: false,
            export_c_name: h.export_c_name.clone(),
            is_nested,
            parent_proc: parent_proc.map(|s| s.to_string()),
            has_closure_env: false,
        },
        body: None,
        locals: {
            // Find the proc scope as a child of the module scope (not by bare name)
            let module_scope = sema.symtab.lookup_module_scope(module_name);
            let proc_scope = module_scope.and_then(|msid| {
                let count = sema.symtab.scope_count();
                // Direct child of module scope
                for id in 0..count {
                    if sema.symtab.scope_name(id) == Some(&h.name)
                        && sema.symtab.scope_parent(id) == Some(msid) {
                        return Some(id);
                    }
                }
                // Grandchild (module → impl scope → proc scope)
                for id in 0..count {
                    if let Some(parent) = sema.symtab.scope_parent(id) {
                        if sema.symtab.scope_name(id) == Some(&h.name)
                            && sema.symtab.scope_parent(parent) == Some(msid) {
                            return Some(id);
                        }
                    }
                }
                // Fallback: any scope with this name
                sema.symtab.lookup_module_scope(&h.name)
            });
            // Look up variables only (not procedures/builtins with same name)
            let lookup_var = |name: &str| -> Option<&crate::symtab::Symbol> {
                let result = proc_scope
                    .and_then(|sid| sema.symtab.lookup_in_scope(sid, name))
                    .or_else(|| sema.symtab.lookup_in_scope_direct(0, name));
                // Only return if it's a variable, not a builtin procedure
                result.filter(|s| matches!(s.kind, SymbolKind::Variable | SymbolKind::Field))
            };
            let lookup = |name: &str| -> Option<&crate::symtab::Symbol> {
                // For types/consts, accept any symbol kind
                proc_scope
                    .and_then(|sid| sema.symtab.lookup_in_scope(sid, name))
                    .or_else(|| sema.symtab.lookup_in_scope_direct(0, name))
            };
            let mut locals = Vec::new();
            for d in block_decls {
                match d {
                    ast::Declaration::Var(v) => {
                        if let Some(sym) = lookup_var(&v.names[0]) {
                            let tid = sym.typ;
                            for name in &v.names {
                                locals.push(HirLocalDecl::Var {
                                    name: name.clone(),
                                    type_id: tid,
                                });
                            }
                        }
                        // If lookup_var fails, the var is not in locals —
                        // gen_proc_decl will fall back to gen_var_decl (AST)
                    }
                    ast::Declaration::Type(t) => {
                        let tid = lookup(&t.name).map(|s| s.typ).unwrap_or(TY_VOID);
                        locals.push(HirLocalDecl::Type {
                            name: t.name.clone(),
                            type_id: tid,
                        });
                    }
                    ast::Declaration::Const(c) => {
                        let sym = lookup(&c.name);
                        let val = sym
                            .and_then(|s| match &s.kind {
                                SymbolKind::Constant(cv) => Some(const_value_to_hir(cv)),
                                _ => None,
                            })
                            .unwrap_or(ConstVal::Integer(0));
                        let tid = sym.map(|s| s.typ).unwrap_or(TY_INTEGER);
                        locals.push(HirLocalDecl::Const(HirConstDecl {
                            name: c.name.clone(),
                            mangled: c.name.clone(),
                            value: val.clone(),
                            type_id: tid,
                            exported: false,
                            c_type: const_val_c_type(&val),
                        }));
                    }
                    ast::Declaration::Exception(e) => {
                        locals.push(HirLocalDecl::Exception {
                            name: e.name.clone(),
                            mangled: format!("M2_EXC_{}", e.name),
                            exc_id: 0,
                        });
                    }
                    _ => {} // Procedure and Module handled separately
                }
            }
            locals
        },
        nested_procs: Vec::new(),
        closure_captures: Vec::new(),
        except_handler: None,
        cfg: None,
        loc: h.loc.clone(),
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
    /// For nested WITH: the parent Place base + projections to chain through.
    /// None for top-level WITH on a real variable.
    parent_base: Option<(PlaceBase, Vec<Projection>)>,
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


mod lower;


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

// ── HIR-based capture analysis ──────────────────────────────────────

/// Collect all variable names referenced in HIR statements.
/// Used for transitive capture propagation.
pub fn collect_hir_var_refs(stmts: &[crate::hir::HirStmt], out: &mut Vec<String>) {
    use crate::hir::*;
    fn collect_from_expr(expr: &HirExpr, out: &mut Vec<String>) {
        match &expr.kind {
            HirExprKind::Place(p) => {
                match &p.base {
                    PlaceBase::Local(sid) | PlaceBase::Global(sid) => {
                        out.push(sid.source_name.clone());
                    }
                    _ => {}
                }
                for proj in &p.projections {
                    if let ProjectionKind::Index(idx) = &proj.kind {
                        collect_from_expr(idx, out);
                    }
                }
            }
            HirExprKind::BinaryOp { left, right, .. } => {
                collect_from_expr(left, out);
                collect_from_expr(right, out);
            }
            HirExprKind::UnaryOp { operand, .. } | HirExprKind::Not(operand) => {
                collect_from_expr(operand, out);
            }
            HirExprKind::DirectCall { args, .. } | HirExprKind::IndirectCall { args, .. } => {
                for arg in args { collect_from_expr(arg, out); }
            }
            _ => {}
        }
    }
    fn collect_from_stmts(stmts: &[HirStmt], out: &mut Vec<String>) {
        for stmt in stmts {
            match &stmt.kind {
                HirStmtKind::Assign { value, .. } => collect_from_expr(value, out),
                HirStmtKind::ProcCall { args, .. } => {
                    for arg in args { collect_from_expr(arg, out); }
                }
                HirStmtKind::If { cond, then_body, elsifs, else_body } => {
                    collect_from_expr(cond, out);
                    collect_from_stmts(then_body, out);
                    for (c, b) in elsifs { collect_from_expr(c, out); collect_from_stmts(b, out); }
                    if let Some(b) = else_body { collect_from_stmts(b, out); }
                }
                HirStmtKind::While { cond, body } => { collect_from_expr(cond, out); collect_from_stmts(body, out); }
                HirStmtKind::Repeat { body, cond } => { collect_from_stmts(body, out); collect_from_expr(cond, out); }
                HirStmtKind::For { body, start, end, .. } => {
                    collect_from_expr(start, out); collect_from_expr(end, out);
                    collect_from_stmts(body, out);
                }
                HirStmtKind::Loop { body } => collect_from_stmts(body, out),
                HirStmtKind::Return { expr } => { if let Some(e) = expr { collect_from_expr(e, out); } }
                _ => {}
            }
        }
    }
    collect_from_stmts(stmts, out);
}

/// Compute captures for a procedure from its HIR body.
/// Replaces AST-walking compute_captures for procs with lowered HIR bodies.
pub fn compute_captures_hir(
    proc_name: &str,
    body: &[crate::hir::HirStmt],
    param_names: &[String],
    local_names: &HashSet<String>,
    outer_vars: &HashMap<String, TypeId>,
    import_map: &HashMap<String, String>,
    imported_modules: &HashSet<String>,
) -> Vec<CapturedVar> {
    let mut all_refs = HashSet::new();
    collect_hir_refs_in_stmts(body, &mut all_refs);

    // Build local set: params + declared locals + proc name itself
    let mut locals = HashSet::new();
    for p in param_names { locals.insert(p.clone()); }
    for l in local_names { locals.insert(l.clone()); }
    locals.insert(proc_name.to_string());

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

    // Auto-capture _high companions
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
    captures.sort_by(|a, b| a.name.cmp(&b.name));
    captures
}

fn collect_hir_refs_in_stmts(stmts: &[crate::hir::HirStmt], out: &mut HashSet<String>) {
    for stmt in stmts { collect_hir_refs_in_stmt(stmt, out); }
}

fn collect_hir_refs_in_stmt(stmt: &crate::hir::HirStmt, out: &mut HashSet<String>) {
    use crate::hir::HirStmtKind::*;
    match &stmt.kind {
        Assign { target, value } => {
            collect_hir_refs_in_place(target, out);
            collect_hir_refs_in_expr(value, out);
        }
        ProcCall { target, args } => {
            match target {
                crate::hir::HirCallTarget::Direct(sid) => { out.insert(sid.source_name.clone()); }
                crate::hir::HirCallTarget::Indirect(expr) => collect_hir_refs_in_expr(expr, out),
            }
            for a in args { collect_hir_refs_in_expr(a, out); }
        }
        If { cond, then_body, elsifs, else_body } => {
            collect_hir_refs_in_expr(cond, out);
            collect_hir_refs_in_stmts(then_body, out);
            for (c, b) in elsifs {
                collect_hir_refs_in_expr(c, out);
                collect_hir_refs_in_stmts(b, out);
            }
            if let Some(eb) = else_body { collect_hir_refs_in_stmts(eb, out); }
        }
        Case { expr, branches, else_body } => {
            collect_hir_refs_in_expr(expr, out);
            for branch in branches { collect_hir_refs_in_stmts(&branch.body, out); }
            if let Some(eb) = else_body { collect_hir_refs_in_stmts(eb, out); }
        }
        While { cond, body } => {
            collect_hir_refs_in_expr(cond, out);
            collect_hir_refs_in_stmts(body, out);
        }
        Repeat { body, cond } => {
            collect_hir_refs_in_stmts(body, out);
            collect_hir_refs_in_expr(cond, out);
        }
        For { var, start, end, step, body, .. } => {
            out.insert(var.clone());
            collect_hir_refs_in_expr(start, out);
            collect_hir_refs_in_expr(end, out);
            if let Some(s) = step { collect_hir_refs_in_expr(s, out); }
            collect_hir_refs_in_stmts(body, out);
        }
        Loop { body } => collect_hir_refs_in_stmts(body, out),
        Return { expr } => {
            if let Some(e) = expr { collect_hir_refs_in_expr(e, out); }
        }
        Exit | Retry => {}
        Try { body, excepts, finally_body } => {
            collect_hir_refs_in_stmts(body, out);
            for ec in excepts { collect_hir_refs_in_stmts(&ec.body, out); }
            if let Some(fb) = finally_body { collect_hir_refs_in_stmts(fb, out); }
        }
        Lock { mutex, body } => {
            collect_hir_refs_in_expr(mutex, out);
            collect_hir_refs_in_stmts(body, out);
        }
        TypeCase { expr, branches, else_body } => {
            collect_hir_refs_in_expr(expr, out);
            for branch in branches { collect_hir_refs_in_stmts(&branch.body, out); }
            if let Some(eb) = else_body { collect_hir_refs_in_stmts(eb, out); }
        }
        Raise { expr } => {
            if let Some(e) = expr { collect_hir_refs_in_expr(e, out); }
        }
        _ => {} // Empty, etc.
    }
}

fn collect_hir_refs_in_expr(expr: &crate::hir::HirExpr, out: &mut HashSet<String>) {
    use crate::hir::HirExprKind::*;
    match &expr.kind {
        IntLit(_) | RealLit(_) | StringLit(_) | CharLit(_) | BoolLit(_) | NilLit => {}
        Place(place) => collect_hir_refs_in_place(place, out),
        AddrOf(place) => collect_hir_refs_in_place(place, out),
        DirectCall { target, args } => {
            out.insert(target.source_name.clone());
            for a in args { collect_hir_refs_in_expr(a, out); }
        }
        IndirectCall { callee, args } => {
            collect_hir_refs_in_expr(callee, out);
            for a in args { collect_hir_refs_in_expr(a, out); }
        }
        UnaryOp { operand, .. } => collect_hir_refs_in_expr(operand, out),
        BinaryOp { left, right, .. } => {
            collect_hir_refs_in_expr(left, out);
            collect_hir_refs_in_expr(right, out);
        }
        SetConstructor { elements } => {
            for elem in elements {
                match elem {
                    crate::hir::HirSetElement::Single(e) => collect_hir_refs_in_expr(e, out),
                    crate::hir::HirSetElement::Range(lo, hi) => {
                        collect_hir_refs_in_expr(lo, out);
                        collect_hir_refs_in_expr(hi, out);
                    }
                }
            }
        }
        Not(e) | Deref(e) | TypeTransfer(e) => collect_hir_refs_in_expr(e, out),
    }
}

fn collect_hir_refs_in_place(place: &crate::hir::Place, out: &mut HashSet<String>) {
    match &place.base {
        crate::hir::PlaceBase::Local(sid) | crate::hir::PlaceBase::Global(sid) => {
            out.insert(sid.source_name.clone());
        }
        crate::hir::PlaceBase::FuncRef(sid) => {
            out.insert(sid.source_name.clone());
        }
        _ => {}
    }
    for proj in &place.projections {
        if let crate::hir::ProjectionKind::Index(idx) = &proj.kind {
            collect_hir_refs_in_expr(idx, out);
        }
    }
}


/// Map a ConstVal to its C type string.
pub fn const_val_c_type(val: &ConstVal) -> String {
    match val {
        ConstVal::Integer(_) | ConstVal::EnumVariant(_) => "int32_t".to_string(),
        ConstVal::Real(_) => "float".to_string(),
        ConstVal::Boolean(_) => "int".to_string(),
        ConstVal::Char(_) => "char".to_string(),
        ConstVal::String(s) if s.len() <= 1 => "char".to_string(),
        ConstVal::String(_) => "const char *".to_string(),
        ConstVal::Set(_) => "uint64_t".to_string(),
        ConstVal::Nil => "void *".to_string(),
    }
}

/// Convert a symtab ConstValue to an HIR ConstVal.
pub fn const_value_to_hir(cv: &ConstValue) -> ConstVal {
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
