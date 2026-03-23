use super::*;

impl LLVMCodeGen {
    // ── Stdlib function declarations ────────────────────────────────

    /// Declare a non-native stdlib function (backed by C runtime).
    /// Native stdlib modules are compiled inline and should NOT use this path —
    /// their functions are defined by gen_embedded_impl_module / gen_proc_decl.
    pub(crate) fn declare_stdlib_function(&mut self, module: &str, name: &str) {
        let c_name = stdlib::map_stdlib_call(module, name)
            .unwrap_or_else(|| format!("{}_{}", module, name));
        let import_name = format!("{}_{}", module, name);

        if self.declared_fns.contains(&c_name) {
            self.stdlib_name_map.insert(import_name, c_name);
            return;
        }

        let sig = self.get_stdlib_signature(module, name);
        self.emit_preambleln(&format!("declare {} @{}({})",
            sig.return_type, c_name, sig.params_str));
        self.declared_fns.insert(c_name.clone());

        self.stdlib_name_map.insert(import_name.clone(), c_name.clone());

        if sig.return_type != "void" {
            self.fn_return_types.insert(c_name.clone(), sig.return_type.clone());
            self.fn_return_types.insert(import_name.clone(), sig.return_type.clone());
        }

        if !sig.param_infos.is_empty() {
            self.proc_params.insert(c_name.clone(), sig.param_infos.clone());
            self.proc_params.insert(import_name, sig.param_infos);
        }
    }

    pub(crate) fn get_stdlib_signature(&self, module: &str, name: &str) -> FnSig {
        // Helper to build open array param info
        let open_array_param = |n: &str| ParamLLVMInfo {
            name: n.to_string(), is_var: false, is_open_array: true, llvm_type: "ptr".to_string(),
            open_array_elem_type: Some("i8".to_string()),
        };
        let val_param = |n: &str, ty: &str| ParamLLVMInfo {
            name: n.to_string(), is_var: false, is_open_array: false, llvm_type: ty.to_string(),
            open_array_elem_type: None,
        };
        let var_param = |n: &str| ParamLLVMInfo {
            name: n.to_string(), is_var: true, is_open_array: false, llvm_type: "ptr".to_string(),
            open_array_elem_type: None,
        };

        // Common stdlib functions — emit correct LLVM signatures.
        // Normalize function name for case-insensitive matching (Modula-2 identifiers
        // may differ in case between def files and import sites).
        let name_lower = name.to_ascii_lowercase();
        let nl = name_lower.as_str();
        match (module, nl) {
            // ── InOut ──────────────────────────────────────────
            // C runtime signatures: m2_WriteString(const char *s), etc.
            // These match the C runtime's actual parameter lists.
            ("InOut", "writestring") => FnSig::with_params("void", "ptr",
                vec![val_param("s", "ptr")]),
            ("InOut", "writeint") => FnSig::with_params("void", "i32, i32",
                vec![val_param("n", "i32"), val_param("w", "i32")]),
            ("InOut", "writecard") => FnSig::with_params("void", "i32, i32",
                vec![val_param("n", "i32"), val_param("w", "i32")]),
            ("InOut", "writeln") => FnSig::new("void", ""),
            ("InOut", "write") => FnSig::with_params("void", "i8",
                vec![val_param("ch", "i8")]),
            ("InOut", "writehex") => FnSig::with_params("void", "i32, i32",
                vec![val_param("n", "i32"), val_param("w", "i32")]),
            ("InOut", "writeoct") => FnSig::with_params("void", "i32, i32",
                vec![val_param("n", "i32"), val_param("w", "i32")]),
            ("InOut", "read") => FnSig::with_params("void", "ptr",
                vec![var_param("ch")]),
            ("InOut", "readstring") => FnSig::with_params("void", "ptr",
                vec![val_param("s", "ptr")]),
            ("InOut", "readint") => FnSig::with_params("void", "ptr",
                vec![var_param("n")]),
            ("InOut", "readcard") => FnSig::with_params("void", "ptr",
                vec![var_param("n")]),
            ("InOut", "done") => FnSig::new("i32", ""),
            ("InOut", "eol") => FnSig::new("void", ""),
            ("InOut", "openinput") => FnSig::with_params("void", "ptr",
                vec![val_param("ext", "ptr")]),
            ("InOut", "openoutput") => FnSig::with_params("void", "ptr",
                vec![val_param("ext", "ptr")]),
            ("InOut", "closeinput") => FnSig::new("void", ""),
            ("InOut", "closeoutput") => FnSig::new("void", ""),
            // ── RealInOut ────────────────────────────────────
            ("RealInOut", "writereal") => FnSig::with_params("void", "float, i32",
                vec![val_param("r", "float"), val_param("w", "i32")]),
            ("RealInOut", "writelongreal") => FnSig::with_params("void", "double, i32",
                vec![val_param("r", "double"), val_param("w", "i32")]),
            ("RealInOut", "writefixpt") => FnSig::with_params("void", "float, i32, i32",
                vec![val_param("r", "float"), val_param("w", "i32"), val_param("d", "i32")]),
            ("RealInOut", "readreal") => FnSig::with_params("void", "ptr",
                vec![var_param("r")]),
            // ── MathLib ──────────────────────────────────────
            ("MathLib", "sqrt") => FnSig::with_params("float", "float",
                vec![val_param("x", "float")]),
            ("MathLib", "sin") => FnSig::with_params("float", "float",
                vec![val_param("x", "float")]),
            ("MathLib", "cos") => FnSig::with_params("float", "float",
                vec![val_param("x", "float")]),
            ("MathLib", "exp") => FnSig::with_params("float", "float",
                vec![val_param("x", "float")]),
            ("MathLib", "ln") => FnSig::with_params("float", "float",
                vec![val_param("x", "float")]),
            ("MathLib", "arctan") => FnSig::with_params("float", "float",
                vec![val_param("x", "float")]),
            ("MathLib", "entier") => FnSig::with_params("i32", "float",
                vec![val_param("x", "float")]),
            // ── Strings ──────────────────────────────────────
            // C runtime: m2_Strings_Assign(src, dst, dst_high)
            ("Strings", "length") => FnSig::with_params("i32", "ptr",
                vec![val_param("s", "ptr")]),
            ("Strings", "assign") => FnSig::with_params("void", "ptr, ptr, i32",
                vec![val_param("src", "ptr"), val_param("dst", "ptr"), val_param("dst_high", "i32")]),
            ("Strings", "concat") => FnSig::with_params("void", "ptr, ptr, ptr, i32",
                vec![val_param("a", "ptr"), val_param("b", "ptr"), val_param("dst", "ptr"), val_param("dst_high", "i32")]),
            ("Strings", "comparestr") => FnSig::with_params("i32", "ptr, ptr",
                vec![val_param("a", "ptr"), val_param("b", "ptr")]),
            ("Strings", "insert") => FnSig::with_params("void", "ptr, ptr, i32, i32",
                vec![val_param("sub", "ptr"), val_param("dst", "ptr"), val_param("dst_high", "i32"), val_param("pos", "i32")]),
            ("Strings", "delete") => FnSig::with_params("void", "ptr, i32, i32, i32",
                vec![val_param("s", "ptr"), val_param("s_high", "i32"), val_param("pos", "i32"), val_param("len", "i32")]),
            ("Strings", "copy") => FnSig::with_params("void", "ptr, i32, i32, ptr, i32",
                vec![val_param("src", "ptr"), val_param("pos", "i32"), val_param("len", "i32"), val_param("dst", "ptr"), val_param("dst_high", "i32")]),
            ("Strings", "pos") => FnSig::with_params("i32", "ptr, ptr",
                vec![val_param("sub", "ptr"), val_param("s", "ptr")]),
            // ── Storage ──────────────────────────────────────
            ("Storage", "allocate") => FnSig::with_params("void", "ptr, i32",
                vec![var_param("p"), val_param("size", "i32")]),
            ("Storage", "deallocate") => FnSig::with_params("void", "ptr, i32",
                vec![var_param("p"), val_param("size", "i32")]),
            // ── BinaryIO ────────────────────────────────────
            ("BinaryIO", "openread") => FnSig::with_params("void", "ptr, ptr",
                vec![val_param("name", "ptr"), var_param("fh")]),
            ("BinaryIO", "openwrite") => FnSig::with_params("void", "ptr, ptr",
                vec![val_param("name", "ptr"), var_param("fh")]),
            ("BinaryIO", "close") => FnSig::with_params("void", "i32",
                vec![val_param("fh", "i32")]),
            ("BinaryIO", "readbyte") => FnSig::with_params("void", "i32, ptr",
                vec![val_param("fh", "i32"), var_param("b")]),
            ("BinaryIO", "writebyte") => FnSig::with_params("void", "i32, i32",
                vec![val_param("fh", "i32"), val_param("b", "i32")]),
            ("BinaryIO", "readbytes") => FnSig::with_params("void", "i32, ptr, i32, ptr",
                vec![val_param("fh", "i32"), val_param("buf", "ptr"), val_param("n", "i32"), var_param("actual")]),
            ("BinaryIO", "writebytes") => FnSig::with_params("void", "i32, ptr, i32",
                vec![val_param("fh", "i32"), val_param("buf", "ptr"), val_param("n", "i32")]),
            ("BinaryIO", "filesize") => FnSig::with_params("void", "i32, ptr",
                vec![val_param("fh", "i32"), var_param("size")]),
            ("BinaryIO", "seek") => FnSig::with_params("void", "i32, i32",
                vec![val_param("fh", "i32"), val_param("pos", "i32")]),
            ("BinaryIO", "tell") => FnSig::with_params("void", "i32, ptr",
                vec![val_param("fh", "i32"), var_param("pos")]),
            ("BinaryIO", "iseof") => FnSig::with_params("i32", "i32",
                vec![val_param("fh", "i32")]),
            ("BinaryIO", "done") => FnSig::new("i32", ""),
            // ── Args ──────────────────────────────────────
            ("Args", "argcount") => FnSig::new("i32", ""),
            ("Args", "getarg") => {
                // GetArg(n: CARDINAL; VAR buf: ARRAY OF CHAR)
                // C runtime: m2_Args_GetArg(uint32_t n, char *buf, uint32_t buf_high)
                let buf_param = ParamLLVMInfo {
                    name: "buf".to_string(), is_var: true, is_open_array: true,
                    llvm_type: "ptr".to_string(), open_array_elem_type: Some("i8".to_string()),
                };
                FnSig { return_type: "void".to_string(), params_str: "i32, ptr, i32".to_string(),
                    param_infos: vec![val_param("n", "i32"), buf_param] }
            }
            _ => {
                // Default: void with no params — caller should override
                FnSig::new("void", "")
            }
        }
    }

    /// Generate call to Strings module functions with extra HIGH arguments.
    pub(crate) fn gen_strings_call(&mut self, name: &str, args: &[Expr]) {
        let runtime_name = self.stdlib_name_map.get(name).cloned().unwrap_or_else(|| name.to_string());

        // Helper: get address and HIGH for a destination array argument
        let get_dst = |this: &mut Self, arg: &Expr| -> (Val, String) {
            if let ExprKind::Designator(d) = &arg.kind {
                let addr = this.gen_designator_addr(d);
                // Get HIGH from the address type if possible
                let high = if addr.ty.starts_with('[') {
                    if let Some(n_str) = addr.ty.strip_prefix('[').and_then(|s| s.split(' ').next()) {
                        if let Ok(n) = n_str.parse::<usize>() {
                            format!("{}", n - 1)
                        } else { this.get_array_high(&d.ident.name) }
                    } else { this.get_array_high(&d.ident.name) }
                } else {
                    this.get_array_high(&d.ident.name)
                };
                (addr, high)
            } else {
                (this.gen_expr(arg), "0".to_string())
            }
        };

        if name.contains("Assign") && args.len() >= 2 {
            // Source is ARRAY OF CHAR — pass address, not loaded value
            let src = match &args[0].kind {
                ExprKind::Designator(d) => {
                    let addr = self.gen_designator_addr(d);
                    addr
                }
                _ => self.gen_expr(&args[0]),
            };
            let (dst_addr, high) = get_dst(self, &args[1]);
            self.emitln(&format!("  call void @{}(ptr {}, ptr {}, i32 {})",
                runtime_name, src.name, dst_addr.name, high));
        } else if name.contains("Concat") && args.len() >= 3 {
            let a = self.gen_expr(&args[0]);
            let b = self.gen_expr(&args[1]);
            let (dst_addr, high) = get_dst(self, &args[2]);
            self.emitln(&format!("  call void @{}(ptr {}, ptr {}, ptr {}, i32 {})",
                runtime_name, a.name, b.name, dst_addr.name, high));
        } else if name.contains("Insert") && args.len() >= 3 {
            let sub = self.gen_expr(&args[0]);
            let (dst_addr, high) = get_dst(self, &args[1]);
            let pos = self.gen_expr(&args[2]);
            self.emitln(&format!("  call void @{}(ptr {}, ptr {}, i32 {}, i32 {})",
                runtime_name, sub.name, dst_addr.name, high, pos.name));
        } else if name.contains("Delete") && args.len() >= 3 {
            let (s_addr, high) = get_dst(self, &args[0]);
            let pos = self.gen_expr(&args[1]);
            let len = self.gen_expr(&args[2]);
            self.emitln(&format!("  call void @{}(ptr {}, i32 {}, i32 {}, i32 {})",
                runtime_name, s_addr.name, high, pos.name, len.name));
        } else if name.contains("Copy") && args.len() >= 4 {
            let src = self.gen_expr(&args[0]);
            let pos = self.gen_expr(&args[1]);
            let len = self.gen_expr(&args[2]);
            let (dst_addr, high) = get_dst(self, &args[3]);
            self.emitln(&format!("  call void @{}(ptr {}, i32 {}, i32 {}, ptr {}, i32 {})",
                runtime_name, src.name, pos.name, len.name, dst_addr.name, high));
        } else {
            self.gen_call(&runtime_name, args, "void");
        }
    }

    pub(crate) fn gen_call(&mut self, name: &str, args: &[Expr], expected_ret: &str) -> Val {
        let param_info = self.proc_params.get(name).cloned().unwrap_or_default();

        let mut arg_vals = Vec::new();
        let mut arg_strs = Vec::new();

        for (i, arg) in args.iter().enumerate() {
            let info = param_info.get(i);
            let is_var = info.map(|p| p.is_var).unwrap_or(false);
            let is_open = info.map(|p| p.is_open_array).unwrap_or(false);

            if is_var && !is_open {
                // VAR param (not open array): pass address of the variable
                let addr = match &arg.kind {
                    ExprKind::Designator(d) => self.gen_designator_addr(d),
                    _ => {
                        let val = self.gen_expr(arg);
                        val // fallback
                    }
                };
                arg_strs.push(format!("ptr {}", addr.name));
            } else if is_open || (is_var && is_open) {
                // Open array: pass ptr + high
                match &arg.kind {
                    ExprKind::StringLit(s) => {
                        let (str_name, _) = self.intern_string(s);
                        arg_strs.push(format!("ptr {}", str_name));
                        arg_strs.push(format!("i32 {}", s.len().saturating_sub(1)));
                    }
                    ExprKind::Designator(d) => {
                        let addr = self.gen_designator_addr(d);
                        // String constants are stored as `global ptr @.str.N` —
                        // need to load the pointer to get the actual string data.
                        if self.string_const_lengths.contains_key(&d.ident.name) {
                            let tmp = self.next_tmp();
                            self.emitln(&format!("  {} = load ptr, ptr {}", tmp, addr.name));
                            arg_strs.push(format!("ptr {}", tmp));
                        } else {
                            arg_strs.push(format!("ptr {}", addr.name));
                        }
                        // Compute HIGH from the resolved address type first,
                        // then fall back to variable-level HIGH
                        let high = if addr.ty.starts_with('[') {
                            if let Some(n_str) = addr.ty.strip_prefix('[').and_then(|s| s.split(' ').next()) {
                                if let Ok(n) = n_str.parse::<usize>() {
                                    format!("{}", n.saturating_sub(1))
                                } else { self.get_array_high(&d.ident.name) }
                            } else { self.get_array_high(&d.ident.name) }
                        } else {
                            self.get_array_high(&d.ident.name)
                        };
                        arg_strs.push(format!("i32 {}", high));
                    }
                    _ => {
                        let val = self.gen_expr(arg);
                        arg_strs.push(format!("{} {}", val.ty, val.name));
                        arg_strs.push("i32 0".to_string());
                    }
                }
            } else {
                // Handle single-char string literal passed as i8 (CHAR) param
                let expected_ty = info.map(|p| p.llvm_type.as_str()).unwrap_or("");
                if expected_ty == "i8" {
                    if let ExprKind::StringLit(s) = &arg.kind {
                        if s.len() == 1 {
                            arg_strs.push(format!("i8 {}", s.as_bytes()[0]));
                            arg_vals.push(Val::new("", ""));
                            continue;
                        } else if s.is_empty() {
                            arg_strs.push("i8 0".to_string());
                            arg_vals.push(Val::new("", ""));
                            continue;
                        }
                    }
                }
                let val = self.gen_expr(arg);
                // Coerce to expected param type if known
                if let Some(pi) = info {
                    // Named array params are passed as ptr (gen_proc_decl converts them)
                    if pi.llvm_type.starts_with('[') {
                        // Pass address instead of loaded value
                        if let ExprKind::Designator(d) = &arg.kind {
                            let addr = self.gen_designator_addr(d);
                            arg_strs.push(format!("ptr {}", addr.name));
                        } else {
                            arg_strs.push(format!("ptr {}", val.name));
                        }
                    } else {
                        // Pass by value — reconcile param info type with actual value type
                        let pass_ty = if val.ty.starts_with('[') {
                            "ptr".to_string()
                        } else if val.ty.starts_with('{') && !pi.llvm_type.starts_with('{') {
                            // Value is a struct but param says scalar — use actual type
                            val.ty.clone()
                        } else if pi.llvm_type.starts_with('{') && val.ty == "ptr" {
                            // Param expects struct but value is ptr — load struct from ptr
                            let tmp = self.next_tmp();
                            self.emitln(&format!("  {} = load {}, ptr {}",
                                tmp, pi.llvm_type, val.name));
                            arg_strs.push(format!("{} {}", pi.llvm_type, tmp));
                            arg_vals.push(Val::new(tmp, pi.llvm_type.clone()));
                            continue;
                        } else {
                            pi.llvm_type.clone()
                        };
                        let coerced = self.coerce_val(&val, &pass_ty);
                        arg_strs.push(format!("{} {}", pass_ty, coerced.name));
                    }
                } else {
                    // Arrays and structs are always passed as ptr
                    let pass_ty = if val.ty.starts_with('[') || val.ty.starts_with('{') {
                        "ptr"
                    } else {
                        &val.ty
                    };
                    arg_strs.push(format!("{} {}", pass_ty, val.name));
                }
            }
            arg_vals.push(Val::new("", ""));
        }

        let args_str = arg_strs.join(", ");

        if let Some(ref unwind_dest) = self.try_unwind_dest.clone() {
            // Inside TRY body — use invoke for unwind support
            let cont = self.next_label("invoke.cont");
            if expected_ret == "void" {
                self.emitln(&format!("  invoke void @{}({}) to label %{} unwind label %{}",
                    name, args_str, cont, unwind_dest));
                self.emitln(&format!("{}:", cont));
                Val::new("", "void")
            } else {
                let tmp = self.next_tmp();
                self.emitln(&format!("  {} = invoke {} @{}({}) to label %{} unwind label %{}",
                    tmp, expected_ret, name, args_str, cont, unwind_dest));
                self.emitln(&format!("{}:", cont));
                Val::new(tmp, expected_ret.to_string())
            }
        } else {
            if expected_ret == "void" {
                self.emitln(&format!("  call void @{}({})", name, args_str));
                Val::new("", "void")
            } else {
                let tmp = self.next_tmp();
                self.emitln(&format!("  {} = call {} @{}({})", tmp, expected_ret, name, args_str));
                Val::new(tmp, expected_ret.to_string())
            }
        }
    }
}
