use super::*;

impl CodeGen {
    pub(crate) fn register_proc_params(&mut self, h: &ProcHeading) {
        let mut param_info = Vec::new();
        for fp in &h.params {
            let is_open_array = matches!(fp.typ, TypeNode::OpenArray { .. });
            let is_char = matches!(&fp.typ, TypeNode::Named(qi) if qi.name == "CHAR");
            for name in &fp.names {
                param_info.push(ParamCodegenInfo {
                    name: name.clone(),
                    is_var: fp.is_var,
                    is_open_array,
                    is_char,
                });
            }
        }
        self.proc_params.insert(h.name.clone(), param_info.clone());
        if let Some(ref ecn) = h.export_c_name {
            self.export_c_names.insert(h.name.clone(), ecn.clone());
            self.proc_params.insert(ecn.clone(), param_info);
        }
        // Register procedure-typed parameters as their own callables
        // so calls like handler(req, resp) get correct VAR param info
        for fp in &h.params {
            if let TypeNode::Named(qi) = &fp.typ {
                if let Some(pinfo) = self.proc_type_params.get(&qi.name).cloned() {
                    for name in &fp.names {
                        self.proc_params.insert(name.clone(), pinfo.clone());
                    }
                }
            } else if let TypeNode::ProcedureType { params: pt_params, .. } = &fp.typ {
                // Inline procedure type: PROCEDURE(VAR Request, VAR Response, ADDRESS)
                let mut pinfo = Vec::new();
                for (idx, ptp) in pt_params.iter().enumerate() {
                    let is_open = matches!(ptp.typ, TypeNode::OpenArray { .. });
                    let is_ch = matches!(&ptp.typ, TypeNode::Named(qi) if qi.name == "CHAR");
                    for pname in &ptp.names {
                        pinfo.push(ParamCodegenInfo {
                            name: pname.clone(),
                            is_var: ptp.is_var,
                            is_open_array: is_open,
                            is_char: is_ch,
                        });
                    }
                    if ptp.names.is_empty() {
                        pinfo.push(ParamCodegenInfo {
                            name: format!("_p{}", idx),
                            is_var: ptp.is_var,
                            is_open_array: is_open,
                            is_char: is_ch,
                        });
                    }
                }
                for name in &fp.names {
                    self.proc_params.insert(name.clone(), pinfo.clone());
                }
            }
        }
    }

    // ── Declarations ────────────────────────────────────────────────

    pub(crate) fn gen_declaration(&mut self, decl: &Declaration) {
        match decl {
            Declaration::Const(c) => {
                let sym = self.sema.symtab.lookup_innermost(&c.name);
                if let Some(s) = sym {
                    if let crate::symtab::SymbolKind::Constant(cv) = &s.kind {
                        let val = crate::hir_build::const_value_to_hir(cv);
                        let hc = crate::hir::HirConstDecl {
                            name: c.name.clone(),
                            mangled: self.mangle(&c.name),
                            value: val.clone(),
                            type_id: s.typ,
                            exported: s.exported,
                            c_type: crate::hir_build::const_val_c_type(&val),
                        };
                        self.gen_hir_const_decl(&hc);
                    }
                }
            }
            Declaration::Type(t) => {
                let sym = self.sema.symtab.lookup_innermost(&t.name);
                let tid = sym.map(|s| s.typ).unwrap_or(crate::types::TY_VOID);
                if tid != crate::types::TY_VOID {
                    self.gen_type_decl_from_id(&t.name, tid);
                }
            }
            Declaration::Var(v) => {
                let sym = self.sema.symtab.lookup_innermost(&v.names[0]);
                let tid = sym.map(|s| s.typ).unwrap_or(TY_INTEGER);
                let resolved = self.resolve_hir_alias(tid);
                let is_proc = matches!(self.sema.types.get(resolved), crate::types::Type::ProcedureType { .. });
                let is_ptr_to_arr = if let crate::types::Type::Pointer { base } = self.sema.types.get(resolved) {
                    matches!(self.sema.types.get(self.resolve_hir_alias(*base)), crate::types::Type::Array { .. })
                } else { false };
                if is_proc {
                    let c_name = self.mangle_decl_name(&v.names[0]);
                    self.emit_indent();
                    let d = self.proc_type_decl_from_id(resolved, &c_name, false);
                    self.emit(&format!("{};\n", d));
                } else if is_ptr_to_arr {
                    if let crate::types::Type::Pointer { base } = self.sema.types.get(resolved) {
                        let (elem_c, arr_suffix) = self.field_type_and_suffix(*base);
                        for name in &v.names {
                            let c_name = self.mangle_decl_name(name);
                            self.emit_indent();
                            self.emit(&format!("{} (*{}){};\n", elem_c, c_name, arr_suffix));
                        }
                    }
                } else {
                    let (ctype, arr_suffix) = self.field_type_and_suffix(resolved);
                    for name in &v.names {
                        let c_name = self.mangle_decl_name(name);
                        self.emit_indent();
                        self.emit(&format!("{} {}{};\n", ctype, c_name, arr_suffix));
                    }
                }
            }
            Declaration::Procedure(p) => self.gen_proc_decl(p),
            Declaration::Module(m) => {
                let inside_proc = !self.parent_proc_stack.is_empty();
                self.emitln(&format!("/* Nested module {} */", m.name));
                for d in &m.block.decls {
                    if inside_proc && matches!(d, Declaration::Procedure(_)) {
                        continue;
                    }
                    self.gen_declaration(d);
                }
            }
            Declaration::Exception(e) => {
                self.exception_names.insert(e.name.clone());
                let mangled = format!("M2_EXC_{}", self.mangle(&e.name));
                self.emitln(&format!("#define {} __COUNTER__", mangled));
            }
        }
    }

    /// Emit a constant declaration from prebuilt HIR.
    /// Since HirConstDecl.value is already fully evaluated, no topo-sort needed.
    pub(crate) fn gen_hir_const_decl(&mut self, c: &crate::hir::HirConstDecl) {
        use crate::hir::ConstVal;
        // Register integer values for array bound inlining
        if let ConstVal::Integer(v) = &c.value {
            self.const_int_values.insert(c.name.clone(), *v);
            if let Some(ref mod_name) = self.generating_for_module {
                self.const_int_values.insert(format!("{}_{}", mod_name, c.name), *v);
            }
        }
        if let ConstVal::EnumVariant(v) = &c.value {
            self.const_int_values.insert(c.name.clone(), *v);
            if let Some(ref mod_name) = self.generating_for_module {
                self.const_int_values.insert(format!("{}_{}", mod_name, c.name), *v);
            }
        }
        let val_str = match &c.value {
            ConstVal::Integer(v) => format!("{}", v),
            ConstVal::Real(v) => {
                let s = format!("{}", v);
                if s.contains('.') || s.contains('e') || s.contains('E') { s } else { format!("{}.0", s) }
            }
            ConstVal::Boolean(b) => if *b { "1".to_string() } else { "0".to_string() },
            ConstVal::Char(ch) => format!("'{}'", escape_c_char(*ch)),
            ConstVal::String(s) => {
                if s.is_empty() {
                    "'\\0'".to_string()
                } else if s.len() == 1 {
                    format!("'{}'", escape_c_char(s.chars().next().unwrap()))
                } else {
                    format!("\"{}\"", escape_c_string(s))
                }
            }
            ConstVal::Set(bits) => format!("0x{:X}ULL", bits),
            ConstVal::Nil => "((void*)0)".to_string(),
            ConstVal::EnumVariant(v) => format!("{}", v),
        };
        self.emitln(&format!("static const {} {} = {};", c.c_type, c.mangled, val_str));
    }

    /// Return the C typedef name for a type declaration.
    /// Inside an embedded module, returns Module_TypeName to avoid collisions.
    pub(crate) fn type_decl_c_name(&self, bare_name: &str) -> String {
        if let Some(ref mod_name) = self.generating_for_module {
            format!("{}_{}", mod_name, self.mangle(bare_name))
        } else {
            self.mangle(bare_name)
        }
    }

    /// Emit a type declaration from TypeId (no AST bridge needed).
    /// Handles: Record, Enum, Pointer, Array, ProcedureType, Set, Subrange, Alias, Opaque.
    pub(crate) fn gen_type_decl_from_id(&mut self, name: &str, type_id: TypeId) {
        self.known_type_names.insert(name.to_string());
        if let Some(ref mod_name) = self.generating_for_module {
            self.known_type_names.insert(format!("{}_{}", mod_name, name));
        }
        let c_type_name = self.type_decl_c_name(name);
        if self.generating_for_module.is_some() {
            self.embedded_enum_types.insert(c_type_name.clone());
        }
        let resolved = self.resolve_hir_alias(type_id);
        // Register TypeId → C name, but never override builtin TypeIds (0..19)
        if type_id >= 20 {
            self.typeid_c_names.insert(type_id, c_type_name.clone());
        }
        if resolved != type_id && resolved >= 20 {
            self.typeid_c_names.insert(resolved, c_type_name.clone());
        }
        let ty = self.sema.types.get(resolved).clone();
        match &ty {
            crate::types::Type::Record { fields, variants } => {
                // Collect field metadata for WITH resolution
                let mut field_names = Vec::new();
                for f in fields {
                    field_names.push(f.name.clone());
                    if !f.type_name.is_empty() {
                        self.record_field_types.insert(
                            (name.to_string(), f.name.clone()),
                            f.type_name.clone(),
                        );
                        if let Some(pinfo) = self.proc_type_params.get(&f.type_name).cloned() {
                            self.field_proc_params.insert(f.name.clone(), pinfo);
                        }
                    }
                }
                self.record_fields.insert(name.to_string(), field_names);
                // Forward typedef + struct body
                self.emitln(&format!("typedef struct {} {};", c_type_name, c_type_name));
                self.emitln(&format!("struct {} {{", c_type_name));
                self.indent += 1;
                for f in fields {
                    let field_resolved = self.resolve_hir_alias(f.typ);
                    // For struct fields, resolve arrays to element type + suffix
                    // (matching emit_record_fields AST behavior: char field[65], not TypedefName field)
                    let (ctype, arr_suffix) = self.field_type_and_suffix(field_resolved);
                    if ctype == "char" && !arr_suffix.is_empty() {
                        self.char_array_fields.insert((name.to_string(), f.name.clone()));
                    }
                    if !arr_suffix.is_empty() || matches!(self.sema.types.get(field_resolved), crate::types::Type::Array { .. }) {
                        self.array_fields.insert((name.to_string(), f.name.clone()));
                    }
                    if matches!(self.sema.types.get(field_resolved), crate::types::Type::Pointer { .. }) {
                        self.pointer_fields.insert(f.name.clone());
                    }
                    // Multi-name pointer fields need separate declarations (C quirk)
                    let is_ptr = ctype.contains('*');
                    self.emit_indent();
                    self.emit(&format!("{} {}{};\n", ctype, f.name, arr_suffix));
                }
                if let Some(vi) = variants {
                    // Variant record — emit tag + union
                    if let Some(ref tag_name) = vi.tag_name {
                        self.emit_indent();
                        let tag_c = self.type_id_to_c(vi.tag_type);
                        self.emit(&format!("{} {};\n", tag_c, tag_name));
                    }
                    self.emitln("union {");
                    self.indent += 1;
                    for (i, vc) in vi.variants.iter().enumerate() {
                        self.emitln("struct {");
                        self.indent += 1;
                        for vf in &vc.fields {
                            let vft = self.type_id_to_c(vf.typ);
                            let vfs = self.type_id_array_suffix(vf.typ);
                            self.emit_indent();
                            self.emit(&format!("{} {}{};\n", vft, vf.name, vfs));
                            self.variant_field_map.insert(
                                (name.to_string(), vf.name.clone()), i);
                            if let Some(fields) = self.record_fields.get_mut(name) {
                                fields.push(vf.name.clone());
                            }
                        }
                        self.indent -= 1;
                        self.emitln(&format!("}} _v{};", i));
                    }
                    self.indent -= 1;
                    self.emitln("} _variant;");
                }
                self.indent -= 1;
                self.emitln("};");
            }
            crate::types::Type::Enumeration { ref variants, .. } => {
                self.emit_indent();
                self.emit("typedef enum { ");
                let type_name = &c_type_name;
                for (i, v) in variants.iter().enumerate() {
                    if i > 0 { self.emit(", "); }
                    let c_name = format!("{}_{}", type_name, v);
                    self.emit(&c_name);
                    if let Some(ref mod_name) = self.generating_for_module {
                        self.enum_variants.insert(format!("{}_{}", mod_name, v), c_name.clone());
                    }
                    if self.generating_for_module.is_none() {
                        self.enum_variants.insert(v.clone(), c_name);
                    }
                }
                self.emit(&format!(" }} {};\n", type_name));
                let n = variants.len();
                self.emitln(&format!("#define m2_min_{} 0", type_name));
                if n > 0 {
                    self.emitln(&format!("#define m2_max_{} {}", type_name, n - 1));
                }
            }
            crate::types::Type::Pointer { base } => {
                let base_id = *base;
                let base_resolved = self.resolve_hir_alias(base_id);
                let base_ty = self.sema.types.get(base_resolved).clone();
                if let crate::types::Type::Record { ref fields, .. } = base_ty {
                    let fields = fields.clone();
                    let tag = format!("{}_r", c_type_name);
                    self.emitln(&format!("typedef struct {} *{};", tag, c_type_name));
                    self.pointer_base_types.insert(c_type_name.clone(), tag.clone());
                    self.pointer_base_types.insert(name.to_string(), tag.clone());
                    let mut field_names = Vec::new();
                    for f in &fields {
                        field_names.push(f.name.clone());
                        if !f.type_name.is_empty() {
                            self.record_field_types.insert(
                                (name.to_string(), f.name.clone()), f.type_name.clone());
                            self.record_field_types.insert(
                                (tag.clone(), f.name.clone()), f.type_name.clone());
                        }
                    }
                    self.record_fields.insert(name.to_string(), field_names.clone());
                    self.record_fields.insert(tag.clone(), field_names);
                    self.emitln(&format!("struct {} {{", tag));
                    self.indent += 1;
                    for f in &fields {
                        let field_resolved = self.resolve_hir_alias(f.typ);
                        let (ctype, arr_suffix) = self.field_type_and_suffix(field_resolved);
                        self.emit_indent();
                        self.emit(&format!("{} {}{};\n", ctype, f.name, arr_suffix));
                    }
                    self.indent -= 1;
                    self.emitln("};");
                } else {
                    let base_c = self.type_id_to_c(base_id);
                    self.emit_indent();
                    self.emit(&format!("typedef {} *{};\n", base_c, c_type_name));
                }
            }
            crate::types::Type::Array { elem_type, high, .. } => {
                let elem_tid = *elem_type;
                let arr_high = *high;
                if elem_tid == TY_CHAR {
                    self.char_array_types.insert(name.to_string());
                    self.char_array_types.insert(c_type_name.clone());
                }
                self.array_types.insert(name.to_string());
                self.array_types.insert(c_type_name.clone());
                self.array_type_high.insert(name.to_string(), format!("{}", arr_high));
                self.array_type_high.insert(c_type_name.clone(), format!("{}", arr_high));
                // Use elem type directly (not resolved array TypeId which maps to itself)
                let ctype = self.type_id_to_c(elem_tid);
                let suffix = format!("[{} + 1]", arr_high);
                self.emit_indent();
                self.emit(&format!("typedef {} {}{};\n", ctype, c_type_name, suffix));
            }
            crate::types::Type::ProcedureType { params, return_type } => {
                // Register param info
                let pinfo: Vec<ParamCodegenInfo> = params.iter().enumerate().map(|(i, p)| {
                    ParamCodegenInfo {
                        name: format!("_p{}", i),
                        is_var: p.is_var,
                        is_open_array: matches!(self.sema.types.get(p.typ), crate::types::Type::OpenArray { .. }),
                        is_char: p.typ == TY_CHAR,
                    }
                }).collect();
                self.proc_type_params.insert(name.to_string(), pinfo);
                // Emit typedef
                self.emit_indent();
                let ret = match return_type { Some(rt) => self.type_id_to_c(*rt), None => "void".to_string() };
                self.emit(&format!("typedef {} (*{})(", ret, c_type_name));
                if params.is_empty() {
                    self.emit("void");
                } else {
                    let mut first = true;
                    for p in params {
                        if !first { self.emit(", "); }
                        first = false;
                        let pt = self.type_id_to_c(p.typ);
                        let is_open = matches!(self.sema.types.get(p.typ), crate::types::Type::OpenArray { .. });
                        if is_open {
                            self.emit(&format!("{} *, uint32_t", pt));
                        } else if p.is_var {
                            self.emit(&format!("{} *", pt));
                        } else {
                            self.emit(&pt);
                        }
                    }
                }
                self.emit(");\n");
            }
            crate::types::Type::Set { base } => {
                let base_id = *base;
                let base_ty = self.sema.types.get(base_id).clone();
                if let crate::types::Type::Enumeration { ref variants, .. } = base_ty {
                    let variants = variants.clone();
                    let enum_name = format!("{}_enum", c_type_name);
                    self.emit_indent();
                    self.emit("typedef enum { ");
                    for (i, v) in variants.iter().enumerate() {
                        if i > 0 { self.emit(", "); }
                        let c_name = format!("{}_{}", c_type_name, v);
                        self.emit(&c_name);
                        if let Some(ref mod_name) = self.generating_for_module {
                            self.enum_variants.insert(format!("{}_{}", mod_name, v), c_name.clone());
                        }
                        if self.generating_for_module.is_none() {
                            self.enum_variants.insert(v.clone(), c_name);
                        }
                    }
                    self.emit(&format!(" }} {};\n", enum_name));
                    self.emitln(&format!("typedef uint32_t {};", c_type_name));
                    let n = variants.len();
                    self.emitln(&format!("#define m2_min_{} 0", c_type_name));
                    if n > 0 {
                        self.emitln(&format!("#define m2_max_{} {}", c_type_name, n - 1));
                    }
                } else {
                    self.emit_indent();
                    self.emit(&format!("typedef uint32_t {};\n", c_type_name));
                }
            }
            crate::types::Type::Subrange { low, high, .. } => {
                self.emit_indent();
                self.emit(&format!("typedef int32_t {};\n", c_type_name));
                self.emitln(&format!("#define m2_min_{} {}", c_type_name, low));
                self.emitln(&format!("#define m2_max_{} {}", c_type_name, high));
            }
            crate::types::Type::Opaque { .. } => {
                self.emit_indent();
                self.emit(&format!("typedef void *{};\n", c_type_name));
            }
            crate::types::Type::Alias { target, name: alias_name, .. } => {
                // Track unsigned aliases
                if *target == TY_CARDINAL || *target == TY_LONGCARD
                    || self.unsigned_type_aliases.contains(alias_name) {
                    self.unsigned_type_aliases.insert(name.to_string());
                    self.unsigned_type_aliases.insert(c_type_name.clone());
                }
                let ctype = self.type_id_to_c(*target);
                self.emit_indent();
                self.emit(&format!("typedef {} {};\n", ctype, c_type_name));
            }
            _ => {
                // Fallback: emit structural C type directly (bypass typeid_c_names
                // to avoid circular typedef when a named type aliases a builtin)
                let ctype = match self.sema.types.get(resolved) {
                    crate::types::Type::Integer => "int32_t",
                    crate::types::Type::Cardinal => "uint32_t",
                    crate::types::Type::Real => "float",
                    crate::types::Type::LongReal => "double",
                    crate::types::Type::Boolean => "int",
                    crate::types::Type::Char => "char",
                    crate::types::Type::Bitset => "uint32_t",
                    crate::types::Type::Address => "void *",
                    crate::types::Type::LongInt => "int64_t",
                    crate::types::Type::LongCard => "uint64_t",
                    crate::types::Type::Word => "uint32_t",
                    crate::types::Type::Byte => "uint8_t",
                    crate::types::Type::Complex => "float _Complex",
                    crate::types::Type::LongComplex => "double _Complex",
                    _ => "int32_t",
                };
                // Track unsigned aliases
                if matches!(self.sema.types.get(resolved),
                    crate::types::Type::Cardinal | crate::types::Type::LongCard) {
                    self.unsigned_type_aliases.insert(name.to_string());
                    self.unsigned_type_aliases.insert(c_type_name.clone());
                }
                self.emit_indent();
                self.emit(&format!("typedef {} {};\n", ctype, c_type_name));
            }
        }
        self.newline();
    }

    /// Collect field metadata (names, types) for a record's fields and register in tracking maps.
    /// Returns the list of field names. `record_name` is the key used in record_fields/record_field_types.

    /// Emit a global variable declaration from prebuilt HIR using TypeId resolution.
    pub(crate) fn gen_hir_global_decl(&mut self, g: &crate::hir::HirGlobalDecl) {
        let tid = g.type_id;
        let resolved = self.resolve_hir_alias(tid);

        // Metadata registration from TypeId
        if let Some(c_name) = self.typeid_c_names.get(&tid).or_else(|| self.typeid_c_names.get(&resolved)).cloned() {
            self.var_types.insert(g.name.clone(), c_name.clone());
            if self.char_array_types.contains(&c_name) { self.char_array_vars.insert(g.name.clone()); }
            if self.array_types.contains(&c_name) { self.array_vars.insert(g.name.clone()); }
            if let Some(pinfo) = self.proc_type_params.get(&c_name).cloned() {
                self.proc_params.insert(g.name.clone(), pinfo);
            }
            if self.unsigned_type_aliases.contains(&c_name) {
                self.cardinal_vars.insert(g.name.clone());
                self.longcard_vars.insert(g.name.clone());
            }
        } else if let crate::types::Type::Alias { name, .. } = self.sema.types.get(tid) {
            self.var_types.insert(g.name.clone(), name.clone());
            if self.char_array_types.contains(name) { self.char_array_vars.insert(g.name.clone()); }
            if self.array_types.contains(name) { self.array_vars.insert(g.name.clone()); }
        }
        if let crate::types::Type::Array { elem_type, .. } = self.sema.types.get(resolved) {
            let elem_c = self.type_id_to_c(*elem_type);
            self.array_var_elem_types.insert(g.name.clone(), elem_c);
            self.array_vars.insert(g.name.clone());
            if *elem_type == TY_CHAR { self.char_array_vars.insert(g.name.clone()); }
        }
        match self.sema.types.get(resolved) {
            crate::types::Type::Set { .. } | crate::types::Type::Bitset => { self.set_vars.insert(g.name.clone()); }
            crate::types::Type::Complex => { self.complex_vars.insert(g.name.clone()); }
            crate::types::Type::LongComplex => { self.longcomplex_vars.insert(g.name.clone()); }
            crate::types::Type::ProcedureType { params, .. } => {
                let pinfo: Vec<ParamCodegenInfo> = params.iter().enumerate().map(|(i, p)| {
                    ParamCodegenInfo {
                        name: format!("_p{}", i),
                        is_var: p.is_var,
                        is_open_array: matches!(self.sema.types.get(p.typ), crate::types::Type::OpenArray { .. }),
                        is_char: p.typ == TY_CHAR,
                    }
                }).collect();
                self.proc_params.insert(g.name.clone(), pinfo);
            }
            _ => {}
        }
        if resolved == TY_CARDINAL || resolved == TY_LONGCARD { self.cardinal_vars.insert(g.name.clone()); }
        if resolved == TY_LONGINT { self.longint_vars.insert(g.name.clone()); }
        if resolved == TY_LONGCARD { self.longcard_vars.insert(g.name.clone()); }

        // C emission via TypeId resolver
        let c_name = self.mangle_decl_name(&g.name);
        let is_proc = matches!(self.sema.types.get(resolved), crate::types::Type::ProcedureType { .. });
        let is_ptr_to_arr = if let crate::types::Type::Pointer { base } = self.sema.types.get(resolved) {
            matches!(self.sema.types.get(self.resolve_hir_alias(*base)), crate::types::Type::Array { .. })
        } else { false };

        if is_proc {
            self.emit_indent();
            let decl = self.proc_type_decl_from_id(resolved, &c_name, false);
            self.emit(&format!("{};\n", decl));
        } else if is_ptr_to_arr {
            if let crate::types::Type::Pointer { base } = self.sema.types.get(resolved) {
                let elem_c = self.type_id_to_c(*base);
                let arr_suffix = self.type_id_array_suffix(*base);
                self.emit_indent();
                self.emit(&format!("{} (*{}){};\n", elem_c, c_name, arr_suffix));
            }
        } else {
            // Resolve to element type + suffix for correct C multi-dimensional arrays
            let (ctype, array_suffix) = self.field_type_and_suffix(resolved);
            self.emit_indent();
            self.emit(&format!("{} {}{};\n", ctype, c_name, array_suffix));
        }
    }

    pub(crate) fn gen_proc_decl(&mut self, p: &ProcDecl) {
        let current_module = self.module_name.clone();
        let early_sig = self.prebuilt_hir.as_ref().and_then(|hir| {
            hir.proc_decls.iter()
                .find(|pd| pd.sig.name == p.heading.name && pd.sig.module == current_module)
                .map(|pd| pd.sig.clone())
                .or_else(|| hir.embedded_modules.iter()
                    .find(|e| e.name == current_module)
                    .and_then(|e| e.procedures.iter()
                        .find(|pd| pd.sig.name == p.heading.name)
                        .map(|pd| pd.sig.clone())))
        });
        if let Some(ref sig) = early_sig {
            self.register_hir_proc_params(sig);
        } else {
            self.register_proc_params(&p.heading);
        }

        // Collect nested procedure declarations and other declarations
        // Also hoist procedures from local modules inside this procedure
        let mut nested_procs = Vec::new();
        let mut other_decls = Vec::new();
        for decl in &p.block.decls {
            match decl {
                Declaration::Procedure(np) => {
                    nested_procs.push(np.clone());
                }
                Declaration::Module(m) => {
                    // Hoist procs from local module (illegal to define C functions inside C functions)
                    for d in &m.block.decls {
                        if let Declaration::Procedure(np) = d {
                            nested_procs.push(np.clone());
                        }
                    }
                    other_decls.push(decl);
                }
                _ => {
                    other_decls.push(decl);
                }
            }
        }

        // ── Closure analysis for nested procedures ──────────────────────
        // Build the set of variables available in this scope (params + locals + env vars)
        let mut scope_vars = self.build_scope_vars(p);
        // Also include vars this proc received through its own env (for deep nesting)
        if let Some(my_env_vars) = self.env_access_names.last() {
            for env_var in my_env_vars {
                if !scope_vars.contains_key(env_var) {
                    // Look up the type from the env struct fields
                    if let Some(my_env_type) = self.closure_env_type.get(&p.heading.name).cloned() {
                        if let Some(fields) = self.closure_env_fields.get(&my_env_type) {
                            for (fname, ftype) in fields {
                                if fname == env_var {
                                    scope_vars.insert(env_var.clone(), ftype.clone());
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        // Compute captures for each nested proc
        let env_type_name = format!("{}_env", p.heading.name);
        let mut all_captures: Vec<(String, String)> = Vec::new(); // (var_name, c_type) union
        let mut child_capture_info: Vec<(String, Vec<String>)> = Vec::new(); // (proc_name, [var_names])
        let mut has_any_captures = false;

        // Build TypeId-based outer_vars for unified HIR capture analysis
        let scope_vars_tid: HashMap<String, crate::types::TypeId> = scope_vars.keys()
            .map(|name| {
                let tid = self.sema.symtab.lookup_any(name)
                    .map(|s| s.typ)
                    .unwrap_or(crate::types::TY_INTEGER);
                (name.clone(), tid)
            })
            .collect();
        let imported_mods: HashSet<String> = self.imported_modules.iter().cloned().collect();

        for np in &nested_procs {
            let hir_captures = crate::hir_build::compute_captures(
                np, &scope_vars_tid, &self.import_map, &imported_mods,
            );
            let captures: Vec<String> = hir_captures.iter().map(|c| c.name.clone()).collect();
            if !captures.is_empty() {
                has_any_captures = true;
                // Add to union env struct
                for cap_name in &captures {
                    if !all_captures.iter().any(|(n, _)| n == cap_name) {
                        let c_type = scope_vars.get(cap_name).cloned().unwrap_or("int32_t".to_string());
                        all_captures.push((cap_name.clone(), c_type));
                    }
                }
                // Register this nested proc as receiving the env
                self.closure_env_type.insert(np.heading.name.clone(), env_type_name.clone());
                child_capture_info.push((np.heading.name.clone(), captures));
            }
        }

        if has_any_captures {
            // Generate the env struct typedef
            self.emitln(&format!("typedef struct {{"));
            self.indent += 1;
            for (name, c_type) in &all_captures {
                self.emitln(&format!("{} *{};", c_type, name));
            }
            self.indent -= 1;
            self.emitln(&format!("}} {};", env_type_name));
            self.newline();

            // Store env fields for later use
            self.closure_env_fields.insert(env_type_name.clone(), all_captures.clone());
        }

        // Push closure context for generating nested procs
        self.child_env_type_stack.push(if has_any_captures { Some(env_type_name.clone()) } else { None });
        self.child_captures_stack.push(child_capture_info.clone());

        // Push parent proc name — stays for entire proc scope (nested proc mangling + Module skip)
        self.parent_proc_stack.push(p.heading.name.clone());

        // Generate nested procs (lifted to top level, with env param if they have captures)
        for np in &nested_procs {
            // Register mangled name for nested procs if we have a parent
            let mangled = format!("{}_{}", p.heading.name, np.heading.name);
            self.nested_proc_names.insert(np.heading.name.clone(), mangled);
            // If this nested proc has captures, push its env access names
            if let Some(_) = self.closure_env_type.get(&np.heading.name) {
                // Compute which vars this specific proc (and its descendants) needs from outer scopes
                let np_hir_caps = crate::hir_build::compute_captures(
                    &np, &scope_vars_tid, &self.import_map, &imported_mods,
                );
                let np_captures: Vec<String> = np_hir_caps.iter().map(|c| c.name.clone()).collect();
                self.env_access_names.push(np_captures.iter().cloned().collect());
            }
            self.gen_proc_decl(&np);
            // Pop env access names if we pushed them
            if self.closure_env_type.contains_key(&np.heading.name) {
                self.env_access_names.pop();
            }
        }

        // Pop closure context
        self.child_env_type_stack.pop();
        self.child_captures_stack.pop();

        // ── Generate this procedure ─────────────────────────────────────
        self.newline();
        self.emit_line_directive(&p.loc);
        if let Some(ref sig) = early_sig {
            self.gen_hir_proc_prototype(sig);
        } else {
            self.gen_proc_prototype(&p.heading);
        }
        self.emit(" {\n");
        self.indent += 1;

        // Push a new VAR param scope and register VAR params
        // Note: VAR open array params are already pointers, so don't register them
        // as VAR (which would cause double dereferencing with (*a)[i])
        self.push_var_scope();
        // Save array var tracking so procedure-local names don't collide with outer scope
        let saved_var_tracking = self.save_var_tracking();
        for fp in &p.heading.params {
            if matches!(fp.typ, TypeNode::OpenArray { .. }) {
                for name in &fp.names {
                    let mangled = self.mangle(name);
                    if let Some(scope) = self.open_array_params.last_mut() {
                        scope.insert(mangled);
                    }
                    // Register _high companion in var_types so HIR can
                    // resolve HIGH(param) via the _high variable.
                    let high_name = format!("{}_high", name);
                    self.var_types.insert(high_name, "uint32_t".to_string());
                }
            } else if fp.is_var {
                for name in &fp.names {
                    self.register_var_param(name);
                }
            }
            // Track named-array value params (array decays to pointer in C)
            if !fp.is_var && !matches!(fp.typ, TypeNode::OpenArray { .. }) {
                if let TypeNode::Named(qi) = &fp.typ {
                    if self.array_types.contains(&qi.name) {
                        for name in &fp.names {
                            if let Some(scope) = self.named_array_value_params.last_mut() {
                                scope.insert(name.clone());
                            }
                        }
                    }
                }
            }
            // Register param type names for designator type tracking
            if let TypeNode::Named(qi) = &fp.typ {
                if qi.module.is_none() {
                    for name in &fp.names {
                        self.var_types.insert(name.clone(), qi.name.clone());
                    }
                }
            }
            // Track CARDINAL/LONGCARD params for unsigned DIV/MOD
            if matches!(&fp.typ, TypeNode::Named(qi) if qi.name == "CARDINAL" || qi.name == "LONGCARD") {
                for name in &fp.names {
                    self.cardinal_vars.insert(name.clone());
                }
            }
            // Track LONGINT params for 64-bit signed DIV/MOD
            if matches!(&fp.typ, TypeNode::Named(qi) if qi.name == "LONGINT") {
                for name in &fp.names {
                    self.longint_vars.insert(name.clone());
                }
            }
            // Track LONGCARD params for 64-bit detection
            if matches!(&fp.typ, TypeNode::Named(qi) if qi.name == "LONGCARD") {
                for name in &fp.names {
                    self.longcard_vars.insert(name.clone());
                }
            }
            // Also track params whose type aliases resolve to CARDINAL/LONGCARD
            // (e.g. Timestamp = LONGCARD)
            if let TypeNode::Named(qi) = &fp.typ {
                if self.unsigned_type_aliases.contains(&qi.name) {
                    for name in &fp.names {
                        self.cardinal_vars.insert(name.clone());
                        self.longcard_vars.insert(name.clone());
                    }
                }
            }
        }

        // Local declarations from HirProcDecl.locals (TypeId-based)
        let current_module = self.module_name.clone();
        let proc_locals = self.prebuilt_hir.as_ref().and_then(|hir| {
            // Search HirProc (legacy, populated by build_proc with correct scope)
            hir.procedures.iter()
                .find(|hp| hp.name.source_name == p.heading.name
                    && hp.name.module.as_deref() == Some(current_module.as_str()))
                .map(|hp| hp.locals.clone())
                .or_else(|| {
                    hir.procedures.iter()
                        .flat_map(|hp| hp.nested_procs.iter())
                        .find(|np| np.name.source_name == p.heading.name
                            && np.name.module.as_deref() == Some(current_module.as_str()))
                        .map(|np| np.locals.clone())
                })
        });
        if let Some(ref locals) = proc_locals {
            for local in locals {
                match local {
                    crate::hir::HirLocalDecl::Var { name, type_id } => {
                        let resolved = self.resolve_hir_alias(*type_id);
                        let c_name = self.mangle_decl_name(name);
                        let is_proc = matches!(self.sema.types.get(resolved), crate::types::Type::ProcedureType { .. });
                        let is_ptr_to_arr = if let crate::types::Type::Pointer { base } = self.sema.types.get(resolved) {
                            matches!(self.sema.types.get(self.resolve_hir_alias(*base)), crate::types::Type::Array { .. })
                        } else { false };
                        if is_proc {
                            self.emit_indent();
                            let d = self.proc_type_decl_from_id(resolved, &c_name, false);
                            self.emit(&format!("{};\n", d));
                        } else if is_ptr_to_arr {
                            if let crate::types::Type::Pointer { base } = self.sema.types.get(resolved) {
                                let (elem_c, arr_suffix) = self.field_type_and_suffix(*base);
                                self.emit_indent();
                                self.emit(&format!("{} (*{}){};\n", elem_c, c_name, arr_suffix));
                            }
                        } else {
                            let (ctype, arr_suffix) = self.field_type_and_suffix(resolved);
                            self.emit_indent();
                            self.emit(&format!("{} {}{};\n", ctype, c_name, arr_suffix));
                        }
                    }
                    crate::hir::HirLocalDecl::Type { name, type_id } => {
                        self.gen_type_decl_from_id(name, *type_id);
                    }
                    crate::hir::HirLocalDecl::Const(hc) => {
                        self.gen_hir_const_decl(hc);
                    }
                    crate::hir::HirLocalDecl::Exception { name, mangled, exc_id } => {
                        self.exception_names.insert(name.clone());
                        self.emitln(&format!("#define {} {}", mangled, exc_id));
                    }
                }
            }
        } else {
            // Proc not in hir.procedures (e.g., deeply nested or native stdlib)
            for decl in &other_decls {
                self.gen_declaration(decl);
            }
        }
        // Module declarations still via AST (nested modules)
        for decl in &other_decls {
            if let Declaration::Module(_) = decl {
                self.gen_declaration(decl);
            }
        }

        // If this proc has nested procs with captures, declare and init the child env
        if has_any_captures {
            self.emitln(&format!("{} _child_env;", env_type_name));
            for (cap_name, _cap_type) in &all_captures {
                self.emit_indent();
                if self.is_env_var(cap_name) {
                    // Forward from our own env
                    self.emit(&format!("_child_env.{} = _env->{};\n", cap_name, cap_name));
                } else if self.is_var_param(cap_name) {
                    // VAR param: already a pointer
                    self.emit(&format!("_child_env.{} = {};\n", cap_name, cap_name));
                } else {
                    // Regular local/param: take address
                    self.emit(&format!("_child_env.{} = &{};\n", cap_name, self.mangle(cap_name)));
                }
            }
        }

        // Push child env context for call site generation in body
        self.child_env_type_stack.push(if has_any_captures { Some(env_type_name.clone()) } else { None });
        self.child_captures_stack.push(child_capture_info);

        // Body (with optional EXCEPT handler)
        let has_except = p.block.except.is_some();
        if has_except {
            self.emitln("m2_exception_active = 1;");
            self.emitln("if (setjmp(m2_exception_buf) == 0) {");
            self.indent += 1;
        }

        // Use prebuilt HIR body if available — search top-level and nested procs
        let prebuilt_body = self.prebuilt_hir.as_ref().and_then(|hir| {
            let proc_name = &p.heading.name;
            let mod_name = &self.module_name;
            // Search top-level procs
            hir.procedures.iter()
                .find(|hp| hp.name.source_name == *proc_name
                    && hp.name.module.as_deref() == Some(mod_name.as_str()))
                .and_then(|hp| hp.body.clone())
                .or_else(|| {
                    // Search nested procs within top-level procs
                    hir.procedures.iter()
                        .flat_map(|hp| hp.nested_procs.iter())
                        .find(|np| np.name.source_name == *proc_name
                            && np.name.module.as_deref() == Some(mod_name.as_str()))
                        .and_then(|np| np.body.clone())
                })
        });
        if let Some(body) = prebuilt_body {
            for stmt in &body {
                self.emit_hir_stmt(stmt);
            }
        } else if let Some(stmts) = &p.block.body {
            // No prebuilt HIR found — this is unexpected if build_module ran.
            // Fall through to empty body (the procedure has no statements).
            let _ = stmts;
        }

        if has_except {
            self.indent -= 1;
            self.emitln("} else {");
            self.indent += 1;
            self.emitln("/* EXCEPT handler */");
            if let Some(except_stmts) = &p.block.except {
                let mut hb = self.make_hir_builder();
                let hir_stmts = hb.lower_stmts(except_stmts);
                for stmt in &hir_stmts { self.emit_hir_stmt(stmt); }
            }
            self.indent -= 1;
            self.emitln("}");
            self.emitln("m2_exception_active = 0;");
        }

        self.child_env_type_stack.pop();
        self.child_captures_stack.pop();
        self.restore_var_tracking(saved_var_tracking);
        self.pop_var_scope();
        self.parent_proc_stack.pop();
        self.indent -= 1;
        self.emitln("}");
    }

    pub(crate) fn gen_proc_prototype(&mut self, h: &ProcHeading) {
        self.emit_indent();
        let ret_type = if let Some(rt) = &h.return_type {
            self.type_to_c(rt)
        } else {
            "void".to_string()
        };
        let c_name = if let Some(ref ecn) = h.export_c_name {
            ecn.clone()
        } else if let Some(mangled) = self.nested_proc_names.get(&h.name).cloned() {
            // Nested proc: use parent-prefixed mangled name
            mangled
        } else {
            self.mangle(&h.name)
        };
        self.emit(&format!("{} {}(", ret_type, c_name));

        // Check if this proc receives a closure environment
        let env_type = self.closure_env_type.get(&h.name).cloned();
        let has_env = env_type.is_some();

        if has_env {
            let et = env_type.unwrap();
            self.emit(&format!("{} *_env", et));
        }

        if h.params.is_empty() && !has_env {
            self.emit("void");
        } else {
            let mut first = !has_env;
            for fp in &h.params {
                let is_open_array = matches!(fp.typ, TypeNode::OpenArray { .. });
                for name in &fp.names {
                    if !first {
                        self.emit(", ");
                    }
                    first = false;
                    let c_param = self.mangle(name);
                    if is_open_array {
                        let ctype = self.type_to_c(&fp.typ);
                        self.emit(&format!("{} *{}, uint32_t {}_high", ctype, c_param, c_param));
                    } else if Self::is_proc_type(&fp.typ) {
                        let decl = self.proc_type_decl(&fp.typ, &c_param, fp.is_var);
                        self.emit(&decl);
                    } else if fp.is_var {
                        let ctype = self.type_to_c(&fp.typ);
                        self.emit(&format!("{} *{}", ctype, c_param));
                    } else {
                        let ctype = self.type_to_c(&fp.typ);
                        self.emit(&format!("{} {}", ctype, c_param));
                    }
                }
            }
        }
        self.emit(")");
    }

    /// Register procedure parameter metadata from HirProcSig.
    pub(crate) fn register_hir_proc_params(&mut self, sig: &crate::hir::HirProcSig) {
        let mut param_info = Vec::new();
        for p in &sig.params {
            param_info.push(ParamCodegenInfo {
                name: p.name.clone(),
                is_var: p.is_var,
                is_open_array: p.is_open_array,
                is_char: p.is_char,
            });
        }
        self.proc_params.insert(sig.name.clone(), param_info.clone());
        if let Some(ref ecn) = sig.export_c_name {
            self.export_c_names.insert(sig.name.clone(), ecn.clone());
            self.proc_params.insert(ecn.clone(), param_info);
        }
        // Register procedure-typed parameters as their own callables (via TypeId)
        for p in &sig.params {
            let resolved = self.resolve_hir_alias(p.type_id);
            // Check if param type is a named alias to a procedure type
            if let crate::types::Type::Alias { name, .. } = self.sema.types.get(p.type_id) {
                if let Some(pinfo) = self.proc_type_params.get(name).cloned() {
                    self.proc_params.insert(p.name.clone(), pinfo);
                    continue;
                }
            }
            // Check if param type is directly a procedure type
            if let crate::types::Type::ProcedureType { params: pt_params, .. } = self.sema.types.get(resolved) {
                let pinfo: Vec<ParamCodegenInfo> = pt_params.iter().enumerate().map(|(idx, pt)| {
                    ParamCodegenInfo {
                        name: format!("_p{}", idx),
                        is_var: pt.is_var,
                        is_open_array: matches!(self.sema.types.get(pt.typ), crate::types::Type::OpenArray { .. }),
                        is_char: pt.typ == TY_CHAR,
                    }
                }).collect();
                self.proc_params.insert(p.name.clone(), pinfo);
            }
        }
    }

    /// Emit a procedure prototype from HirProcSig using pure TypeId resolution.
    pub(crate) fn gen_hir_proc_prototype(&mut self, sig: &crate::hir::HirProcSig) {
        self.emit_indent();
        let c_name = if let Some(ref ecn) = sig.export_c_name {
            ecn.clone()
        } else if let Some(mangled) = self.nested_proc_names.get(&sig.name).cloned() {
            mangled
        } else {
            self.mangle(&sig.name)
        };
        let ret_type = match sig.return_type {
            Some(rt) => self.type_id_to_c(rt),
            None => "void".to_string(),
        };
        self.emit(&format!("{} {}(", ret_type, c_name));

        let env_type = self.closure_env_type.get(&sig.name).cloned();
        let has_env = env_type.is_some();
        if has_env {
            self.emit(&format!("{} *_env", env_type.unwrap()));
        }

        if sig.params.is_empty() && !has_env {
            self.emit("void");
        } else {
            let mut first = !has_env;
            for p in &sig.params {
                if !first { self.emit(", "); }
                first = false;
                let c_param = self.mangle(&p.name);
                let resolved_tid = self.resolve_hir_alias(p.type_id);
                let is_proc = p.is_proc_type
                    || matches!(self.sema.types.get(resolved_tid), crate::types::Type::ProcedureType { .. });
                if p.is_open_array {
                    let c_type = self.type_id_to_c(p.type_id);
                    self.emit(&format!("{} *{}, uint32_t {}_high", c_type, c_param, c_param));
                } else if is_proc {
                    let decl = self.proc_type_decl_from_id(p.type_id, &c_param, p.is_var);
                    self.emit(&decl);
                } else {
                    let c_type = self.type_id_to_c(p.type_id);
                    if p.is_var {
                        self.emit(&format!("{} *{}", c_type, c_param));
                    } else {
                        self.emit(&format!("{} {}", c_type, c_param));
                    }
                }
            }
        }
        self.emit(")");
    }

    // ── Statements ──────────────────────────────────────────────────

    /// Emit extern declarations for all foreign (C ABI) definition modules.
    pub(crate) fn gen_foreign_extern_decls(&mut self) {
        const STDLIB_C_HELPERS: &[&str] = &["CStr", "CIO", "CMem", "CMath", "CRand"];
        for def in self.foreign_def_modules.clone() {
            if STDLIB_C_HELPERS.contains(&def.name.as_str()) { continue; }
            self.emitln(&format!("/* Foreign C bindings: {} */", def.name));
            for d in &def.definitions {
                match d {
                    Definition::Procedure(h) => {
                        self.emit_indent();
                        self.emit("extern ");
                        let ret_type = if let Some(rt) = &h.return_type {
                            self.type_to_c(rt)
                        } else {
                            "void".to_string()
                        };
                        // Bare C name — no module prefix, no mangle
                        self.emit(&format!("{} {}", ret_type, h.name));
                        self.emit("(");
                        if h.params.is_empty() {
                            self.emit("void");
                        } else {
                            let mut first = true;
                            for fp in &h.params {
                                let ctype = self.type_to_c(&fp.typ);
                                let is_open_array = matches!(fp.typ, TypeNode::OpenArray { .. });
                                for name in &fp.names {
                                    if !first { self.emit(", "); }
                                    first = false;
                                    let c_param = self.mangle(name);
                                    if is_open_array {
                                        self.emit(&format!("{} *{}, uint32_t {}_high", ctype, c_param, c_param));
                                    } else if fp.is_var {
                                        self.emit(&format!("{} *{}", ctype, c_param));
                                    } else {
                                        self.emit(&format!("{} {}", ctype, c_param));
                                    }
                                }
                            }
                        }
                        self.emit(");\n");
                    }
                    Definition::Var(v) => {
                        let sym = self.sema.symtab.lookup_innermost(&v.names[0]);
                        let tid = sym.map(|s| s.typ).unwrap_or(TY_INTEGER);
                        let (ctype, arr_suffix) = self.field_type_and_suffix(self.resolve_hir_alias(tid));
                        for name in &v.names {
                            self.emit_indent();
                            self.emitln(&format!("extern {} {}{};", ctype, name, arr_suffix));
                        }
                    }
                    Definition::Const(c) => {
                        let sym = self.sema.symtab.lookup_innermost(&c.name);
                        if let Some(s) = sym {
                            if let crate::symtab::SymbolKind::Constant(cv) = &s.kind {
                                let val = crate::hir_build::const_value_to_hir(cv);
                                let hc = crate::hir::HirConstDecl {
                                    name: c.name.clone(),
                                    mangled: self.mangle(&c.name),
                                    value: val.clone(),
                                    type_id: s.typ,
                                    exported: s.exported,
                                    c_type: crate::hir_build::const_val_c_type(&val),
                                };
                                self.gen_hir_const_decl(&hc);
                            }
                        }
                    }
                    Definition::Type(t) => {
                        let sym = self.sema.symtab.lookup_innermost(&t.name);
                        let tid = sym.map(|s| s.typ).unwrap_or(crate::types::TY_VOID);
                        if tid != crate::types::TY_VOID {
                            self.gen_type_decl_from_id(&t.name, tid);
                        }
                    }
                    Definition::Exception(_) => {}
                }
            }
            self.newline();
        }
    }

    /// Build a map of variable name → C type for a procedure's own params and local vars
    pub(crate) fn build_scope_vars(&self, p: &ProcDecl) -> HashMap<String, String> {
        let mut vars = HashMap::new();
        for fp in &p.heading.params {
            let c_type = self.type_to_c(&fp.typ);
            let is_open = matches!(fp.typ, TypeNode::OpenArray { .. });
            for name in &fp.names {
                if is_open {
                    // Open array params are passed as pointers in C (e.g., char *s),
                    // so the scope var type must be the pointer type, not the element type.
                    // The env struct format "{} *{}" adds another pointer level for indirection.
                    vars.insert(name.clone(), format!("{}*", c_type));
                    // Also track the _high companion
                    vars.insert(format!("{}_high", name), "uint32_t".to_string());
                } else {
                    vars.insert(name.clone(), c_type.clone());
                }
            }
        }
        for decl in &p.block.decls {
            if let Declaration::Var(v) = decl {
                let c_type = self.type_to_c(&v.typ);
                for name in &v.names {
                    vars.insert(name.clone(), c_type.clone());
                }
            }
        }
        vars
    }

}
