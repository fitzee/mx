use super::*;

impl CodeGen {
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
    /// Get the M2 type name for a TypeId.
    fn type_name_for_type_id(&self, id: TypeId) -> String {
        use crate::types::Type;
        match self.sema.types.get(id) {
            Type::Alias { name, .. } => name.clone(),
            Type::Enumeration { name, .. } => name.clone(),
            Type::Opaque { name, .. } => name.clone(),
            Type::Ref { .. } => {
                // Ref types don't carry their name — look up from typeid_c_names
                self.typeid_c_names.get(&id).cloned().unwrap_or_default()
            }
            Type::Object { name, .. } => name.clone(),
            _ => String::new(),
        }
    }

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
                // Skip variant-related fields in the regular fields loop:
                // - the tag field (emitted separately below with the union)
                // - the synthetic "variant" pseudo-field (the union itself covers this)
                let tag_name = variants.as_ref().and_then(|vi| vi.tag_name.clone());
                let has_variants = variants.is_some();
                for f in fields {
                    if Some(&f.name) == tag_name.as_ref() {
                        continue;
                    }
                    if has_variants && f.name == "variant" {
                        continue;
                    }
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
                // If the base is a named record type that already has a C struct,
                // emit typedef BaseRecord *TypeName instead of creating a new struct.
                let base_c_name = self.typeid_c_names.get(&base_resolved).cloned()
                    .or_else(|| self.typeid_c_names.get(&base_id).cloned());
                let base_has_struct = base_c_name.as_ref()
                    .map(|n| self.known_type_names.contains(n))
                    .unwrap_or(false);
                if base_has_struct {
                    let base_name = base_c_name.unwrap();
                    self.emitln(&format!("typedef {} *{};", base_name, c_type_name));
                    self.pointer_base_types.insert(c_type_name.clone(), base_name.clone());
                    self.pointer_base_types.insert(name.to_string(), base_name.clone());
                    if let Some(fields) = self.record_fields.get(&base_name).cloned() {
                        self.record_fields.insert(name.to_string(), fields);
                    }
                } else {
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
                } // end else (no existing base C type)
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
                // Use field_type_and_suffix for correct multi-dimensional arrays
                let (base_type, suffix) = self.field_type_and_suffix(resolved);
                self.emit_indent();
                self.emit(&format!("typedef {} {}{};\n", base_type, c_type_name, suffix));
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
            crate::types::Type::Ref { target, .. } => {
                // REF type: typedef base* TypeName; + register RTTI descriptor
                let base_c = self.type_id_to_c(*target);
                self.emit_indent();
                self.emit(&format!("typedef {} *{};\n", base_c, c_type_name));
                let td_sym = self.register_type_desc(name, name, None);
                self.ref_type_descs.insert(name.to_string(), td_sym);
            }
            crate::types::Type::Object { parent, fields, .. } => {
                // OBJECT type: struct + pointer typedef + RTTI descriptor
                let struct_name = format!("{}_r", c_type_name);
                self.emit_indent();
                self.emit(&format!("typedef struct {} *{};\n", struct_name, c_type_name));
                self.emit_indent();
                self.emit(&format!("struct {} {{\n", struct_name));
                self.indent += 1;
                for f in fields {
                    let ft = self.type_id_to_c(f.typ);
                    self.emit_indent();
                    self.emit(&format!("{} {};\n", ft, f.name));
                }
                self.indent -= 1;
                self.emit_indent();
                self.emit("};\n");
                let parent_td = parent.as_ref().and_then(|pid| {
                    let pname = self.type_name_for_type_id(*pid);
                    if !pname.is_empty() {
                        self.object_type_descs.get(&pname).cloned()
                    } else {
                        None
                    }
                });
                let td_sym = self.register_type_desc(name, name, parent_td);
                self.object_type_descs.insert(name.to_string(), td_sym);
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
                // Emit typedef with correct multi-dimensional suffix
                let (base_type, suffix) = self.field_type_and_suffix(resolved);
                self.emit_indent();
                self.emit(&format!("typedef {} {}{};\n", base_type, c_type_name, suffix));
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
                    crate::types::Type::Complex => "m2_COMPLEX",
                    crate::types::Type::LongComplex => "m2_LONGCOMPLEX",
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

    /// Search all nested procs at any depth for a matching name.
    fn find_nested_proc_sig(&self, proc_name: &str, module: &str) -> Option<crate::hir::HirProcSig> {
        self.prebuilt_hir.as_ref().and_then(|hir| {
            fn search_nested(procs: &[crate::hir::HirProcDecl], name: &str) -> Option<crate::hir::HirProcSig> {
                for pd in procs {
                    if pd.sig.name == name { return Some(pd.sig.clone()); }
                    if let Some(sig) = search_nested(&pd.nested_procs, name) { return Some(sig); }
                }
                None
            }
            // Search top-level
            hir.proc_decls.iter()
                .find(|pd| pd.sig.name == proc_name && pd.sig.module == module)
                .map(|pd| pd.sig.clone())
                .or_else(|| search_nested(&hir.proc_decls.iter()
                    .flat_map(|pd| pd.nested_procs.clone())
                    .collect::<Vec<_>>(), proc_name))
                .or_else(|| hir.proc_decls.iter()
                    .flat_map(|pd| search_nested(&pd.nested_procs, proc_name))
                    .next())
                .or_else(|| hir.embedded_modules.iter()
                    .find(|e| e.name == module)
                    .and_then(|e| {
                        e.procedures.iter()
                            .find(|pd| pd.sig.name == proc_name)
                            .map(|pd| pd.sig.clone())
                            .or_else(|| e.procedures.iter()
                                .flat_map(|pd| search_nested(&pd.nested_procs, proc_name))
                                .next())
                    }))
        })
    }

    /// Search all nested procs at any depth for a matching body.
    fn find_nested_proc_body(&self, proc_name: &str, parent_name: Option<&str>) -> Option<Vec<crate::hir::HirStmt>> {
        self.prebuilt_hir.as_ref().and_then(|hir| {
            fn search_body(procs: &[crate::hir::HirProcDecl], name: &str) -> Option<Vec<crate::hir::HirStmt>> {
                for pd in procs {
                    if pd.sig.name == name { return pd.body.clone(); }
                    if let Some(body) = search_body(&pd.nested_procs, name) { return Some(body); }
                }
                None
            }
            let mod_name = &self.module_name;
            // Top-level
            hir.proc_decls.iter()
                .find(|pd| pd.sig.name == proc_name && pd.sig.module == *mod_name)
                .and_then(|pd| pd.body.clone())
                // Search by parent if given
                .or_else(|| if let Some(parent) = parent_name {
                    hir.proc_decls.iter()
                        .find(|pd| pd.sig.name == parent)
                        .and_then(|pd| search_body(&pd.nested_procs, proc_name))
                } else { None })
                // Search all nested at any depth
                .or_else(|| hir.proc_decls.iter()
                    .flat_map(|pd| search_body(&pd.nested_procs, proc_name))
                    .next())
                // Embedded modules
                .or_else(|| hir.embedded_modules.iter()
                    .find(|e| e.name == *mod_name)
                    .and_then(|e| e.procedures.iter()
                        .find(|pd| pd.sig.name == proc_name)
                        .and_then(|pd| pd.body.clone())
                        .or_else(|| e.procedures.iter()
                            .flat_map(|pd| search_body(&pd.nested_procs, proc_name))
                            .next())))
        })
    }

    pub(crate) fn gen_proc_by_name(&mut self, proc_name: &str) {
        let current_module = self.module_name.clone();
        let early_sig = self.find_nested_proc_sig(proc_name, &current_module);
        if let Some(ref sig) = early_sig {
            self.register_hir_proc_params(sig);
        }

        // Get nested proc names from HIR (replaces AST Declaration::Procedure iteration)
        let nested_proc_names: Vec<String> = self.prebuilt_hir.as_ref().map(|hir| {
            fn find_children(procs: &[crate::hir::HirProcDecl], name: &str) -> Option<Vec<String>> {
                for pd in procs {
                    if pd.sig.name == name {
                        return Some(pd.nested_procs.iter().map(|np| np.sig.name.clone()).collect());
                    }
                    if let Some(names) = find_children(&pd.nested_procs, name) {
                        return Some(names);
                    }
                }
                None
            }
            // Search proc_decls at any depth
            find_children(&hir.proc_decls, proc_name)
                // Legacy HirProc
                .or_else(|| hir.procedures.iter()
                    .find(|hp| hp.name.source_name == proc_name)
                    .map(|hp| hp.nested_procs.iter().map(|np| np.name.source_name.clone()).collect()))
                // Embedded modules
                .or_else(|| hir.embedded_modules.iter()
                    .find(|e| e.name == current_module)
                    .and_then(|e| find_children(&e.procedures, proc_name)))
                .unwrap_or_default()
        }).unwrap_or_default();

        // ── Closure analysis for nested procedures ──────────────────────
        // Build the set of variables available in this scope (params + locals + env vars)
        let mut scope_vars = self.build_scope_vars(&proc_name);
        // Also include vars this proc received through its own env (for deep nesting)
        if let Some(my_env_vars) = self.env_access_names.last() {
            for env_var in my_env_vars {
                if !scope_vars.contains_key(env_var) {
                    // Look up the type from the env struct fields
                    if let Some(my_env_type) = self.closure_env_type.get(proc_name).cloned() {
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
        let env_type_name = format!("{}_env", proc_name);
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

        for np_name in &nested_proc_names {
            let current_mod = self.module_name.clone();
            // Find nested proc in HIR — use deep search helpers
            let np_body = self.find_nested_proc_body(np_name, Some(proc_name));
            let np_sig = self.find_nested_proc_sig(np_name, &current_mod);
            let np_params: Vec<String> = np_sig.as_ref()
                .map(|s| s.params.iter().map(|p| p.name.clone()).collect())
                .unwrap_or_default();
            let np_locals: HashSet<String> = self.prebuilt_hir.as_ref().and_then(|hir| {
                fn find_locals(procs: &[crate::hir::HirProcDecl], name: &str) -> Option<HashSet<String>> {
                    for pd in procs {
                        if pd.sig.name == name {
                            return Some(pd.locals.iter()
                                .filter_map(|l| if let crate::hir::HirLocalDecl::Var { name, .. } = l { Some(name.clone()) } else { None })
                                .collect());
                        }
                        if let Some(r) = find_locals(&pd.nested_procs, name) { return Some(r); }
                    }
                    None
                }
                find_locals(&hir.proc_decls, np_name)
            }).unwrap_or_default();
            let hir_proc_info: Option<(Option<Vec<crate::hir::HirStmt>>, Vec<String>, HashSet<String>)> =
                Some((np_body, np_params, np_locals));
            let hir_captures = if let Some((Some(ref body), ref param_names, ref local_names)) = hir_proc_info {
                crate::hir_build::compute_captures_hir(
                    np_name, body, &param_names, &local_names,
                    &scope_vars_tid, &self.import_map, &imported_mods,
                )
            } else {
                Vec::new() // No HIR body available — no captures
            };
            let captures: Vec<String> = hir_captures.iter().map(|c| c.name.clone()).collect();
            if !captures.is_empty() {
                has_any_captures = true;
                for cap_name in &captures {
                    if !all_captures.iter().any(|(n, _)| n == cap_name) {
                        let c_type = scope_vars.get(cap_name).cloned().unwrap_or("int32_t".to_string());
                        all_captures.push((cap_name.clone(), c_type));
                    }
                }
                self.closure_env_type.insert(np_name.clone(), env_type_name.clone());
                child_capture_info.push((np_name.clone(), captures));
            }

            // Propagate transitive captures: scan the nested proc's children's bodies
            // for references to outer-scope variables that need to be forwarded.
            if let Some(ref hir) = self.prebuilt_hir {
                fn find_proc_and_scan(procs: &[crate::hir::HirProcDecl], name: &str) -> Vec<String> {
                    for pd in procs {
                        if pd.sig.name == name {
                            // Collect all var refs from all grandchild bodies
                            let mut refs = Vec::new();
                            fn collect_nested_refs(nested: &[crate::hir::HirProcDecl], out: &mut Vec<String>) {
                                for np in nested {
                                    if let Some(ref body) = np.body {
                                        crate::hir_build::collect_hir_var_refs(body, out);
                                    }
                                    collect_nested_refs(&np.nested_procs, out);
                                }
                            }
                            collect_nested_refs(&pd.nested_procs, &mut refs);
                            return refs;
                        }
                        let result = find_proc_and_scan(&pd.nested_procs, name);
                        if !result.is_empty() { return result; }
                    }
                    Vec::new()
                }
                let grandchild_refs = find_proc_and_scan(&hir.proc_decls, np_name);
                for ref_name in &grandchild_refs {
                    if !scope_vars.contains_key(ref_name) { continue; }
                    if !all_captures.iter().any(|(n, _)| n == ref_name) {
                        has_any_captures = true;
                        let c_type = scope_vars.get(ref_name).cloned().unwrap_or("int32_t".to_string());
                        all_captures.push((ref_name.clone(), c_type));
                    }
                    // Also add to this proc's capture info (for env forwarding)
                    if let Some((_, caps)) = child_capture_info.iter_mut().find(|(n, _)| n == np_name) {
                        if !caps.contains(ref_name) {
                            caps.push(ref_name.clone());
                        }
                    } else {
                        self.closure_env_type.insert(np_name.clone(), env_type_name.clone());
                        child_capture_info.push((np_name.clone(), vec![ref_name.clone()]));
                    }
                }
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
        self.parent_proc_stack.push(proc_name.to_string());

        // Generate nested procs (lifted to top level, with env param if they have captures)
        for np_name in &nested_proc_names {
            let mangled = format!("{}_{}", proc_name, np_name);
            self.nested_proc_names.insert(np_name.clone(), mangled);
            if let Some(_) = self.closure_env_type.get(np_name) {
                let current_mod = self.module_name.clone();
                // Use deep search helpers for body, sig, locals
                let np_body2 = self.find_nested_proc_body(np_name, Some(proc_name));
                let np_sig2 = self.find_nested_proc_sig(np_name, &current_mod);
                let np_params2: Vec<String> = np_sig2.as_ref()
                    .map(|s| s.params.iter().map(|p| p.name.clone()).collect())
                    .unwrap_or_default();
                let np_locals2: HashSet<String> = self.prebuilt_hir.as_ref().and_then(|hir| {
                    fn find_locals3(procs: &[crate::hir::HirProcDecl], name: &str) -> Option<HashSet<String>> {
                        for pd in procs {
                            if pd.sig.name == name {
                                return Some(pd.locals.iter()
                                    .filter_map(|l| if let crate::hir::HirLocalDecl::Var { name, .. } = l { Some(name.clone()) } else { None })
                                    .collect());
                            }
                            if let Some(l) = find_locals3(&pd.nested_procs, name) { return Some(l); }
                        }
                        None
                    }
                    find_locals3(&hir.proc_decls, np_name)
                }).unwrap_or_default();
                let np_hir_caps = if let Some(ref body) = np_body2 {
                    crate::hir_build::compute_captures_hir(
                        np_name, body, &np_params2, &np_locals2,
                        &scope_vars_tid, &self.import_map, &imported_mods,
                    )
                } else {
                    Vec::new()
                };
                let mut np_captures: Vec<String> = np_hir_caps.iter().map(|c| c.name.clone()).collect();
                // Include transitive captures from child_capture_info
                if let Some((_, transitive_caps)) = child_capture_info.iter().find(|(n, _)| n == np_name) {
                    for tc in transitive_caps {
                        if !np_captures.contains(tc) {
                            np_captures.push(tc.clone());
                        }
                    }
                }
                self.env_access_names.push(np_captures.iter().cloned().collect());
            }
            self.gen_proc_by_name(np_name);
            if self.closure_env_type.contains_key(np_name) {
                self.env_access_names.pop();
            }
        }

        // Pop closure context
        self.child_env_type_stack.pop();
        self.child_captures_stack.pop();

        // ── Generate this procedure ─────────────────────────────────────
        self.newline();
        if let Some(ref sig) = early_sig {
            self.gen_hir_proc_prototype(sig);
        }
        self.emit(" {\n");
        self.indent += 1;

        // Suppress -Wunused-parameter for generated code
        if let Some(ref sig) = early_sig {
            for hp in &sig.params {
                let mangled = self.mangle(&hp.name);
                self.emitln(&format!("(void){};", mangled));
                if hp.needs_high {
                    self.emitln(&format!("(void){}_high;", mangled));
                }
            }
        }

        // Push a new VAR param scope and register VAR params
        // Note: VAR open array params are already pointers, so don't register them
        // as VAR (which would cause double dereferencing with (*a)[i])
        self.push_var_scope();
        // Save array var tracking so procedure-local names don't collide with outer scope
        let saved_var_tracking = self.save_var_tracking();
        // Register param tracking from HIR sig (TypeId-based, no AST TypeNode)
        let hir_params = early_sig.as_ref()
            .map(|s| s.params.clone())
            .unwrap_or_default();
        for hp in &hir_params {
            let resolved = self.resolve_hir_alias(hp.type_id);
            let is_open = matches!(self.sema.types.get(resolved), crate::types::Type::OpenArray { .. });
            if is_open {
                let mangled = self.mangle(&hp.name);
                if let Some(scope) = self.open_array_params.last_mut() {
                    scope.insert(mangled);
                }
                let high_name = format!("{}_high", &hp.name);
                self.var_types.insert(high_name, "uint32_t".to_string());
            } else if hp.is_var {
                self.register_var_param(&hp.name);
            }
            // Track named-array value params (array decays to pointer in C)
            if !hp.is_var && !is_open {
                if matches!(self.sema.types.get(resolved), crate::types::Type::Array { .. }) {
                    if let Some(scope) = self.named_array_value_params.last_mut() {
                        scope.insert(hp.name.clone());
                    }
                }
            }
            // Register param type name for designator type tracking
            if let Some(type_name) = self.type_id_source_name(hp.type_id) {
                self.var_types.insert(hp.name.clone(), type_name);
            }
            // Track unsigned/long params for DIV/MOD codegen
            if crate::types::is_unsigned_type(&self.sema.types, resolved) {
                self.cardinal_vars.insert(hp.name.clone());
            }
            if resolved == TY_LONGINT {
                self.longint_vars.insert(hp.name.clone());
            }
            if resolved == TY_LONGCARD {
                self.longcard_vars.insert(hp.name.clone());
            }
            // Aliases to unsigned types (e.g. Timestamp = LONGCARD)
            if let Some(type_name) = self.type_id_source_name(hp.type_id) {
                if self.unsigned_type_aliases.contains(&type_name) {
                    self.cardinal_vars.insert(hp.name.clone());
                    self.longcard_vars.insert(hp.name.clone());
                }
            }
        }

        // Local declarations from HIR (search at any nesting depth)
        let proc_locals = self.prebuilt_hir.as_ref().and_then(|hir| {
            fn search_locals(procs: &[crate::hir::HirProcDecl], name: &str) -> Option<Vec<crate::hir::HirLocalDecl>> {
                for pd in procs {
                    if pd.sig.name == name { return Some(pd.locals.clone()); }
                    if let Some(locals) = search_locals(&pd.nested_procs, name) { return Some(locals); }
                }
                None
            }
            let current_module = &self.module_name;
            // Search proc_decls at any depth
            hir.proc_decls.iter()
                .find(|pd| pd.sig.name == proc_name && pd.sig.module == *current_module)
                .map(|pd| pd.locals.clone())
                .or_else(|| hir.proc_decls.iter()
                    .flat_map(|pd| search_locals(&pd.nested_procs, proc_name))
                    .next())
                // Legacy HirProc fallback
                .or_else(|| hir.procedures.iter()
                    .find(|hp| hp.name.source_name == proc_name)
                    .map(|hp| hp.locals.clone()))
                .or_else(|| hir.procedures.iter()
                    .flat_map(|hp| hp.nested_procs.iter())
                    .find(|np| np.name.source_name == proc_name)
                    .map(|np| np.locals.clone()))
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
        }
        // Local declarations from HIR cover all cases — no AST fallback needed

        // If this proc has nested procs with captures, declare and init the child env
        if has_any_captures {
            self.emitln(&format!("{} _child_env;", env_type_name));
            // Collect parent env fields (if current proc receives an env)
            let parent_env_fields: HashSet<String> = early_sig.as_ref()
                .and_then(|_| self.env_access_names.last())
                .map(|s| s.clone())
                .unwrap_or_default();
            for (cap_name, _cap_type) in &all_captures {
                self.emit_indent();
                if self.is_env_var(cap_name) || parent_env_fields.contains(cap_name.as_str()) {
                    // Forward from our own env (direct or transitive capture)
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

        // Body from HIR — search at any nesting depth
        let parent_proc_name = if self.parent_proc_stack.len() >= 2 {
            let parent = &self.parent_proc_stack[self.parent_proc_stack.len() - 2];
            Some(parent.rsplit('_').next().unwrap_or(parent).to_string())
        } else {
            None
        };
        let prebuilt_body = self.find_nested_proc_body(proc_name, parent_proc_name.as_deref());
        // Check for procedure-level EXCEPT handler
        let except_body = self.prebuilt_hir.as_ref().and_then(|hir| {
            let mod_name = self.module_name.clone();
            // Search proc_decls for the except handler
            hir.proc_decls.iter()
                .find(|pd| pd.sig.name == proc_name && pd.sig.module == mod_name)
                .and_then(|pd| pd.except_handler.clone())
                .or_else(|| hir.proc_decls.iter()
                    .flat_map(|pd| pd.nested_procs.iter())
                    .find(|np| np.sig.name == proc_name && np.sig.module == mod_name)
                    .and_then(|np| np.except_handler.clone()))
                .or_else(|| hir.embedded_modules.iter()
                    .find(|e| e.name == mod_name)
                    .and_then(|e| e.procedures.iter()
                        .find(|pd| pd.sig.name == proc_name)
                        .and_then(|pd| pd.except_handler.clone())))
        });

        if let Some(ref except_stmts) = except_body {
            // Procedure-level EXCEPT: wrap body in M2_TRY/M2_CATCH
            self.emitln("m2_ExcFrame _ef;");
            self.emitln("M2_TRY(_ef) {");
            self.indent += 1;
            if let Some(body) = prebuilt_body {
                for stmt in &body {
                    self.emit_hir_stmt(stmt);
                }
            }
            self.emitln("M2_ENDTRY(_ef);");
            self.indent -= 1;
            self.emitln("} M2_CATCH {");
            self.indent += 1;
            self.emitln("M2_ENDTRY(_ef);");
            for stmt in except_stmts {
                self.emit_hir_stmt(stmt);
            }
            self.indent -= 1;
            self.emitln("}");
        } else if let Some(body) = prebuilt_body {
            for stmt in &body {
                self.emit_hir_stmt(stmt);
            }
        }

        self.child_env_type_stack.pop();
        self.child_captures_stack.pop();
        self.restore_var_tracking(saved_var_tracking);
        self.pop_var_scope();
        self.parent_proc_stack.pop();
        self.indent -= 1;
        self.emitln("}");
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
        let foreign_names: Vec<String> = self.foreign_modules.iter()
            .filter(|n| !STDLIB_C_HELPERS.contains(&n.as_str()))
            .cloned()
            .collect();
        for mod_name in &foreign_names {
            self.emitln(&format!("/* Foreign C bindings: {} */", mod_name));
            if let Some(scope_id) = self.sema.symtab.lookup_module_scope(mod_name) {
                let syms: Vec<(String, crate::symtab::SymbolKind, crate::types::TypeId, bool)> =
                    self.sema.symtab.symbols_in_scope(scope_id).iter()
                        .map(|s| (s.name.clone(), s.kind.clone(), s.typ, s.exported))
                        .collect();
                for (name, kind, typ, exported) in &syms {
                    match kind {
                        crate::symtab::SymbolKind::Procedure { params, return_type, .. } => {
                            self.emit_indent();
                            self.emit("extern ");
                            let ret_type = match return_type {
                                Some(rt) => self.type_id_to_c(*rt),
                                None => "void".to_string(),
                            };
                            self.emit(&format!("{} {}(", ret_type, name));
                            if params.is_empty() {
                                self.emit("void");
                            } else {
                                let mut first = true;
                                for p in params {
                                    if !first { self.emit(", "); }
                                    first = false;
                                    let resolved = self.resolve_hir_alias(p.typ);
                                    let ctype = self.type_id_to_c(p.typ);
                                    let c_param = self.mangle(&p.name);
                                    if matches!(self.sema.types.get(resolved), crate::types::Type::OpenArray { .. }) {
                                        self.emit(&format!("{} *{}, uint32_t {}_high", ctype, c_param, c_param));
                                    } else if p.is_var {
                                        self.emit(&format!("{} *{}", ctype, c_param));
                                    } else {
                                        self.emit(&format!("{} {}", ctype, c_param));
                                    }
                                }
                            }
                            self.emit(");\n");
                        }
                        crate::symtab::SymbolKind::Variable => {
                            let (ctype, arr_suffix) = self.field_type_and_suffix(self.resolve_hir_alias(*typ));
                            self.emit_indent();
                            self.emitln(&format!("extern {} {}{};", ctype, name, arr_suffix));
                        }
                        crate::symtab::SymbolKind::Constant(cv) => {
                            let val = crate::hir_build::const_value_to_hir(cv);
                            let hc = crate::hir::HirConstDecl {
                                name: name.clone(),
                                mangled: self.mangle(name),
                                value: val.clone(),
                                type_id: *typ,
                                exported: *exported,
                                c_type: crate::hir_build::const_val_c_type(&val),
                            };
                            self.gen_hir_const_decl(&hc);
                        }
                        crate::symtab::SymbolKind::Type => {
                            if *typ != crate::types::TY_VOID {
                                self.gen_type_decl_from_id(name, *typ);
                            }
                        }
                        _ => {}
                    }
                }
            }
            self.newline();
        }
    }

    /// Build a map of variable name → C type for a procedure's own params and local vars.
    /// Uses HirProcSig params + HirProc.locals for TypeId-based resolution.
    pub(crate) fn build_scope_vars(&self, proc_name: &str) -> HashMap<String, String> {
        let mut vars = HashMap::new();
        let current_module = self.module_name.clone();
        // Use deep search for sig
        let hir_sig = self.find_nested_proc_sig(proc_name, &current_module);
        if let Some(ref sig) = hir_sig {
            for param in &sig.params {
                let c_type = self.type_id_to_c(param.type_id);
                if param.is_open_array {
                    vars.insert(param.name.clone(), format!("{}*", c_type));
                    vars.insert(format!("{}_high", param.name), "uint32_t".to_string());
                } else {
                    vars.insert(param.name.clone(), c_type);
                }
            }
        }
        // Local vars from HirProc.locals
        let hir_locals = self.prebuilt_hir.as_ref().and_then(|hir| {
            fn search_locals_deep(procs: &[crate::hir::HirProcDecl], name: &str) -> Option<Vec<crate::hir::HirLocalDecl>> {
                for pd in procs {
                    if pd.sig.name == name { return Some(pd.locals.clone()); }
                    if let Some(l) = search_locals_deep(&pd.nested_procs, name) { return Some(l); }
                }
                None
            }
            search_locals_deep(&hir.proc_decls, proc_name)
                .or_else(|| hir.procedures.iter()
                    .find(|hp| hp.name.source_name == proc_name)
                    .map(|hp| hp.locals.clone()))
        });
        if let Some(ref locals) = hir_locals {
            for local in locals {
                if let crate::hir::HirLocalDecl::Var { name, type_id } = local {
                    let c_type = self.type_id_to_c(*type_id);
                    vars.insert(name.clone(), c_type);
                }
            }
        }
        // For nested procs, also include vars from ancestor proc scopes.
        // These are available via the closure env chain.
        for parent in self.parent_proc_stack.iter().rev() {
            let parent_source = parent.rsplit('_').next().unwrap_or(parent);
            if parent_source == proc_name { continue; }
            if let Some(sig) = self.find_nested_proc_sig(parent_source, &current_module) {
                for param in &sig.params {
                    vars.entry(param.name.clone())
                        .or_insert_with(|| self.type_id_to_c(param.type_id));
                }
            }
            // Parent's locals
            if let Some(ref hir) = self.prebuilt_hir {
                fn search_locals_deep2(procs: &[crate::hir::HirProcDecl], name: &str) -> Option<Vec<crate::hir::HirLocalDecl>> {
                    for pd in procs {
                        if pd.sig.name == name { return Some(pd.locals.clone()); }
                        if let Some(l) = search_locals_deep2(&pd.nested_procs, name) { return Some(l); }
                    }
                    None
                }
                if let Some(locals) = search_locals_deep2(&hir.proc_decls, parent_source) {
                    for local in &locals {
                        if let crate::hir::HirLocalDecl::Var { name, type_id } = local {
                            vars.entry(name.clone())
                                .or_insert_with(|| self.type_id_to_c(*type_id));
                        }
                    }
                }
            }
        }
        vars
    }

}
