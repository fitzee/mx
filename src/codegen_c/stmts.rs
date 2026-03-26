use super::*;

impl CodeGen {
    pub(crate) fn gen_statement(&mut self, stmt: &Statement) {
        self.emit_line_directive(&stmt.loc);
        match &stmt.kind {
            StatementKind::Empty => {}
            StatementKind::Assign { desig, expr } => {
                // Check if RHS is a string literal (multi-char) → strcpy
                let is_string_literal_assign = self.is_string_expr(expr);

                // Check if LHS is an array type (variable or field) → memcpy
                let is_array_assign = if desig.selectors.is_empty() {
                    // Simple variable: check array_vars
                    self.array_vars.contains(&desig.ident.name)
                } else if let Some(Selector::Field(fname, _)) = desig.selectors.last() {
                    // Record field: use type-aware check to avoid false positives when
                    // different records have same-named fields with different types
                    // (e.g., BufRec.body is ARRAY OF CHAR, ByteBuf.Buf.body is a RECORD)
                    if let Some(rec_type) = self.resolve_field_record_type(desig) {
                        self.is_array_field_of(&rec_type, fname)
                    } else {
                        // Fallback (type resolution failed): use name-only check but
                        // guard against false positives with pointer and scalar checks
                        self.is_array_field(fname)
                            && !self.pointer_fields.contains(fname)
                            && !self.is_scalar_expr(expr)
                    }
                } else {
                    false
                };

                if is_string_literal_assign && is_array_assign {
                    // String literal to array of char: zero-fill then copy literal bytes
                    if let ExprKind::StringLit(s) = &expr.kind {
                        let lit_size = s.len() + 1; // include NUL terminator
                        self.emit_indent();
                        self.emit("memset(");
                        self.gen_designator(desig);
                        self.emit(", 0, sizeof(");
                        self.gen_designator(desig);
                        self.emit("));\n");
                        self.emit_indent();
                        self.emit("memcpy(");
                        self.gen_designator(desig);
                        self.emit(", ");
                        self.gen_expr(expr);
                        self.emit(&format!(", {});\n", lit_size));
                    } else {
                        // char array variable to array → normal memcpy is safe
                        self.emit_indent();
                        self.emit("memcpy(");
                        self.gen_designator(desig);
                        self.emit(", ");
                        self.gen_expr(expr);
                        self.emit(", sizeof(");
                        self.gen_designator(desig);
                        self.emit("));\n");
                    }
                } else if is_string_literal_assign && !is_array_assign {
                    // String literal to 1D char array → strcpy
                    self.emit_indent();
                    self.emit("strcpy(");
                    self.gen_designator(desig);
                    self.emit(", ");
                    self.gen_expr(expr);
                    self.emit(");\n");
                } else if is_array_assign {
                    // Array type assignment → memcpy
                    self.emit_indent();
                    self.emit("memcpy(");
                    self.gen_designator(desig);
                    self.emit(", ");
                    self.gen_expr(expr);
                    self.emit(", sizeof(");
                    self.gen_designator(desig);
                    self.emit("));\n");
                } else {
                    self.emit_indent();
                    self.gen_designator(desig);
                    self.emit(" = ");
                    // Special case: single-char or empty string assigned to char variable
                    if let ExprKind::StringLit(s) = &expr.kind {
                        if s.is_empty() {
                            self.emit("'\\0'");
                        } else if s.len() == 1 {
                            let ch = s.chars().next().unwrap();
                            self.emit(&format!("'{}'", escape_c_char(ch)));
                        } else {
                            self.gen_expr(expr);
                        }
                    } else {
                        self.gen_expr(expr);
                    }
                    self.emit(";\n");
                }
            }
            StatementKind::ProcCall { desig, args } => {
                // Resolve the actual procedure name (may be module-qualified)
                let module_qualified = self.resolve_module_qualified(desig);
                let actual_name = if let Some((_, proc_name)) = module_qualified {
                    proc_name.to_string()
                } else {
                    desig.ident.name.clone()
                };
                if actual_name == "NEW" && self.m2plus && !args.is_empty() {
                    // M2+ typed NEW: use M2_ref_alloc for REF/OBJECT types
                    let arg_str = self.expr_to_string(&args[0]);
                    // Look up the variable's type to find its type descriptor
                    let var_type = self.resolve_var_type_name(&arg_str);
                    let td_sym = var_type.as_ref().and_then(|vt| {
                        self.ref_type_descs.get(vt).cloned()
                            .or_else(|| self.object_type_descs.get(vt).cloned())
                    });
                    self.emit_indent();
                    if let Some(td) = td_sym {
                        self.emit(&format!("{} = M2_ref_alloc(sizeof(*{}), &{});\n", arg_str, arg_str, td));
                    } else {
                        // Fallback: plain GC_MALLOC for non-typed or unknown types
                        self.emit(&builtins::codegen_builtin("NEW", &[arg_str]));
                        self.emit(";\n");
                    }
                } else if actual_name == "DISPOSE" && self.m2plus && !args.is_empty() {
                    // M2+ typed DISPOSE: use M2_ref_free for REF/OBJECT types
                    let arg_str = self.expr_to_string(&args[0]);
                    let var_type = self.resolve_var_type_name(&arg_str);
                    let has_td = var_type.as_ref().map(|vt| {
                        self.ref_type_descs.contains_key(vt) || self.object_type_descs.contains_key(vt)
                    }).unwrap_or(false);
                    self.emit_indent();
                    if has_td {
                        self.emit(&format!("M2_ref_free({});\n", arg_str));
                    } else {
                        self.emit(&builtins::codegen_builtin("DISPOSE", &[arg_str]));
                        self.emit(";\n");
                    }
                } else if builtins::is_builtin_proc(&actual_name) {
                    self.emit_indent();
                    let char_builtins = ["CAP", "ORD", "CHR", "Write"];
                    let is_set_elem_builtin = actual_name == "INCL" || actual_name == "EXCL";
                    let arg_strs: Vec<String> = args.iter().enumerate().map(|(idx, a)| {
                        if char_builtins.contains(&actual_name.as_ref()) {
                            self.expr_to_char_string(a)
                        } else if is_set_elem_builtin && idx == 1 {
                            // Second arg of INCL/EXCL is the element — coerce char
                            self.expr_to_char_string(a)
                        } else {
                            self.expr_to_string(a)
                        }
                    }).collect();
                    self.emit(&builtins::codegen_builtin(&actual_name, &arg_strs));
                    self.emit(";\n");
                } else {
                    self.emit_indent();
                    // Check if this is a call through a complex designator (pointer deref,
                    // array indexing, record field access chain). These need the full
                    // designator string, not just the resolved proc name.
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
                    // Look up param info: try module-prefixed name first (most precise),
                    // then fall back to bare name / symtab lookup
                    let mut param_info = if let Some((mod_name, _)) = module_qualified {
                        let prefixed = format!("{}_{}", mod_name, actual_name);
                        let info = self.get_param_info(&prefixed);
                        if info.is_empty() { self.get_param_info(&actual_name) } else { info }
                    } else {
                        // For FROM-imported names, prefer the module-prefixed lookup
                        // to avoid symtab collisions with same-named procs in other modules
                        let mut info = Vec::new();
                        if let Some(module) = self.import_map.get(&actual_name).cloned() {
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
                    self.gen_call_args_for(&actual_name, args, &param_info);
                    self.emit(");\n");
                }
            }
            StatementKind::If {
                cond,
                then_body,
                elsifs,
                else_body,
            } => {
                self.emit_indent();
                self.emit("if (");
                self.gen_expr(cond);
                self.emit(") {\n");
                self.indent += 1;
                for s in then_body {
                    self.gen_statement(s);
                }
                self.indent -= 1;
                for (ec, eb) in elsifs {
                    self.emit_indent();
                    self.emit("} else if (");
                    self.gen_expr(ec);
                    self.emit(") {\n");
                    self.indent += 1;
                    for s in eb {
                        self.gen_statement(s);
                    }
                    self.indent -= 1;
                }
                if let Some(eb) = else_body {
                    self.emitln("} else {");
                    self.indent += 1;
                    for s in eb {
                        self.gen_statement(s);
                    }
                    self.indent -= 1;
                }
                self.emitln("}");
            }
            StatementKind::Case { expr, branches, else_body } => {
                // Check if any branches have ranges - if so, use if-else chain
                // otherwise use switch for efficiency
                let has_ranges = branches.iter().any(|b| {
                    b.labels.iter().any(|l| matches!(l, CaseLabel::Range(_, _)))
                });

                if has_ranges {
                    // Use if-else chain for portability (no GCC extension)
                    let case_var = format!("_case_{}", self.indent);
                    self.emit_indent();
                    self.emit("{\n");
                    self.indent += 1;
                    self.emit_indent();
                    let case_val = self.expr_to_string(expr);
                    self.emit(&format!("int32_t {} = {};\n", case_var, case_val));

                    let mut first = true;
                    for branch in branches {
                        self.emit_indent();
                        if !first {
                            self.emit("} else ");
                        }
                        first = false;
                        self.emit("if (");
                        for (li, label) in branch.labels.iter().enumerate() {
                            if li > 0 {
                                self.emit(" || ");
                            }
                            match label {
                                CaseLabel::Single(e) => {
                                    self.emit(&format!("{} == ", case_var));
                                    self.gen_expr_for_binop(e);
                                }
                                CaseLabel::Range(lo, hi) => {
                                    self.emit(&format!("({} >= ", case_var));
                                    self.gen_expr_for_binop(lo);
                                    self.emit(&format!(" && {} <= ", case_var));
                                    self.gen_expr_for_binop(hi);
                                    self.emit(")");
                                }
                            }
                        }
                        self.emit(") {\n");
                        self.indent += 1;
                        for s in &branch.body {
                            self.gen_statement(s);
                        }
                        self.indent -= 1;
                    }
                    if let Some(eb) = else_body {
                        self.emitln("} else {");
                        self.indent += 1;
                        for s in eb {
                            self.gen_statement(s);
                        }
                        self.indent -= 1;
                    }
                    self.emitln("}");
                    self.indent -= 1;
                    self.emitln("}");
                } else {
                    // Standard switch statement
                    self.emit_indent();
                    self.emit("switch (");
                    self.gen_expr(expr);
                    self.emit(") {\n");
                    self.indent += 1;
                    for branch in branches {
                        for label in &branch.labels {
                            self.emit_indent();
                            if let CaseLabel::Single(e) = label {
                                self.emit("case ");
                                self.gen_expr_for_binop(e);
                                self.emit(":\n");
                            }
                        }
                        self.indent += 1;
                        for s in &branch.body {
                            self.gen_statement(s);
                        }
                        self.emitln("break;");
                        self.indent -= 1;
                    }
                    if let Some(eb) = else_body {
                        self.emitln("default:");
                        self.indent += 1;
                        for s in eb {
                            self.gen_statement(s);
                        }
                        self.emitln("break;");
                        self.indent -= 1;
                    }
                    self.indent -= 1;
                    self.emitln("}");
                }
            }
            StatementKind::While { cond, body } => {
                self.emit_indent();
                self.emit("while (");
                self.gen_expr(cond);
                self.emit(") {\n");
                self.indent += 1;
                for s in body {
                    self.gen_statement(s);
                }
                self.indent -= 1;
                self.emitln("}");
            }
            StatementKind::Repeat { body, cond } => {
                self.emitln("do {");
                self.indent += 1;
                for s in body {
                    self.gen_statement(s);
                }
                self.indent -= 1;
                self.emit_indent();
                self.emit("} while (!(");
                self.gen_expr(cond);
                self.emit("));\n");
            }
            StatementKind::For {
                var,
                start,
                end,
                step,
                body,
            } => {
                self.emit_indent();
                let step_str = if let Some(s) = step {
                    self.expr_to_string(s)
                } else {
                    "1".to_string()
                };
                // Determine loop direction (via unified HIR analysis)
                let is_downward = {
                    let hb = crate::hir_build::HirBuilder::new(
                        &self.sema.types, &self.sema.symtab, &self.module_name,
                        &self.sema.foreign_modules,
                    );
                    hb.for_direction(step.as_ref()) == crate::hir::ForDirection::Down
                };
                let cmp_op = if is_downward { ">=" } else { "<=" };
                let var_c = self.mangle(var);
                self.emit(&format!("for ({} = ", var_c));
                self.gen_expr_for_binop(start);
                self.emit(&format!("; {} {} ", var_c, cmp_op));
                self.gen_expr_for_binop(end);
                self.emit(&format!("; {} += {}) {{\n", var_c, step_str));
                self.indent += 1;
                for s in body {
                    self.gen_statement(s);
                }
                self.indent -= 1;
                self.emitln("}");
            }
            StatementKind::Loop { body } => {
                self.emitln("for (;;) {");
                self.indent += 1;
                for s in body {
                    self.gen_statement(s);
                }
                self.indent -= 1;
                self.emitln("}");
            }
            StatementKind::With { desig, body } => {
                // WITH r DO ... END
                // Inside the body, bare field names of r's record type resolve as r.field.
                // We resolve the designator's type to find which fields are available.
                let desig_str = self.designator_to_string(desig);

                // Resolve the record type name from the designator
                let var_name = &desig.ident.name;
                let mut type_name = self.var_types.get(var_name).cloned();

                // If var_name is not a direct variable, check if it's a field of an
                // enclosing WITH scope and resolve its type via record_field_types
                if type_name.is_none() {
                    for (_, fields, _) in self.with_aliases.iter().rev() {
                        if fields.contains(&var_name.to_string()) {
                            // Find the record type that contains this field
                            for (key, val) in &self.record_field_types {
                                if key.1 == *var_name {
                                    type_name = Some(val.clone());
                                    break;
                                }
                            }
                            break;
                        }
                    }
                }

                // Resolve through pointer_base_types if type_name points to a pointer-to-record
                if let Some(ref tn) = type_name {
                    let fields = self.record_fields.get(tn);
                    if fields.is_none() || fields.map_or(false, |f| f.is_empty()) {
                        if let Some(base) = self.pointer_base_types.get(tn).cloned() {
                            type_name = Some(base);
                        }
                    }
                }

                let field_names = if let Some(tn) = &type_name {
                    self.record_fields.get(tn).cloned().unwrap_or_default()
                } else {
                    Vec::new()
                };

                self.emitln("{");
                self.indent += 1;
                self.emit_indent();
                self.emit(&format!("/* WITH {} */\n", desig_str));
                self.with_aliases.push((desig_str.clone(), field_names, type_name.clone()));
                for s in body {
                    self.gen_statement(s);
                }
                self.with_aliases.pop();
                self.indent -= 1;
                self.emitln("}");
            }
            StatementKind::Return { expr } => {
                self.emit_indent();
                if let Some(e) = expr {
                    self.emit("return ");
                    self.gen_expr(e);
                    self.emit(";\n");
                } else if self.in_module_body {
                    self.emit("return 0;\n");
                } else {
                    self.emit("return;\n");
                }
            }
            StatementKind::Exit => {
                self.emitln("break;");
            }
            StatementKind::Raise { expr } => {
                self.emit_indent();
                if let Some(e) = expr {
                    // Check if the expression is a known exception name
                    if let ExprKind::Designator(d) = &e.kind {
                        if d.selectors.is_empty() && self.exception_names.contains(&d.ident.name) {
                            let exc_c = format!("M2_EXC_{}", self.mangle(&d.ident.name));
                            self.emit(&format!("m2_raise({}, \"{}\", NULL);\n", exc_c, d.ident.name));
                        } else {
                            self.emit("{ int _exc_id = (int)(");
                            self.gen_expr(e);
                            self.emit("); m2_raise(_exc_id, NULL, NULL); }\n");
                        }
                    } else {
                        self.emit("{ int _exc_id = (int)(");
                        self.gen_expr(e);
                        self.emit("); m2_raise(_exc_id, NULL, NULL); }\n");
                    }
                } else {
                    self.emitln("m2_raise(1, NULL, NULL);");
                }
            }
            StatementKind::Retry => {
                self.emitln("longjmp(m2_exception_buf, -1); /* RETRY */");
            }
            StatementKind::Try { body, excepts, finally_body } => {
                self.gen_try_statement(body, excepts, finally_body);
            }
            StatementKind::Lock { mutex, body } => {
                self.gen_lock_statement(mutex, body);
            }
            StatementKind::TypeCase { expr, branches, else_body } => {
                self.gen_typecase_statement(expr, branches, else_body);
            }
        }
    }

    // ── Expressions ─────────────────────────────────────────────────

}
