use super::*;

impl CodeGen {
    pub(crate) fn named_type_to_c(&self, qi: &QualIdent) -> String {
        // If module-qualified (e.g., Stack.Stack), prefix with module name
        if let Some(module) = &qi.module {
            if self.foreign_modules.contains(module.as_str()) {
                return self.mangle(&qi.name);
            }
            let prefixed = format!("{}_{}", module, self.mangle(&qi.name));
            // For re-exported types (e.g., Promise.Status where Promise imports Status
            // from Scheduler), resolve to the original source module's prefixed name
            if self.embedded_enum_types.contains(&prefixed) {
                return prefixed;
            }
            // Check if this module re-exports the type from another module via sema
            if let Some(scope_id) = self.sema.symtab.lookup_module_scope(module) {
                if let Some(sym) = self.sema.symtab.lookup_in_scope(scope_id, &qi.name) {
                    if let Some(ref src_mod) = sym.module {
                        if src_mod != module {
                            let source_prefixed = format!("{}_{}", src_mod, self.mangle(&qi.name));
                            if self.embedded_enum_types.contains(&source_prefixed) {
                                return source_prefixed;
                            }
                        }
                    }
                }
            }
            return prefixed;
        }
        match qi.name.as_str() {
            "INTEGER" => "int32_t".to_string(),
            "CARDINAL" => "uint32_t".to_string(),
            "REAL" => "float".to_string(),
            "LONGREAL" => "double".to_string(),
            "BOOLEAN" => "int".to_string(),
            "CHAR" => "char".to_string(),
            "BITSET" => "uint32_t".to_string(),
            "WORD" => "uint32_t".to_string(),
            "BYTE" => "uint8_t".to_string(),
            "ADDRESS" => "void *".to_string(),
            "LONGINT" => "int64_t".to_string(),
            "LONGCARD" => "uint64_t".to_string(),
            "COMPLEX" => "m2_COMPLEX".to_string(),
            "LONGCOMPLEX" => "m2_LONGCOMPLEX".to_string(),
            "PROC" => "void (*)(void)".to_string(),
            "File" if self.import_map.get("File").map_or(false, |m| {
                matches!(m.as_str(), "FileSystem" | "FIO" | "RawIO" | "StreamFile")
            }) => "m2_File".to_string(),
            other => {
                // Check if this is a module-local enum type in an embedded implementation
                // (e.g., "Status" inside Poller module → "Poller_Status")
                let local_prefixed = format!("{}_{}", self.module_name, self.mangle(other));
                if self.embedded_enum_types.contains(&local_prefixed) {
                    return local_prefixed;
                }
                // Check if imported from another embedded module
                // (e.g., "Status" from Stream → "Stream_Status",
                //  "Renderer" from Gfx → "Gfx_Renderer")
                if let Some(module) = self.import_map.get(other) {
                    let prefixed = format!("{}_{}", module, self.mangle(other));
                    if self.embedded_enum_types.contains(&prefixed) || self.known_type_names.contains(&prefixed) {
                        return prefixed;
                    }
                }
                self.mangle(other)
            },
        }
    }

    /// Mangle a bare type name (e.g. from TSIZE argument) using the same
    /// resolution logic as named_type_to_c: check import_map, known_type_names,
    /// embedded_enum_types, and module-local prefixing.
    pub(crate) fn mangle_type_name(&self, name: &str) -> String {
        // Built-in types pass through as-is (builtins.rs handles the C mapping)
        match name {
            "INTEGER" | "CARDINAL" | "REAL" | "LONGREAL" | "BOOLEAN" | "CHAR"
            | "BITSET" | "WORD" | "BYTE" | "ADDRESS" | "LONGINT" | "LONGCARD" => {
                return name.to_string();
            }
            _ => {}
        }
        // Module-local type?
        let local_prefixed = format!("{}_{}", self.module_name, self.mangle(name));
        if self.embedded_enum_types.contains(&local_prefixed) || self.known_type_names.contains(&local_prefixed) {
            return local_prefixed;
        }
        // Imported type?
        if let Some(module) = self.import_map.get(name) {
            let prefixed = format!("{}_{}", module, self.mangle(name));
            if self.embedded_enum_types.contains(&prefixed) || self.known_type_names.contains(&prefixed) {
                return prefixed;
            }
        }
        self.mangle(name)
    }

    pub(crate) fn qualident_to_c(&self, qi: &QualIdent) -> String {
        if let Some(module) = &qi.module {
            if self.foreign_modules.contains(module.as_str()) {
                return qi.name.clone();
            }
            format!("{}_{}", module, qi.name)
        } else {
            self.named_type_to_c(qi)
        }
    }

    pub(crate) fn is_open_array_param(&self, name: &str) -> bool {
        // Check scoped open_array_params (current procedure's params only)
        for scope in self.open_array_params.iter().rev() {
            if scope.contains(name) {
                return true;
            }
        }
        // Also check env vars: if both 'name' and 'name_high' are captured,
        // then 'name' is a captured open array parameter from an enclosing scope
        if let Some(env_vars) = self.env_access_names.last() {
            let high_name = format!("{}_high", name);
            if env_vars.contains(&name.to_string()) && env_vars.contains(&high_name) {
                return true;
            }
        }
        false
    }

    pub(crate) fn is_named_array_value_param(&self, name: &str) -> bool {
        for scope in self.named_array_value_params.iter().rev() {
            if scope.contains(name) {
                return true;
            }
        }
        false
    }

    pub(crate) fn get_named_array_param_high(&self, name: &str) -> Option<String> {
        if !self.is_named_array_value_param(name) {
            return None;
        }
        if let Some(type_name) = self.var_types.get(name) {
            if let Some(high) = self.array_type_high.get(type_name) {
                return Some(high.clone());
            }
        }
        None
    }

    /// Check if a variable name is accessed through the _env pointer in the current context
    pub(crate) fn is_env_var(&self, name: &str) -> bool {
        if let Some(env_vars) = self.env_access_names.last() {
            env_vars.contains(name)
        } else {
            false
        }
    }

    pub(crate) fn is_var_param(&self, name: &str) -> bool {
        for scope in self.var_params.iter().rev() {
            if let Some(&is_var) = scope.get(name) {
                return is_var;
            }
        }
        false
    }

    pub(crate) fn push_var_scope(&mut self) {
        self.var_params.push(HashMap::new());
        self.open_array_params.push(HashSet::new());
        self.named_array_value_params.push(HashSet::new());
    }

    pub(crate) fn pop_var_scope(&mut self) {
        self.var_params.pop();
        self.open_array_params.pop();
        self.named_array_value_params.pop();
    }

    /// Save all per-procedure variable tracking sets before entering a scope
    pub(crate) fn save_var_tracking(&self) -> VarTrackingScope {
        VarTrackingScope {
            array_vars: self.array_vars.clone(),
            char_array_vars: self.char_array_vars.clone(),
            set_vars: self.set_vars.clone(),
            cardinal_vars: self.cardinal_vars.clone(),
            longint_vars: self.longint_vars.clone(),
            longcard_vars: self.longcard_vars.clone(),
            complex_vars: self.complex_vars.clone(),
            longcomplex_vars: self.longcomplex_vars.clone(),
            var_types: self.var_types.clone(),
        }
    }

    /// Restore all per-procedure variable tracking sets after leaving a scope
    pub(crate) fn restore_var_tracking(&mut self, saved: VarTrackingScope) {
        self.array_vars = saved.array_vars;
        self.char_array_vars = saved.char_array_vars;
        self.set_vars = saved.set_vars;
        self.cardinal_vars = saved.cardinal_vars;
        self.longint_vars = saved.longint_vars;
        self.longcard_vars = saved.longcard_vars;
        self.complex_vars = saved.complex_vars;
        self.longcomplex_vars = saved.longcomplex_vars;
        self.var_types = saved.var_types;
    }

    /// Resolve a variable name (C expression string) to its M2 type name (mangled).
    /// Used to find type descriptors for M2+ NEW calls.
    pub(crate) fn resolve_var_type_name(&self, var_expr: &str) -> Option<String> {
        // Direct variable name lookup
        if let Some(type_name) = self.var_types.get(var_expr) {
            return Some(self.mangle(type_name));
        }
        None
    }


    /// Get parameter codegen info for a named procedure
    pub(crate) fn get_param_info(&self, name: &str) -> Vec<ParamCodegenInfo> {
        // Check our tracked proc params
        if let Some(params) = self.proc_params.get(name) {
            return params.clone();
        }
        // Check symtab: try current scope first, then all scopes as fallback
        // (codegen doesn't manage scope stack, so current scope may be wrong for locals)
        let sym_opt = self.sema.symtab.lookup(name)
            .or_else(|| self.sema.symtab.lookup_any(name));
        if let Some(sym) = sym_opt {
            if let crate::symtab::SymbolKind::Procedure { params, .. } = &sym.kind {
                return params.iter().map(|p| {
                    let is_open = matches!(self.sema.types.get(p.typ), Type::OpenArray { .. });
                    ParamCodegenInfo {
                        name: p.name.clone(),
                        is_var: p.is_var,
                        is_open_array: is_open,
                        is_char: p.typ == TY_CHAR,
                    }
                }).collect();
            }
            // For variables/params with procedure type, extract param info from the type
            if matches!(sym.kind, crate::symtab::SymbolKind::Variable) {
                if let Some(info) = self.param_info_from_proc_type(sym.typ) {
                    return info;
                }
            }
        }
        Vec::new()
    }

    /// Extract parameter info from a ProcedureType, following aliases.
    pub(crate) fn param_info_from_proc_type(&self, mut tid: TypeId) -> Option<Vec<ParamCodegenInfo>> {
        // Follow aliases to find the underlying type
        loop {
            match self.sema.types.get(tid) {
                Type::Alias { target, .. } => tid = *target,
                Type::ProcedureType { params, .. } => {
                    return Some(params.iter().enumerate().map(|(i, p)| {
                        let ptyp = self.sema.types.get(p.typ);
                        let is_open = matches!(ptyp, Type::OpenArray { .. });
                        let is_char = p.typ == TY_CHAR;
                        ParamCodegenInfo {
                            name: format!("p{}", i),
                            is_var: p.is_var,
                            is_open_array: is_open,
                            is_char,
                        }
                    }).collect());
                }
                _ => return None,
            }
        }
    }

    /// Follow Alias types to the underlying type
    pub(crate) fn unwrap_type_aliases(&self, mut tid: TypeId) -> TypeId {
        loop {
            match self.sema.types.get(tid) {
                Type::Alias { target, .. } => tid = *target,
                _ => return tid,
            }
        }
    }

    /// Unwrap Pointer/Ref to get the base type
    pub(crate) fn unwrap_pointers(&self, mut tid: TypeId) -> TypeId {
        tid = self.unwrap_type_aliases(tid);
        match self.sema.types.get(tid) {
            Type::Pointer { base } => *base,
            Type::Ref { target, .. } => *target,
            _ => tid,
        }
    }

    /// Get VAR parameter flags for a named procedure
    pub(crate) fn get_var_param_flags(&self, name: &str) -> Vec<bool> {
        self.get_param_info(name).iter().map(|p| p.is_var).collect()
    }

    /// Resolve a local name (possibly an alias) to the original imported name.
    pub(crate) fn original_import_name<'a>(&'a self, local_name: &'a str) -> &'a str {
        self.import_alias_map.get(local_name).map(|s| s.as_str()).unwrap_or(local_name)
    }

}
