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
}
