use super::*;

impl CodeGen {
    pub(crate) fn type_to_c(&self, tn: &TypeNode) -> String {
        match tn {
            TypeNode::Named(qi) => self.named_type_to_c(qi),
            TypeNode::Array { elem_type, .. } => self.type_to_c(elem_type),
            TypeNode::OpenArray { elem_type, .. } => self.type_to_c(elem_type),
            TypeNode::Record { .. } => "struct /* record */".to_string(),
            TypeNode::Pointer { base, .. } => format!("{} *", self.type_to_c(base)),
            TypeNode::Set { .. } => "uint32_t".to_string(),
            TypeNode::Enumeration { .. } => "int".to_string(),
            TypeNode::Subrange { .. } => "int32_t".to_string(),
            TypeNode::ProcedureType {
                params,
                return_type,
                ..
            } => {
                let ret = if let Some(rt) = return_type {
                    self.type_to_c(rt)
                } else {
                    "void".to_string()
                };
                format!("{} (*)", ret) // simplified — use proc_type_decl for full declarations
            }
            TypeNode::Ref { target, .. } => format!("{} *", self.type_to_c(target)),
            TypeNode::RefAny { .. } => "void *".to_string(),
            TypeNode::Object { .. } => "void * /* OBJECT */".to_string(),
        }
    }

    /// Generate a proper C function pointer declaration with the variable/param name
    /// embedded in the correct position: `RetType (*name)(param_types)`
    /// If `is_ptr` is true, generates `RetType (**name)(param_types)` for VAR parameters.
    pub(crate) fn proc_type_decl(&mut self, tn: &TypeNode, name: &str, is_ptr: bool) -> String {
        if let TypeNode::ProcedureType { params, return_type, .. } = tn {
            let ret = if let Some(rt) = return_type {
                self.type_to_c(rt)
            } else {
                "void".to_string()
            };
            let star = if is_ptr { "**" } else { "*" };
            let mut param_strs = Vec::new();
            if params.is_empty() {
                param_strs.push("void".to_string());
            } else {
                for fp in params {
                    let pt = self.type_to_c(&fp.typ);
                    let is_open = matches!(fp.typ, TypeNode::OpenArray { .. });
                    for _ in &fp.names {
                        if is_open {
                            param_strs.push(format!("{} *", pt));
                            param_strs.push("uint32_t".to_string());
                        } else if fp.is_var {
                            param_strs.push(format!("{} *", pt));
                        } else {
                            param_strs.push(pt.clone());
                        }
                    }
                    // If no names (unnamed params), still emit the type
                    if fp.names.is_empty() {
                        if is_open {
                            param_strs.push(format!("{} *", pt));
                            param_strs.push("uint32_t".to_string());
                        } else if fp.is_var {
                            param_strs.push(format!("{} *", pt));
                        } else {
                            param_strs.push(pt.clone());
                        }
                    }
                }
            }
            format!("{} ({}{})({})", ret, star, name, param_strs.join(", "))
        } else {
            // Not a procedure type — fallback to normal declaration
            let ctype = self.type_to_c(tn);
            if is_ptr {
                format!("{} *{}", ctype, name)
            } else {
                format!("{} {}", ctype, name)
            }
        }
    }

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
            // Check if this module re-exports the type from another module
            if let Some(def_mod) = self.def_modules.get(module.as_str()) {
                for imp in &def_mod.imports {
                    if let Some(ref from_mod) = imp.from_module {
                        if imp.names.iter().any(|n| n.name == qi.name) {
                            let source_prefixed = format!("{}_{}", from_mod, self.mangle(&qi.name));
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

    pub(crate) fn type_array_suffix(&self, tn: &TypeNode) -> String {
        match tn {
            TypeNode::Array { index_types, elem_type, .. } => {
                let mut s = String::new();
                for idx in index_types {
                    s.push_str(&self.index_type_to_size(idx));
                }
                // If elem_type is also an array, recurse for its suffix
                s.push_str(&self.type_array_suffix(elem_type));
                s
            }
            _ => String::new(),
        }
    }

    pub(crate) fn index_type_to_size(&self, idx: &TypeNode) -> String {
        match idx {
            TypeNode::Subrange { low, high, .. } => {
                let hi = self.const_expr_to_string(high);
                // Allocate high+1 elements so indices up to high are valid.
                format!("[{} + 1]", hi)
            }
            TypeNode::Named(qi) => {
                match qi.name.as_str() {
                    "BOOLEAN" => "[2]".to_string(),
                    "CHAR" => "[256]".to_string(),
                    _ => {
                        // Enum or other named ordinal type — use m2_max_Name + 1
                        let c_name = self.qualident_to_c(qi);
                        format!("[m2_max_{} + 1]", c_name)
                    }
                }
            }
            TypeNode::Enumeration { variants, .. } => {
                format!("[{}]", variants.len())
            }
            _ => "[/* size */]".to_string(),
        }
    }

    pub(crate) fn const_expr_to_string(&self, expr: &Expr) -> String {
        // Try to evaluate to a compile-time integer first
        if let Some(val) = self.try_eval_const_int(expr) {
            return format!("{}", val);
        }
        match &expr.kind {
            ExprKind::IntLit(v) => format!("{}", v),
            ExprKind::CharLit(c) => format!("'{}'", c),
            ExprKind::Designator(d) => {
                if let Some(module) = &d.ident.module {
                    if self.foreign_modules.contains(module.as_str()) {
                        d.ident.name.clone()
                    } else {
                        format!("{}_{}", module, d.ident.name)
                    }
                } else {
                    self.mangle(&d.ident.name)
                }
            }
            ExprKind::BinaryOp { op, left, right } => {
                let l = self.const_expr_to_string(left);
                let r = self.const_expr_to_string(right);
                let op_str = match op {
                    BinaryOp::Add => "+",
                    BinaryOp::Sub => "-",
                    BinaryOp::Mul => "*",
                    _ => "?",
                };
                format!("({} {} {})", l, op_str, r)
            }
            _ => "0".to_string(),
        }
    }

    pub(crate) fn is_proc_type(tn: &TypeNode) -> bool {
        match tn {
            TypeNode::ProcedureType { .. } => true,
            TypeNode::Named(qi) if qi.module.is_none() && qi.name == "PROC" => true,
            _ => false,
        }
    }

    /// Check if a TypeNode is ARRAY [...] OF CHAR
    pub(crate) fn is_char_array_type(&self, tn: &TypeNode) -> bool {
        match tn {
            TypeNode::Array { elem_type, .. } => {
                matches!(elem_type.as_ref(), TypeNode::Named(qi) if qi.name == "CHAR")
            }
            TypeNode::Named(qi) => self.char_array_types.contains(&qi.name),
            _ => false,
        }
    }

    /// Check if a TypeNode is a pointer type (POINTER TO ...)
    pub(crate) fn is_pointer_type(&self, tn: &TypeNode) -> bool {
        match tn {
            TypeNode::Pointer { .. } => true,
            TypeNode::Named(qi) => {
                // Check if the named type resolves to a pointer typedef
                // by checking if it's NOT in array_types and the C type ends with *
                let c = self.type_to_c(tn);
                c.ends_with('*')
            }
            _ => false,
        }
    }

    /// Check if a TypeNode is any array type (for memcpy assignment)
    pub(crate) fn is_array_type(&self, tn: &TypeNode) -> bool {
        match tn {
            TypeNode::Array { .. } => true,
            TypeNode::Named(qi) => self.array_types.contains(&qi.name),
            _ => false,
        }
    }

    /// Check if a field name belongs to an array-typed record field (name-only, may false-positive)
    pub(crate) fn is_array_field(&self, field_name: &str) -> bool {
        for ((_rec_name, fname)) in &self.array_fields {
            if fname == field_name {
                return true;
            }
        }
        false
    }

    /// Type-aware check: is `field_name` an array field of record type `record_type`?
    pub(crate) fn is_array_field_of(&self, record_type: &str, field_name: &str) -> bool {
        self.array_fields.contains(&(record_type.to_string(), field_name.to_string()))
    }

    /// Check if an expression is obviously a scalar (literal, arithmetic, non-array variable,
    /// or a field access to a non-array field). Used as a safety guard to prevent emitting
    /// memcpy for scalar sources when type resolution fails in the fallback path.
    pub(crate) fn is_scalar_expr(&self, expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::IntLit(_)
            | ExprKind::RealLit(_)
            | ExprKind::CharLit(_)
            | ExprKind::BoolLit(_)
            | ExprKind::NilLit => true,
            ExprKind::BinaryOp { .. } | ExprKind::UnaryOp { .. } | ExprKind::Not(_) | ExprKind::Deref(_) => true,
            ExprKind::Designator(d) => {
                if d.selectors.is_empty() {
                    // Simple variable: scalar if not an array var
                    !self.array_vars.contains(&d.ident.name)
                } else if let Some(Selector::Field(fname, _)) = d.selectors.last() {
                    // Field access: try type-aware check first
                    if let Some(rec_type) = self.resolve_field_record_type(d) {
                        !self.is_array_field_of(&rec_type, fname)
                    } else {
                        // Fallback: if no record type has this as an array field, it's scalar
                        !self.is_array_field(fname)
                    }
                } else {
                    false
                }
            }
            ExprKind::FuncCall { .. } => true,
            _ => false,
        }
    }

    /// Check if an expression is a multi-char string or a char array variable
    pub(crate) fn is_string_expr(&self, expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::StringLit(s) => s.len() > 1,
            ExprKind::Designator(d) => {
                d.ident.module.is_none() && d.selectors.is_empty()
                    && self.char_array_vars.contains(&d.ident.name)
            }
            _ => false,
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

    pub(crate) fn is_char_array_field(&self, field_name: &str) -> bool {
        // Check if any record type has a field with this name that is a char array
        for ((_rec_name, fname)) in &self.char_array_fields {
            if fname == field_name {
                return true;
            }
        }
        false
    }

    pub(crate) fn is_set_type(&self, tn: &TypeNode) -> bool {
        match tn {
            TypeNode::Named(qi) => qi.name == "BITSET",
            TypeNode::Set { .. } => true,
            _ => false,
        }
    }

    /// Check if an expression is a set value (set constructor or known set variable)
    pub(crate) fn is_set_expr(&self, expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::SetConstructor { .. } => true,
            ExprKind::Designator(d) => {
                d.ident.module.is_none() && d.selectors.is_empty()
                    && self.set_vars.contains(&d.ident.name)
            }
            ExprKind::FuncCall { desig, .. } => {
                // BITSET(x) is a set expression
                desig.ident.name == "BITSET" && desig.ident.module.is_none()
            }
            ExprKind::BinaryOp { left, right, .. } => {
                // If either operand is a set, the result is a set
                self.is_set_expr(left) || self.is_set_expr(right)
            }
            ExprKind::Not(inner) => self.is_set_expr(inner),
            ExprKind::Deref(_) => false,
            _ => false,
        }
    }

    /// Check if an expression is likely CARDINAL/unsigned (for DIV/MOD codegen)
    pub(crate) fn is_address_expr(&self, expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::Designator(d) => {
                d.ident.module.is_none() && d.selectors.is_empty()
                    && self.var_types.get(&d.ident.name).map_or(false, |t| t == "ADDRESS")
            }
            ExprKind::BinaryOp { left, right, .. } => {
                self.is_address_expr(left) || self.is_address_expr(right)
            }
            _ => false,
        }
    }

    pub(crate) fn is_unsigned_expr(&self, expr: &Expr) -> bool {
        if self.is_address_expr(expr) {
            return true;
        }
        match &expr.kind {
            ExprKind::Designator(d) => {
                if d.selectors.is_empty() && d.ident.module.is_none() {
                    if self.cardinal_vars.contains(&d.ident.name) {
                        return true;
                    }
                    // Check if variable's type is a known unsigned alias
                    if let Some(type_name) = self.var_types.get(&d.ident.name) {
                        if self.unsigned_type_aliases.contains(type_name) {
                            return true;
                        }
                    }
                }
                false
            }
            ExprKind::FuncCall { desig, args } => {
                // CARDINAL/LONGCARD type transfer, ORD, HIGH, SIZE, TSIZE, SHR, SHL, BAND, BOR, BXOR, BNOT
                match desig.ident.name.as_str() {
                    "CARDINAL" | "LONGCARD" | "ORD" | "HIGH" | "SIZE" | "TSIZE"
                    | "SHR" | "SHL" | "BAND" | "BOR" | "BXOR" | "BNOT" | "SHIFT" | "ROTATE" => true,
                    "VAL" => {
                        // VAL(CARDINAL, x) or VAL(LONGCARD, x) is unsigned
                        if let Some(first_arg) = args.first() {
                            if let ExprKind::Designator(d) = &first_arg.kind {
                                matches!(d.ident.name.as_str(), "CARDINAL" | "LONGCARD")
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    }
                    _ => false,
                }
            }
            ExprKind::IntLit(n) => {
                // Literals that exceed signed 32-bit range are unsigned
                *n > i32::MAX as i64
            }
            ExprKind::BinaryOp { left, right, .. } => {
                self.is_unsigned_expr(left) || self.is_unsigned_expr(right)
            }
            _ => false,
        }
    }

    /// Returns true if the expression is a 64-bit type (LONGINT, LONGCARD, or alias thereof).
    /// Used to select m2_div64/m2_mod64 over the 32-bit versions.
    pub(crate) fn is_long_expr(&self, expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::Designator(d) => {
                if d.selectors.is_empty() && d.ident.module.is_none() {
                    if self.longint_vars.contains(&d.ident.name)
                        || self.longcard_vars.contains(&d.ident.name) {
                        return true;
                    }
                    if let Some(type_name) = self.var_types.get(&d.ident.name) {
                        if type_name == "LONGINT" || type_name == "LONGCARD"
                            || self.unsigned_type_aliases.contains(type_name) {
                            return true;
                        }
                    }
                }
                false
            }
            ExprKind::FuncCall { desig, args } => {
                match desig.ident.name.as_str() {
                    "LONGINT" | "LONGCARD" | "LONG" => true,
                    "VAL" => {
                        if let Some(first_arg) = args.first() {
                            if let ExprKind::Designator(d) = &first_arg.kind {
                                matches!(d.ident.name.as_str(), "LONGINT" | "LONGCARD")
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    }
                    _ => false,
                }
            }
            ExprKind::IntLit(n) => {
                *n > i32::MAX as i64 || *n < i32::MIN as i64
            }
            ExprKind::BinaryOp { left, right, .. } => {
                self.is_long_expr(left) || self.is_long_expr(right)
            }
            _ => false,
        }
    }

    pub(crate) fn is_complex_type(&self, tn: &TypeNode) -> bool {
        match tn {
            TypeNode::Named(qi) => qi.name == "COMPLEX",
            _ => false,
        }
    }

    pub(crate) fn is_longcomplex_type(&self, tn: &TypeNode) -> bool {
        match tn {
            TypeNode::Named(qi) => qi.name == "LONGCOMPLEX",
            _ => false,
        }
    }

    pub(crate) fn is_complex_expr(&self, expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::Designator(d) => {
                d.ident.module.is_none() && d.selectors.is_empty()
                    && (self.complex_vars.contains(&d.ident.name)
                        || self.longcomplex_vars.contains(&d.ident.name))
            }
            ExprKind::FuncCall { desig, .. } => {
                // CMPLX() returns complex
                desig.ident.name == "CMPLX"
            }
            ExprKind::BinaryOp { left, right, .. } => {
                self.is_complex_expr(left) || self.is_complex_expr(right)
            }
            ExprKind::UnaryOp { operand, .. } => self.is_complex_expr(operand),
            _ => false,
        }
    }

    pub(crate) fn is_longcomplex_expr(&self, expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::Designator(d) => {
                d.ident.module.is_none() && d.selectors.is_empty()
                    && self.longcomplex_vars.contains(&d.ident.name)
            }
            ExprKind::BinaryOp { left, right, .. } => {
                self.is_longcomplex_expr(left) || self.is_longcomplex_expr(right)
            }
            _ => false,
        }
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


    // ── Modula-2+ OBJECT Type Codegen ────────────────────────────────

    /// Walk a designator's selectors to determine the record type that owns the last field.
    /// Returns None if we can't resolve the type (e.g., through pointer deref or array indexing).
    pub(crate) fn resolve_field_record_type(&self, desig: &Designator) -> Option<String> {
        let mut current = self.var_types.get(&desig.ident.name).cloned();
        let sels = &desig.selectors;
        if sels.is_empty() {
            return None;
        }
        // Walk all selectors except the last (which is the field we want the *owner* type for)
        let stop = sels.len() - 1;
        for i in 0..stop {
            match &sels[i] {
                Selector::Field(name, _) => {
                    if let Some(ref tn) = current {
                        current = self.record_field_types.get(&(tn.clone(), name.clone())).cloned();
                    }
                }
                Selector::Deref(_) => {
                    // Pointer deref: type info not tracked, bail out
                    current = None;
                }
                Selector::Index(_, _) => {
                    // Array indexing: resolve element type if tracked
                    if i == 0 {
                        // First selector is index on the base variable
                        current = self.array_var_elem_types.get(&desig.ident.name).cloned();
                    } else {
                        // Nested index — bail
                        current = None;
                    }
                }
            }
        }
        current
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

    /// Resolve the type of a complex designator by walking through selectors.
    /// Returns the TypeId of the final resolved type, or None if resolution fails.
    pub(crate) fn resolve_designator_type(&self, desig: &Designator) -> Option<TypeId> {
        use crate::types::Type;
        // Look up base variable
        let base_name = if let Some(ref m) = desig.ident.module {
            // Module-qualified: look for Module_Name
            format!("{}_{}", m, desig.ident.name)
        } else {
            desig.ident.name.clone()
        };
        let sym = self.sema.symtab.lookup(&base_name)
            .or_else(|| self.sema.symtab.lookup_any(&base_name))?;
        let mut tid = sym.typ;

        for sel in &desig.selectors {
            // Follow aliases and pointers
            tid = self.unwrap_type_aliases(tid);
            match sel {
                Selector::Field(fname, _) => {
                    // Unwrap pointer/ref if implicit deref
                    tid = self.unwrap_pointers(tid);
                    tid = self.unwrap_type_aliases(tid);
                    match self.sema.types.get(tid) {
                        Type::Record { fields, .. } => {
                            if let Some(f) = fields.iter().find(|f| f.name == *fname) {
                                tid = f.typ;
                            } else {
                                return None;
                            }
                        }
                        Type::Object { fields, .. } => {
                            if let Some(f) = fields.iter().find(|f| f.name == *fname) {
                                tid = f.typ;
                            } else {
                                return None;
                            }
                        }
                        _ => return None,
                    }
                }
                Selector::Index(_, _) => {
                    match self.sema.types.get(tid) {
                        Type::Array { elem_type, .. } => tid = *elem_type,
                        Type::OpenArray { elem_type } => tid = *elem_type,
                        _ => return None,
                    }
                }
                Selector::Deref(_) => {
                    tid = self.unwrap_pointers(tid);
                }
            }
        }
        Some(tid)
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

    /// Get proc param info for a complex designator call by resolving its type.
    pub(crate) fn get_designator_proc_param_info(&self, desig: &Designator) -> Vec<ParamCodegenInfo> {
        // First try full type resolution through the sema type system
        if let Some(tid) = self.resolve_designator_type(desig) {
            if let Some(info) = self.param_info_from_proc_type(tid) {
                return info;
            }
        }
        // Fallback: check the last Field selector against field_proc_params
        // (for embedded modules where the symtab doesn't have local params)
        if let Some(last_sel) = desig.selectors.last() {
            if let Selector::Field(fname, _) = last_sel {
                if let Some(info) = self.field_proc_params.get(fname) {
                    return info.clone();
                }
            }
        }
        Vec::new()
    }

    /// Get VAR parameter flags for a named procedure
    pub(crate) fn get_var_param_flags(&self, name: &str) -> Vec<bool> {
        self.get_param_info(name).iter().map(|p| p.is_var).collect()
    }

    /// Resolve a local name (possibly an alias) to the original imported name.
    pub(crate) fn original_import_name<'a>(&'a self, local_name: &'a str) -> &'a str {
        self.import_alias_map.get(local_name).map(|s| s.as_str()).unwrap_or(local_name)
    }

    /// Check if a name is a known type and return the C type name for casting.
    /// Returns None if the name is not a type (i.e., it's a procedure or variable).
    pub(crate) fn resolve_type_cast_name(&self, name: &str) -> Option<String> {
        // Check the known_type_names set (populated from def modules and gen_type_decl)
        if self.known_type_names.contains(name) {
            // It's a type — return the mangled C name
            // Check if this is a module-local type in an embedded module
            // (works both during type decl phase and procedure body phase)
            let local_prefixed = format!("{}_{}", self.module_name, self.mangle(name));
            if self.embedded_enum_types.contains(&local_prefixed) {
                return Some(local_prefixed);
            }
            // Check if it's an imported type (module-prefixed in C)
            if let Some(source_mod) = self.import_map.get(name) {
                let orig = self.original_import_name(name);
                let import_prefixed = format!("{}_{}", source_mod, self.mangle(orig));
                if self.embedded_enum_types.contains(&import_prefixed) {
                    return Some(import_prefixed);
                }
            }
            return Some(self.mangle(name));
        }
        // Also check sema symtab as fallback
        if let Some(sym) = self.sema.symtab.lookup_any(name) {
            if matches!(sym.kind, crate::symtab::SymbolKind::Type) {
                let qi = crate::ast::QualIdent {
                    module: None,
                    name: name.to_string(),
                    loc: crate::errors::SourceLoc::default(),
                };
                return Some(self.type_to_c(&crate::ast::TypeNode::Named(qi)));
            }
        }
        None
    }

}
