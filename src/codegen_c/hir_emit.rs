//! C backend emission from HIR expressions and statements.
//!
//! This replaces the AST-walking `gen_expr` and `gen_statement` with a
//! single path through the HIR. Designator resolution, open array expansion,
//! WITH elimination, and constant folding are all handled upstream by the
//! HIR builder — this module just emits C text.

use crate::hir::*;
use crate::types::*;
use crate::ast::{BinaryOp, UnaryOp};

impl super::CodeGen {
    // ── HIR Expression Emission ─────────────────────────────────────

    /// Emit a C expression string from an HIR expression.
    pub(crate) fn emit_hir_expr(&mut self, expr: &HirExpr) {
        let s = self.hir_expr_to_string(expr);
        self.emit(&s);
    }

    /// Convert an HIR expression to a C string (non-emitting).
    pub(crate) fn hir_expr_to_string(&mut self, expr: &HirExpr) -> String {
        match &expr.kind {
            HirExprKind::IntLit(v) => format!("{}", v),
            HirExprKind::RealLit(v) => {
                let s = format!("{}", v);
                if s.contains('.') || s.contains('e') || s.contains('E') { s }
                else { format!("{}.0", s) }
            }
            HirExprKind::StringLit(s) => {
                if expr.ty == TY_CHAR {
                    if s.is_empty() {
                        "'\\0'".to_string()
                    } else {
                        format!("'{}'", super::escape_c_char(s.chars().next().unwrap()))
                    }
                } else {
                    format!("\"{}\"", super::escape_c_string(s))
                }
            }
            HirExprKind::CharLit(c) => {
                format!("'{}'", super::escape_c_char(*c))
            }
            HirExprKind::BoolLit(b) => {
                if *b { "1".to_string() } else { "0".to_string() }
            }
            HirExprKind::NilLit => "NULL".to_string(),

            HirExprKind::Place(place) => {
                self.emit_place_c(place)
            }

            HirExprKind::AddrOf(place) => {
                let place_str = self.emit_place_c(place);
                format!("&{}", place_str)
            }

            HirExprKind::DirectCall { target, args } => {
                let name = &target.source_name;
                // Builtin handling
                if crate::builtins::is_builtin_proc(name) {
                    // ADR on open array / VAR open array: the param is already
                    // a pointer in C, so emit (void *)arg, not (void *)&(arg).
                    if name == "ADR" && args.len() == 1 {
                        if let HirExprKind::Place(ref place) = args[0].kind {
                            // Bare open array param: already a pointer, skip &
                            let is_bare_open = place.projections.is_empty()
                                && match &place.base {
                                    PlaceBase::Local(sid) | PlaceBase::Global(sid) =>
                                        sid.is_open_array,
                                    _ => false,
                                };
                            if is_bare_open {
                                let s = self.hir_expr_to_string(&args[0]);
                                return format!("((void *)({}))", s);
                            }
                        }
                    }
                    let arg_strs: Vec<String> = args.iter()
                        .enumerate()
                        .map(|(idx, a)| {
                            if (name == "TSIZE" || name == "SIZE") && idx == 0 {
                                let s = self.hir_expr_to_string(a);
                                self.mangle_type_name(&s)
                            } else {
                                self.hir_expr_to_string(a)
                            }
                        })
                        .collect();
                    return crate::builtins::codegen_builtin(name, &arg_strs);
                }
                // Regular function call
                let c_name = self.resolve_call_name(target);
                let orig_name = self.original_import_name(&target.source_name);
                let native_module = target.module.as_ref()
                    .filter(|m| crate::stdlib::is_native_stdlib(m)
                        && crate::stdlib::map_stdlib_call(m, orig_name).is_some());
                let arg_strs = if let Some(module) = native_module {
                    // Native stdlib: strip _high companions (the inline C
                    // functions don't take open array high params)
                    let m = module.clone();
                    let p = orig_name.to_string();
                    self.hir_args_for_native_stdlib(args, &c_name, &m, &p)
                } else {
                    self.hir_args_to_string(args, &c_name)
                };
                format!("{}({})", c_name, arg_strs)
            }

            HirExprKind::IndirectCall { callee, args } => {
                let callee_str = self.hir_expr_to_string(callee);
                let arg_strs: Vec<String> = args.iter()
                    .map(|a| self.hir_expr_to_string(a))
                    .collect();
                format!("{}({})", callee_str, arg_strs.join(", "))
            }

            HirExprKind::UnaryOp { op, operand } => {
                let inner = self.hir_expr_to_string(operand);
                match op {
                    UnaryOp::Neg => format!("(-{})", inner),
                    UnaryOp::Pos => inner,
                }
            }

            HirExprKind::BinaryOp { op, left, right } => {
                // For comparisons, coerce single-char strings to scalar
                let is_cmp = matches!(op, BinaryOp::Eq | BinaryOp::Ne
                    | BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge);
                let l = if is_cmp { self.hir_expr_to_scalar_string(left) }
                        else { self.hir_expr_to_string(left) };
                let r = if is_cmp { self.hir_expr_to_scalar_string(right) }
                        else { self.hir_expr_to_string(right) };
                self.emit_binary_op_c(op, &l, &r, left, right)
            }

            HirExprKind::Not(operand) => {
                let inner = self.hir_expr_to_string(operand);
                format!("(!{})", inner)
            }

            HirExprKind::Deref(operand) => {
                let inner = self.hir_expr_to_string(operand);
                format!("(*{})", inner)
            }

            HirExprKind::SetConstructor { elements } => {
                if elements.is_empty() {
                    "0u".to_string()
                } else {
                    let parts: Vec<String> = elements.iter().map(|e| {
                        match e {
                            HirSetElement::Single(expr) => {
                                let v = self.hir_expr_to_string(expr);
                                format!("(1u << {})", v)
                            }
                            HirSetElement::Range(lo, hi) => {
                                let l = self.hir_expr_to_string(lo);
                                let h = self.hir_expr_to_string(hi);
                                format!("m2_set_range({}, {})", l, h)
                            }
                        }
                    }).collect();
                    format!("({})", parts.join(" | "))
                }
            }

            HirExprKind::TypeTransfer(inner) => {
                let inner_str = self.hir_expr_to_string(inner);
                let target_c = self.type_id_to_c(expr.ty);
                format!("(({})({}))", target_c, inner_str)
            }
        }
    }

    /// Emit a binary operation as C.
    fn emit_binary_op_c(
        &self, op: &BinaryOp, l: &str, r: &str,
        left: &HirExpr, right: &HirExpr,
    ) -> String {
        match op {
            BinaryOp::IntDiv => {
                if self.is_hir_unsigned(left) || self.is_hir_unsigned(right) {
                    format!("({} / {})", l, r)
                } else if self.is_hir_long(left) || self.is_hir_long(right) {
                    format!("m2_div64({}, {})", l, r)
                } else {
                    format!("m2_div({}, {})", l, r)
                }
            }
            BinaryOp::Mod => {
                if self.is_hir_unsigned(left) || self.is_hir_unsigned(right) {
                    format!("({} % {})", l, r)
                } else if self.is_hir_long(left) || self.is_hir_long(right) {
                    format!("m2_mod64({}, {})", l, r)
                } else {
                    format!("m2_mod({}, {})", l, r)
                }
            }
            BinaryOp::RealDiv => {
                format!("((double)({}) / (double)({}))", l, r)
            }
            BinaryOp::In => {
                format!("(({} >> {}) & 1)", r, l)
            }
            // Comparison and logical — no outer parens to avoid -Wparentheses-equality
            BinaryOp::Eq => format!("{} == {}", l, r),
            BinaryOp::Ne => format!("{} != {}", l, r),
            BinaryOp::Lt => format!("{} < {}", l, r),
            BinaryOp::Le => format!("{} <= {}", l, r),
            BinaryOp::Gt => format!("{} > {}", l, r),
            BinaryOp::Ge => format!("{} >= {}", l, r),
            BinaryOp::And => format!("{} && {}", l, r),
            BinaryOp::Or => format!("{} || {}", l, r),
            // Arithmetic — wrap in parens for precedence
            BinaryOp::Add => format!("({} + {})", l, r),
            BinaryOp::Sub => format!("({} - {})", l, r),
            BinaryOp::Mul => format!("({} * {})", l, r),
        }
    }

    /// Check if an HIR expression is unsigned (CARDINAL/LONGCARD).
    fn is_hir_unsigned(&self, expr: &HirExpr) -> bool {
        expr.ty == TY_CARDINAL || expr.ty == TY_LONGCARD
    }

    /// Check if an HIR expression is 64-bit.
    fn is_hir_long(&self, expr: &HirExpr) -> bool {
        expr.ty == TY_LONGINT || expr.ty == TY_LONGCARD
    }

    /// Adapt HIR-expanded args for native stdlib C inline functions.
    /// The HIR adds _high companions for ALL open array params, but native
    /// C functions only need _high for VAR (writable) open array params.
    /// Non-VAR open arrays (read-only sources) use strlen/implicit size.
    fn hir_args_for_native_stdlib(&mut self, args: &[HirExpr], _proc_name: &str, module: &str, proc: &str) -> String {
        let params = crate::stdlib::get_stdlib_proc_params(module, proc)
            .unwrap_or_default();
        let mut result = Vec::new();
        let mut hir_idx = 0;
        for param in &params {
            if hir_idx >= args.len() { break; }
            let (_name, is_var, _is_char, is_open) = param;
            let mut s = self.hir_expr_to_string(&args[hir_idx]);
            // Open array params: strip & (arrays decay to pointers in C)
            if *is_open && s.starts_with('&') {
                s = s[1..].to_string();
            }
            // ALLOCATE/DEALLOCATE: cast first arg to (void **)
            let is_alloc = hir_idx == 0 && (proc.eq_ignore_ascii_case("ALLOCATE") || proc.eq_ignore_ascii_case("DEALLOCATE"));
            if is_alloc && s.starts_with('&') {
                result.push(format!("(void **){}", s));
            } else {
                result.push(s);
            }
            hir_idx += 1;
            if *is_open && hir_idx < args.len() {
                if *is_var {
                    // VAR open array: keep _high (destination buffer needs size)
                    result.push(self.hir_expr_to_string(&args[hir_idx]));
                }
                // Non-VAR open array: skip _high (source, C uses strlen)
                hir_idx += 1;
            }
        }
        while hir_idx < args.len() {
            result.push(self.hir_expr_to_string(&args[hir_idx]));
            hir_idx += 1;
        }
        result.join(", ")
    }

    /// Convert HIR call args to a comma-separated C string.
    fn hir_args_to_string(&mut self, args: &[HirExpr], proc_name: &str) -> String {
        args.iter().enumerate().map(|(idx, a)| {
            // ALLOCATE/DEALLOCATE: cast first arg to (void **)
            let is_alloc = idx == 0 && (proc_name.ends_with("ALLOCATE") || proc_name.ends_with("DEALLOCATE"));
            let s = self.hir_expr_to_string(a);
            if is_alloc && s.starts_with('&') {
                format!("(void **){}", s)
            } else {
                s
            }
        }).collect::<Vec<_>>().join(", ")
    }

    /// Resolve a call target SymbolId to a C function name.
    fn resolve_call_name(&self, target: &SymbolId) -> String {
        let orig = self.original_import_name(&target.source_name);
        if let Some(ref module) = target.module {
            // Same module AND it's the main module (not embedded): no prefix
            if module == &self.module_name && !self.embedded_local_procs.contains(&target.source_name) {
                return self.mangle(&target.source_name);
            }
            if self.foreign_modules.contains(module.as_str()) {
                return orig.to_string();
            }
            if crate::stdlib::is_stdlib_module(module) && !crate::stdlib::is_native_stdlib(module) {
                if let Some(c_name) = crate::stdlib::map_stdlib_call(module, orig) {
                    return c_name;
                }
            }
            if crate::stdlib::is_native_stdlib(module) {
                if let Some(c_name) = crate::stdlib::map_stdlib_call(module, orig) {
                    return c_name;
                }
                return format!("{}_{}", module, orig);
            }
            format!("{}_{}", module, orig)
        } else {
            self.mangle(&target.source_name)
        }
    }

    /// Map a TypeId to a C type string (for casts).
    pub(crate) fn type_id_to_c(&self, tid: TypeId) -> String {
        match tid {
            TY_INTEGER => "int32_t".to_string(),
            TY_CARDINAL => "uint32_t".to_string(),
            TY_REAL => "float".to_string(),
            TY_LONGREAL => "double".to_string(),
            TY_BOOLEAN => "int".to_string(),
            TY_CHAR => "char".to_string(),
            TY_BITSET => "uint32_t".to_string(),
            TY_ADDRESS => "void *".to_string(),
            TY_LONGINT => "int64_t".to_string(),
            TY_LONGCARD => "uint64_t".to_string(),
            TY_WORD => "uint32_t".to_string(),
            TY_BYTE => "uint8_t".to_string(),
            TY_VOID => "void".to_string(),
            _ => {
                let resolved = self.resolve_hir_alias(tid);
                // Try to find a source name and resolve it context-dependently
                // (same logic as named_type_to_c: import_map, embedded_enum_types, etc.)
                let source_name = self.type_source_name(resolved)
                    .or_else(|| self.type_source_name(tid));
                if let Some(name) = source_name {
                    let qi = crate::ast::QualIdent {
                        name, module: None,
                        loc: crate::errors::SourceLoc::default(),
                    };
                    return self.named_type_to_c(&qi);
                }
                // Structural type fallback (no named source — use structure)
                match self.sema.types.get(resolved) {
                    crate::types::Type::Pointer { base } => {
                        // If base is a proc type, can't just append " *" — use void *
                        if matches!(self.sema.types.get(self.resolve_hir_alias(*base)),
                            crate::types::Type::ProcedureType { .. }) {
                            "void *".to_string()
                        } else {
                            format!("{} *", self.type_id_to_c(*base))
                        }
                    }
                    crate::types::Type::Array { elem_type, .. } => self.type_id_to_c(*elem_type),
                    crate::types::Type::OpenArray { elem_type } => self.type_id_to_c(*elem_type),
                    crate::types::Type::Set { .. } | crate::types::Type::Bitset => "uint32_t".to_string(),
                    crate::types::Type::Subrange { .. } => "int32_t".to_string(),
                    crate::types::Type::Ref { .. } | crate::types::Type::RefAny
                    | crate::types::Type::Object { .. } => "void *".to_string(),
                    crate::types::Type::ProcedureType { params, return_type } => {
                        let ret = match return_type {
                            Some(rt) => self.type_id_to_c(*rt),
                            None => "void".to_string(),
                        };
                        let param_strs: Vec<String> = if params.is_empty() {
                            vec!["void".to_string()]
                        } else {
                            params.iter().map(|p| {
                                let pt = self.type_id_to_c(p.typ);
                                if p.is_var { format!("{} *", pt) } else { pt }
                            }).collect()
                        };
                        format!("{} (*)({})", ret, param_strs.join(", "))
                    }
                    crate::types::Type::Record { .. } => "int32_t".to_string(),
                    crate::types::Type::Complex => "float _Complex".to_string(),
                    crate::types::Type::LongComplex => "double _Complex".to_string(),
                    crate::types::Type::Opaque { .. } => "void *".to_string(),
                    crate::types::Type::StringLit(_) => "const char *".to_string(),
                    crate::types::Type::Nil => "void *".to_string(),
                    crate::types::Type::Address => "void *".to_string(),
                    _ => "int32_t".to_string(),
                }
            }
        }
    }

    /// Compute the C array dimension suffix from a TypeId (e.g., "[256]" or "[32][64]").
    pub(crate) fn type_id_array_suffix(&self, tid: TypeId) -> String {
        let resolved = self.resolve_hir_alias(tid);
        match self.sema.types.get(resolved) {
            crate::types::Type::Array { elem_type, high, .. } => {
                let size = format!("[{} + 1]", high);
                let inner = self.type_id_array_suffix(*elem_type);
                format!("{}{}", size, inner)
            }
            _ => String::new(),
        }
    }

    /// Generate a C function pointer declaration from TypeId: `RetType (*name)(params)`
    pub(crate) fn proc_type_decl_from_id(&self, tid: TypeId, name: &str, is_ptr: bool) -> String {
        let resolved = self.resolve_hir_alias(tid);
        match self.sema.types.get(resolved) {
            crate::types::Type::ProcedureType { params, return_type } => {
                let ret = match return_type {
                    Some(rt) => self.type_id_to_c(*rt),
                    None => "void".to_string(),
                };
                let star = if is_ptr { "**" } else { "*" };
                let param_strs: Vec<String> = if params.is_empty() {
                    vec!["void".to_string()]
                } else {
                    params.iter().flat_map(|p| {
                        let is_open = matches!(self.sema.types.get(p.typ), crate::types::Type::OpenArray { .. });
                        let pt = self.type_id_to_c(p.typ);
                        if is_open {
                            vec![format!("{} *", pt), "uint32_t".to_string()]
                        } else if p.is_var {
                            vec![format!("{} *", pt)]
                        } else {
                            vec![pt]
                        }
                    }).collect()
                };
                format!("{} ({}{})({})", ret, star, name, param_strs.join(", "))
            }
            _ => {
                let ctype = self.type_id_to_c(tid);
                if is_ptr { format!("{} *{}", ctype, name) } else { format!("{} {}", ctype, name) }
            }
        }
    }

    // ── HIR Statement Emission ──────────────────────────────────────

    /// Emit a C statement from an HIR statement.
    pub(crate) fn emit_hir_stmt(&mut self, stmt: &HirStmt) {
        self.emit_line_directive(&stmt.loc);
        match &stmt.kind {
            HirStmtKind::Empty => {}

            HirStmtKind::Assign { target, value } => {
                let target_str = self.emit_place_c(target);
                // Coerce single-char string to char for CHAR targets
                let value_str = if target.ty == TY_CHAR {
                    // Check for string literal (direct or via constant)
                    let str_val = match &value.kind {
                        HirExprKind::StringLit(s) => Some(s.clone()),
                        HirExprKind::Place(p) if p.projections.is_empty() => {
                            if let PlaceBase::Constant(ConstVal::String(s)) = &p.base {
                                Some(s.clone())
                            } else { None }
                        }
                        _ => None,
                    };
                    if let Some(s) = str_val {
                        if s.is_empty() {
                            "'\\0'".to_string()
                        } else if s.len() == 1 {
                            format!("'{}'", super::escape_c_char(s.chars().next().unwrap()))
                        } else {
                            self.hir_expr_to_string(value)
                        }
                    } else {
                        self.hir_expr_to_string(value)
                    }
                } else {
                    self.hir_expr_to_string(value)
                };

                // Array/record assignment: use direct assign for records
                // (C supports struct assignment), memcpy for arrays.
                let resolved = self.resolve_hir_alias(target.ty);
                let is_array = matches!(self.sema.types.get(resolved), crate::types::Type::Array { .. });
                if is_array {
                    self.emit_indent();
                    self.emit(&format!("memcpy({}, {}, sizeof({}));\n",
                        target_str, value_str, target_str));
                } else if self.is_aggregate_type(resolved) {
                    // Record: direct struct assignment (avoids &rvalue issue)
                    self.emit_indent();
                    self.emit(&format!("{} = {};\n", target_str, value_str));
                } else {
                    self.emit_indent();
                    self.emit(&format!("{} = {};\n", target_str, value_str));
                }
            }

            HirStmtKind::ProcCall { target, args } => {
                let (name, arg_str) = match target {
                    HirCallTarget::Direct(sid) => {
                        let name = if crate::builtins::is_builtin_proc(&sid.source_name) {
                            sid.source_name.clone()
                        } else {
                            self.resolve_call_name(sid)
                        };
                        // Builtins
                        if crate::builtins::is_builtin_proc(&sid.source_name) {
                            let arg_strs: Vec<String> = args.iter()
                                .map(|a| self.hir_expr_to_string(a))
                                .collect();
                            // M2+ NEW/DISPOSE: use M2_ref_alloc for REF types
                            if (sid.source_name == "NEW" || sid.source_name == "DISPOSE")
                                && self.m2plus && !arg_strs.is_empty()
                            {
                                let var = &arg_strs[0];
                                let td = self.resolve_var_type_name(var).and_then(|vt| {
                                    self.ref_type_descs.get(&vt).cloned()
                                        .or_else(|| self.object_type_descs.get(&vt).cloned())
                                });
                                if let Some(td_name) = td {
                                    self.emit_indent();
                                    if sid.source_name == "NEW" {
                                        self.emit(&format!("{} = M2_ref_alloc(sizeof(*{}), &{});\n", var, var, td_name));
                                    } else {
                                        self.emit(&format!("M2_ref_free({});\n", var));
                                    }
                                    return;
                                }
                            }
                            let code = crate::builtins::codegen_builtin(&sid.source_name, &arg_strs);
                            self.emit_indent();
                            self.emit(&code);
                            self.emit(";\n");
                            return;
                        }
                        let orig = self.original_import_name(&sid.source_name).to_string();
                        let native_mod = sid.module.as_ref()
                            .filter(|m| crate::stdlib::is_native_stdlib(m)
                                && crate::stdlib::map_stdlib_call(m, &orig).is_some());
                        let args_s = if let Some(module) = native_mod {
                            let m = module.clone();
                            let p = orig.clone();
                            self.hir_args_for_native_stdlib(args, &name, &m, &p)
                        } else {
                            self.hir_args_to_string(args, &name)
                        };
                        (name, args_s)
                    }
                    HirCallTarget::Indirect(callee) => {
                        let callee_str = self.hir_expr_to_string(callee);
                        let args_s: Vec<String> = args.iter()
                            .map(|a| self.hir_expr_to_string(a))
                            .collect();
                        (callee_str, args_s.join(", "))
                    }
                };
                self.emit_indent();
                self.emit(&format!("{}({});\n", name, arg_str));
            }

            HirStmtKind::If { cond, then_body, elsifs, else_body } => {
                self.emit_indent();
                self.emit("if (");
                self.emit_hir_expr(cond);
                self.emit(") {\n");
                self.indent += 1;
                for s in then_body { self.emit_hir_stmt(s); }
                self.indent -= 1;
                for (econd, ebody) in elsifs {
                    self.emit_indent();
                    self.emit("} else if (");
                    self.emit_hir_expr(econd);
                    self.emit(") {\n");
                    self.indent += 1;
                    for s in ebody { self.emit_hir_stmt(s); }
                    self.indent -= 1;
                }
                if let Some(eb) = else_body {
                    self.emit_indent();
                    self.emit("} else {\n");
                    self.indent += 1;
                    for s in eb { self.emit_hir_stmt(s); }
                    self.indent -= 1;
                }
                self.emitln("}");
            }

            HirStmtKind::While { cond, body } => {
                self.emit_indent();
                self.emit("while (");
                self.emit_hir_expr(cond);
                self.emit(") {\n");
                self.indent += 1;
                for s in body { self.emit_hir_stmt(s); }
                self.indent -= 1;
                self.emitln("}");
            }

            HirStmtKind::Repeat { body, cond } => {
                self.emitln("do {");
                self.indent += 1;
                for s in body { self.emit_hir_stmt(s); }
                self.indent -= 1;
                self.emit_indent();
                self.emit("} while (!(");
                self.emit_hir_expr(cond);
                self.emit("));\n");
            }

            HirStmtKind::For { var, var_ty: _, start, end, step, direction, body } => {
                let var_c = self.mangle(var);
                let start_s = self.hir_expr_to_string(start);
                let end_s = self.hir_expr_to_string(end);
                self.emit_indent();
                self.emit(&format!("{} = {};\n", var_c, start_s));
                self.emit_indent();
                match direction {
                    ForDirection::Up => {
                        self.emit(&format!("while ({} <= {}) {{\n", var_c, end_s));
                    }
                    ForDirection::Down => {
                        self.emit(&format!("while ({} >= {}) {{\n", var_c, end_s));
                    }
                }
                self.indent += 1;
                for s in body { self.emit_hir_stmt(s); }
                // Step
                self.emit_indent();
                if let Some(step_expr) = step {
                    let step_s = self.hir_expr_to_string(step_expr);
                    // Always += : step is positive for Up, negative for Down
                    self.emit(&format!("{} += {};\n", var_c, step_s));
                } else {
                    match direction {
                        ForDirection::Up => self.emit(&format!("({})++;\n", var_c)),
                        ForDirection::Down => self.emit(&format!("({})--;\n", var_c)),
                    }
                }
                self.indent -= 1;
                self.emitln("}");
            }

            HirStmtKind::Loop { body } => {
                self.emitln("for (;;) {");
                self.indent += 1;
                for s in body { self.emit_hir_stmt(s); }
                self.indent -= 1;
                self.emitln("}");
            }

            HirStmtKind::Case { expr, branches, else_body } => {
                self.emit_indent();
                self.emit("switch (");
                self.emit_hir_expr(expr);
                self.emit(") {\n");
                for branch in branches {
                    for label in &branch.labels {
                        self.emit_indent();
                        match label {
                            HirCaseLabel::Single(v) => {
                                self.emit("case ");
                                let s = self.hir_expr_to_scalar_string(v);
                                self.emit(&s);
                                self.emit(":\n");
                            }
                            HirCaseLabel::Range(lo, hi) => {
                                self.emit("case ");
                                let ls = self.hir_expr_to_scalar_string(lo);
                                self.emit(&ls);
                                self.emit(" ... ");
                                let hs = self.hir_expr_to_scalar_string(hi);
                                self.emit(&hs);
                                self.emit(":\n");
                            }
                        }
                    }
                    self.indent += 1;
                    for s in &branch.body { self.emit_hir_stmt(s); }
                    self.emitln("break;");
                    self.indent -= 1;
                }
                if let Some(eb) = else_body {
                    self.emit_indent();
                    self.emit("default:\n");
                    self.indent += 1;
                    for s in eb { self.emit_hir_stmt(s); }
                    self.emitln("break;");
                    self.indent -= 1;
                }
                self.emitln("}");
            }

            HirStmtKind::Return { expr } => {
                self.emit_indent();
                if let Some(e) = expr {
                    self.emit("return ");
                    self.emit_hir_expr(e);
                    self.emit(";\n");
                } else {
                    self.emit("return;\n");
                }
            }

            HirStmtKind::Exit => {
                self.emitln("break;");
            }

            // ── M2+ Statements ───────────────────────────────────

            HirStmtKind::Raise { expr } => {
                self.emit_indent();
                if let Some(e) = expr {
                    // Check if the value is an exception TypeId — resolve to M2_EXC_Name
                    let exc_c_name = match &e.kind {
                        HirExprKind::IntLit(v) => {
                            // Look up exception name from the TypeId
                            if let crate::types::Type::Exception { name } = self.sema.types.get(*v as crate::types::TypeId) {
                                Some(format!("M2_EXC_{}", self.mangle(&name)))
                            } else { None }
                        }
                        _ => None,
                    };
                    if let Some(c_name) = exc_c_name {
                        self.emit(&format!("m2_raise({}, \"{}\", NULL);\n", c_name,
                            c_name.strip_prefix("M2_EXC_").unwrap_or(&c_name)));
                    } else {
                        let s = self.hir_expr_to_string(e);
                        self.emit(&format!("m2_raise((int)({}), NULL, NULL);\n", s));
                    }
                } else {
                    self.emitln("m2_raise(1, NULL, NULL);");
                }
            }

            HirStmtKind::Retry => {
                self.emitln("longjmp(m2_exception_buf, -1); /* RETRY */");
            }

            HirStmtKind::Try { body, excepts, finally_body } => {
                let has_finally = finally_body.is_some();
                let needs_deferred = has_finally
                    && (excepts.is_empty() || excepts.iter().all(|ec| ec.exception.is_some()));

                self.emitln("{");
                self.indent += 1;
                self.emitln("m2_ExcFrame _ef;");
                if needs_deferred { self.emitln("int _ef_exc = 0;"); }
                self.emitln("M2_TRY(_ef) {");
                self.indent += 1;
                for s in body { self.emit_hir_stmt(s); }
                self.emitln("M2_ENDTRY(_ef);");
                self.indent -= 1;
                self.emitln("} M2_CATCH {");
                self.indent += 1;
                self.emitln("M2_ENDTRY(_ef);");

                if excepts.is_empty() {
                    if has_finally {
                        self.emitln("_ef_exc = 1;");
                    } else {
                        self.emitln("m2_raise(_ef.exception_id, _ef.exception_name, _ef.exception_arg);");
                    }
                } else {
                    let mut first = true;
                    let mut has_catch_all = false;
                    for ec in excepts {
                        self.emit_indent();
                        if !first { self.emit("} else "); }
                        first = false;
                        if let Some(ref exc) = ec.exception {
                            let c_name = if let Some(ref m) = exc.module {
                                format!("M2_EXC_{}_{}", m, exc.source_name)
                            } else {
                                format!("M2_EXC_{}", self.mangle(&exc.source_name))
                            };
                            self.emit(&format!("if (_ef.exception_id == {}) {{\n", c_name));
                        } else {
                            has_catch_all = true;
                            self.emit("{\n");
                        }
                        self.indent += 1;
                        for s in &ec.body { self.emit_hir_stmt(s); }
                        self.indent -= 1;
                    }
                    if !has_catch_all {
                        if has_finally {
                            self.emitln("} else {");
                            self.indent += 1;
                            self.emitln("_ef_exc = 1;");
                            self.indent -= 1;
                        } else {
                            self.emitln("} else {");
                            self.indent += 1;
                            self.emitln("m2_raise(_ef.exception_id, _ef.exception_name, _ef.exception_arg);");
                            self.indent -= 1;
                        }
                    }
                    self.emitln("}");
                }
                self.indent -= 1;
                self.emitln("}");

                if let Some(fb) = finally_body {
                    for s in fb { self.emit_hir_stmt(s); }
                    if needs_deferred {
                        self.emitln("if (_ef_exc) {");
                        self.indent += 1;
                        self.emitln("m2_raise(_ef.exception_id, _ef.exception_name, _ef.exception_arg);");
                        self.indent -= 1;
                        self.emitln("}");
                    }
                }
                self.indent -= 1;
                self.emitln("}");
            }

            HirStmtKind::Lock { mutex, body } => {
                self.emitln("{");
                self.indent += 1;
                let mutex_str = self.hir_expr_to_string(mutex);
                self.emitln(&format!("m2_Mutex_Lock({});", mutex_str));
                self.emitln("m2_ExcFrame _lf;");
                self.emitln("M2_TRY(_lf) {");
                self.indent += 1;
                for s in body { self.emit_hir_stmt(s); }
                self.emitln("M2_ENDTRY(_lf);");
                self.indent -= 1;
                self.emitln("} M2_CATCH {");
                self.indent += 1;
                self.emitln("M2_ENDTRY(_lf);");
                self.emitln(&format!("m2_Mutex_Unlock({});", mutex_str));
                self.emitln("m2_raise(_lf.exception_id, _lf.exception_name, _lf.exception_arg);");
                self.indent -= 1;
                self.emitln("}");
                self.emitln(&format!("m2_Mutex_Unlock({});", mutex_str));
                self.indent -= 1;
                self.emitln("}");
            }

            HirStmtKind::TypeCase { expr, branches, else_body } => {
                self.emitln("{");
                self.indent += 1;
                let expr_str = self.hir_expr_to_string(expr);
                self.emitln(&format!("void *_tc_val = (void *)({});", expr_str));
                let mut first = true;
                for branch in branches {
                    self.emit_indent();
                    if !first { self.emit("} else "); }
                    first = false;
                    self.emit("if (_tc_val && (");
                    for (i, ty) in branch.types.iter().enumerate() {
                        if i > 0 { self.emit(" || "); }
                        let type_name = if let Some(ref m) = ty.module {
                            format!("{}_{}", m, ty.source_name)
                        } else {
                            self.mangle(&ty.source_name)
                        };
                        self.emit(&format!("M2_ISA(_tc_val, &M2_TD_{})", type_name));
                    }
                    self.emit(")) {\n");
                    self.indent += 1;
                    if let Some(ref var_name) = branch.var {
                        if let Some(first_type) = branch.types.first() {
                            let type_name = if let Some(ref m) = first_type.module {
                                format!("{}_{}", m, first_type.source_name)
                            } else {
                                self.mangle(&first_type.source_name)
                            };
                            self.emitln(&format!("{} {} = ({})_tc_val;", type_name, var_name, type_name));
                        }
                    }
                    for s in &branch.body { self.emit_hir_stmt(s); }
                    self.indent -= 1;
                }
                if let Some(eb) = else_body {
                    if !first {
                        self.emitln("} else {");
                    } else {
                        self.emitln("{");
                    }
                    self.indent += 1;
                    for s in eb { self.emit_hir_stmt(s); }
                    self.indent -= 1;
                }
                if !first || else_body.is_some() {
                    self.emitln("}");
                }
                self.indent -= 1;
                self.emitln("}");
            }
        }
    }

    /// Emit an HIR expr as a scalar C value — single-char strings become 'c'.
    fn hir_expr_to_scalar_string(&mut self, expr: &HirExpr) -> String {
        if let HirExprKind::StringLit(s) = &expr.kind {
            if s.len() == 1 {
                return format!("'{}'", super::escape_c_char(s.chars().next().unwrap()));
            }
            if s.is_empty() {
                return "'\\0'".to_string();
            }
        }
        self.hir_expr_to_string(expr)
    }

    /// Check if a TypeId is an aggregate (record or array) — needs memcpy.
    fn is_aggregate_type(&self, tid: TypeId) -> bool {
        match self.sema.types.get(tid) {
            crate::types::Type::Record { .. } => true,
            crate::types::Type::Array { .. } => true,
            _ => false,
        }
    }

    /// Resolve alias for HIR type checks.
    /// Extract the M2 source name for a TypeId (for context-dependent C name resolution).
    fn type_source_name(&self, tid: TypeId) -> Option<String> {
        match self.sema.types.get(tid) {
            crate::types::Type::Alias { name, .. } => Some(name.clone()),
            crate::types::Type::Enumeration { name, .. } => Some(name.clone()),
            crate::types::Type::Opaque { name, .. } => Some(name.clone()),
            crate::types::Type::Exception { name, .. } => Some(name.clone()),
            crate::types::Type::Object { name, .. } => Some(name.clone()),
            // Record/Array/Pointer/Set etc. are structural — no source name
            _ => None,
        }
    }

    pub(crate) fn resolve_hir_alias(&self, tid: TypeId) -> TypeId {
        let mut id = tid;
        let mut depth = 0;
        loop {
            match self.sema.types.get(id) {
                crate::types::Type::Alias { target, .. } => {
                    id = *target;
                    depth += 1;
                    if depth > 50 { break; }
                }
                _ => break,
            }
        }
        id
    }
}
