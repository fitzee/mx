use super::*;

impl CodeGen {
    pub(crate) fn gen_expr(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::IntLit(v) => self.emit(&format!("{}", v)),
            ExprKind::RealLit(v) => {
                let s = format!("{}", v);
                self.emit(&s);
                if !s.contains('.') && !s.contains('e') && !s.contains('E') {
                    self.emit(".0");
                }
            }
            ExprKind::StringLit(s) => {
                self.emit("\"");
                self.emit(&escape_c_string(s));
                self.emit("\"");
            }
            ExprKind::CharLit(c) => {
                self.emit(&format!("'{}'", escape_c_char(*c)));
            }
            ExprKind::BoolLit(b) => {
                self.emit(if *b { "1" } else { "0" });
            }
            ExprKind::NilLit => self.emit("NULL"),
            ExprKind::Designator(d) => self.gen_designator(d),
            ExprKind::FuncCall { desig, args } => {
                // Resolve the actual procedure name (may be module-qualified)
                let module_qualified = self.resolve_module_qualified(desig);
                let actual_name = if let Some((_, proc_name)) = module_qualified {
                    proc_name.to_string()
                } else {
                    desig.ident.name.clone()
                };
                // Handle type transfer functions: TypeName(expr) → C cast
                if args.len() == 1 && desig.selectors.is_empty() && desig.ident.module.is_none() {
                    let c_cast = match actual_name.as_str() {
                        "CARDINAL" => Some("(uint32_t)"),
                        "INTEGER"  => Some("(int32_t)"),
                        "LONGINT"  => Some("(int64_t)"),
                        "LONGCARD" => Some("(uint64_t)"),
                        "BITSET"   => Some("(uint32_t)"),
                        "BOOLEAN"  => Some("(int)"),
                        "CHAR"     => Some("(char)"),
                        "REAL"     => Some("(float)"),
                        "LONGREAL" => Some("(double)"),
                        "ADDRESS"  => Some("(void *)"),
                        "WORD"     => Some("(uint32_t)"),
                        "BYTE"     => Some("(uint8_t)"),
                        _ => None,
                    };
                    if let Some(cast) = c_cast {
                        self.emit(&format!("({}(", cast));
                        self.gen_expr(&args[0]);
                        self.emit("))");
                        return;
                    }
                    // User-defined type cast: TypeName(expr) → (CTypeName)(expr)
                    // Check if name is a known type (from def modules or local declarations)
                    if let Some(c_type) = self.resolve_type_cast_name(&actual_name) {
                        self.emit(&format!("(({})(", c_type));
                        self.gen_expr(&args[0]);
                        self.emit("))");
                        return;
                    }
                }
                if builtins::is_builtin_proc(&actual_name) {
                    // ADR on open/named-array params: emit (void *)(name) instead of (void *)&(name)
                    // In C, array params decay to pointers, so &buf gives char** not char*
                    if actual_name == "ADR" && args.len() == 1 {
                        if let ExprKind::Designator(ref d) = args[0].kind {
                            if d.selectors.is_empty() && d.ident.module.is_none()
                                && (self.is_open_array_param(&self.mangle(&d.ident.name))
                                    || self.is_named_array_value_param(&d.ident.name))
                            {
                                let arg_str = self.expr_to_string(&args[0]);
                                self.emit(&format!("((void *)({}))", arg_str));
                                return;
                            }
                        }
                    }
                    // HIGH on non-open-array: emit sizeof-based constant
                    // HIGH on open-array env var: emit (*_env->name_high)
                    if actual_name == "HIGH" && args.len() == 1 {
                        if let ExprKind::Designator(ref d) = args[0].kind {
                            // Simple variable that's not an open array param
                            let is_open = d.selectors.is_empty()
                                && d.ident.module.is_none()
                                && self.is_open_array_param(&self.mangle(&d.ident.name));
                            if !is_open {
                                let dname = &d.ident.name;
                                if let Some(high) = self.get_named_array_param_high(dname) {
                                    self.emit(&high);
                                } else {
                                    let arg_str = self.expr_to_string(&args[0]);
                                    self.emit(&format!("(sizeof({}) / sizeof({}[0])) - 1", arg_str, arg_str));
                                }
                                return;
                            }
                            // Open array accessed through closure env — emit (*_env->name_high)
                            if is_open && self.is_env_var(&d.ident.name) {
                                self.emit(&format!("(*_env->{}_high)", d.ident.name));
                                return;
                            }
                        }
                    }
                    // For builtins that take char args, convert single-char strings to char literals
                    let char_builtins = ["CAP", "ORD", "CHR", "Write"];
                    let is_set_elem_builtin = actual_name == "INCL" || actual_name == "EXCL";
                    let is_size_builtin = actual_name == "TSIZE" || actual_name == "SIZE";
                    let arg_strs: Vec<String> = args.iter().enumerate().map(|(idx, a)| {
                        if char_builtins.contains(&actual_name.as_ref()) {
                            self.expr_to_char_string(a)
                        } else if is_set_elem_builtin && idx == 1 {
                            self.expr_to_char_string(a)
                        } else if is_size_builtin && idx == 0 {
                            // TSIZE/SIZE first arg is a type name — mangle it
                            // so user-defined types get the module prefix.
                            let s = self.expr_to_string(a);
                            self.mangle_type_name(&s)
                        } else {
                            self.expr_to_string(a)
                        }
                    }).collect();
                    self.emit(&builtins::codegen_builtin(&actual_name, &arg_strs));
                } else {
                    // Check for complex designator (pointer deref, indexing, etc.)
                    let has_complex_selectors = desig.selectors.iter().any(|s| {
                        matches!(s, Selector::Deref(_) | Selector::Index(_, _))
                    }) || (!desig.selectors.is_empty()
                        && desig.ident.module.is_none()
                        && !self.imported_modules.contains(&desig.ident.name));
                    let c_name = if has_complex_selectors {
                        self.designator_to_string(desig)
                    } else {
                        self.resolve_proc_name(desig)
                    };
                    // Look up param info: try module-prefixed name, then actual name,
                    // then FROM-import prefixed name
                    let mut param_info = if let Some((mod_name, _)) = module_qualified {
                        let prefixed = format!("{}_{}", mod_name, actual_name);
                        let info = self.get_param_info(&prefixed);
                        if info.is_empty() { self.get_param_info(&actual_name) } else { info }
                    } else {
                        let mut info = Vec::new();
                        // Try import-prefixed first (avoids collision when two modules
                        // export same-named procs — bare name gets overwritten)
                        if let Some(module) = self.import_map.get(&actual_name) {
                            let orig = self.original_import_name(&actual_name).to_string();
                            let prefixed = format!("{}_{}", module, orig);
                            info = self.get_param_info(&prefixed);
                        }
                        if info.is_empty() {
                            info = self.get_param_info(&actual_name);
                        }
                        info
                    };
                    // Fallback: for calls through complex designators (record field proc vars),
                    // resolve the designator's type to get param info
                    if param_info.is_empty() && has_complex_selectors {
                        param_info = self.get_designator_proc_param_info(desig);
                    }
                    self.emit(&format!("{}(", c_name));
                    // Pass closure env if this is a nested proc with captures
                    if self.closure_env_type.contains_key(actual_name.as_str()) {
                        self.emit("&_child_env");
                        if !args.is_empty() {
                            self.emit(", ");
                        }
                    }
                    self.gen_call_args(args, &param_info);
                    self.emit(")");
                }
            }
            ExprKind::UnaryOp { op, operand } => {
                match op {
                    UnaryOp::Pos => {
                        self.emit("(+");
                        self.gen_expr(operand);
                        self.emit(")");
                    }
                    UnaryOp::Neg => {
                        if self.is_complex_expr(operand) {
                            let prefix = if self.is_longcomplex_expr(operand) { "m2_lcomplex" } else { "m2_complex" };
                            self.emit(&format!("{}_neg(", prefix));
                            self.gen_expr(operand);
                            self.emit(")");
                        } else {
                            self.emit("(-");
                            self.gen_expr(operand);
                            self.emit(")");
                        }
                    }
                }
            }
            ExprKind::Not(operand) => {
                self.emit("(!");
                self.gen_expr(operand);
                self.emit(")");
            }
            ExprKind::Deref(operand) => {
                self.emit("(*");
                self.gen_expr(operand);
                self.emit(")");
            }
            ExprKind::BinaryOp { op, left, right } => {
                // Handle IN specially
                if matches!(op, BinaryOp::In) {
                    // x IN s => ((s >> x) & 1)
                    self.emit("((");
                    self.gen_expr(right);
                    self.emit(" >> ");
                    self.gen_expr_for_binop(left);
                    self.emit(") & 1)");
                } else if matches!(op, BinaryOp::IntDiv) {
                    if self.is_address_expr(left) || self.is_address_expr(right) {
                        // ADDRESS DIV: cast to uintptr_t for pointer arithmetic
                        self.emit("(void*)((uintptr_t)");
                        self.gen_expr(left);
                        self.emit(" / (uintptr_t)");
                        self.gen_expr(right);
                        self.emit(")");
                    } else if self.is_unsigned_expr(left) || self.is_unsigned_expr(right) {
                        // Unsigned DIV (CARDINAL or LONGCARD): plain C division.
                        // No explicit cast — operands already have the correct
                        // unsigned type; casting to uint32_t would truncate LONGCARD.
                        self.emit("(");
                        self.gen_expr(left);
                        self.emit(" / ");
                        self.gen_expr(right);
                        self.emit(")");
                    } else {
                        // PIM4 DIV: truncates toward negative infinity (floored division)
                        let func = if self.is_long_expr(left) || self.is_long_expr(right) {
                            "m2_div64"
                        } else {
                            "m2_div"
                        };
                        self.emit(&format!("{}(", func));
                        self.gen_expr(left);
                        self.emit(", ");
                        self.gen_expr(right);
                        self.emit(")");
                    }
                } else if matches!(op, BinaryOp::Mod) {
                    if self.is_address_expr(left) || self.is_address_expr(right) {
                        // ADDRESS MOD: cast to uintptr_t for pointer arithmetic
                        self.emit("(void*)((uintptr_t)");
                        self.gen_expr(left);
                        self.emit(" % (uintptr_t)");
                        self.gen_expr(right);
                        self.emit(")");
                    } else if self.is_unsigned_expr(left) || self.is_unsigned_expr(right) {
                        // Unsigned MOD (CARDINAL or LONGCARD): plain C modulo.
                        // No explicit cast — operands already have the correct
                        // unsigned type; casting to uint32_t would truncate LONGCARD.
                        self.emit("(");
                        self.gen_expr(left);
                        self.emit(" % ");
                        self.gen_expr(right);
                        self.emit(")");
                    } else {
                        // PIM4 MOD: result is always non-negative
                        let func = if self.is_long_expr(left) || self.is_long_expr(right) {
                            "m2_mod64"
                        } else {
                            "m2_mod"
                        };
                        self.emit(&format!("{}(", func));
                        self.gen_expr(left);
                        self.emit(", ");
                        self.gen_expr(right);
                        self.emit(")");
                    }
                } else if matches!(op, BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::RealDiv | BinaryOp::Eq | BinaryOp::Ne)
                    && (self.is_complex_expr(left) || self.is_complex_expr(right))
                {
                    // Complex number operations
                    let is_long = self.is_longcomplex_expr(left) || self.is_longcomplex_expr(right);
                    let prefix = if is_long { "m2_lcomplex" } else { "m2_complex" };
                    let func = match op {
                        BinaryOp::Add => "add",
                        BinaryOp::Sub => "sub",
                        BinaryOp::Mul => "mul",
                        BinaryOp::RealDiv => "div",
                        BinaryOp::Eq => "eq",
                        BinaryOp::Ne => "eq", // negated below
                        _ => unreachable!(),
                    };
                    if matches!(op, BinaryOp::Ne) {
                        self.emit("(!");
                    }
                    self.emit(&format!("{}_{}(", prefix, func));
                    self.gen_expr(left);
                    self.emit(", ");
                    self.gen_expr(right);
                    self.emit(")");
                    if matches!(op, BinaryOp::Ne) {
                        self.emit(")");
                    }
                } else if matches!(op, BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::RealDiv)
                    && (self.is_set_expr(left) || self.is_set_expr(right))
                {
                    // Set operations: + → union (|), * → intersection (&),
                    // - → difference (& ~), / → symmetric difference (^)
                    match op {
                        BinaryOp::Add => {
                            // Union: s1 + s2 → s1 | s2
                            self.emit("(");
                            self.gen_expr(left);
                            self.emit(" | ");
                            self.gen_expr(right);
                            self.emit(")");
                        }
                        BinaryOp::Mul => {
                            // Intersection: s1 * s2 → s1 & s2
                            self.emit("(");
                            self.gen_expr(left);
                            self.emit(" & ");
                            self.gen_expr(right);
                            self.emit(")");
                        }
                        BinaryOp::Sub => {
                            // Difference: s1 - s2 → s1 & ~s2
                            self.emit("(");
                            self.gen_expr(left);
                            self.emit(" & ~");
                            self.gen_expr(right);
                            self.emit(")");
                        }
                        BinaryOp::RealDiv => {
                            // Symmetric difference: s1 / s2 → s1 ^ s2
                            self.emit("(");
                            self.gen_expr(left);
                            self.emit(" ^ ");
                            self.gen_expr(right);
                            self.emit(")");
                        }
                        _ => unreachable!(),
                    }
                } else if matches!(op, BinaryOp::RealDiv) {
                    // Force float context to avoid integer division
                    self.emit("((double)(");
                    self.gen_expr(left);
                    self.emit(") / (double)(");
                    self.gen_expr(right);
                    self.emit("))");
                } else if matches!(op, BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge)
                    && (self.is_set_expr(left) || self.is_set_expr(right))
                {
                    // Set comparison operators
                    match op {
                        BinaryOp::Eq => {
                            self.emit("(");
                            self.gen_expr(left);
                            self.emit(" == ");
                            self.gen_expr(right);
                            self.emit(")");
                        }
                        BinaryOp::Ne => {
                            self.emit("(");
                            self.gen_expr(left);
                            self.emit(" != ");
                            self.gen_expr(right);
                            self.emit(")");
                        }
                        BinaryOp::Le => {
                            // s1 <= s2 means s1 is a subset of s2: (s1 & ~s2) == 0
                            self.emit("((");
                            self.gen_expr(left);
                            self.emit(" & ~");
                            self.gen_expr(right);
                            self.emit(") == 0)");
                        }
                        BinaryOp::Ge => {
                            // s1 >= s2 means s1 is a superset of s2: (s2 & ~s1) == 0
                            self.emit("((");
                            self.gen_expr(right);
                            self.emit(" & ~");
                            self.gen_expr(left);
                            self.emit(") == 0)");
                        }
                        BinaryOp::Lt => {
                            // s1 < s2 means s1 is a proper subset of s2
                            self.emit("(((");
                            self.gen_expr(left);
                            self.emit(" & ~");
                            self.gen_expr(right);
                            self.emit(") == 0) && (");
                            self.gen_expr(left);
                            self.emit(" != ");
                            self.gen_expr(right);
                            self.emit("))");
                        }
                        BinaryOp::Gt => {
                            // s1 > s2 means s1 is a proper superset of s2
                            self.emit("(((");
                            self.gen_expr(right);
                            self.emit(" & ~");
                            self.gen_expr(left);
                            self.emit(") == 0) && (");
                            self.gen_expr(left);
                            self.emit(" != ");
                            self.gen_expr(right);
                            self.emit("))");
                        }
                        _ => unreachable!(),
                    }
                } else if matches!(op, BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge)
                    && (self.is_string_expr(left) || self.is_string_expr(right))
                {
                    // String comparison using strcmp
                    let cmp_op = match op {
                        BinaryOp::Eq => " == 0",
                        BinaryOp::Ne => " != 0",
                        BinaryOp::Lt => " < 0",
                        BinaryOp::Le => " <= 0",
                        BinaryOp::Gt => " > 0",
                        BinaryOp::Ge => " >= 0",
                        _ => unreachable!(),
                    };
                    self.emit("(strcmp(");
                    self.gen_expr(left);
                    self.emit(", ");
                    self.gen_expr(right);
                    self.emit(&format!("){})", cmp_op));
                } else {
                    // Skip outer parens for comparison/logical ops — they have
                    // low precedence so the if() wrapper suffices. This avoids
                    // ((x == y)) which triggers -Wparentheses-equality.
                    let is_cmp_or_logical = matches!(op,
                        BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt |
                        BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge |
                        BinaryOp::And | BinaryOp::Or);
                    if !is_cmp_or_logical { self.emit("("); }
                    self.gen_expr_for_binop(left);
                    let c_op = match op {
                        BinaryOp::Add => " + ",
                        BinaryOp::Sub => " - ",
                        BinaryOp::Mul => " * ",
                        BinaryOp::And => " && ",
                        BinaryOp::Or => " || ",
                        BinaryOp::Eq => " == ",
                        BinaryOp::Ne => " != ",
                        BinaryOp::Lt => " < ",
                        BinaryOp::Le => " <= ",
                        BinaryOp::Gt => " > ",
                        BinaryOp::Ge => " >= ",
                        _ => unreachable!(),
                    };
                    self.emit(c_op);
                    self.gen_expr_for_binop(right);
                    if !is_cmp_or_logical { self.emit(")"); }
                }
            }
            ExprKind::SetConstructor { elements, .. } => {
                if elements.is_empty() {
                    self.emit("0u");
                } else {
                    self.emit("(");
                    for (i, elem) in elements.iter().enumerate() {
                        if i > 0 {
                            self.emit(" | ");
                        }
                        match elem {
                            SetElement::Single(e) => {
                                self.emit("(1u << ");
                                self.gen_expr_for_binop(e);
                                self.emit(")");
                            }
                            SetElement::Range(lo, hi) => {
                                // Generate a mask: ((2u << hi) - (1u << lo))
                                self.emit("((2u << ");
                                self.gen_expr_for_binop(hi);
                                self.emit(") - (1u << ");
                                self.gen_expr_for_binop(lo);
                                self.emit("))");
                            }
                        }
                    }
                    self.emit(")");
                }
            }
        }
    }

    /// Like gen_expr but converts single-char string literals to char literals.
    /// Used in binary ops where single-char strings should be treated as CHAR.
    pub(crate) fn gen_expr_for_binop(&mut self, expr: &Expr) {
        if let ExprKind::StringLit(s) = &expr.kind {
            if s.is_empty() {
                self.emit("'\\0'");
                return;
            } else if s.len() == 1 {
                let ch = s.chars().next().unwrap();
                self.emit(&format!("'{}'", escape_c_char(ch)));
                return;
            }
        }
        self.gen_expr(expr);
    }

    pub(crate) fn expr_to_string(&mut self, expr: &Expr) -> String {
        let saved = std::mem::take(&mut self.output);
        self.gen_expr(expr);
        let result = std::mem::replace(&mut self.output, saved);
        result
    }

    /// Like expr_to_string but for expressions that should be chars (single-char strings become char literals)
    pub(crate) fn expr_to_char_string(&mut self, expr: &Expr) -> String {
        if let ExprKind::StringLit(s) = &expr.kind {
            if s.is_empty() {
                return "'\\0'".to_string();
            } else if s.len() == 1 {
                let ch = s.chars().next().unwrap();
                return format!("'{}'", escape_c_char(ch));
            }
        }
        self.expr_to_string(expr)
    }

    // ── Type mapping ────────────────────────────────────────────────

    /// Generate arguments for a procedure/function call, handling VAR and open array params
    pub(crate) fn gen_call_args_for(&mut self, proc_name: &str, args: &[Expr], param_info: &[ParamCodegenInfo]) {
        self.gen_call_args_inner(proc_name, args, param_info);
    }

    pub(crate) fn gen_call_args(&mut self, args: &[Expr], param_info: &[ParamCodegenInfo]) {
        self.gen_call_args_inner("", args, param_info);
    }

    fn gen_call_args_inner(&mut self, proc_name: &str, args: &[Expr], param_info: &[ParamCodegenInfo]) {
        let mut first = true;
        let mut pi = 0; // param info index
        for arg in args {
            if !first {
                self.emit(", ");
            }
            first = false;

            let info = param_info.get(pi);
            let is_var = info.map_or(false, |p| p.is_var);
            let is_open_array = info.map_or(false, |p| p.is_open_array);

            let is_char = info.map_or(false, |p| p.is_char);

            if is_open_array {
                // Pass pointer to first element and HIGH value
                let arg_str = self.expr_to_string(arg);
                self.emit(&arg_str);
                self.emit(", ");
                // If arg is itself an open array param, use its _high companion
                // instead of sizeof (which gives pointer size for open array params)
                if self.is_open_array_param(&arg_str) {
                    self.emit(&format!("{}_high", arg_str));
                } else if let Some(high) = self.get_named_array_param_high(&arg_str) {
                    self.emit(&high);
                } else {
                    self.emit(&format!("(sizeof({}) / sizeof({}[0])) - 1", arg_str, arg_str));
                }
            } else if is_var {
                // Cast first arg of ALLOCATE/DEALLOCATE to (void **) for
                // clang 16+ which errors on typed-ptr* → void** conversion.
                let is_alloc = pi == 0 && (proc_name == "ALLOCATE" || proc_name == "DEALLOCATE"
                    || proc_name.ends_with("_ALLOCATE") || proc_name.ends_with("_DEALLOCATE"));
                if is_alloc { self.emit("(void **)"); }
                self.gen_var_arg(arg);
            } else if is_char {
                // Convert single-char string literals to char literals for CHAR parameters
                self.gen_expr_for_binop(arg);
            } else {
                self.gen_expr(arg);
            }
            pi += 1;
        }
    }

    /// Generate an argument passed to a VAR parameter (pass address)
    pub(crate) fn gen_var_arg(&mut self, arg: &Expr) {
        match &arg.kind {
            ExprKind::Designator(d) => {
                if d.selectors.is_empty() && self.is_env_var(&d.ident.name) {
                    // Env variable: _env->name is already a pointer
                    self.emit(&format!("_env->{}", d.ident.name));
                } else if d.selectors.is_empty() && self.is_var_param(&d.ident.name) {
                    // VAR param: already a pointer, just pass it through
                    self.emit(&self.mangle(&d.ident.name).to_string());
                } else {
                    // Take address of the designator
                    let desig_str = self.designator_to_string(d);
                    self.emit(&format!("&{}", desig_str));
                }
            }
            _ => {
                // For non-designator expressions, just pass as-is (shouldn't happen for VAR)
                self.gen_expr(arg);
            }
        }
    }

    pub(crate) fn infer_c_type(&self, expr: &Expr) -> String {
        match &expr.kind {
            ExprKind::IntLit(_) => "int32_t".to_string(),
            ExprKind::RealLit(_) => "float".to_string(),
            ExprKind::StringLit(s) if s.len() <= 1 => "char".to_string(),
            ExprKind::StringLit(_) => "const char *".to_string(),
            ExprKind::CharLit(_) => "char".to_string(),
            ExprKind::BoolLit(_) => "int".to_string(),
            ExprKind::UnaryOp { operand, .. } => self.infer_c_type(operand),
            _ => "int32_t".to_string(),
        }
    }

}
