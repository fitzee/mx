use super::*;
use crate::hir;
use crate::hir_build::HirBuilder;

impl CodeGen {
    // ── HIR integration ─────────────────────────────────────────────

    /// Build an HirBuilder from the current C codegen state.
    pub(crate) fn make_hir_builder(&self) -> HirBuilder<'_> {
        let mut hb = HirBuilder::new(
            &self.sema.types,
            &self.sema.symtab,
            &self.module_name,
            &self.sema.foreign_modules,
        );
        hb.set_import_alias_map(self.import_alias_map.clone());
        hb.set_imported_modules(self.imported_modules.iter().cloned().collect());
        // Register variable types from sema (C backend's var_types has C type
        // strings; we need TypeIds from symtab for the HIR builder)
        for name in self.var_types.keys() {
            if let Some(sym) = self.sema.symtab.lookup_any(name) {
                hb.register_var(name, sym.typ);
            } else if name.ends_with("_high") {
                // Synthetic _high companion for open array params
                hb.register_var(name, crate::types::TY_INTEGER);
                hb.register_local(name);
            }
        }
        // Mirror WITH stack
        for (record_var, _fields, with_type) in &self.with_aliases {
            // Resolve type: prefer scope-aware lookup (avoids shadowing),
            // then type name string (for nested WITH on field names).
            let tid = hb.scope_lookup_type(record_var)
                .or_else(|| {
                    with_type.as_ref().and_then(|tn| {
                        self.sema.symtab.find_type(tn)
                    })
                });
            if let Some(tid) = tid {
                hb.push_with(record_var, tid);
            }
        }
        if !self.parent_proc_stack.is_empty() {
            if let Some(proc_name) = self.parent_proc_stack.last() {
                hb.enter_procedure_named(proc_name);
            }
        }
        hb
    }

    /// Convert an HIR expression to a C expression string (for index exprs etc.)
    fn hir_expr_to_c_string(&mut self, expr: &hir::HirExpr) -> String {
        use crate::hir::HirExprKind::*;
        match &expr.kind {
            IntLit(v) => format!("{}", v),
            CharLit(c) => format!("{}", *c as u32),
            BoolLit(b) => if *b { "1".into() } else { "0".into() },
            Place(place) => self.emit_place_c(place),
            BinaryOp { op, left, right } => {
                let l = self.hir_expr_to_c_string(left);
                let r = self.hir_expr_to_c_string(right);
                let c_op = match op {
                    crate::ast::BinaryOp::Add => "+",
                    crate::ast::BinaryOp::Sub => "-",
                    crate::ast::BinaryOp::Mul => "*",
                    crate::ast::BinaryOp::IntDiv => "/",
                    crate::ast::BinaryOp::Mod => "%",
                    _ => "?",
                };
                format!("({} {} {})", l, c_op, r)
            }
            UnaryOp { op: crate::ast::UnaryOp::Neg, operand } => {
                format!("(-{})", self.hir_expr_to_c_string(operand))
            }
            TypeTransfer(arg) => {
                // Type cast — for index expressions, just emit the inner value
                self.hir_expr_to_c_string(arg)
            }
            _ => format!("/* HIR expr */0"),
        }
    }

    /// Convert an HIR Place to a C expression string.
    pub(crate) fn emit_place_c(&mut self, place: &hir::Place) -> String {
        use hir::*;

        let base_name = match &place.base {
            PlaceBase::Local(sid) => {
                if self.is_env_var(&sid.source_name) {
                    format!("(*_env->{})", sid.source_name)
                } else if self.is_var_param(&sid.source_name) {
                    format!("(*{})", self.mangle(&sid.source_name))
                } else {
                    self.mangle(&sid.source_name)
                }
            }
            PlaceBase::Global(sid) => {
                let name = &sid.source_name;
                if self.is_env_var(name) {
                    format!("(*_env->{})", name)
                } else if self.is_var_param(name) {
                    format!("(*{})", self.mangle(name))
                } else if self.embedded_local_vars.contains(name)
                    || self.embedded_local_procs.contains(name)
                {
                    format!("{}_{}", self.module_name, name)
                } else if let Some(module) = sid.module.as_ref()
                    .filter(|m| m.as_str() != self.module_name)
                    .cloned()
                    .or_else(|| self.import_map.get(name).cloned()) {
                    // Imported from another module: Module_Name
                    let orig = self.original_import_name(name).to_string();
                    if self.foreign_modules.contains(module.as_str()) {
                        self.mangle(&orig)
                    } else if stdlib::is_stdlib_module(&module) && !stdlib::is_native_stdlib(&module) {
                        if let Some(c_name) = stdlib::map_stdlib_call(&module, &orig) {
                            c_name
                        } else {
                            self.mangle(name)
                        }
                    } else {
                        // For native stdlib, normalize to definition case
                        let canonical = if stdlib::is_native_stdlib(&module) {
                            self.resolve_native_stdlib_name(&module, &orig)
                        } else {
                            orig
                        };
                        format!("{}_{}", module, canonical)
                    }
                } else {
                    self.mangle(name)
                }
            }
            PlaceBase::Constant(cv) => {
                let base_str = match cv {
                    ConstVal::Integer(v) => format!("{}", v),
                    ConstVal::Real(v) => format!("{:e}", v),
                    ConstVal::Boolean(b) => if *b { "1".into() } else { "0".into() },
                    ConstVal::Char(c) => format!("'{}'", c),
                    ConstVal::String(s) => format!("\"{}\"", s),
                    ConstVal::Set(v) => format!("{}u", v),
                    ConstVal::Nil => "NULL".into(),
                    ConstVal::EnumVariant(v) => format!("{}", v),
                };
                // Apply projections (e.g., string constant indexing: "ABC"[i])
                if place.projections.is_empty() {
                    return base_str;
                }
                let mut result = base_str;
                for proj in &place.projections {
                    if let ProjectionKind::Index(idx_expr) = &proj.kind {
                        let idx_str = self.hir_expr_to_c_string(idx_expr);
                        result.push('[');
                        result.push_str(&idx_str);
                        result.push(']');
                    }
                }
                return result;
            }
            PlaceBase::FuncRef(sid) => {
                return self.resolve_func_ref_name(sid);
            }
        };

        if place.projections.is_empty() {
            return base_name;
        }

        let mut result = base_name;
        let mut i = 0;
        while i < place.projections.len() {
            let proj = &place.projections[i];
            match &proj.kind {
                ProjectionKind::Field { name, .. } => {
                    result.push('.');
                    result.push_str(name);
                }
                ProjectionKind::VariantField { variant_index, name, .. } => {
                    result.push_str(&format!(".variant.v{}.{}", variant_index, name));
                }
                ProjectionKind::Index(idx_expr) => {
                    let idx_str = self.hir_expr_to_c_string(idx_expr);
                    result.push('[');
                    result.push_str(&idx_str);
                    result.push(']');
                }
                ProjectionKind::Deref => {
                    // Optimize Deref+Field → ->field
                    if i + 1 < place.projections.len() {
                        // Deref+Index: check if this is ADDRESS byte access
                        // (Deref ty=Char, no further projections) vs array access
                        // (Deref ty=Array, further field projections possible)
                        if let ProjectionKind::Index(idx_expr) = &place.projections[i + 1].kind {
                            let idx_str = self.hir_expr_to_c_string(idx_expr);
                            if proj.ty == crate::types::TY_CHAR && i + 2 >= place.projections.len() {
                                // ADDRESS^[i] — byte access, cast to char*
                                result = format!("((char*){})[{}]", result, idx_str);
                            } else {
                                // POINTER TO ARRAY^[i] — normal array deref+index
                                result = format!("(*{})[{}]", result, idx_str);
                            }
                            i += 2;
                            continue;
                        }
                        if let ProjectionKind::Field { name, .. } = &place.projections[i + 1].kind {
                            result.push_str("->");
                            result.push_str(name);
                            i += 2;
                            continue;
                        }
                        if let ProjectionKind::VariantField { variant_index, name, .. } = &place.projections[i + 1].kind {
                            result.push_str(&format!("->variant.v{}.{}", variant_index, name));
                            i += 2;
                            continue;
                        }
                    }
                    result = format!("(*{})", result);
                }
            }
            i += 1;
        }
        result
    }

    /// Resolve an enum variant through a module's re-exports.
    /// When module M re-exports a type from module S (e.g., Promise re-exports Status from Scheduler),
    /// a reference like M.OK needs to resolve to S_Status_OK via S_OK in enum_variants.
    pub(crate) fn resolve_reexported_enum_variant(&self, module: &str, name: &str) -> Option<String> {
        if let Some(def_mod) = self.def_modules.get(module) {
            for imp in &def_mod.imports {
                if let Some(ref from_mod) = imp.from_module {
                    // Check if source_module has this name as an enum variant
                    let source_key = format!("{}_{}", from_mod, name);
                    if let Some(c_name) = self.enum_variants.get(&source_key) {
                        return Some(c_name.clone());
                    }
                }
            }
        }
        None
    }

    /// Resolve an import name to the canonical case used in the native
    /// stdlib .def/.mod file. PIM4 is case-sensitive, but users may import
    /// with different casing (e.g., `Entier` when .def has `entier`).
    /// Returns the .def name if a case-insensitive match is found.
    pub(crate) fn resolve_native_stdlib_name(&self, module: &str, import_name: &str) -> String {
        // Search the module's sema scope for a case-insensitive procedure name match
        if let Some(scope_id) = self.sema.symtab.lookup_module_scope(module) {
            let lower = import_name.to_ascii_lowercase();
            for sym in self.sema.symtab.symbols_in_scope(scope_id) {
                if matches!(sym.kind, crate::symtab::SymbolKind::Procedure { .. })
                    && sym.name.to_ascii_lowercase() == lower
                {
                    return sym.name.clone();
                }
            }
        }
        import_name.to_string()
    }

    /// Mangle a variable name for declaration: module-prefix if it's a
    /// module-level embedded var, but NOT if we're inside a procedure
    /// (procedure locals shadow module-level names).
    pub(crate) fn mangle_decl_name(&self, name: &str) -> String {
        if self.parent_proc_stack.is_empty() && self.embedded_local_vars.contains(name) {
            format!("{}_{}", self.module_name, name)
        } else {
            self.mangle(name)
        }
    }

    /// Resolve a FuncRef SymbolId to the correct C function name.
    /// Handles: current module (no prefix), embedded modules (Module_Name),
    /// native stdlib (m2_Module_Name), foreign modules (bare name).
    pub(crate) fn resolve_func_ref_name(&self, sid: &crate::hir::SymbolId) -> String {
        let orig = self.original_import_name(&sid.source_name);
        // Check nested proc mangled names first
        if let Some(mangled) = self.nested_proc_names.get(&sid.source_name) {
            return mangled.clone();
        }
        if let Some(ref module) = sid.module {
            if module == &self.module_name && !self.embedded_local_procs.contains(&sid.source_name) {
                return self.mangle(&sid.source_name);
            }
            if self.foreign_modules.contains(module.as_str()) {
                return orig.to_string();
            }
            if crate::stdlib::is_native_stdlib(module) {
                if let Some(c_name) = crate::stdlib::map_stdlib_call(module, orig) {
                    return c_name;
                }
                return format!("{}_{}", module, orig);
            }
            if crate::stdlib::is_stdlib_module(module) && !crate::stdlib::is_native_stdlib(module) {
                if let Some(c_name) = crate::stdlib::map_stdlib_call(module, orig) {
                    return c_name;
                }
            }
            return format!("{}_{}", module, orig);
        }
        self.mangle(&sid.source_name)
    }

    pub(crate) fn mangle(&self, name: &str) -> String {
        match name {
            // Modula-2 built-in constants
            "NIL" => "NULL".to_string(),
            "TRUE" => "1".to_string(),
            "FALSE" => "0".to_string(),
            // Avoid clashing with m2_ runtime prefix
            _ if name.starts_with("m2_") => format!("m2v_{}", &name[3..]),
            // C keywords and standard library names
            _ if C_RESERVED.contains(name) => format!("m2_{}", name),
            _ => name.to_string(),
        }
    }

}
