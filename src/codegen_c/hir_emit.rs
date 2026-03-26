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
                let native_module = target.module.as_ref()
                    .filter(|m| crate::stdlib::is_native_stdlib(m)
                        && crate::stdlib::map_stdlib_call(m, &target.source_name).is_some());
                let arg_strs = if let Some(module) = native_module {
                    // Native stdlib: strip _high companions (the inline C
                    // functions don't take open array high params)
                    let m = module.clone();
                    let p = target.source_name.clone();
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
                let l = self.hir_expr_to_string(left);
                let r = self.hir_expr_to_string(right);
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
    fn type_id_to_c(&self, tid: TypeId) -> String {
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
            _ => {
                // Check type registry for pointer/record/enum types
                let resolved = self.resolve_hir_alias(tid);
                match self.sema.types.get(resolved) {
                    crate::types::Type::Pointer { .. } => "void *".to_string(),
                    crate::types::Type::Enumeration { .. } => "int".to_string(),
                    _ => "int32_t".to_string(), // fallback
                }
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
                            let code = crate::builtins::codegen_builtin(&sid.source_name, &arg_strs);
                            self.emit_indent();
                            self.emit(&code);
                            self.emit(";\n");
                            return;
                        }
                        let native_mod = sid.module.as_ref()
                            .filter(|m| crate::stdlib::is_native_stdlib(m)
                                && crate::stdlib::map_stdlib_call(m, &sid.source_name).is_some());
                        let args_s = if let Some(module) = native_mod {
                            let m = module.clone();
                            let p = sid.source_name.clone();
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
                    match direction {
                        ForDirection::Up => self.emit(&format!("{} += {};\n", var_c, step_s)),
                        ForDirection::Down => self.emit(&format!("{} -= {};\n", var_c, step_s)),
                    }
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

            // M2+ features — delegate to existing M2+ codegen for now
            HirStmtKind::Try { .. } |
            HirStmtKind::Lock { .. } |
            HirStmtKind::TypeCase { .. } |
            HirStmtKind::Raise { .. } |
            HirStmtKind::Retry => {
                // TODO: implement M2+ HIR statement emission
                self.emitln("/* M2+ HIR stmt not yet implemented */");
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
    fn resolve_hir_alias(&self, tid: TypeId) -> TypeId {
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
