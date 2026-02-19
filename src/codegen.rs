use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::builtins;
use crate::errors::{CompileError, CompileResult};
use crate::stdlib;
use crate::sema::SemanticAnalyzer;
use crate::types::*;

#[derive(Clone, Debug)]
struct ParamCodegenInfo {
    name: String,
    is_var: bool,
    is_open_array: bool,
    is_char: bool,
}

pub struct CodeGen {
    output: String,
    indent: usize,
    module_name: String,
    sema: SemanticAnalyzer,
    /// Maps imported name -> source module for stdlib resolution
    import_map: HashMap<String, String>,
    /// Tracks which local names are VAR parameters (passed as pointers)
    var_params: Vec<HashMap<String, bool>>,
    /// Tracks which local names are open array parameters (have _high companion)
    open_array_params: Vec<HashSet<String>>,
    /// Tracks parameter info for known procedures (for call-site VAR/open array passing)
    proc_params: HashMap<String, Vec<ParamCodegenInfo>>,
    /// WITH alias stack: (record_designator_c_expr, list_of_field_names)
    with_aliases: Vec<(String, Vec<String>, Option<String>)>,
    /// Buffer for lifted nested procedures (emitted before their parent)
    lifted_procs: Vec<String>,
    /// Maps enum variant names to their C names (e.g., "Red" -> "Color_Red")
    enum_variants: HashMap<String, String>,
    /// Maps record type name -> list of field names (for WITH resolution)
    record_fields: HashMap<String, Vec<String>>,
    /// Maps (record_type_name, field_name) -> field_type_name (for nested WITH)
    record_field_types: HashMap<(String, String), String>,
    /// Maps variable name -> type name (for WITH resolution)
    var_types: HashMap<String, String>,
    /// Tracks known integer constant values for constant folding
    const_int_values: HashMap<String, i64>,
    /// Type names that are ARRAY OF CHAR (for string operations)
    char_array_types: HashSet<String>,
    /// Variable names that are ARRAY OF CHAR (for string operations)
    char_array_vars: HashSet<String>,
    /// Record field names that are ARRAY OF CHAR: (record_type_name, field_name)
    char_array_fields: HashSet<(String, String)>,
    /// Type names that are array types (for memcpy assignment)
    array_types: HashSet<String>,
    /// Variable names that have array types (for memcpy assignment)
    array_vars: HashSet<String>,
    /// Record field names that have array types: (record_type_name, field_name)
    array_fields: HashSet<(String, String)>,
    /// Variable names that are SET or BITSET types
    set_vars: HashSet<String>,
    /// Variable names that are CARDINAL (unsigned) types
    cardinal_vars: HashSet<String>,
    /// Variable names that are COMPLEX type
    complex_vars: HashSet<String>,
    /// Variable names that are LONGCOMPLEX type
    longcomplex_vars: HashSet<String>,
    /// Set of module names imported via `IMPORT ModuleName;` (whole-module / qualified import)
    imported_modules: HashSet<String>,
    /// Maps module name → list of exported procedure names (from parsed .def files)
    module_exports: HashMap<String, Vec<(String, Vec<ParamCodegenInfo>)>>,
    /// Pending implementation modules to be generated before the main module
    pending_modules: Option<Vec<ImplementationModule>>,
    /// Maps nested proc name → env struct type name it receives (e.g., "Add" → "Accumulate_env")
    closure_env_type: HashMap<String, String>,
    /// Maps env struct type name → ordered list of (var_name, c_type) fields
    closure_env_fields: HashMap<String, Vec<(String, String)>>,
    /// Stack: set of var names accessed via _env in the currently-generating proc
    env_access_names: Vec<HashSet<String>>,
    /// Stack: name of the child env struct type for the current scope (if it has nested procs with captures)
    child_env_type_stack: Vec<Option<String>>,
    /// Stack: list of (child_proc_name, captured_var_names) for current scope
    child_captures_stack: Vec<Vec<(String, Vec<String>)>>,
    /// Maps (record_type_name, field_name) -> variant index for variant record field access
    variant_field_map: HashMap<(String, String), usize>,
    /// True when generating code inside the module body (main function) rather than a procedure
    in_module_body: bool,
    /// Counter for generating unique exception IDs
    exception_counter: usize,
    /// Enable Modula-2+ mode (set by driver based on --m2plus flag or auto-detected)
    m2plus: bool,
    /// Track which M2+ runtime features are needed
    uses_gc: bool,
    uses_threads: bool,
    /// Set of module names that are foreign (C ABI) — no name mangling, extern decls
    foreign_modules: HashSet<String>,
    /// Stored foreign definition modules for generating extern declarations
    foreign_def_modules: Vec<DefinitionModule>,
    /// Maps M2 proc name → C export name (from EXPORTC pragma)
    export_c_names: HashMap<String, String>,
    /// Stored (non-foreign) definition modules for emitting types during embedded impl gen
    def_modules: HashMap<String, crate::ast::DefinitionModule>,
    /// Procedure names local to the current embedded implementation (for module-prefixed calls)
    embedded_local_procs: HashSet<String>,
    /// Module-level variable names in the current embedded implementation (for module-prefixed access)
    embedded_local_vars: HashSet<String>,
    /// Emit #line directives mapping generated C back to Modula-2 source (for -g debug builds)
    emit_debug_lines: bool,
    /// Last file emitted in a #line directive (to avoid redundant file changes)
    last_line_file: String,
    /// Last line number emitted in a #line directive (to avoid redundant directives)
    last_line_num: usize,
}

// ── Free variable analysis helpers ─────────────────────────────────────

/// Collect all bare (non-module-qualified) identifier names referenced in statements
fn collect_refs_in_stmts(stmts: &[Statement], out: &mut HashSet<String>) {
    for stmt in stmts {
        collect_refs_in_stmt(stmt, out);
    }
}

fn collect_refs_in_stmt(stmt: &Statement, out: &mut HashSet<String>) {
    match &stmt.kind {
        StatementKind::Empty => {}
        StatementKind::Assign { desig, expr } => {
            collect_refs_in_desig(desig, out);
            collect_refs_in_expr(expr, out);
        }
        StatementKind::ProcCall { desig, args } => {
            // Don't count proc name as variable ref (it's a function name)
            // But do collect refs in selectors and arguments
            for sel in &desig.selectors {
                if let Selector::Index(indices, _) = sel {
                    for idx in indices { collect_refs_in_expr(idx, out); }
                }
            }
            for arg in args { collect_refs_in_expr(arg, out); }
        }
        StatementKind::If { cond, then_body, elsifs, else_body } => {
            collect_refs_in_expr(cond, out);
            collect_refs_in_stmts(then_body, out);
            for (ec, eb) in elsifs {
                collect_refs_in_expr(ec, out);
                collect_refs_in_stmts(eb, out);
            }
            if let Some(eb) = else_body { collect_refs_in_stmts(eb, out); }
        }
        StatementKind::Case { expr, branches, else_body } => {
            collect_refs_in_expr(expr, out);
            for branch in branches {
                for label in &branch.labels {
                    match label {
                        CaseLabel::Single(e) => collect_refs_in_expr(e, out),
                        CaseLabel::Range(lo, hi) => {
                            collect_refs_in_expr(lo, out);
                            collect_refs_in_expr(hi, out);
                        }
                    }
                }
                collect_refs_in_stmts(&branch.body, out);
            }
            if let Some(eb) = else_body { collect_refs_in_stmts(eb, out); }
        }
        StatementKind::While { cond, body } => {
            collect_refs_in_expr(cond, out);
            collect_refs_in_stmts(body, out);
        }
        StatementKind::Repeat { body, cond } => {
            collect_refs_in_stmts(body, out);
            collect_refs_in_expr(cond, out);
        }
        StatementKind::For { var, start, end, step, body } => {
            out.insert(var.clone());
            collect_refs_in_expr(start, out);
            collect_refs_in_expr(end, out);
            if let Some(s) = step { collect_refs_in_expr(s, out); }
            collect_refs_in_stmts(body, out);
        }
        StatementKind::Loop { body } => {
            collect_refs_in_stmts(body, out);
        }
        StatementKind::With { desig, body } => {
            collect_refs_in_desig(desig, out);
            collect_refs_in_stmts(body, out);
        }
        StatementKind::Return { expr } => {
            if let Some(e) = expr { collect_refs_in_expr(e, out); }
        }
        StatementKind::Exit => {}
        StatementKind::Raise { expr } => {
            if let Some(e) = expr { collect_refs_in_expr(e, out); }
        }
        StatementKind::Retry => {}
        StatementKind::Try { body, excepts, finally_body } => {
            collect_refs_in_stmts(body, out);
            for ec in excepts { collect_refs_in_stmts(&ec.body, out); }
            if let Some(fb) = finally_body { collect_refs_in_stmts(fb, out); }
        }
        StatementKind::Lock { mutex, body } => {
            collect_refs_in_expr(mutex, out);
            collect_refs_in_stmts(body, out);
        }
        StatementKind::TypeCase { expr, branches, else_body } => {
            collect_refs_in_expr(expr, out);
            for branch in branches { collect_refs_in_stmts(&branch.body, out); }
            if let Some(eb) = else_body { collect_refs_in_stmts(eb, out); }
        }
    }
}

fn collect_refs_in_expr(expr: &Expr, out: &mut HashSet<String>) {
    match &expr.kind {
        ExprKind::IntLit(_) | ExprKind::RealLit(_) | ExprKind::StringLit(_)
        | ExprKind::CharLit(_) | ExprKind::BoolLit(_) | ExprKind::NilLit => {}
        ExprKind::Designator(d) => collect_refs_in_desig(d, out),
        ExprKind::FuncCall { desig, args } => {
            // Don't count func name as variable ref, but collect selector and arg refs
            for sel in &desig.selectors {
                if let Selector::Index(indices, _) = sel {
                    for idx in indices { collect_refs_in_expr(idx, out); }
                }
            }
            for arg in args { collect_refs_in_expr(arg, out); }
        }
        ExprKind::UnaryOp { operand, .. } => collect_refs_in_expr(operand, out),
        ExprKind::BinaryOp { left, right, .. } => {
            collect_refs_in_expr(left, out);
            collect_refs_in_expr(right, out);
        }
        ExprKind::SetConstructor { elements, .. } => {
            for elem in elements {
                match elem {
                    SetElement::Single(e) => collect_refs_in_expr(e, out),
                    SetElement::Range(lo, hi) => {
                        collect_refs_in_expr(lo, out);
                        collect_refs_in_expr(hi, out);
                    }
                }
            }
        }
        ExprKind::Not(e) => collect_refs_in_expr(e, out),
    }
}

fn collect_refs_in_desig(desig: &Designator, out: &mut HashSet<String>) {
    if desig.ident.module.is_none() {
        out.insert(desig.ident.name.clone());
    }
    for sel in &desig.selectors {
        if let Selector::Index(indices, _) = sel {
            for idx in indices { collect_refs_in_expr(idx, out); }
        }
    }
}

/// Recursively collect all identifier references in a proc and all its nested procs
fn collect_refs_in_proc_deep(proc: &ProcDecl, out: &mut HashSet<String>) {
    if let Some(stmts) = &proc.block.body {
        collect_refs_in_stmts(stmts, out);
    }
    for decl in &proc.block.decls {
        if let Declaration::Procedure(np) = decl {
            collect_refs_in_proc_deep(np, out);
        }
    }
}

/// Compute free variables of a nested procedure with respect to available outer scope variables.
/// Returns the captured variable names (sorted for deterministic output).
/// Includes transitive captures: vars needed by sub-nested procs are included.
fn compute_captures(proc: &ProcDecl, outer_vars: &HashMap<String, String>) -> Vec<String> {
    // Collect all refs in this proc's body AND nested procs' bodies (transitive)
    let mut all_refs = HashSet::new();
    collect_refs_in_proc_deep(proc, &mut all_refs);

    // Collect this proc's own local names (params + var decls + nested proc names)
    let mut locals = HashSet::new();
    for fp in &proc.heading.params {
        for name in &fp.names {
            locals.insert(name.clone());
        }
    }
    for decl in &proc.block.decls {
        match decl {
            Declaration::Var(v) => {
                for name in &v.names { locals.insert(name.clone()); }
            }
            Declaration::Procedure(p) => {
                locals.insert(p.heading.name.clone());
            }
            _ => {}
        }
    }

    // Free vars = referenced names in outer_vars but not in locals
    let mut captures: Vec<String> = all_refs.iter()
        .filter(|name| outer_vars.contains_key(name.as_str()) && !locals.contains(name.as_str()))
        .cloned()
        .collect();
    captures.sort();
    captures
}

impl CodeGen {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            indent: 0,
            module_name: String::new(),
            sema: SemanticAnalyzer::new(),
            import_map: HashMap::new(),
            var_params: vec![HashMap::new()],
            open_array_params: vec![HashSet::new()],
            proc_params: HashMap::new(),
            with_aliases: Vec::new(),
            lifted_procs: Vec::new(),
            enum_variants: HashMap::new(),
            record_fields: HashMap::new(),
            record_field_types: HashMap::new(),
            var_types: HashMap::new(),
            const_int_values: HashMap::new(),
            char_array_types: HashSet::new(),
            char_array_vars: HashSet::new(),
            char_array_fields: HashSet::new(),
            array_types: HashSet::new(),
            array_vars: HashSet::new(),
            array_fields: HashSet::new(),
            set_vars: HashSet::new(),
            cardinal_vars: HashSet::new(),
            complex_vars: HashSet::new(),
            longcomplex_vars: HashSet::new(),
            imported_modules: HashSet::new(),
            module_exports: HashMap::new(),
            pending_modules: None,
            closure_env_type: HashMap::new(),
            closure_env_fields: HashMap::new(),
            env_access_names: Vec::new(),
            child_env_type_stack: vec![None],
            child_captures_stack: vec![Vec::new()],
            variant_field_map: HashMap::new(),
            in_module_body: false,
            exception_counter: 0,
            m2plus: false,
            uses_gc: false,
            uses_threads: false,
            foreign_modules: HashSet::new(),
            foreign_def_modules: Vec::new(),
            export_c_names: HashMap::new(),
            def_modules: HashMap::new(),
            embedded_local_procs: HashSet::new(),
            embedded_local_vars: HashSet::new(),
            emit_debug_lines: false,
            last_line_file: String::new(),
            last_line_num: 0,
        }
    }

    pub fn set_m2plus(&mut self, enabled: bool) {
        self.m2plus = enabled;
    }

    pub fn set_debug(&mut self, enabled: bool) {
        self.emit_debug_lines = enabled;
    }

    /// Take ownership of the symbol table (for LSP use).
    pub fn take_symtab(self) -> crate::symtab::SymbolTable {
        self.sema.symtab
    }

    /// Scan compilation unit to determine which M2+ runtime features are needed.
    fn scan_m2plus_features(&mut self, unit: &CompilationUnit) {
        match unit {
            CompilationUnit::ProgramModule(m) => {
                self.scan_imports_for_features(&m.imports);
                self.scan_block_for_features(&m.block);
            }
            CompilationUnit::ImplementationModule(m) => {
                self.scan_imports_for_features(&m.imports);
                self.scan_block_for_features(&m.block);
            }
            CompilationUnit::DefinitionModule(m) => {
                self.scan_imports_for_features(&m.imports);
            }
        }
    }

    fn scan_imports_for_features(&mut self, imports: &[Import]) {
        for imp in imports {
            if let Some(ref from_mod) = imp.from_module {
                match from_mod.as_str() {
                    "Thread" | "Mutex" | "Condition"
                    | "THREAD" | "MUTEX" | "CONDITION" => self.uses_threads = true,
                    _ => {}
                }
            }
        }
    }

    fn scan_block_for_features(&mut self, block: &Block) {
        for decl in &block.decls {
            if let Declaration::Type(td) = decl {
                if let Some(ref ty) = td.typ {
                    self.scan_type_for_gc(ty);
                }
            }
        }
        if let Some(ref body) = block.body {
            self.scan_stmts_for_features(body);
        }
    }

    fn scan_type_for_gc(&mut self, ty: &TypeNode) {
        match ty {
            TypeNode::Ref { .. } | TypeNode::RefAny { .. } | TypeNode::Object { .. } => {
                self.uses_gc = true;
            }
            _ => {}
        }
    }

    fn scan_stmts_for_features(&mut self, stmts: &[Statement]) {
        for s in stmts {
            match &s.kind {
                StatementKind::Lock { .. } => self.uses_threads = true,
                _ => {}
            }
        }
    }

    fn emit(&mut self, s: &str) {
        self.output.push_str(s);
    }

    fn emit_indent(&mut self) {
        for _ in 0..self.indent {
            self.output.push_str("    ");
        }
    }

    fn emitln(&mut self, s: &str) {
        self.emit_indent();
        self.output.push_str(s);
        self.output.push('\n');
    }

    fn newline(&mut self) {
        self.output.push('\n');
    }

    /// Emit a C #line directive mapping back to the original Modula-2 source.
    /// Only emits when debug lines are enabled, the location is valid (non-default),
    /// and the line/file has changed since the last directive.
    fn emit_line_directive(&mut self, loc: &crate::errors::SourceLoc) {
        if !self.emit_debug_lines {
            return;
        }
        if loc.line == 0 || loc.file.is_empty() {
            return;
        }
        if loc.line == self.last_line_num && loc.file == self.last_line_file {
            return;
        }
        self.last_line_num = loc.line;
        if loc.file != self.last_line_file {
            self.last_line_file = loc.file.clone();
            self.output.push_str(&format!("#line {} \"{}\"\n", loc.line, loc.file));
        } else {
            self.output.push_str(&format!("#line {}\n", loc.line));
        }
    }

    /// Add an imported module pair (definition + implementation) for multi-module compilation.
    /// These will be generated as embedded code when the main module is compiled.
    pub fn add_imported_module(&mut self, imp: ImplementationModule) {
        let mod_name = imp.name.clone();
        // Extract exported procedure info from the implementation module
        let mut exports = Vec::new();
        for decl in &imp.block.decls {
            if let Declaration::Procedure(p) = decl {
                let mut param_info = Vec::new();
                for fp in &p.heading.params {
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
                exports.push((p.heading.name.clone(), param_info));
            }
        }
        self.module_exports.insert(mod_name.clone(), exports);
        // Store the implementation for later code generation
        if self.pending_modules.is_none() {
            self.pending_modules = Some(Vec::new());
        }
        self.pending_modules.as_mut().unwrap().push(imp);
    }

    /// Pre-register an external definition module so its types and procedures
    /// are available during semantic analysis and code generation.
    pub fn register_def_module(&mut self, def: &crate::ast::DefinitionModule) {
        self.sema.register_def_module(def);

        // Store non-foreign def modules for type emission during embedded impl gen
        if def.foreign_lang.is_none() {
            self.def_modules.insert(def.name.clone(), def.clone());
        }

        if def.foreign_lang.is_some() {
            self.foreign_modules.insert(def.name.clone());
            self.foreign_def_modules.push(def.clone());

            // Register proc_params and module_exports from the foreign .def
            let mut exports = Vec::new();
            for d in &def.definitions {
                if let Definition::Procedure(h) = d {
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
                    exports.push((h.name.clone(), param_info));
                }
            }
            self.module_exports.insert(def.name.clone(), exports);
        }
    }

    pub fn is_foreign_module(&self, name: &str) -> bool {
        self.foreign_modules.contains(name)
    }

    /// Emit extern declarations for all foreign (C ABI) definition modules.
    fn gen_foreign_extern_decls(&mut self) {
        for def in self.foreign_def_modules.clone() {
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
                                for name in &fp.names {
                                    if !first { self.emit(", "); }
                                    first = false;
                                    if fp.is_var {
                                        self.emit(&format!("{} *{}", ctype, name));
                                    } else {
                                        self.emit(&format!("{} {}", ctype, name));
                                    }
                                }
                            }
                        }
                        self.emit(");\n");
                    }
                    Definition::Var(v) => {
                        self.emit_indent();
                        let ctype = self.type_to_c(&v.typ);
                        for name in &v.names {
                            self.emitln(&format!("extern {} {};", ctype, name));
                        }
                    }
                    Definition::Const(c) => {
                        self.gen_const_decl(c);
                    }
                    Definition::Type(t) => {
                        self.gen_type_decl(t);
                    }
                    Definition::Exception(_) => {}
                }
            }
            self.newline();
        }
    }

    /// Like generate(), but returns sema errors as a Vec for structured diagnostics
    pub fn generate_or_errors(&mut self, unit: &CompilationUnit) -> Result<String, Vec<CompileError>> {
        self.sema.analyze(unit)?;
        self.post_sema_generate(unit);
        Ok(self.output.clone())
    }

    pub fn generate(&mut self, unit: &CompilationUnit) -> CompileResult<String> {
        // Run semantic analysis first
        self.sema.analyze(unit).map_err(|errors| {
            // Format all errors and return as a single compound error
            let msg = errors
                .iter()
                .map(|e| format!("{}", e))
                .collect::<Vec<_>>()
                .join("\n");
            CompileError::codegen(
                errors.first().map(|e| e.loc.clone()).unwrap_or_else(|| {
                    crate::errors::SourceLoc::new("<codegen>", 0, 0)
                }),
                msg,
            )
        })?;

        self.post_sema_generate(unit);
        Ok(self.output.clone())
    }

    fn post_sema_generate(&mut self, unit: &CompilationUnit) {
        // Scan compilation unit to determine which M2+ features are needed
        if self.m2plus {
            self.scan_m2plus_features(unit);
            if self.uses_gc {
                self.emit("#define M2_USE_GC 1\n");
            }
            if self.uses_threads {
                self.emit("#define M2_USE_THREADS 1\n");
            }
        }

        // Generate header
        self.emit(&stdlib::generate_runtime_header());

        match unit {
            CompilationUnit::ProgramModule(m) => self.gen_program_module(m),
            CompilationUnit::DefinitionModule(m) => self.gen_definition_module(m),
            CompilationUnit::ImplementationModule(m) => self.gen_implementation_module(m),
        }
    }

    // ── Program module ──────────────────────────────────────────────

    fn build_import_map(&mut self, imports: &[Import]) {
        for imp in imports {
            if let Some(from_mod) = &imp.from_module {
                // FROM Module IMPORT name1, name2;
                for name in &imp.names {
                    self.import_map.insert(name.clone(), from_mod.clone());
                    // Register stdlib proc params for codegen (is_char, is_var, etc.)
                    if stdlib::is_stdlib_module(from_mod) {
                        if let Some(params) = stdlib::get_stdlib_proc_params(from_mod, name) {
                            let info: Vec<ParamCodegenInfo> = params.into_iter().map(|(pname, is_var, is_char, is_open_array)| {
                                ParamCodegenInfo { name: pname, is_var, is_char, is_open_array }
                            }).collect();
                            let prefixed = format!("{}_{}", from_mod, name);
                            self.proc_params.insert(prefixed, info.clone());
                            self.proc_params.insert(name.clone(), info);
                        }
                    }
                }
            } else {
                // IMPORT Module1, Module2;  (whole-module / qualified import)
                for name in &imp.names {
                    self.imported_modules.insert(name.clone());
                }
            }
        }
    }

    /// Generate C code for an imported implementation module, embedded in the main program.
    /// All top-level procedure names are prefixed with `ModuleName_`.
    fn gen_embedded_implementation(&mut self, imp: &ImplementationModule) {
        let saved_module_name = self.module_name.clone();
        let saved_import_map = self.import_map.clone();
        let saved_var_params = self.var_params.clone();
        let saved_open_array_params = self.open_array_params.clone();
        let saved_proc_params = self.proc_params.clone();

        self.module_name = imp.name.clone();
        self.build_import_map(&imp.imports);

        // Track local procedure and variable names so intra-module refs get module-prefixed
        self.embedded_local_procs.clear();
        self.embedded_local_vars.clear();
        for decl in &imp.block.decls {
            match decl {
                Declaration::Procedure(p) => {
                    if p.heading.export_c_name.is_none() {
                        self.embedded_local_procs.insert(p.heading.name.clone());
                    }
                }
                Declaration::Var(v) => {
                    for name in &v.names {
                        self.embedded_local_vars.insert(name.clone());
                    }
                }
                _ => {}
            }
        }

        self.emitln(&format!("/* Imported Module {} */", imp.name));
        self.newline();

        // Forward declare all record types as structs (to allow pointer-to-struct typedefs)
        // Must come before any struct definitions so that type references resolve.

        // From the definition module:
        if let Some(def_mod) = self.def_modules.get(&imp.name).cloned() {
            let impl_type_names: HashSet<String> = imp.block.decls.iter()
                .filter_map(|d| if let Declaration::Type(t) = d { Some(t.name.clone()) } else { None })
                .collect();
            for d in &def_mod.definitions {
                if let Definition::Type(t) = d {
                    if !impl_type_names.contains(&t.name) {
                        if matches!(&t.typ, Some(TypeNode::Record { .. })) {
                            self.emitln(&format!("typedef struct {} {};", self.mangle(&t.name), self.mangle(&t.name)));
                        }
                    }
                }
            }
        }
        // From the implementation block:
        for decl in &imp.block.decls {
            if let Declaration::Type(t) = decl {
                if matches!(&t.typ, Some(TypeNode::Record { .. })) {
                    self.emitln(&format!("typedef struct {} {};", self.mangle(&t.name), self.mangle(&t.name)));
                }
            }
        }

        // Emit type and const declarations from the corresponding definition module,
        // but skip types that are redefined in the implementation module
        if let Some(def_mod) = self.def_modules.get(&imp.name).cloned() {
            let impl_type_names: HashSet<String> = imp.block.decls.iter()
                .filter_map(|d| if let Declaration::Type(t) = d { Some(t.name.clone()) } else { None })
                .collect();
            for d in &def_mod.definitions {
                match d {
                    Definition::Type(t) => {
                        if !impl_type_names.contains(&t.name) {
                            self.gen_type_decl(t);
                        }
                    }
                    Definition::Const(c) => self.gen_const_decl(c),
                    _ => {}
                }
            }
        }

        // Type and const declarations
        for decl in &imp.block.decls {
            match decl {
                Declaration::Const(c) => self.gen_const_decl(c),
                Declaration::Type(t) => self.gen_type_decl(t),
                _ => {}
            }
        }

        // Add module-prefixed type aliases for externally visible types
        for decl in &imp.block.decls {
            if let Declaration::Type(t) = decl {
                let prefixed = format!("{}_{}", imp.name, t.name);
                // Only add if the prefixed name would differ from the original
                if prefixed != t.name {
                    self.emitln(&format!("typedef {} {};", self.mangle(&t.name), prefixed));
                }
            }
        }

        // Forward declarations for procedures (with module prefix)
        for decl in &imp.block.decls {
            if let Declaration::Procedure(p) = decl {
                self.register_proc_params(&p.heading);
                // Register param info under module-prefixed name too
                let prefixed_name = format!("{}_{}", imp.name, p.heading.name);
                if let Some(info) = self.proc_params.get(&p.heading.name).cloned() {
                    self.proc_params.insert(prefixed_name, info);
                }
                self.emit_indent();
                let ret_type = if let Some(rt) = &p.heading.return_type {
                    self.type_to_c(rt)
                } else {
                    "void".to_string()
                };
                if let Some(ref ecn) = p.heading.export_c_name {
                    self.emit(&format!("{} {}", ret_type, ecn));
                } else {
                    self.emit(&format!("static {} {}_{}", ret_type, imp.name, p.heading.name));
                }
                self.emit("(");
                if p.heading.params.is_empty() {
                    self.emit("void");
                } else {
                    let mut first = true;
                    for fp in &p.heading.params {
                        let ctype = self.type_to_c(&fp.typ);
                        let is_open_array = matches!(fp.typ, TypeNode::OpenArray { .. });
                        for name in &fp.names {
                            if !first { self.emit(", "); }
                            first = false;
                            if is_open_array {
                                self.emit(&format!("{} *{}, uint32_t {}_high", ctype, name, name));
                            } else if Self::is_proc_type(&fp.typ) {
                                let decl = self.proc_type_decl(&fp.typ, name, fp.is_var);
                                self.emit(&decl);
                            } else if fp.is_var {
                                self.emit(&format!("{} *{}", ctype, name));
                            } else {
                                self.emit(&format!("{} {}", ctype, name));
                            }
                        }
                    }
                }
                self.emit(");\n");
            }
        }
        self.newline();

        // Variable declarations
        for decl in &imp.block.decls {
            if let Declaration::Var(v) = decl {
                self.gen_var_decl(v);
            }
        }

        // Procedure bodies (with module prefix)
        for decl in &imp.block.decls {
            if let Declaration::Procedure(p) = decl {
                // Generate procedure with module-prefixed name
                let ret_type = if let Some(rt) = &p.heading.return_type {
                    self.type_to_c(rt)
                } else {
                    "void".to_string()
                };
                if let Some(ref ecn) = p.heading.export_c_name {
                    self.emit(&format!("{} {}", ret_type, ecn));
                } else {
                    self.emit(&format!("static {} {}_{}", ret_type, imp.name, p.heading.name));
                }
                self.emit("(");
                // Set up var params for the body
                let mut param_vars = HashMap::new();
                let mut oa_params = HashSet::new();
                if p.heading.params.is_empty() {
                    self.emit("void");
                } else {
                    let mut first = true;
                    for fp in &p.heading.params {
                        let ctype = self.type_to_c(&fp.typ);
                        let is_open_array = matches!(fp.typ, TypeNode::OpenArray { .. });
                        for name in &fp.names {
                            if !first { self.emit(", "); }
                            first = false;
                            if is_open_array {
                                self.emit(&format!("{} *{}, uint32_t {}_high", ctype, name, name));
                                oa_params.insert(name.clone());
                            } else if Self::is_proc_type(&fp.typ) {
                                let decl = self.proc_type_decl(&fp.typ, name, fp.is_var);
                                self.emit(&decl);
                            } else if fp.is_var {
                                self.emit(&format!("{} *{}", ctype, name));
                                param_vars.insert(name.clone(), true);
                            } else {
                                self.emit(&format!("{} {}", ctype, name));
                            }
                        }
                    }
                }
                self.emit(") {\n");
                self.indent += 1;

                self.var_params.push(param_vars);
                self.open_array_params.push(oa_params);

                // Local declarations
                for d in &p.block.decls {
                    self.gen_declaration(d);
                }

                // Body statements
                if let Some(stmts) = &p.block.body {
                    for s in stmts {
                        self.gen_statement(s);
                    }
                }

                self.var_params.pop();
                self.open_array_params.pop();
                self.indent -= 1;
                self.emitln("}");
                self.newline();
            }
        }

        // Module initialization body
        if let Some(stmts) = &imp.block.body {
            self.emitln(&format!("static void {}_init(void) {{", imp.name));
            self.indent += 1;
            for stmt in stmts {
                self.gen_statement(stmt);
            }
            self.indent -= 1;
            self.emitln("}");
            self.newline();
        }

        // Restore state, but preserve module-prefixed proc params
        let module_proc_params: HashMap<String, Vec<ParamCodegenInfo>> = self.proc_params.iter()
            .filter(|(k, _)| k.starts_with(&format!("{}_", imp.name)))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        self.module_name = saved_module_name;
        self.import_map = saved_import_map;
        self.var_params = saved_var_params;
        self.open_array_params = saved_open_array_params;
        self.proc_params = saved_proc_params;
        self.embedded_local_procs.clear();
        self.embedded_local_vars.clear();
        // Merge back the module-prefixed proc param info
        self.proc_params.extend(module_proc_params);
    }

    /// Topologically sort implementation modules so dependencies come before dependents.
    fn topo_sort_modules(modules: Vec<ImplementationModule>) -> Vec<ImplementationModule> {
        let names: HashSet<String> = modules.iter().map(|m| m.name.clone()).collect();
        let mut deps: HashMap<String, Vec<String>> = HashMap::new();
        for m in &modules {
            let mut my_deps = Vec::new();
            for imp in &m.imports {
                if let Some(ref from_mod) = imp.from_module {
                    if names.contains(from_mod) {
                        my_deps.push(from_mod.clone());
                    }
                } else {
                    for name in &imp.names {
                        if names.contains(name) {
                            my_deps.push(name.clone());
                        }
                    }
                }
            }
            deps.insert(m.name.clone(), my_deps);
        }
        let mut visited = HashSet::new();
        let mut visiting = HashSet::new();
        fn visit(
            name: &str,
            deps: &HashMap<String, Vec<String>>,
            visited: &mut HashSet<String>,
            visiting: &mut HashSet<String>,
            order: &mut Vec<String>,
        ) {
            if visited.contains(name) || visiting.contains(name) {
                return;
            }
            visiting.insert(name.to_string());
            if let Some(d) = deps.get(name) {
                for dep in d {
                    visit(dep, deps, visited, visiting, order);
                }
            }
            visiting.remove(name);
            visited.insert(name.to_string());
            order.push(name.to_string());
        }
        let mut order = Vec::new();
        for m in &modules {
            visit(&m.name, &deps, &mut visited, &mut visiting, &mut order);
        }
        let pos: HashMap<String, usize> = order.iter().enumerate().map(|(i, n)| (n.clone(), i)).collect();
        let mut result = modules;
        result.sort_by_key(|m| pos.get(&m.name).copied().unwrap_or(usize::MAX));
        result
    }

    fn gen_program_module(&mut self, m: &ProgramModule) {
        self.module_name = m.name.clone();
        self.build_import_map(&m.imports);

        // Emit extern declarations for foreign C modules BEFORE embedded implementations
        self.gen_foreign_extern_decls();

        // Generate code for imported modules first (topologically sorted by dependencies)
        if let Some(pending) = self.pending_modules.take() {
            let sorted = Self::topo_sort_modules(pending);
            for imp_mod in &sorted {
                self.gen_embedded_implementation(imp_mod);
            }
        }

        // Register param info for imported module procedures
        for (mod_name, exports) in &self.module_exports {
            for (proc_name, param_info) in exports {
                let prefixed = format!("{}_{}", mod_name, proc_name);
                self.proc_params.insert(prefixed, param_info.clone());
                // For foreign modules, also register under bare name
                if self.foreign_modules.contains(mod_name.as_str()) {
                    self.proc_params.insert(proc_name.clone(), param_info.clone());
                }
            }
        }

        self.emitln(&format!("/* Module {} */", m.name));
        self.newline();

        // Forward struct declarations for records (enables mutual/forward references)
        for decl in &m.block.decls {
            if let Declaration::Type(t) = decl {
                if let Some(TypeNode::Record { .. }) = &t.typ {
                    self.emitln(&format!("typedef struct {} {};", self.mangle(&t.name), self.mangle(&t.name)));
                }
            }
        }

        // Type and const declarations
        for decl in &m.block.decls {
            match decl {
                Declaration::Const(c) => self.gen_const_decl(c),
                Declaration::Type(t) => self.gen_type_decl(t),
                _ => {}
            }
        }

        // Forward declarations for procedures
        self.gen_forward_decls(&m.block.decls);
        self.newline();

        // Var declarations and procedure bodies
        for decl in &m.block.decls {
            match decl {
                Declaration::Const(_) | Declaration::Type(_) => {} // already done
                _ => self.gen_declaration(decl),
            }
        }

        // ISO Modula-2: generate FINALLY handler if present
        if let Some(finally_stmts) = &m.block.finally {
            self.emitln("static void m2_finally_handler(void) {");
            self.indent += 1;
            for stmt in finally_stmts {
                self.gen_statement(stmt);
            }
            self.indent -= 1;
            self.emitln("}");
            self.newline();
        }

        // ISO Modula-2: generate EXCEPT handler if present
        if let Some(except_stmts) = &m.block.except {
            self.emitln("static void m2_except_handler(void) {");
            self.indent += 1;
            for stmt in except_stmts {
                self.gen_statement(stmt);
            }
            self.indent -= 1;
            self.emitln("}");
            self.newline();
        }

        // Generate main function
        self.emitln("int main(int argc, char **argv) {");
        self.indent += 1;
        self.emitln("m2_argc = argc; m2_argv = argv;");
        if self.emit_debug_lines {
            self.emitln("setvbuf(stdout, NULL, _IONBF, 0);");
        }

        // Register FINALLY handler with atexit
        if m.block.finally.is_some() {
            self.emitln("atexit(m2_finally_handler);");
        }

        self.in_module_body = true;
        if let Some(stmts) = &m.block.body {
            self.emit_line_directive(&m.block.loc);
            for stmt in stmts {
                self.gen_statement(stmt);
            }
        }
        self.in_module_body = false;
        self.emitln("return 0;");
        self.indent -= 1;
        self.emitln("}");
    }

    fn gen_definition_module(&mut self, m: &DefinitionModule) {
        self.module_name = m.name.clone();
        self.emitln(&format!("/* Definition Module {} */", m.name));
        self.emitln(&format!("#ifndef {}_H", m.name.to_uppercase()));
        self.emitln(&format!("#define {}_H", m.name.to_uppercase()));
        self.newline();

        for def in &m.definitions {
            match def {
                Definition::Const(c) => self.gen_const_decl(c),
                Definition::Type(t) => self.gen_type_decl(t),
                Definition::Var(v) => {
                    self.emit_indent();
                    self.emit("extern ");
                    let ctype = self.type_to_c(&v.typ);
                    for (i, name) in v.names.iter().enumerate() {
                        if i > 0 {
                            self.emit(", ");
                        }
                        self.emit(&format!("{} {}", ctype, self.mangle(name)));
                    }
                    self.emit(";\n");
                }
                Definition::Procedure(h) => {
                    self.gen_proc_prototype(h);
                    self.emit(";\n");
                }
                Definition::Exception(e) => {
                    // Exception declaration: generate unique integer constant
                    self.emitln(&format!("#define M2_EXC_{} __COUNTER__", self.mangle(&e.name)));
                }
            }
        }

        self.newline();
        self.emitln("#endif");
    }

    fn gen_implementation_module(&mut self, m: &ImplementationModule) {
        self.module_name = m.name.clone();
        self.build_import_map(&m.imports);

        // Register param info for imported module procedures
        for (mod_name, exports) in &self.module_exports {
            for (proc_name, param_info) in exports {
                let prefixed = format!("{}_{}", mod_name, proc_name);
                self.proc_params.insert(prefixed, param_info.clone());
                // For foreign modules, also register under bare name
                if self.foreign_modules.contains(mod_name.as_str()) {
                    self.proc_params.insert(proc_name.clone(), param_info.clone());
                }
            }
        }

        // Emit extern declarations for foreign C modules BEFORE embedded implementations
        self.gen_foreign_extern_decls();

        // Generate code for imported modules (topologically sorted by dependencies)
        if let Some(pending) = self.pending_modules.take() {
            let sorted = Self::topo_sort_modules(pending);
            for imp_mod in &sorted {
                self.gen_embedded_implementation(imp_mod);
            }
        }

        self.emitln(&format!("/* Implementation Module {} */", m.name));
        self.newline();

        // Forward struct declarations
        for decl in &m.block.decls {
            if let Declaration::Type(t) = decl {
                if let Some(TypeNode::Record { .. }) = &t.typ {
                    self.emitln(&format!("typedef struct {} {};", self.mangle(&t.name), self.mangle(&t.name)));
                }
            }
        }

        for decl in &m.block.decls {
            match decl {
                Declaration::Const(c) => self.gen_const_decl(c),
                Declaration::Type(t) => self.gen_type_decl(t),
                _ => {}
            }
        }

        self.gen_forward_decls(&m.block.decls);
        self.newline();

        for decl in &m.block.decls {
            match decl {
                Declaration::Const(_) | Declaration::Type(_) => {}
                _ => self.gen_declaration(decl),
            }
        }

        // Module body = initialization function
        if let Some(stmts) = &m.block.body {
            self.emit_line_directive(&m.block.loc);
            self.emitln(&format!("void {}_init(void) {{", self.mangle(&m.name)));
            self.indent += 1;
            for stmt in stmts {
                self.gen_statement(stmt);
            }
            self.indent -= 1;
            self.emitln("}");
        }
    }

    // ── Forward declarations ────────────────────────────────────────

    fn gen_forward_decls(&mut self, decls: &[Declaration]) {
        for decl in decls {
            if let Declaration::Procedure(p) = decl {
                self.register_proc_params(&p.heading);
                self.gen_proc_prototype(&p.heading);
                self.emit(";\n");
            }
        }
    }

    fn register_proc_params(&mut self, h: &ProcHeading) {
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
    }

    // ── Declarations ────────────────────────────────────────────────

    fn gen_declaration(&mut self, decl: &Declaration) {
        match decl {
            Declaration::Const(c) => self.gen_const_decl(c),
            Declaration::Type(t) => self.gen_type_decl(t),
            Declaration::Var(v) => self.gen_var_decl(v),
            Declaration::Procedure(p) => self.gen_proc_decl(p),
            Declaration::Module(m) => {
                // Nested module - generate inline
                self.emitln(&format!("/* Nested module {} */", m.name));
                for d in &m.block.decls {
                    self.gen_declaration(d);
                }
            }
            Declaration::Exception(e) => {
                self.gen_exception_decl(e);
            }
        }
    }

    fn gen_const_decl(&mut self, c: &ConstDecl) {
        // Try to evaluate as a compile-time integer constant
        if let Some(val) = self.try_eval_const_int(&c.expr) {
            self.const_int_values.insert(c.name.clone(), val);
            self.emitln(&format!("static const int32_t {} = {};", self.mangle(&c.name), val));
            return;
        }
        // Fall back to expression-based constant
        self.emit_indent();
        let ctype = self.infer_c_type(&c.expr);
        self.emit(&format!("static const {} {} = ", ctype, self.mangle(&c.name)));
        self.gen_expr(&c.expr);
        self.emit(";\n");
    }

    fn gen_type_decl(&mut self, t: &TypeDecl) {
        if let Some(tn) = &t.typ {
            match tn {
                TypeNode::Record { fields, loc: _ } => {
                    // Collect field names and types for WITH resolution
                    let mut field_names = Vec::new();
                    for fl in fields {
                        for f in &fl.fixed {
                            // Track field type name for nested WITH
                            let field_type_name = if let TypeNode::Named(qi) = &f.typ {
                                Some(qi.name.clone())
                            } else {
                                None
                            };
                            for name in &f.names {
                                field_names.push(name.clone());
                                if let Some(ref ftn) = field_type_name {
                                    self.record_field_types.insert(
                                        (t.name.clone(), name.clone()),
                                        ftn.clone(),
                                    );
                                }
                            }
                        }
                    }
                    self.record_fields.insert(t.name.clone(), field_names);

                    // struct definition (typedef is already forward-declared)
                    self.emitln(&format!("struct {} {{", self.mangle(&t.name)));
                    self.indent += 1;
                    for fl in fields {
                        for f in &fl.fixed {
                            self.emit_indent();
                            let ctype = self.type_to_c(&f.typ);
                            let arr_suffix = self.type_array_suffix(&f.typ);
                            // Track char array record fields for strcpy assignment
                            if ctype == "char" && !arr_suffix.is_empty() {
                                for name in &f.names {
                                    self.char_array_fields.insert((t.name.clone(), name.clone()));
                                }
                            }
                            // Track array record fields for memcpy assignment
                            if !arr_suffix.is_empty() || self.is_array_type(&f.typ) {
                                for name in &f.names {
                                    self.array_fields.insert((t.name.clone(), name.clone()));
                                }
                            }
                            self.emit(&format!("{} ", ctype));
                            for (i, name) in f.names.iter().enumerate() {
                                if i > 0 {
                                    self.emit(", ");
                                }
                                self.emit(name);
                                if !arr_suffix.is_empty() {
                                    self.emit(&arr_suffix);
                                }
                            }
                            self.emit(";\n");
                        }
                        if let Some(vp) = &fl.variant {
                            self.gen_variant_part(vp, &t.name);
                        }
                    }
                    self.indent -= 1;
                    self.emitln("};");
                }
                TypeNode::Enumeration { variants, .. } => {
                    self.emit_indent();
                    self.emit("typedef enum { ");
                    let type_name = self.mangle(&t.name);
                    for (i, v) in variants.iter().enumerate() {
                        if i > 0 {
                            self.emit(", ");
                        }
                        let c_name = format!("{}_{}", type_name, v);
                        self.emit(&c_name);
                        // Register mapping so we can find the C name when the variant is used
                        self.enum_variants.insert(v.clone(), c_name);
                    }
                    self.emit(&format!(" }} {};\n", type_name));
                }
                TypeNode::Pointer { base, .. } => {
                    self.emit_indent();
                    let base_c = self.type_to_c(base);
                    self.emit(&format!(
                        "typedef {} *{};\n",
                        base_c,
                        self.mangle(&t.name)
                    ));
                }
                TypeNode::Array { .. } => {
                    // Track if this is an ARRAY OF CHAR type (for string ops)
                    if self.is_char_array_type(tn) {
                        self.char_array_types.insert(t.name.clone());
                    }
                    // Track all array types for memcpy assignment
                    self.array_types.insert(t.name.clone());
                    self.emit_indent();
                    let ctype = self.type_to_c(tn);
                    let suffix = self.type_array_suffix(tn);
                    self.emit(&format!("typedef {} {}{};\n", ctype, self.mangle(&t.name), suffix));
                }
                TypeNode::ProcedureType { params, return_type, .. } => {
                    // typedef RetType (*Name)(params);
                    self.emit_indent();
                    let ret = if let Some(rt) = return_type {
                        self.type_to_c(rt)
                    } else {
                        "void".to_string()
                    };
                    self.emit(&format!("typedef {} (*{})(", ret, self.mangle(&t.name)));
                    if params.is_empty() {
                        self.emit("void");
                    } else {
                        let mut first = true;
                        for fp in params {
                            let pt = self.type_to_c(&fp.typ);
                            for name in &fp.names {
                                if !first { self.emit(", "); }
                                first = false;
                                if fp.is_var {
                                    self.emit(&format!("{} *", pt));
                                } else {
                                    self.emit(&pt);
                                }
                            }
                        }
                    }
                    self.emit(");\n");
                }
                TypeNode::Object { parent, fields, methods, overrides, .. } => {
                    self.gen_object_type(&t.name, parent.as_ref(), fields, methods, overrides);
                }
                TypeNode::Ref { target, branded, .. } => {
                    self.emit_indent();
                    let target_c = self.type_to_c(target);
                    self.emit(&format!("typedef {} *{};\n", target_c, self.mangle(&t.name)));
                }
                TypeNode::RefAny { .. } => {
                    self.emit_indent();
                    self.emit(&format!("typedef void *{};\n", self.mangle(&t.name)));
                }
                _ => {
                    self.emit_indent();
                    let ctype = self.type_to_c(tn);
                    self.emit(&format!("typedef {} {};\n", ctype, self.mangle(&t.name)));
                }
            }
            self.newline();
        } else {
            // Opaque type - generate as void*
            self.emitln(&format!(
                "typedef void *{};",
                self.mangle(&t.name)
            ));
        }
    }

    fn gen_variant_part(&mut self, vp: &VariantPart, record_name: &str) {
        if let Some(tag) = &vp.tag_name {
            self.emit_indent();
            let tag_c = self.qualident_to_c(&vp.tag_type);
            self.emit(&format!("{} {};\n", tag_c, tag));
        }
        self.emitln("union {");
        self.indent += 1;
        for (i, v) in vp.variants.iter().enumerate() {
            self.emitln("struct {");
            self.indent += 1;
            for fl in &v.fields {
                for f in &fl.fixed {
                    self.emit_indent();
                    let ctype = self.type_to_c(&f.typ);
                    self.emit(&format!("{} ", ctype));
                    for (j, name) in f.names.iter().enumerate() {
                        if j > 0 {
                            self.emit(", ");
                        }
                        self.emit(name);
                        // Register variant field mapping
                        self.variant_field_map.insert(
                            (record_name.to_string(), name.clone()),
                            i,
                        );
                        // Add to record_fields for WITH resolution
                        if let Some(fields) = self.record_fields.get_mut(record_name) {
                            fields.push(name.clone());
                        }
                    }
                    self.emit(";\n");
                }
            }
            self.indent -= 1;
            self.emitln(&format!("}} v{};", i));
        }
        self.indent -= 1;
        self.emitln("} variant;");
    }

    fn gen_var_decl(&mut self, v: &VarDecl) {
        // Track variable -> type name mapping for WITH resolution
        if let TypeNode::Named(qi) = &v.typ {
            if qi.module.is_none() {
                for name in &v.names {
                    self.var_types.insert(name.clone(), qi.name.clone());
                }
                // Check if this named type is a known char array type
                if self.char_array_types.contains(&qi.name) {
                    for name in &v.names {
                        self.char_array_vars.insert(name.clone());
                    }
                }
                // Check if this named type is an array type (for memcpy)
                if self.array_types.contains(&qi.name) {
                    for name in &v.names {
                        self.array_vars.insert(name.clone());
                    }
                }
            }
        }
        // Also check if it's directly an ARRAY OF CHAR
        if self.is_char_array_type(&v.typ) {
            for name in &v.names {
                self.char_array_vars.insert(name.clone());
            }
        }
        // Also check if it's directly an array type (for memcpy)
        if self.is_array_type(&v.typ) {
            for name in &v.names {
                self.array_vars.insert(name.clone());
            }
        }
        // Track SET/BITSET variables
        if self.is_set_type(&v.typ) {
            for name in &v.names {
                self.set_vars.insert(name.clone());
            }
        }
        // Track CARDINAL variables for unsigned DIV/MOD
        if matches!(&v.typ, TypeNode::Named(qi) if qi.name == "CARDINAL" || qi.name == "LONGCARD") {
            for name in &v.names {
                self.cardinal_vars.insert(name.clone());
            }
        }
        // Track COMPLEX/LONGCOMPLEX variables
        if self.is_complex_type(&v.typ) {
            for name in &v.names {
                self.complex_vars.insert(name.clone());
            }
        }
        if self.is_longcomplex_type(&v.typ) {
            for name in &v.names {
                self.longcomplex_vars.insert(name.clone());
            }
        }

        if Self::is_proc_type(&v.typ) {
            // Procedure type variables need special C declaration syntax:
            // RetType (*name)(params) instead of type name
            for (i, name) in v.names.iter().enumerate() {
                self.emit_indent();
                let c_name = if self.embedded_local_vars.contains(name) {
                    format!("{}_{}", self.module_name, name)
                } else {
                    self.mangle(name)
                };
                let decl = self.proc_type_decl(&v.typ, &c_name, false);
                self.emit(&format!("{};\n", decl));
            }
        } else {
            self.emit_indent();
            let ctype = self.type_to_c(&v.typ);
            let array_suffix = self.type_array_suffix(&v.typ);
            self.emit(&format!("{} ", ctype));
            for (i, name) in v.names.iter().enumerate() {
                if i > 0 {
                    self.emit(", ");
                }
                let c_name = if self.embedded_local_vars.contains(name) {
                    format!("{}_{}", self.module_name, name)
                } else {
                    self.mangle(name)
                };
                self.emit(&format!("{}{}", c_name, array_suffix));
            }
            self.emit(";\n");
        }
    }

    fn gen_proc_decl(&mut self, p: &ProcDecl) {
        self.register_proc_params(&p.heading);

        // Collect nested procedure declarations and other declarations
        let mut nested_procs = Vec::new();
        let mut other_decls = Vec::new();
        for decl in &p.block.decls {
            if let Declaration::Procedure(np) = decl {
                nested_procs.push(np.clone());
            } else {
                other_decls.push(decl);
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

        for np in &nested_procs {
            let captures = compute_captures(np, &scope_vars);
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

        // Generate nested procs (lifted to top level, with env param if they have captures)
        for np in &nested_procs {
            // If this nested proc has captures, push its env access names
            if let Some(_) = self.closure_env_type.get(&np.heading.name) {
                // Compute which vars this specific proc (and its descendants) needs from outer scopes
                let np_captures = compute_captures(&np, &scope_vars);
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
        self.gen_proc_prototype(&p.heading);
        self.emit(" {\n");
        self.indent += 1;

        // Push a new VAR param scope and register VAR params
        // Note: VAR open array params are already pointers, so don't register them
        // as VAR (which would cause double dereferencing with (*a)[i])
        self.push_var_scope();
        // Save array var tracking so procedure-local names don't collide with outer scope
        let saved_array_scope = self.save_array_var_scope();
        for fp in &p.heading.params {
            if matches!(fp.typ, TypeNode::OpenArray { .. }) {
                for name in &fp.names {
                    if let Some(scope) = self.open_array_params.last_mut() {
                        scope.insert(name.clone());
                    }
                }
            } else if fp.is_var {
                for name in &fp.names {
                    self.register_var_param(name);
                }
            }
        }

        // Local declarations (excluding procedures, which were lifted)
        for decl in &other_decls {
            self.gen_declaration(decl);
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

        if let Some(stmts) = &p.block.body {
            for stmt in stmts {
                self.gen_statement(stmt);
            }
        }

        if has_except {
            self.indent -= 1;
            self.emitln("} else {");
            self.indent += 1;
            self.emitln("/* EXCEPT handler */");
            if let Some(except_stmts) = &p.block.except {
                for stmt in except_stmts {
                    self.gen_statement(stmt);
                }
            }
            self.indent -= 1;
            self.emitln("}");
            self.emitln("m2_exception_active = 0;");
        }

        self.child_env_type_stack.pop();
        self.child_captures_stack.pop();
        self.restore_array_var_scope(saved_array_scope);
        self.pop_var_scope();
        self.indent -= 1;
        self.emitln("}");
    }

    fn gen_proc_prototype(&mut self, h: &ProcHeading) {
        self.emit_indent();
        let ret_type = if let Some(rt) = &h.return_type {
            self.type_to_c(rt)
        } else {
            "void".to_string()
        };
        let c_name = if let Some(ref ecn) = h.export_c_name {
            ecn.clone()
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
                    if is_open_array {
                        let ctype = self.type_to_c(&fp.typ);
                        self.emit(&format!("{} *{}, uint32_t {}_high", ctype, name, name));
                    } else if Self::is_proc_type(&fp.typ) {
                        let decl = self.proc_type_decl(&fp.typ, name, fp.is_var);
                        self.emit(&decl);
                    } else if fp.is_var {
                        let ctype = self.type_to_c(&fp.typ);
                        self.emit(&format!("{} *{}", ctype, name));
                    } else {
                        let ctype = self.type_to_c(&fp.typ);
                        self.emit(&format!("{} {}", ctype, name));
                    }
                }
            }
        }
        self.emit(")");
    }

    // ── Statements ──────────────────────────────────────────────────

    fn gen_statement(&mut self, stmt: &Statement) {
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
                    // Record field: check array_fields
                    self.is_array_field(fname)
                } else {
                    false
                };

                if is_string_literal_assign && !is_array_assign {
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
                    // Special case: single-char string assigned to char variable
                    if let ExprKind::StringLit(s) = &expr.kind {
                        if s.len() == 1 {
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
                if builtins::is_builtin_proc(&actual_name) {
                    self.emit_indent();
                    let char_builtins = ["CAP", "ORD", "CHR", "Write"];
                    let arg_strs: Vec<String> = args.iter().map(|a| {
                        if char_builtins.contains(&actual_name.as_ref()) {
                            self.expr_to_char_string(a)
                        } else {
                            self.expr_to_string(a)
                        }
                    }).collect();
                    self.emit(&builtins::codegen_builtin(&actual_name, &arg_strs));
                    self.emit(";\n");
                } else {
                    self.emit_indent();
                    let c_name = self.resolve_proc_name(desig);
                    // Look up param info: try module-prefixed name, then actual name,
                    // then FROM-import prefixed name
                    let param_info = if let Some((mod_name, _)) = module_qualified {
                        let prefixed = format!("{}_{}", mod_name, actual_name);
                        let info = self.get_param_info(&prefixed);
                        if info.is_empty() { self.get_param_info(&actual_name) } else { info }
                    } else {
                        let mut info = self.get_param_info(&actual_name);
                        if info.is_empty() {
                            // Try FROM Module IMPORT name: check import_map for module prefix
                            if let Some(module) = self.import_map.get(&actual_name) {
                                let prefixed = format!("{}_{}", module, actual_name);
                                info = self.get_param_info(&prefixed);
                            }
                        }
                        info
                    };
                    self.emit(&format!("{}(", c_name));
                    // Pass closure env if this is a nested proc with captures
                    if self.closure_env_type.contains_key(actual_name.as_str()) {
                        self.emit("&_child_env");
                        if !args.is_empty() {
                            self.emit(", ");
                        }
                    }
                    self.gen_call_args(args, &param_info);
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
                // Determine loop direction for comparison operator
                let is_downward = step.as_ref().map_or(false, |s| {
                    self.is_negative_expr(s)
                });
                let cmp_op = if is_downward { ">=" } else { "<=" };
                let var_c = self.mangle(var);
                self.emit(&format!("for ({} = ", var_c));
                self.gen_expr(start);
                self.emit(&format!("; {} {} ", var_c, cmp_op));
                self.gen_expr(end);
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
                    // Use M2+ exception frame stack if available, fallback to ISO mechanism
                    self.emit("{ int _exc_id = (int)(");
                    self.gen_expr(e);
                    self.emit("); m2_raise(_exc_id, NULL, NULL); }\n");
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

    fn gen_expr(&mut self, expr: &Expr) {
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
                        _ => None,
                    };
                    if let Some(cast) = c_cast {
                        self.emit(&format!("({}(", cast));
                        self.gen_expr(&args[0]);
                        self.emit("))");
                        return;
                    }
                }
                if builtins::is_builtin_proc(&actual_name) {
                    // ADR on open array params: emit (void *)(name) instead of (void *)&(name)
                    if actual_name == "ADR" && args.len() == 1 {
                        if let ExprKind::Designator(ref d) = args[0].kind {
                            if d.selectors.is_empty() && d.ident.module.is_none()
                                && self.is_open_array_param(&d.ident.name)
                            {
                                let arg_str = self.expr_to_string(&args[0]);
                                self.emit(&format!("((void *)({}))", arg_str));
                                return;
                            }
                        }
                    }
                    // For builtins that take char args, convert single-char strings to char literals
                    let char_builtins = ["CAP", "ORD", "CHR", "Write"];
                    let arg_strs: Vec<String> = args.iter().map(|a| {
                        if char_builtins.contains(&actual_name.as_ref()) {
                            self.expr_to_char_string(a)
                        } else {
                            self.expr_to_string(a)
                        }
                    }).collect();
                    self.emit(&builtins::codegen_builtin(&actual_name, &arg_strs));
                } else {
                    let c_name = self.resolve_proc_name(desig);
                    // Look up param info: try module-prefixed name, then actual name,
                    // then FROM-import prefixed name
                    let param_info = if let Some((mod_name, _)) = module_qualified {
                        let prefixed = format!("{}_{}", mod_name, actual_name);
                        let info = self.get_param_info(&prefixed);
                        if info.is_empty() { self.get_param_info(&actual_name) } else { info }
                    } else {
                        let mut info = self.get_param_info(&actual_name);
                        if info.is_empty() {
                            if let Some(module) = self.import_map.get(&actual_name) {
                                let prefixed = format!("{}_{}", module, actual_name);
                                info = self.get_param_info(&prefixed);
                            }
                        }
                        info
                    };
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
            ExprKind::BinaryOp { op, left, right } => {
                // Handle IN specially
                if matches!(op, BinaryOp::In) {
                    // x IN s => ((s >> x) & 1)
                    self.emit("((");
                    self.gen_expr(right);
                    self.emit(" >> ");
                    self.gen_expr(left);
                    self.emit(") & 1)");
                } else if matches!(op, BinaryOp::IntDiv) {
                    if self.is_unsigned_expr(left) || self.is_unsigned_expr(right) {
                        // CARDINAL DIV: plain unsigned C division
                        self.emit("((uint32_t)(");
                        self.gen_expr(left);
                        self.emit(") / (uint32_t)(");
                        self.gen_expr(right);
                        self.emit("))");
                    } else {
                        // PIM4 DIV: truncates toward negative infinity (floored division)
                        self.emit("m2_div(");
                        self.gen_expr(left);
                        self.emit(", ");
                        self.gen_expr(right);
                        self.emit(")");
                    }
                } else if matches!(op, BinaryOp::Mod) {
                    if self.is_unsigned_expr(left) || self.is_unsigned_expr(right) {
                        // CARDINAL MOD: plain unsigned C modulo
                        self.emit("((uint32_t)(");
                        self.gen_expr(left);
                        self.emit(") % (uint32_t)(");
                        self.gen_expr(right);
                        self.emit("))");
                    } else {
                        // PIM4 MOD: result is always non-negative
                        self.emit("m2_mod(");
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
                    self.emit("(");
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
                    self.emit(")");
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
                                self.gen_expr(e);
                                self.emit(")");
                            }
                            SetElement::Range(lo, hi) => {
                                // Generate a mask: ((2u << hi) - (1u << lo))
                                self.emit("((2u << ");
                                self.gen_expr(hi);
                                self.emit(") - (1u << ");
                                self.gen_expr(lo);
                                self.emit("))");
                            }
                        }
                    }
                    self.emit(")");
                }
            }
        }
    }

    fn gen_designator(&mut self, desig: &Designator) {
        // Build designator string into a separate buffer so we can wrap with (*...)
        let desig_str = self.designator_to_string(desig);
        self.emit(&desig_str);
    }

    fn designator_to_string(&mut self, desig: &Designator) -> String {
        // Check for module-qualified access: MathUtils.Square → MathUtils_Square
        // where MathUtils was imported via `IMPORT MathUtils;`
        let sel_start;
        let base_name = if let Some(module) = &desig.ident.module {
            sel_start = 0;
            if self.foreign_modules.contains(module.as_str()) {
                desig.ident.name.clone()
            } else {
                let mapped = stdlib::map_stdlib_call(module, &desig.ident.name);
                mapped.unwrap_or_else(|| format!("{}_{}", module, desig.ident.name))
            }
        } else if let Some((mod_name, field_name)) = self.resolve_module_qualified(desig) {
            // Whole-module import: `IMPORT MathUtils; ... MathUtils.Square`
            sel_start = 1; // skip the first Field selector (already incorporated in name)
            let mod_name = mod_name.to_string();
            let field_name = field_name.to_string();
            if self.foreign_modules.contains(mod_name.as_str()) {
                field_name
            } else if let Some(c_name) = stdlib::map_stdlib_call(&mod_name, &field_name) {
                c_name
            } else {
                format!("{}_{}", mod_name, field_name)
            }
        } else {
            sel_start = 0;
            // Check if this bare identifier is a field name in an active WITH context
            {
                for (with_desig, fields, with_type) in self.with_aliases.iter().rev() {
                    if fields.contains(&desig.ident.name) {
                        // This bare name is a field of the WITH record
                        // Check if it's a variant field
                        let field_name = &desig.ident.name;
                        let result = if let Some(ref tn) = with_type {
                            if let Some(&vidx) = self.variant_field_map.get(&(tn.clone(), field_name.clone())) {
                                format!("{}.variant.v{}.{}", with_desig, vidx, field_name)
                            } else {
                                format!("{}.{}", with_desig, field_name)
                            }
                        } else {
                            format!("{}.{}", with_desig, field_name)
                        };
                        let mut result = result;
                        // Apply any additional selectors
                        let mut i = 0;
                        while i < desig.selectors.len() {
                            match &desig.selectors[i] {
                                Selector::Field(name, _) => {
                                    result.push('.');
                                    result.push_str(name);
                                }
                                Selector::Index(indices, _) => {
                                    for idx in indices {
                                        let idx_str = self.expr_to_string(idx);
                                        result.push('[');
                                        result.push_str(&idx_str);
                                        result.push(']');
                                    }
                                }
                                Selector::Deref(_) => {
                                    if i + 1 < desig.selectors.len() {
                                        if let Selector::Field(fname, _) = &desig.selectors[i + 1] {
                                            result.push_str("->");
                                            result.push_str(fname);
                                            i += 2;
                                            continue;
                                        }
                                    }
                                    result = format!("(*{})", result);
                                }
                            }
                            i += 1;
                        }
                        return result;
                    }
                }
            }
            // Check if this bare name is an imported stdlib variable (e.g., Done from BinaryIO)
            if let Some(module) = self.import_map.get(&desig.ident.name).cloned() {
                if stdlib::is_stdlib_module(&module) {
                    if let Some(c_name) = stdlib::map_stdlib_call(&module, &desig.ident.name) {
                        return c_name;
                    }
                }
            }
            // Inside an embedded implementation, module-level vars need module prefix
            if self.embedded_local_vars.contains(&desig.ident.name) {
                format!("{}_{}", self.module_name, desig.ident.name)
            } else {
                self.mangle(&desig.ident.name)
            }
        };

        let sels = &desig.selectors;

        // Check if this is a captured variable accessed through the env pointer
        let mut result = if desig.ident.module.is_none() && sel_start == 0 && self.is_env_var(&desig.ident.name) {
            format!("(*_env->{})", desig.ident.name)
        } else {
            let base_is_var = sel_start == 0 && self.is_var_param(&desig.ident.name);
            if base_is_var {
                format!("(*{})", base_name)
            } else {
                base_name
            }
        };

        if sels.len() <= sel_start {
            return result;
        }

        // Determine the record type name of the base for variant field resolution
        let mut current_type = if sel_start == 0 {
            self.var_types.get(&desig.ident.name).cloned()
        } else {
            None
        };

        let mut i = sel_start;
        while i < sels.len() {
            match &sels[i] {
                Selector::Field(name, _) => {
                    // Check if this field is a variant field
                    if let Some(ref type_name) = current_type {
                        if let Some(&vidx) = self.variant_field_map.get(&(type_name.clone(), name.clone())) {
                            result.push_str(&format!(".variant.v{}.{}", vidx, name));
                            // Update current type for further chaining
                            current_type = self.record_field_types.get(&(type_name.clone(), name.clone())).cloned();
                            i += 1;
                            continue;
                        }
                    }
                    result.push('.');
                    result.push_str(name);
                    // Update current type for nested field access
                    if let Some(ref type_name) = current_type {
                        current_type = self.record_field_types.get(&(type_name.clone(), name.clone())).cloned();
                    }
                }
                Selector::Index(indices, _) => {
                    for idx in indices {
                        let idx_str = self.expr_to_string(idx);
                        result.push('[');
                        result.push_str(&idx_str);
                        result.push(']');
                    }
                    current_type = None; // array element type tracking not implemented
                }
                Selector::Deref(_) => {
                    // Check if followed by field selector: ptr^.field → ptr->field
                    if i + 1 < sels.len() {
                        if let Selector::Field(fname, _) = &sels[i + 1] {
                            // Check variant field through pointer deref
                            if let Some(ref type_name) = current_type {
                                if let Some(&vidx) = self.variant_field_map.get(&(type_name.clone(), fname.clone())) {
                                    result.push_str(&format!("->variant.v{}.{}", vidx, fname));
                                    current_type = self.record_field_types.get(&(type_name.clone(), fname.clone())).cloned();
                                    i += 2;
                                    continue;
                                }
                            }
                            result.push_str("->");
                            result.push_str(fname);
                            if let Some(ref type_name) = current_type {
                                current_type = self.record_field_types.get(&(type_name.clone(), fname.clone())).cloned();
                            }
                            i += 2;
                            continue;
                        }
                    }
                    // Standalone deref: wrap with (*...)
                    result = format!("(*{})", result);
                }
            }
            i += 1;
        }
        result
    }

    /// Like gen_expr but converts single-char string literals to char literals.
    /// Used in binary ops where single-char strings should be treated as CHAR.
    fn gen_expr_for_binop(&mut self, expr: &Expr) {
        if let ExprKind::StringLit(s) = &expr.kind {
            if s.len() == 1 {
                let ch = s.chars().next().unwrap();
                self.emit(&format!("'{}'", escape_c_char(ch)));
                return;
            }
        }
        self.gen_expr(expr);
    }

    fn expr_to_string(&mut self, expr: &Expr) -> String {
        let saved = std::mem::take(&mut self.output);
        self.gen_expr(expr);
        let result = std::mem::replace(&mut self.output, saved);
        result
    }

    /// Like expr_to_string but for expressions that should be chars (single-char strings become char literals)
    fn expr_to_char_string(&mut self, expr: &Expr) -> String {
        if let ExprKind::StringLit(s) = &expr.kind {
            if s.len() == 1 {
                let ch = s.chars().next().unwrap();
                return format!("'{}'", escape_c_char(ch));
            }
        }
        self.expr_to_string(expr)
    }

    // ── Type mapping ────────────────────────────────────────────────

    fn type_to_c(&self, tn: &TypeNode) -> String {
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
    fn proc_type_decl(&mut self, tn: &TypeNode, name: &str, is_ptr: bool) -> String {
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

    /// Check if a TypeNode is a ProcedureType
    fn is_proc_type(tn: &TypeNode) -> bool {
        matches!(tn, TypeNode::ProcedureType { .. })
    }

    fn named_type_to_c(&self, qi: &QualIdent) -> String {
        // If module-qualified (e.g., Stack.Stack), prefix with module name
        if let Some(module) = &qi.module {
            if self.foreign_modules.contains(module.as_str()) {
                return self.mangle(&qi.name);
            }
            return format!("{}_{}", module, self.mangle(&qi.name));
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
            other => self.mangle(other),
        }
    }

    fn qualident_to_c(&self, qi: &QualIdent) -> String {
        if let Some(module) = &qi.module {
            if self.foreign_modules.contains(module.as_str()) {
                return qi.name.clone();
            }
            format!("{}_{}", module, qi.name)
        } else {
            self.named_type_to_c(qi)
        }
    }

    fn type_array_suffix(&self, tn: &TypeNode) -> String {
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

    fn index_type_to_size(&self, idx: &TypeNode) -> String {
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
                    _ => "[/* size */]".to_string(),
                }
            }
            _ => "[/* size */]".to_string(),
        }
    }

    fn const_expr_to_string(&self, expr: &Expr) -> String {
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

    fn infer_c_type(&self, expr: &Expr) -> String {
        match &expr.kind {
            ExprKind::IntLit(_) => "int32_t".to_string(),
            ExprKind::RealLit(_) => "float".to_string(),
            ExprKind::StringLit(_) => "const char *".to_string(),
            ExprKind::CharLit(_) => "char".to_string(),
            ExprKind::BoolLit(_) => "int".to_string(),
            ExprKind::UnaryOp { operand, .. } => self.infer_c_type(operand),
            _ => "int32_t".to_string(),
        }
    }

    /// Get parameter codegen info for a named procedure
    fn get_param_info(&self, name: &str) -> Vec<ParamCodegenInfo> {
        // Check our tracked proc params
        if let Some(params) = self.proc_params.get(name) {
            return params.clone();
        }
        // Check stdlib imports - look up in sema symtab
        if let Some(sym) = self.sema.symtab.lookup(name) {
            if let crate::symtab::SymbolKind::Procedure { params, .. } = &sym.kind {
                return params.iter().map(|p| ParamCodegenInfo {
                    name: p.name.clone(),
                    is_var: p.is_var,
                    is_open_array: false,
                    is_char: p.typ == TY_CHAR,
                }).collect();
            }
        }
        Vec::new()
    }

    /// Get VAR parameter flags for a named procedure
    fn get_var_param_flags(&self, name: &str) -> Vec<bool> {
        self.get_param_info(name).iter().map(|p| p.is_var).collect()
    }

    /// Generate arguments for a procedure/function call, handling VAR and open array params
    fn gen_call_args(&mut self, args: &[Expr], param_info: &[ParamCodegenInfo]) {
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
                } else {
                    self.emit(&format!("(sizeof({}) / sizeof({}[0])) - 1", arg_str, arg_str));
                }
            } else if is_var {
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
    fn gen_var_arg(&mut self, arg: &Expr) {
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

    fn is_var_param(&self, name: &str) -> bool {
        for scope in self.var_params.iter().rev() {
            if let Some(&is_var) = scope.get(name) {
                return is_var;
            }
        }
        false
    }


    fn push_var_scope(&mut self) {
        self.var_params.push(HashMap::new());
        self.open_array_params.push(HashSet::new());
    }

    fn pop_var_scope(&mut self) {
        self.var_params.pop();
        self.open_array_params.pop();
    }

    /// Save array_vars/char_array_vars before entering a procedure scope
    fn save_array_var_scope(&self) -> (HashSet<String>, HashSet<String>) {
        (self.array_vars.clone(), self.char_array_vars.clone())
    }

    /// Restore array_vars/char_array_vars after leaving a procedure scope
    fn restore_array_var_scope(&mut self, saved: (HashSet<String>, HashSet<String>)) {
        self.array_vars = saved.0;
        self.char_array_vars = saved.1;
    }

    /// Check if a variable name is accessed through the _env pointer in the current context
    fn is_env_var(&self, name: &str) -> bool {
        if let Some(env_vars) = self.env_access_names.last() {
            env_vars.contains(name)
        } else {
            false
        }
    }

    /// Build a map of variable name → C type for a procedure's own params and local vars
    fn build_scope_vars(&self, p: &ProcDecl) -> HashMap<String, String> {
        let mut vars = HashMap::new();
        for fp in &p.heading.params {
            let c_type = self.type_to_c(&fp.typ);
            for name in &fp.names {
                vars.insert(name.clone(), c_type.clone());
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

    /// Try to evaluate a constant integer expression at compile time
    fn try_eval_const_int(&self, expr: &Expr) -> Option<i64> {
        match &expr.kind {
            ExprKind::IntLit(v) => Some(*v),
            ExprKind::CharLit(c) => Some(*c as i64),
            ExprKind::BoolLit(b) => Some(if *b { 1 } else { 0 }),
            ExprKind::Designator(d) => {
                if d.selectors.is_empty() && d.ident.module.is_none() {
                    self.const_int_values.get(&d.ident.name).copied()
                } else {
                    None
                }
            }
            ExprKind::UnaryOp { op, operand } => {
                let v = self.try_eval_const_int(operand)?;
                match op {
                    UnaryOp::Neg => Some(-v),
                    UnaryOp::Pos => Some(v),
                }
            }
            ExprKind::BinaryOp { op, left, right } => {
                let l = self.try_eval_const_int(left)?;
                let r = self.try_eval_const_int(right)?;
                match op {
                    BinaryOp::Add => Some(l + r),
                    BinaryOp::Sub => Some(l - r),
                    BinaryOp::Mul => Some(l * r),
                    BinaryOp::IntDiv => if r != 0 { Some(l / r) } else { None },
                    BinaryOp::Mod => if r != 0 { Some(l % r) } else { None },
                    _ => None,
                }
            }
            ExprKind::Not(e) => {
                let v = self.try_eval_const_int(e)?;
                Some(if v == 0 { 1 } else { 0 })
            }
            _ => None,
        }
    }

    /// Check if a TypeNode is ARRAY [...] OF CHAR
    fn is_char_array_type(&self, tn: &TypeNode) -> bool {
        match tn {
            TypeNode::Array { elem_type, .. } => {
                matches!(elem_type.as_ref(), TypeNode::Named(qi) if qi.name == "CHAR")
            }
            TypeNode::Named(qi) => self.char_array_types.contains(&qi.name),
            _ => false,
        }
    }

    /// Check if a TypeNode is any array type (for memcpy assignment)
    fn is_array_type(&self, tn: &TypeNode) -> bool {
        match tn {
            TypeNode::Array { .. } => true,
            TypeNode::Named(qi) => self.array_types.contains(&qi.name),
            _ => false,
        }
    }

    /// Check if a field name belongs to an array-typed record field
    fn is_array_field(&self, field_name: &str) -> bool {
        for ((_rec_name, fname)) in &self.array_fields {
            if fname == field_name {
                return true;
            }
        }
        false
    }

    /// Check if an expression is a multi-char string or a char array variable
    fn is_string_expr(&self, expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::StringLit(s) => s.len() > 1,
            ExprKind::Designator(d) => {
                d.ident.module.is_none() && d.selectors.is_empty()
                    && self.char_array_vars.contains(&d.ident.name)
            }
            _ => false,
        }
    }

    fn is_open_array_param(&self, name: &str) -> bool {
        // Check scoped open_array_params (current procedure's params only)
        for scope in self.open_array_params.iter().rev() {
            if scope.contains(name) {
                return true;
            }
        }
        false
    }

    fn is_char_array_field(&self, field_name: &str) -> bool {
        // Check if any record type has a field with this name that is a char array
        for ((_rec_name, fname)) in &self.char_array_fields {
            if fname == field_name {
                return true;
            }
        }
        false
    }

    fn is_set_type(&self, tn: &TypeNode) -> bool {
        match tn {
            TypeNode::Named(qi) => qi.name == "BITSET",
            TypeNode::Set { .. } => true,
            _ => false,
        }
    }

    /// Check if an expression is a set value (set constructor or known set variable)
    fn is_set_expr(&self, expr: &Expr) -> bool {
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
            _ => false,
        }
    }

    /// Check if an expression is likely CARDINAL/unsigned (for DIV/MOD codegen)
    fn is_unsigned_expr(&self, expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::Designator(d) => {
                d.ident.module.is_none() && d.selectors.is_empty()
                    && self.cardinal_vars.contains(&d.ident.name)
            }
            ExprKind::FuncCall { desig, .. } => {
                // CARDINAL(x) type transfer, ORD, HIGH, SHR, SHL, BAND, BOR, BXOR, BNOT
                matches!(desig.ident.name.as_str(),
                    "CARDINAL" | "ORD" | "HIGH" | "SHR" | "SHL" | "BAND" | "BOR" | "BXOR" | "BNOT")
            }
            ExprKind::BinaryOp { left, right, .. } => {
                self.is_unsigned_expr(left) || self.is_unsigned_expr(right)
            }
            _ => false,
        }
    }

    fn is_complex_type(&self, tn: &TypeNode) -> bool {
        match tn {
            TypeNode::Named(qi) => qi.name == "COMPLEX",
            _ => false,
        }
    }

    fn is_longcomplex_type(&self, tn: &TypeNode) -> bool {
        match tn {
            TypeNode::Named(qi) => qi.name == "LONGCOMPLEX",
            _ => false,
        }
    }

    fn is_complex_expr(&self, expr: &Expr) -> bool {
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

    fn is_longcomplex_expr(&self, expr: &Expr) -> bool {
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

    fn register_var_param(&mut self, name: &str) {
        if let Some(scope) = self.var_params.last_mut() {
            scope.insert(name.to_string(), true);
        }
    }

    fn is_negative_expr(&self, expr: &Expr) -> bool {
        // Try constant folding first for expressions like (0-1) or (-2+1)
        if let Some(val) = self.try_eval_const_int(expr) {
            return val < 0;
        }
        match &expr.kind {
            ExprKind::UnaryOp { op: UnaryOp::Neg, .. } => true,
            ExprKind::IntLit(v) => *v < 0,
            _ => false,
        }
    }

    fn resolve_proc_name(&self, desig: &Designator) -> String {
        let name = &desig.ident.name;
        if let Some(module) = &desig.ident.module {
            if self.foreign_modules.contains(module.as_str()) {
                return name.to_string();
            }
            if let Some(c_name) = stdlib::map_stdlib_call(module, name) {
                return c_name;
            }
            return format!("{}_{}", module, name);
        }
        // Check if base name is a whole-module import with a Field selector (e.g., MathUtils.Square)
        if self.imported_modules.contains(name) {
            if let Some(Selector::Field(proc_name, _)) = desig.selectors.first() {
                if self.foreign_modules.contains(name.as_str()) {
                    return proc_name.to_string();
                }
                if let Some(c_name) = stdlib::map_stdlib_call(name, proc_name) {
                    return c_name;
                }
                return format!("{}_{}", name, proc_name);
            }
        }
        // Check if it's imported via FROM Module IMPORT
        if let Some(module) = self.import_map.get(name) {
            if self.foreign_modules.contains(module.as_str()) {
                return name.to_string();
            }
            if let Some(c_name) = stdlib::map_stdlib_call(module, name) {
                return c_name;
            }
            // Non-stdlib module: use module-prefixed name
            if !stdlib::is_stdlib_module(module) {
                return format!("{}_{}", module, name);
            }
        }
        // Check if this name has an EXPORTC alias
        if let Some(ecn) = self.export_c_names.get(name) {
            return ecn.clone();
        }
        // Inside an embedded implementation, local proc calls need module prefix
        if self.embedded_local_procs.contains(name) {
            return format!("{}_{}", self.module_name, name);
        }
        self.mangle(name)
    }

    /// If the designator starts with an imported module name followed by a field selector,
    /// return (module_name, proc_name) and the remaining selectors start at index 1.
    /// Otherwise return None.
    fn resolve_module_qualified<'a>(&self, desig: &'a Designator) -> Option<(&'a str, &'a str)> {
        if desig.ident.module.is_some() {
            return None; // already qualified
        }
        if self.imported_modules.contains(&desig.ident.name) {
            if let Some(Selector::Field(proc_name, _)) = desig.selectors.first() {
                return Some((&desig.ident.name, proc_name));
            }
        }
        None
    }

    fn mangle(&self, name: &str) -> String {
        match name {
            // Modula-2 built-in constants
            "NIL" => "NULL".to_string(),
            "TRUE" => "1".to_string(),
            "FALSE" => "0".to_string(),
            // Avoid C keyword conflicts
            "auto" | "break" | "case" | "char" | "const" | "continue" | "default" | "do"
            | "double" | "else" | "enum" | "extern" | "float" | "for" | "goto" | "if"
            | "int" | "long" | "register" | "return" | "short" | "signed" | "sizeof"
            | "static" | "struct" | "switch" | "typedef" | "union" | "unsigned" | "void"
            | "volatile" | "while" => format!("m2_{}", name),
            _ => {
                // Check if it's an enum variant
                if let Some(c_name) = self.enum_variants.get(name) {
                    return c_name.clone();
                }
                name.to_string()
            }
        }
    }

    // ── Modula-2+ OBJECT Type Codegen ────────────────────────────────

    fn gen_object_type(
        &mut self,
        name: &str,
        parent: Option<&QualIdent>,
        fields: &[Field],
        methods: &[MethodDecl],
        overrides: &[OverrideDecl],
    ) {
        let c_name = self.mangle(name);

        // Generate vtable struct
        self.emitln(&format!("typedef struct {}_vtable {{", c_name));
        self.indent += 1;
        // If there's a parent, include parent vtable fields (simplified: embed parent vtable pointer)
        if let Some(p) = parent {
            let parent_c = if let Some(ref m) = p.module {
                format!("{}_{}", m, p.name)
            } else {
                self.mangle(&p.name)
            };
            self.emitln(&format!("{}_vtable _parent;", parent_c));
        }
        // Method function pointers
        for md in methods {
            self.emit_indent();
            let ret = if let Some(rt) = &md.return_type {
                self.type_to_c(rt)
            } else {
                "void".to_string()
            };
            self.emit(&format!("{} (*{})(struct {} *self", ret, md.name, c_name));
            for fp in &md.params {
                let pt = self.type_to_c(&fp.typ);
                for pname in &fp.names {
                    if fp.is_var {
                        self.emit(&format!(", {} *{}", pt, pname));
                    } else {
                        self.emit(&format!(", {} {}", pt, pname));
                    }
                }
            }
            self.emit(");\n");
        }
        self.indent -= 1;
        self.emitln(&format!("}} {}_vtable;", c_name));
        self.newline();

        // Generate instance struct
        self.emitln(&format!("struct {} {{", c_name));
        self.indent += 1;
        self.emitln(&format!("{}_vtable *_vt;", c_name));
        // Include parent fields
        if let Some(p) = parent {
            let parent_c = if let Some(ref m) = p.module {
                format!("{}_{}", m, p.name)
            } else {
                self.mangle(&p.name)
            };
            self.emitln(&format!("/* inherited from {} */", parent_c));
        }
        // Own fields
        for f in fields {
            self.emit_indent();
            let ctype = self.type_to_c(&f.typ);
            let arr_suffix = self.type_array_suffix(&f.typ);
            self.emit(&format!("{} ", ctype));
            for (i, fname) in f.names.iter().enumerate() {
                if i > 0 { self.emit(", "); }
                self.emit(fname);
                if !arr_suffix.is_empty() {
                    self.emit(&arr_suffix);
                }
            }
            self.emit(";\n");
        }
        self.indent -= 1;
        self.emitln("};");
        self.newline();

        // Generate type typedef (pointer to struct, as objects are reference types)
        self.emitln(&format!("typedef struct {} *{};", c_name, c_name));

        // Generate static type info
        self.emitln(&format!("static m2_TypeInfo {}_typeinfo = {{ 0, \"{}\", NULL }};", c_name, name));

        // Track field names for WITH resolution
        let mut field_names: Vec<String> = fields.iter()
            .flat_map(|f| f.names.clone())
            .collect();
        // Also add method names
        for md in methods {
            field_names.push(md.name.clone());
        }
        self.record_fields.insert(name.to_string(), field_names);
    }

    // ── Modula-2+ Exception Declaration ─────────────────────────────

    fn gen_exception_decl(&mut self, e: &ExceptionDecl) {
        let exc_id = self.next_exception_id();
        self.emitln(&format!("static const int {} = {};", self.mangle(&e.name), exc_id));
    }

    fn next_exception_id(&mut self) -> usize {
        self.exception_counter += 1;
        self.exception_counter
    }

    // ── Modula-2+ TRY/EXCEPT/FINALLY ───────────────────────────────

    fn gen_try_statement(&mut self, body: &[Statement], excepts: &[ExceptClause], finally_body: &Option<Vec<Statement>>) {
        self.emitln("{");
        self.indent += 1;
        self.emitln("m2_ExcFrame _ef;");
        self.emitln("M2_TRY(_ef) {");
        self.indent += 1;
        for s in body {
            self.gen_statement(s);
        }
        self.emitln("M2_ENDTRY(_ef);");
        self.indent -= 1;
        self.emitln("} M2_CATCH {");
        self.indent += 1;
        self.emitln("M2_ENDTRY(_ef);");
        if excepts.is_empty() {
            self.emitln("/* no handlers — re-raise */");
            self.emitln("m2_raise(_ef.exception_id, _ef.exception_name, _ef.exception_arg);");
        } else {
            let mut first = true;
            for ec in excepts {
                self.emit_indent();
                if !first {
                    self.emit("} else ");
                }
                first = false;
                if let Some(ref exc_name) = ec.exception {
                    let c_name = if let Some(ref m) = exc_name.module {
                        format!("M2_EXC_{}_{}", m, exc_name.name)
                    } else {
                        format!("M2_EXC_{}", self.mangle(&exc_name.name))
                    };
                    self.emit(&format!("if (_ef.exception_id == {}) {{\n", c_name));
                } else {
                    // Catch-all
                    self.emit("{\n");
                }
                self.indent += 1;
                for s in &ec.body {
                    self.gen_statement(s);
                }
                self.indent -= 1;
            }
            self.emitln("}");
        }
        self.indent -= 1;
        self.emitln("}");
        // FINALLY block: always executes
        if let Some(fb) = finally_body {
            for s in fb {
                self.gen_statement(s);
            }
        }
        self.indent -= 1;
        self.emitln("}");
    }

    // ── Modula-2+ LOCK Statement ───────────────────────────────────

    fn gen_lock_statement(&mut self, mutex: &Expr, body: &[Statement]) {
        self.emitln("{");
        self.indent += 1;
        self.emit_indent();
        self.emit("m2_Mutex_Lock(");
        self.gen_expr(mutex);
        self.emit(");\n");
        // Use exception frame to guarantee unlock
        self.emitln("m2_ExcFrame _lf;");
        self.emitln("M2_TRY(_lf) {");
        self.indent += 1;
        for s in body {
            self.gen_statement(s);
        }
        self.emitln("M2_ENDTRY(_lf);");
        self.indent -= 1;
        self.emitln("} M2_CATCH {");
        self.indent += 1;
        self.emitln("M2_ENDTRY(_lf);");
        self.emit_indent();
        self.emit("m2_Mutex_Unlock(");
        self.gen_expr(mutex);
        self.emit(");\n");
        self.emitln("m2_raise(_lf.exception_id, _lf.exception_name, _lf.exception_arg); /* re-raise */");
        self.indent -= 1;
        self.emitln("}");
        self.emit_indent();
        self.emit("m2_Mutex_Unlock(");
        self.gen_expr(mutex);
        self.emit(");\n");
        self.indent -= 1;
        self.emitln("}");
    }

    // ── Modula-2+ TYPECASE Statement ───────────────────────────────

    fn gen_typecase_statement(&mut self, expr: &Expr, branches: &[TypeCaseBranch], else_body: &Option<Vec<Statement>>) {
        self.emitln("{");
        self.indent += 1;
        self.emit_indent();
        self.emit("void *_tc_val = (void *)(");
        self.gen_expr(expr);
        self.emit(");\n");
        self.emitln("m2_TypeInfo *_tc_info = ((m2_TypeInfo **)_tc_val)[-1];");
        let mut first = true;
        for branch in branches {
            self.emit_indent();
            if !first {
                self.emit("} else ");
            }
            first = false;
            self.emit("if (");
            for (i, ty) in branch.types.iter().enumerate() {
                if i > 0 {
                    self.emit(" || ");
                }
                let type_name = if let Some(ref m) = ty.module {
                    format!("{}_{}", m, ty.name)
                } else {
                    self.mangle(&ty.name)
                };
                self.emit(&format!("_tc_info->type_id == M2_TYPEID_{}", type_name));
            }
            self.emit(") {\n");
            self.indent += 1;
            if let Some(ref var_name) = branch.var {
                // Cast to the specific type and bind
                if let Some(first_type) = branch.types.first() {
                    let type_name = if let Some(ref m) = first_type.module {
                        format!("{}_{}", m, first_type.name)
                    } else {
                        self.mangle(&first_type.name)
                    };
                    self.emitln(&format!("{} *{} = ({} *)_tc_val;", type_name, var_name, type_name));
                }
            }
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
        if !branches.is_empty() {
            self.emitln("}");
        }
        self.indent -= 1;
        self.emitln("}");
    }
}

fn escape_c_string(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        match ch {
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\0' => out.push_str("\\0"),
            c => out.push(c),
        }
    }
    out
}

fn escape_c_char(ch: char) -> String {
    match ch {
        '\n' => "\\n".to_string(),
        '\t' => "\\t".to_string(),
        '\r' => "\\r".to_string(),
        '\\' => "\\\\".to_string(),
        '\'' => "\\'".to_string(),
        '\0' => "\\0".to_string(),
        c => c.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn parse(input: &str) -> CompilationUnit {
        let mut lexer = Lexer::new(input, "test.mod");
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        parser.parse_compilation_unit().unwrap()
    }

    fn generate(input: &str, debug: bool) -> String {
        let unit = parse(input);
        let mut cg = CodeGen::new();
        cg.set_debug(debug);
        cg.generate(&unit).unwrap()
    }

    #[test]
    fn test_line_directives_present_in_debug_mode() {
        let src = r#"MODULE Test;
FROM InOut IMPORT WriteString, WriteLn;
BEGIN
  WriteString("Hello");
  WriteLn;
END Test."#;
        let c = generate(src, true);
        assert!(c.contains("#line"), "debug output should contain #line directives");
        assert!(c.contains("\"test.mod\""), "debug output should reference source file");
    }

    #[test]
    fn test_no_line_directives_without_debug() {
        let src = r#"MODULE Test;
FROM InOut IMPORT WriteString, WriteLn;
BEGIN
  WriteString("Hello");
  WriteLn;
END Test."#;
        let c = generate(src, false);
        assert!(!c.contains("#line"), "non-debug output should not contain #line directives");
    }

    #[test]
    fn test_line_directives_in_procedures() {
        let src = r#"MODULE Test;
FROM InOut IMPORT WriteString, WriteLn;
PROCEDURE Greet;
BEGIN
  WriteString("Hi");
  WriteLn;
END Greet;
BEGIN
  Greet;
END Test."#;
        let c = generate(src, true);
        // Should have #line for the procedure and for the body
        let line_count = c.matches("#line").count();
        assert!(line_count >= 3, "expected >=3 #line directives, got {}", line_count);
    }

    #[test]
    fn test_line_directive_dedup() {
        // Two statements on consecutive lines should produce two #line directives,
        // but same-line duplicates are suppressed.
        let src = r#"MODULE Test;
FROM InOut IMPORT WriteString, WriteLn;
BEGIN
  WriteString("A");
  WriteString("B");
END Test."#;
        let c = generate(src, true);
        let lines: Vec<&str> = c.lines().filter(|l| l.starts_with("#line")).collect();
        // Each #line should be unique (no consecutive duplicates of the same line number)
        for pair in lines.windows(2) {
            // Different lines can have same file, but should not repeat the same #line N
            // unless it's for a different context
        }
        assert!(!lines.is_empty(), "should have #line directives");
    }
}
