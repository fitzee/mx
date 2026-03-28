use super::*;
use crate::types::TypeId;
use super::type_lowering::FieldInfo;

impl LLVMCodeGen {
    // ── TypeLowering bridge helpers ──────────────────────────────────
    // These check TypeLowering first, fall back to legacy maps.
    // Once all call sites use these, the legacy maps can be removed.

    /// Look up a record field by name, given the variable's TypeId.
    /// Returns field info from TypeLowering, or None if not found.
    pub(crate) fn tl_lookup_field(&self, type_id: TypeId, field_name: &str) -> Option<FieldInfo> {
        let tl = self.type_lowering.as_ref()?;
        let resolved = tl.resolve_alias(&self.sema.types, type_id);
        // Try direct record lookup
        if let Some(fi) = tl.lookup_field(resolved, field_name) {
            return Some(fi.clone());
        }
        // Try through pointer target
        if let Some(target) = tl.pointer_target(resolved) {
            let target_resolved = tl.resolve_alias(&self.sema.types, target);
            if let Some(fi) = tl.lookup_field(target_resolved, field_name) {
                return Some(fi.clone());
            }
        }
        None
    }

    /// Get the LLVM type string for a TypeId.
    /// Get the LLVM type string for a semantic TypeId.
    /// Never returns "void" — unresolved types default to "i32".
    /// Callers that need "void" for function returns should check explicitly.
    pub(crate) fn tl_type_str(&self, type_id: TypeId) -> String {
        if let Some(ref tl) = self.type_lowering {
            let s = tl.get_type_str(type_id);
            if s == "void" { "i32".into() } else { s }
        } else {
            "i32".into()
        }
    }

    /// Get LLVM type string for function return, allowing "void".
    pub(crate) fn tl_return_type_str(&self, type_id: TypeId) -> String {
        if let Some(ref tl) = self.type_lowering {
            tl.get_type_str(type_id)
        } else {
            "i32".into()
        }
    }

    /// Get the pointer target TypeId for a type.
    pub(crate) fn tl_pointer_target(&self, type_id: TypeId) -> Option<TypeId> {
        let tl = self.type_lowering.as_ref()?;
        let resolved = tl.resolve_alias(&self.sema.types, type_id);
        tl.pointer_target(resolved)
    }

    /// Get the array element TypeId for a type.
    pub(crate) fn tl_array_element(&self, type_id: TypeId) -> Option<TypeId> {
        let tl = self.type_lowering.as_ref()?;
        let resolved = tl.resolve_alias(&self.sema.types, type_id);
        tl.array_element_type(resolved)
    }

    /// Get the array size for a type.
    pub(crate) fn tl_array_size(&self, type_id: TypeId) -> Option<usize> {
        let tl = self.type_lowering.as_ref()?;
        let resolved = tl.resolve_alias(&self.sema.types, type_id);
        tl.array_size(resolved)
    }

    /// Get the LLVM type string for the record type that contains a given field.
    /// This is needed for GEP: getelementptr inbounds {record_type}, ptr, i32 0, i32 {field_index}
    pub(crate) fn tl_record_type_str(&self, type_id: TypeId) -> Option<String> {
        let tl = self.type_lowering.as_ref()?;
        let resolved = tl.resolve_alias(&self.sema.types, type_id);
        if tl.get_record_layout(resolved).is_some() {
            return Some(tl.get_type_str(resolved));
        }
        if let Some(target) = tl.pointer_target(resolved) {
            let target_resolved = tl.resolve_alias(&self.sema.types, target);
            if tl.get_record_layout(target_resolved).is_some() {
                return Some(tl.get_type_str(target_resolved));
            }
        }
        None
    }

    /// Resolve a TypeId through aliases to the "real" type.
    pub(crate) fn tl_resolve(&self, type_id: TypeId) -> TypeId {
        if let Some(ref tl) = self.type_lowering {
            tl.resolve_alias(&self.sema.types, type_id)
        } else {
            type_id
        }
    }

    /// Resolve a TypeNode to a semantic TypeId.
    /// Uses symtab for Named types, TypeRegistry search for structural types.
    /// This avoids the scope-ambiguity issues of lookup_any on variable names.
    pub(crate) fn resolve_type_node_to_id(&self, tn: &TypeNode) -> Option<TypeId> {
        match tn {
            TypeNode::Named(qi) => {
                match qi.name.as_str() {
                    "INTEGER" => Some(crate::types::TY_INTEGER),
                    "CARDINAL" => Some(crate::types::TY_CARDINAL),
                    "LONGINT" => Some(crate::types::TY_LONGINT),
                    "LONGCARD" => Some(crate::types::TY_LONGCARD),
                    "REAL" => Some(crate::types::TY_REAL),
                    "LONGREAL" => Some(crate::types::TY_LONGREAL),
                    "BOOLEAN" => Some(crate::types::TY_BOOLEAN),
                    "CHAR" => Some(crate::types::TY_CHAR),
                    "ADDRESS" => Some(crate::types::TY_ADDRESS),
                    _ => {
                        if let Some(ref module) = qi.module {
                            // Qualified: Module.Type
                            self.sema.symtab.lookup_qualified(module, &qi.name)
                                .filter(|s| matches!(s.kind, crate::symtab::SymbolKind::Type))
                                .map(|s| s.typ)
                        } else {
                            // Unqualified: try current module first (including
                            // non-exported .mod-only types), then global search.
                            let result = self.sema.symtab.find_type_in_module(&self.module_name, &qi.name)
                                .or_else(|| self.sema.symtab.find_type(&qi.name));
                            result
                        }
                    }
                }
            }
            TypeNode::Pointer { base, .. } => {
                // Search TypeRegistry for a Pointer whose base matches
                let base_tid = self.resolve_type_node_to_id(base)?;
                let count = self.sema.types.len();
                for id in 0..count {
                    if let crate::types::Type::Pointer { base: b } = self.sema.types.get(id) {
                        if *b == base_tid {
                            return Some(id);
                        }
                    }
                }
                None
            }
            TypeNode::Array { index_types, elem_type, .. } => {
                let elem_tid = self.resolve_type_node_to_id(elem_type)?;
                // Compute expected array size from index type
                let expected_high = if let Some(TypeNode::Subrange { high, .. }) = index_types.first() {
                    self.const_eval_expr(high)
                } else { None };
                let count = self.sema.types.len();
                for id in 0..count {
                    if let crate::types::Type::Array { elem_type: e, high: h, .. } = self.sema.types.get(id) {
                        if *e == elem_tid {
                            // Match size if we know it
                            if let Some(eh) = expected_high {
                                if *h == eh { return Some(id); }
                            } else {
                                return Some(id);
                            }
                        }
                    }
                }
                None
            }
            TypeNode::Ref { .. } | TypeNode::RefAny { .. } => {
                // REF and REFANY are pointer types
                Some(crate::types::TY_ADDRESS)
            }
            TypeNode::OpenArray { elem_type, .. } => {
                let elem_tid = self.resolve_type_node_to_id(elem_type)?;
                let count = self.sema.types.len();
                for id in 0..count {
                    if let crate::types::Type::OpenArray { elem_type: e } = self.sema.types.get(id) {
                        if *e == elem_tid {
                            return Some(id);
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// Emit LLVM IR that computes sizeof(type) at IR level.
    /// Uses the GEP-from-null trick: getelementptr T, ptr null, i32 1 → ptrtoint → size.
    /// This is guaranteed correct regardless of alignment/padding.
    pub(crate) fn emit_sizeof(&mut self, ty: &str) -> String {
        let tmp1 = self.next_tmp();
        self.emitln(&format!("  {} = getelementptr {}, ptr null, i32 1", tmp1, ty));
        let tmp2 = self.next_tmp();
        self.emitln(&format!("  {} = ptrtoint ptr {} to i64", tmp2, tmp1));
        tmp2
    }

    /// Emit a memcpy using LLVM's sizeof trick for correct size.
    pub(crate) fn emit_struct_memcpy(&mut self, dst: &str, src: &str, ty: &str) {
        let size = self.emit_sizeof(ty);
        self.emitln(&format!("  call ptr @memcpy(ptr {}, ptr {}, i64 {})", dst, src, size));
    }

    // ── Type mapping (legacy) ───────────────────────────────────────

    pub(crate) fn llvm_type_for_type_node(&self, tn: &TypeNode) -> String {
        let ty = self.llvm_type_for_type_node_raw(tn);
        // Sanitize: "void" means an unresolved imported type —
        // replace with i32. This function is only used for data types
        // (record fields, array elements, variable types), never for
        // function return types (which use llvm_type_for_type_id).
        if ty.contains("void") {
            ty.replace("void", "i32")
        } else {
            ty
        }
    }

    fn llvm_type_for_type_node_raw(&self, tn: &TypeNode) -> String {
        match tn {
            TypeNode::Named(qi) => {
                let name = &qi.name;
                match name.as_str() {
                    "INTEGER" => "i32".to_string(),
                    "CARDINAL" => "i32".to_string(),
                    "LONGINT" => "i64".to_string(),
                    "LONGCARD" => "i64".to_string(),
                    "REAL" => "float".to_string(),
                    "LONGREAL" => "double".to_string(),
                    "BOOLEAN" => "i32".to_string(),
                    "CHAR" => "i8".to_string(),
                    "BITSET" => "i32".to_string(),
                    "WORD" => "i32".to_string(),
                    "BYTE" => "i8".to_string(),
                    "ADDRESS" => "ptr".to_string(),
                    "PROC" => "ptr".to_string(),
                    "COMPLEX" => "{ float, float }".to_string(),
                    "LONGCOMPLEX" => "{ double, double }".to_string(),
                    _ => {
                        // Resolve through sema (handles cross-module aliases and records)
                        if let Some(tid) = self.resolve_type_node_to_id(tn) {
                            let ty_str = self.tl_type_str(tid);
                            if ty_str != "void" {
                                return ty_str;
                            }
                        }
                        // Fallback: symtab lookup, then type_map
                        if let Some(sym) = self.sema.symtab.lookup_any(name) {
                            if matches!(sym.kind, crate::symtab::SymbolKind::Type) {
                                let ty_str = self.tl_type_str(sym.typ);
                                if ty_str != "void" {
                                    ty_str
                                } else {
                                    self.type_map.get(name).cloned()
                                        .unwrap_or("i32".to_string())
                                }
                            } else {
                                self.type_map.get(name).cloned().unwrap_or("ptr".to_string())
                            }
                        } else if let Some(ty) = self.type_map.get(name) {
                            ty.clone()
                        } else {
                            "ptr".to_string()
                        }
                    }
                }
            }
            TypeNode::Array { index_types, elem_type, .. } => {
                // Try sema-based resolution first — handles cross-module CONSTs correctly
                if let Some(tid) = self.resolve_type_node_to_id(tn) {
                    let ty_str = self.tl_type_str(tid);
                    if ty_str != "void" && ty_str.starts_with('[') {
                        return ty_str;
                    }
                }
                let raw_elem_ty = self.llvm_type_for_type_node(elem_type);
                // Guard: void is never a valid array element type —
                // unresolved imported type, default to i32.
                let base_elem_ty = if raw_elem_ty == "void" {
                    "i32".to_string()
                } else {
                    raw_elem_ty
                };
                // Fallback: build type from AST dimensions
                // For ARRAY [1..3],[1..3] OF INTEGER → [4 x [4 x i32]]
                let mut sizes = Vec::new();
                for idx_tn in index_types {
                    let size = if let TypeNode::Subrange { high, .. } = idx_tn {
                        if let Some(hi) = self.const_eval_expr(high) {
                            (hi + 1) as usize
                        } else { 0 }
                    } else if let TypeNode::Named(qi) = idx_tn {
                        match qi.name.as_str() {
                            "CHAR" => 256,
                            "BOOLEAN" => 2,
                            _ => {
                                // Check if it's an enum type
                                if let Some(&v) = self.enum_variants.iter()
                                    .filter(|(k, _)| k.starts_with(&format!("{}_", qi.name)))
                                    .map(|(_, v)| v)
                                    .max() {
                                    (v + 1) as usize
                                } else { 0 }
                            }
                        }
                    } else if let TypeNode::Enumeration { variants, .. } = idx_tn {
                        variants.len()
                    } else { 0 };
                    sizes.push(size);
                }
                // Build nested array type: last size wraps elem_ty, then wrap outward
                let mut result_ty = base_elem_ty;
                for &size in sizes.iter().rev() {
                    if size > 0 {
                        result_ty = format!("[{} x {}]", size, result_ty);
                    } else {
                        result_ty = format!("[0 x {}]", result_ty);
                    }
                }
                result_ty
            }
            TypeNode::OpenArray { elem_type, .. } => {
                // Open arrays are passed as ptr + i32 high
                let _elem_ty = self.llvm_type_for_type_node(elem_type);
                "ptr".to_string()
            }
            TypeNode::Pointer { .. } => "ptr".to_string(),
            TypeNode::Set { .. } => "i32".to_string(),
            TypeNode::Enumeration { .. } => "i32".to_string(),
            TypeNode::Subrange { .. } => "i32".to_string(),
            TypeNode::ProcedureType { .. } => "ptr".to_string(),
            TypeNode::Record { fields, .. } => {
                // Record → generate struct type inline (including variant fields)
                let mut field_types = Vec::new();
                for fl in fields {
                    for f in &fl.fixed {
                        let ft = self.llvm_type_for_type_node(&f.typ);
                        for _ in &f.names {
                            field_types.push(ft.clone());
                        }
                    }
                    if let Some(ref vp) = fl.variant {
                        if vp.tag_name.is_some() {
                            field_types.push("i32".to_string()); // tag
                        }
                        // Add fields from the largest variant (recursively including nested CASE)
                        let mut max_fields = Vec::new();
                        for variant in &vp.variants {
                            let mut vf = Vec::new();
                            for vfl in &variant.fields {
                                for f in &vfl.fixed {
                                    let ft = self.llvm_type_for_type_node(&f.typ);
                                    for _ in &f.names { vf.push(ft.clone()); }
                                }
                                // Recursively handle nested variant parts
                                if let Some(ref inner_vp) = vfl.variant {
                                    if inner_vp.tag_name.is_some() {
                                        vf.push("i32".to_string()); // inner tag
                                    }
                                    let mut inner_max = Vec::new();
                                    for iv in &inner_vp.variants {
                                        let mut ivf = Vec::new();
                                        for ivfl in &iv.fields {
                                            for f in &ivfl.fixed {
                                                let ft = self.llvm_type_for_type_node(&f.typ);
                                                for _ in &f.names { ivf.push(ft.clone()); }
                                            }
                                        }
                                        if ivf.len() > inner_max.len() { inner_max = ivf; }
                                    }
                                    vf.extend(inner_max);
                                }
                            }
                            if vf.len() > max_fields.len() { max_fields = vf; }
                        }
                        field_types.extend(max_fields);
                    }
                }
                if field_types.is_empty() {
                    "{ i8 }".to_string() // Empty struct needs at least one field
                } else {
                    format!("{{ {} }}", field_types.join(", "))
                }
            }
            TypeNode::Ref { .. } | TypeNode::RefAny { .. } => "ptr".to_string(),
            TypeNode::Object { .. } => "ptr".to_string(),
        }
    }

    pub(crate) fn llvm_type_for_type_id(&self, id: TypeId) -> String {
        match self.sema.types.get(id) {
            Type::Integer => "i32".to_string(),
            Type::Cardinal => "i32".to_string(),
            Type::LongInt => "i64".to_string(),
            Type::LongCard => "i64".to_string(),
            Type::Real => "float".to_string(),
            Type::LongReal => "double".to_string(),
            Type::Boolean => "i32".to_string(),
            Type::Char => "i8".to_string(),
            Type::Bitset => "i32".to_string(),
            Type::Word => "i32".to_string(),
            Type::Byte => "i8".to_string(),
            Type::Address => "ptr".to_string(),
            Type::Nil => "ptr".to_string(),
            Type::Void => "void".to_string(),
            Type::StringLit(_) => "ptr".to_string(),
            Type::Pointer { .. } => "ptr".to_string(),
            Type::Set { .. } => "i32".to_string(),
            Type::Enumeration { .. } => "i32".to_string(),
            Type::Subrange { .. } => "i32".to_string(),
            Type::ProcedureType { .. } => "ptr".to_string(),
            Type::Array { elem_type, high, .. } => {
                let elem_ty = self.llvm_type_for_type_id(*elem_type);
                // Match C backend: allocate (high + 1) elements for 1-based indexing
                let size = (*high + 1) as usize;
                format!("[{} x {}]", size, elem_ty)
            }
            Type::OpenArray { .. } => "ptr".to_string(),
            Type::Record { fields, .. } => {
                let field_types: Vec<String> = fields.iter()
                    .map(|f| self.llvm_type_for_type_id(f.typ))
                    .collect();
                format!("{{ {} }}", field_types.join(", "))
            }
            Type::Alias { target, .. } => self.llvm_type_for_type_id(*target),
            Type::Opaque { .. } => "ptr".to_string(),
            Type::Ref { .. } | Type::RefAny => "ptr".to_string(),
            Type::Object { .. } => "ptr".to_string(),
            Type::Exception { .. } => "i32".to_string(),
            Type::Complex => "{ float, float }".to_string(),
            Type::LongComplex => "{ double, double }".to_string(),
            Type::Error => "i32".to_string(), // poison type — should not reach codegen
        }
    }

    pub(crate) fn is_float_type(ty: &str) -> bool {
        ty == "float" || ty == "double"
    }

    /// Check if a TypeId is a set type (BITSET or user-defined SET).
    pub(crate) fn is_set_tid(&self, tid: Option<crate::types::TypeId>) -> bool {
        let Some(tid) = tid else { return false };
        if tid == crate::types::TY_BITSET { return true; }
        matches!(self.sema.types.get(tid), crate::types::Type::Set { .. })
    }

    pub(crate) fn is_int_type(ty: &str) -> bool {
        ty.starts_with('i') || ty == "ptr"
    }

    /// Return the element size in bytes for pointer arithmetic.
    pub(crate) fn _ptr_elem_size(&self, _ty: &str) -> usize {
        // Default: byte-addressable
        1
    }

    /// Check if a type name corresponds to an unsigned M2 type.
    pub(crate) fn is_unsigned_var(&self, name: &str) -> bool {
        if let Some(type_name) = self.var_type_names.get(name) {
            matches!(type_name.as_str(), "CARDINAL" | "LONGCARD" | "BITSET" | "WORD" | "BYTE" | "ADDRESS")
        } else {
            false
        }
    }

    // ── Local variable helpers ──────────────────────────────────────

    pub(crate) fn lookup_local(&self, name: &str) -> Option<&(String, String)> {
        for scope in self.locals.iter().rev() {
            if let Some(v) = scope.get(name) {
                return Some(v);
            }
        }
        None
    }

    /// Resolve a procedure variable's ProcedureType to get ParamLLVMInfo.
    /// Used for indirect calls to correctly distinguish open array params
    /// (need ptr + HIGH) from fixed array params (ptr only).
    pub(crate) fn resolve_proc_var_params(&self, var_name: &str) -> Vec<super::ParamLLVMInfo> {
        let tid = match self.var_types.get(var_name) {
            Some(&t) => t,
            None => {
                let mangled = self.mangle(var_name);
                match self.var_types.get(&mangled) {
                    Some(&t) => t,
                    None => return Vec::new(),
                }
            }
        };
        // Resolve through aliases
        let mut resolved = tid;
        for _ in 0..10 {
            match self.sema.types.get(resolved) {
                crate::types::Type::Alias { target, .. } => resolved = *target,
                crate::types::Type::ProcedureType { params, .. } => {
                    return params.iter().map(|p| {
                        let pt = self.sema.types.get(p.typ);
                        let is_open = matches!(pt, crate::types::Type::OpenArray { .. });
                        let llvm_ty = self.tl_type_str(p.typ);
                        super::ParamLLVMInfo {
                            name: String::new(),
                            is_var: p.is_var,
                            is_open_array: is_open,
                            llvm_type: if p.is_var { "ptr".to_string() } else { llvm_ty },
                            open_array_elem_type: if is_open {
                                if let crate::types::Type::OpenArray { elem_type } = pt {
                                    Some(self.tl_type_str(*elem_type))
                                } else { None }
                            } else { None },
                        }
                    }).collect();
                }
                _ => return Vec::new(),
            }
        }
        Vec::new()
    }

    pub(crate) fn is_named_array_param(&self, name: &str) -> bool {
        for scope in self.named_array_params.iter().rev() {
            if scope.contains(name) {
                return true;
            }
        }
        false
    }

    // ── Type coercion ───────────────────────────────────────────────

    pub(crate) fn coerce_val(&mut self, val: &Val, target_ty: &str) -> Val {
        if val.ty == target_ty {
            return val.clone();
        }

        // Same bit width — no conversion needed for integer types
        if val.ty.starts_with('i') && target_ty.starts_with('i') {
            let src_bits = self.int_bits(&val.ty);
            let dst_bits = self.int_bits(target_ty);
            if src_bits == dst_bits {
                return val.clone();
            }
            let tmp = self.next_tmp();
            if src_bits < dst_bits {
                // Use zext for unsigned types (CARDINAL, LONGCARD, BYTE, WORD, BITSET, CHAR, ADDRESS)
                let is_unsigned = val.type_id
                    .map(|tid| crate::types::is_unsigned_type(&self.sema.types, tid))
                    .unwrap_or(false);
                let ext_op = if is_unsigned { "zext" } else { "sext" };
                self.emitln(&format!("  {} = {} {} {} to {}", tmp, ext_op, val.ty, val.name, target_ty));
            } else {
                self.emitln(&format!("  {} = trunc {} {} to {}", tmp, val.ty, val.name, target_ty));
            }
            return Val::new(tmp, target_ty.to_string());
        }

        // Int to float
        if val.ty.starts_with('i') && Self::is_float_type(target_ty) {
            let tmp = self.next_tmp();
            let is_unsigned = val.type_id
                .map(|tid| crate::types::is_unsigned_type(&self.sema.types, tid))
                .unwrap_or(false);
            let conv_op = if is_unsigned { "uitofp" } else { "sitofp" };
            self.emitln(&format!("  {} = {} {} {} to {}", tmp, conv_op, val.ty, val.name, target_ty));
            return Val::new(tmp, target_ty.to_string());
        }

        // Float to int
        if Self::is_float_type(&val.ty) && target_ty.starts_with('i') {
            let tmp = self.next_tmp();
            self.emitln(&format!("  {} = fptosi {} {} to {}", tmp, val.ty, val.name, target_ty));
            return Val::new(tmp, target_ty.to_string());
        }

        // Float to float (float <-> double)
        if Self::is_float_type(&val.ty) && Self::is_float_type(target_ty) {
            let tmp = self.next_tmp();
            if val.ty == "float" && target_ty == "double" {
                self.emitln(&format!("  {} = fpext float {} to double", tmp, val.name));
            } else {
                self.emitln(&format!("  {} = fptrunc double {} to float", tmp, val.name));
            }
            return Val::new(tmp, target_ty.to_string());
        }

        // Int to ptr
        if val.ty.starts_with('i') && target_ty == "ptr" {
            let tmp = self.next_tmp();
            self.emitln(&format!("  {} = inttoptr {} {} to ptr", tmp, val.ty, val.name));
            return Val::new(tmp, "ptr".to_string());
        }

        // Ptr to int
        if val.ty == "ptr" && target_ty.starts_with('i') {
            let tmp = self.next_tmp();
            self.emitln(&format!("  {} = ptrtoint ptr {} to {}", tmp, val.name, target_ty));
            return Val::new(tmp, target_ty.to_string());
        }

        // Fallback: just use the value as-is (may produce invalid IR)
        val.clone()
    }

    /// Check if a Val is a single-char string constant and return the char value instead.
    pub(crate) fn coerce_string_to_char(&self, val: &Val) -> Option<Val> {
        if val.ty == "ptr" && val.name.starts_with("@.str.") {
            // Look up in string pool
            for (content, name, _) in &self.string_pool {
                if *name == val.name && content.len() == 1 {
                    return Some(Val::new(format!("{}", content.as_bytes()[0]), "i8".to_string()));
                }
            }
        }
        None
    }

    pub(crate) fn common_type(&self, a: &str, b: &str) -> String {
        if a == b { return a.to_string(); }

        // Float wins over int
        if a == "double" || b == "double" { return "double".to_string(); }
        if a == "float" || b == "float" { return "float".to_string(); }

        // Wider int wins
        if a == "i64" || b == "i64" { return "i64".to_string(); }
        if a == "i32" || b == "i32" { return "i32".to_string(); }
        if a == "i16" || b == "i16" { return "i16".to_string(); }
        if a == "i8" || b == "i8" { return "i8".to_string(); }

        // Pointer
        if a == "ptr" || b == "ptr" { return "ptr".to_string(); }

        a.to_string()
    }

    pub(crate) fn int_bits(&self, ty: &str) -> usize {
        match ty {
            "i1" => 1,
            "i8" => 8,
            "i16" => 16,
            "i32" => 32,
            "i64" => 64,
            _ => 32,
        }
    }

    pub(crate) fn extract_array_elem_type(&self, ty: &str) -> String {
        // Parse "[N x T]" → "T"
        if let Some(rest) = ty.strip_prefix('[') {
            if let Some(x_pos) = rest.find(" x ") {
                let elem = &rest[x_pos + 3..];
                if let Some(elem) = elem.strip_suffix(']') {
                    return elem.to_string();
                }
            }
        }
        "i8".to_string()
    }

    pub(crate) fn llvm_type_for_name(&self, name: &str) -> String {
        match name {
            "INTEGER" => "i32".to_string(),
            "CARDINAL" => "i32".to_string(),
            "LONGINT" => "i64".to_string(),
            "LONGCARD" => "i64".to_string(),
            "REAL" => "float".to_string(),
            "LONGREAL" => "double".to_string(),
            "BOOLEAN" => "i32".to_string(),
            "CHAR" => "i8".to_string(),
            "ADDRESS" => "ptr".to_string(),
            _ => {
                if let Some(sym) = self.sema.symtab.lookup_any(name) {
                    self.tl_type_str(sym.typ)
                } else if let Some(ty) = self.type_map.get(name) {
                    ty.clone()
                } else {
                    "i32".to_string()
                }
            }
        }
    }

    pub(crate) fn llvm_zero_initializer(&self, ty: &str) -> String {
        if ty.starts_with('i') {
            "0".to_string()
        } else if ty == "float" {
            "0.0".to_string()
        } else if ty == "double" {
            "0.0".to_string()
        } else if ty == "ptr" {
            "null".to_string()
        } else if ty.starts_with('[') {
            "zeroinitializer".to_string()
        } else if ty.starts_with('{') || ty.starts_with('%') {
            "zeroinitializer".to_string()
        } else {
            "zeroinitializer".to_string()
        }
    }

    pub(crate) fn const_eval_expr(&self, expr: &Expr) -> Option<i64> {
        match &expr.kind {
            ExprKind::IntLit(v) => Some(*v),
            ExprKind::CharLit(c) => Some(*c as i64),
            ExprKind::StringLit(s) if s.len() == 1 => Some(s.as_bytes()[0] as i64),
            ExprKind::BoolLit(b) => Some(if *b { 1 } else { 0 }),
            ExprKind::Designator(d) if d.selectors.is_empty() => {
                match d.ident.name.as_str() {
                    "TRUE" => return Some(1),
                    "FALSE" => return Some(0),
                    _ => {}
                }
                self.const_values.get(&d.ident.name).copied()
                    .or_else(|| self.enum_variants.get(&d.ident.name).copied())
            }
            ExprKind::UnaryOp { op: UnaryOp::Neg, operand } => {
                self.const_eval_expr(operand).map(|v| -v)
            }
            ExprKind::FuncCall { desig, args } if desig.selectors.is_empty() => {
                match desig.ident.name.as_str() {
                    "MAX" => {
                        if let Some(arg) = args.first() {
                            if let ExprKind::Designator(d) = &arg.kind {
                                match d.ident.name.as_str() {
                                    "INTEGER" | "LONGINT" => return Some(i32::MAX as i64),
                                    "CARDINAL" | "LONGCARD" => return Some(u32::MAX as i64),
                                    "CHAR" => return Some(255),
                                    "BOOLEAN" => return Some(1),
                                    _ => {}
                                }
                            }
                        }
                        None
                    }
                    "MIN" => {
                        if let Some(arg) = args.first() {
                            if let ExprKind::Designator(d) = &arg.kind {
                                match d.ident.name.as_str() {
                                    "INTEGER" | "LONGINT" => return Some(i32::MIN as i64),
                                    "CARDINAL" | "LONGCARD" => return Some(0),
                                    "CHAR" => return Some(0),
                                    "BOOLEAN" => return Some(0),
                                    _ => {}
                                }
                            }
                        }
                        None
                    }
                    "SIZE" | "TSIZE" => {
                        if let Some(arg) = args.first() {
                            if let ExprKind::Designator(d) = &arg.kind {
                                match d.ident.name.as_str() {
                                    "INTEGER" | "CARDINAL" | "REAL" => return Some(4),
                                    "LONGINT" | "LONGCARD" | "LONGREAL" | "ADDRESS" => return Some(8),
                                    "CHAR" | "BYTE" => return Some(1),
                                    "BOOLEAN" => return Some(4),
                                    _ => {}
                                }
                            }
                        }
                        None
                    }
                    "ORD" => {
                        if let Some(arg) = args.first() { self.const_eval_expr(arg) } else { None }
                    }
                    "CHR" => {
                        if let Some(arg) = args.first() { self.const_eval_expr(arg) } else { None }
                    }
                    _ => None,
                }
            }
            ExprKind::BinaryOp { op, left, right } => {
                let l = self.const_eval_expr(left)?;
                let r = self.const_eval_expr(right)?;
                Some(match op {
                    BinaryOp::Add => l + r,
                    BinaryOp::Sub => l - r,
                    BinaryOp::Mul => l * r,
                    BinaryOp::IntDiv => if r != 0 { l / r } else { 0 },
                    BinaryOp::Mod => if r != 0 { l % r } else { 0 },
                    _ => return None,
                })
            }
            _ => None,
        }
    }
}
