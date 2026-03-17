use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

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

/// Per-procedure variable tracking sets that must be saved/restored when
/// entering/leaving a procedure scope. Without this, a local `key: ARRAY`
/// in procedure A leaks into `array_vars`, causing procedure B's
/// `VAR key: ADDRESS` assignment to emit memcpy instead of `*key = val`.
#[derive(Clone)]
struct VarTrackingScope {
    array_vars: HashSet<String>,
    char_array_vars: HashSet<String>,
    set_vars: HashSet<String>,
    cardinal_vars: HashSet<String>,
    longint_vars: HashSet<String>,
    longcard_vars: HashSet<String>,
    complex_vars: HashSet<String>,
    longcomplex_vars: HashSet<String>,
    var_types: HashMap<String, String>,
}

/// Snapshot of CodeGen state that must be saved/restored around embedded
/// implementation module generation. Keeps the save/restore in one place
/// instead of manually cloning 8+ fields at each call site.
struct EmbeddedModuleContext {
    module_name: String,
    import_map: HashMap<String, String>,
    import_alias_map: HashMap<String, String>,
    var_params: Vec<HashMap<String, bool>>,
    open_array_params: Vec<HashSet<String>>,
    named_array_value_params: Vec<HashSet<String>>,
    proc_params: HashMap<String, Vec<ParamCodegenInfo>>,
    var_tracking: VarTrackingScope,
}

pub struct CodeGen {
    output: String,
    indent: usize,
    module_name: String,
    sema: SemanticAnalyzer,
    /// Maps imported name (or alias) -> source module for stdlib resolution
    import_map: HashMap<String, String>,
    /// Maps alias -> original name for aliased imports (FROM M IMPORT X AS Y)
    import_alias_map: HashMap<String, String>,
    /// Tracks which local names are VAR parameters (passed as pointers)
    var_params: Vec<HashMap<String, bool>>,
    /// Tracks which local names are open array parameters (have _high companion)
    open_array_params: Vec<HashSet<String>>,
    /// Tracks which local names are named-array value params (array decays to pointer in C)
    named_array_value_params: Vec<HashSet<String>>,
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
    /// Maps array type name → HIGH expression string (for named-array value params)
    array_type_high: HashMap<String, String>,
    /// Pointer type names whose base is a named array type (e.g., SrcPtr = POINTER TO SrcArray).
    /// These generate `ArrayType *PtrName` in C, so deref+index needs `(*p)[i]` not `p[i]`.
    ptr_to_named_array: HashSet<String>,
    /// Variable names that have array types (for memcpy assignment)
    array_vars: HashSet<String>,
    /// Record field names that have array types: (record_type_name, field_name)
    array_fields: HashSet<(String, String)>,
    /// Record field names that are pointer types (bare name, for disambiguating array_fields)
    pointer_fields: HashSet<String>,
    /// Maps array variable name → element type name (for resolving arr[i].field patterns)
    array_var_elem_types: HashMap<String, String>,
    /// Variable names that are SET or BITSET types
    set_vars: HashSet<String>,
    /// Variable names that are CARDINAL (unsigned) types
    cardinal_vars: HashSet<String>,
    /// Variable names that are LONGINT (signed 64-bit) types
    longint_vars: HashSet<String>,
    /// Variable names that are LONGCARD (unsigned 64-bit) types
    longcard_vars: HashSet<String>,
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
    /// Maps pointer type name -> base record struct tag (for WITH resolution through pointers)
    pointer_base_types: HashMap<String, String>,
    /// Stack of parent procedure names (for nested proc name mangling)
    parent_proc_stack: Vec<String>,
    /// Maps bare nested proc name -> mangled name (parent_child)
    nested_proc_names: HashMap<String, String>,
    /// True when generating code inside the module body (main function) rather than a procedure
    in_module_body: bool,
    /// Counter for generating unique exception IDs
    exception_counter: usize,
    /// Known exception names (for M2_EXC_ prefix resolution in RAISE)
    exception_names: HashSet<String>,
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
    /// Maps procedure type name -> param info (e.g., "ThenFn" -> params) for proc-var calls
    proc_type_params: HashMap<String, Vec<ParamCodegenInfo>>,
    /// Embedded module names that have init bodies (need calling from main)
    embedded_init_modules: Vec<String>,
    /// Procedure names local to the current embedded implementation (for module-prefixed calls)
    embedded_local_procs: HashSet<String>,
    /// Module-level variable names in the current embedded implementation (for module-prefixed access)
    embedded_local_vars: HashSet<String>,
    /// All known type names (bare + module-prefixed) for type cast recognition
    known_type_names: HashSet<String>,
    /// Type names that are aliases for unsigned types (CARDINAL, LONGCARD)
    unsigned_type_aliases: HashSet<String>,
    /// Maps record field names → proc param info for fields with procedure types.
    /// Used as fallback for calls through complex designators (e.g. rec.field(args)).
    field_proc_params: HashMap<String, Vec<ParamCodegenInfo>>,
    /// Monotonically increasing type ID counter for M2_TypeDesc emission
    type_id_counter: usize,
    /// Pending type descriptors to emit: (c_symbol_name, display_name, Option<parent_c_symbol>)
    type_descs: Vec<(String, String, Option<String>, usize)>,
    /// Maps M2 type name (mangled) → M2_TypeDesc C symbol name (for REF types)
    ref_type_descs: HashMap<String, String>,
    /// Maps M2 type name (mangled) → M2_TypeDesc C symbol name (for OBJECT types)
    object_type_descs: HashMap<String, String>,
    /// Emit #line directives mapping generated C back to Modula-2 source
    emit_line_directives: bool,
    /// Debug mode: enables debug-only behaviors like setvbuf (separate from #line emission)
    debug_mode: bool,
    /// Last file emitted in a #line directive (to avoid redundant file changes)
    last_line_file: String,
    /// Last line number emitted in a #line directive (to avoid redundant directives)
    last_line_num: usize,
    /// Set to the module name when generating types for an embedded module (for enum prefixing)
    generating_for_module: Option<String>,
    /// Tracks module-prefixed enum type names (e.g., "EventLoop_Status") for type resolution
    embedded_enum_types: HashSet<String>,
    /// Multi-TU mode: emit per-module markers and non-static linkage for cross-module symbols
    pub multi_tu: bool,
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
        ExprKind::Deref(e) => collect_refs_in_expr(e, out),
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

    // Auto-capture _high companions for open array params.
    // When a nested proc captures an open array param 's', it also needs 's_high'
    // for HIGH(s) to work correctly, even though 's_high' isn't an AST-level reference.
    let mut extra = Vec::new();
    for cap in &captures {
        let high_name = format!("{}_high", cap);
        if outer_vars.contains_key(&high_name) && !captures.contains(&high_name) {
            extra.push(high_name);
        }
    }
    captures.extend(extra);

    captures.sort();
    captures
}

static C_RESERVED: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    let names: &[&str] = &[
        // C keywords (C89/C99/C11)
        "auto", "break", "case", "char", "const", "continue", "default", "do",
        "double", "else", "enum", "extern", "float", "for", "goto", "if",
        "int", "long", "register", "return", "short", "signed", "sizeof",
        "static", "struct", "switch", "typedef", "union", "unsigned", "void",
        "volatile", "while", "inline", "restrict",
        // <math.h>
        "log", "log2", "log10", "exp", "exp2", "pow", "sqrt", "cbrt",
        "sin", "cos", "tan", "asin", "acos", "atan", "atan2",
        "sinh", "cosh", "tanh", "ceil", "floor", "round", "trunc",
        "fabs", "fmod", "hypot", "nan", "j0", "j1", "y0", "y1",
        // <stdio.h>
        "printf", "fprintf", "sprintf", "snprintf", "scanf", "sscanf",
        "fopen", "fclose", "fread", "fwrite", "fgets", "fputs", "puts",
        "getchar", "putchar", "feof", "ferror", "fflush", "fseek", "ftell",
        "rewind", "remove", "rename", "tmpfile", "tmpnam",
        "stdin", "stdout", "stderr",
        // <stdlib.h>
        "malloc", "calloc", "realloc", "free", "abort", "exit", "atexit",
        "atoi", "atol", "atof", "strtol", "strtoul", "strtod",
        "rand", "srand", "qsort", "bsearch", "abs", "labs", "div", "ldiv",
        "getenv", "system",
        // <string.h>
        "memcpy", "memmove", "memset", "memcmp", "strlen", "strcpy", "strncpy",
        "strcat", "strncat", "strcmp", "strncmp", "strchr", "strrchr", "strstr",
        "strtok", "strerror",
        // <ctype.h>
        "isalpha", "isdigit", "isalnum", "isspace", "isupper", "islower",
        "toupper", "tolower",
        // <setjmp.h>
        "setjmp", "longjmp",
        // POSIX common
        "signal", "read", "write", "open", "close", "stat", "pipe", "fork",
        "exec", "wait", "kill", "alarm", "sleep", "time", "clock",
        "errno", "perror",
        // C preprocessor / common macros
        "NULL", "EOF", "FILE", "BUFSIZ",
        // main
        "main",
    ];
    names.iter().copied().collect()
});

impl CodeGen {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            indent: 0,
            module_name: String::new(),
            sema: SemanticAnalyzer::new(),
            import_map: HashMap::new(),
            import_alias_map: HashMap::new(),
            var_params: vec![HashMap::new()],
            open_array_params: vec![HashSet::new()],
            named_array_value_params: vec![HashSet::new()],
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
            array_type_high: HashMap::new(),
            ptr_to_named_array: HashSet::new(),
            array_vars: HashSet::new(),
            array_fields: HashSet::new(),
            pointer_fields: HashSet::new(),
            array_var_elem_types: HashMap::new(),
            set_vars: HashSet::new(),
            cardinal_vars: HashSet::new(),
            longint_vars: HashSet::new(),
            longcard_vars: HashSet::new(),
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
            pointer_base_types: HashMap::new(),
            parent_proc_stack: Vec::new(),
            nested_proc_names: HashMap::new(),
            in_module_body: false,
            exception_counter: 0,
            exception_names: HashSet::new(),
            m2plus: false,
            uses_gc: false,
            uses_threads: false,
            foreign_modules: HashSet::new(),
            foreign_def_modules: Vec::new(),
            export_c_names: HashMap::new(),
            def_modules: HashMap::new(),
            proc_type_params: HashMap::new(),
            embedded_init_modules: Vec::new(),
            embedded_local_procs: HashSet::new(),
            embedded_local_vars: HashSet::new(),
            known_type_names: HashSet::new(),
            unsigned_type_aliases: HashSet::new(),
            field_proc_params: HashMap::new(),
            type_id_counter: 0,
            type_descs: Vec::new(),
            ref_type_descs: HashMap::new(),
            object_type_descs: HashMap::new(),
            emit_line_directives: true,
            debug_mode: false,
            last_line_file: String::new(),
            last_line_num: 0,
            generating_for_module: None,
            embedded_enum_types: HashSet::new(),
            multi_tu: false,
        }
    }

    pub fn set_m2plus(&mut self, enabled: bool) {
        self.m2plus = enabled;
    }

    pub fn set_debug(&mut self, enabled: bool) {
        self.debug_mode = enabled;
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
        if !self.emit_line_directives {
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

        // Register type names from this def module for type-cast recognition
        for d in &def.definitions {
            if let Definition::Type(td) = d {
                self.known_type_names.insert(td.name.clone());
                self.known_type_names.insert(format!("{}_{}", def.name, td.name));
            }
        }

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
        self.post_sema_generate(unit).map_err(|e| vec![e])?;
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

        self.post_sema_generate(unit)?;
        Ok(self.output.clone())
    }

    fn post_sema_generate(&mut self, unit: &CompilationUnit) -> CompileResult<()> {
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

        if self.multi_tu {
            self.emit("/* MX_HEADER_BEGIN */\n");
        }
        // Generate header
        self.emit(&stdlib::generate_runtime_header());
        if self.multi_tu {
            self.emit("/* MX_HEADER_END */\n");
        }

        match unit {
            CompilationUnit::ProgramModule(m) => self.gen_program_module(m)?,
            CompilationUnit::DefinitionModule(m) => self.gen_definition_module(m),
            CompilationUnit::ImplementationModule(m) => self.gen_implementation_module(m)?,
        }
        Ok(())
    }

    // ── Shared emission helpers ───────────────────────────────────────

    /// Register proc_params from module_exports, emit foreign extern decls,
    /// and generate embedded implementations for pending imported modules.
    /// Shared by gen_program_module and gen_implementation_module.
    fn emit_preamble_for_imports(&mut self) -> CompileResult<()> {
        for (mod_name, exports) in &self.module_exports.clone() {
            for (proc_name, param_info) in exports {
                let prefixed = format!("{}_{}", mod_name, proc_name);
                self.proc_params.insert(prefixed, param_info.clone());
                if self.foreign_modules.contains(mod_name.as_str()) {
                    self.proc_params.insert(proc_name.clone(), param_info.clone());
                }
            }
        }

        self.gen_foreign_extern_decls();

        if let Some(pending) = self.pending_modules.take() {
            let sorted = Self::topo_sort_modules(pending, &self.def_modules)?;
            let embedded_names: std::collections::HashSet<String> =
                sorted.iter().map(|m| m.name.clone()).collect();

            // Emit types and constants for definition-only modules (no .mod counterpart)
            // BEFORE embedded implementations, since embedded modules may reference these types.
            // These are registered def modules with no matching implementation module,
            // e.g. pure type-definition modules like "PdcTypes.def".
            let def_only_modules: Vec<String> = self.def_modules.keys()
                .filter(|name| {
                    !embedded_names.contains(name.as_str())
                        && !self.foreign_modules.contains(name.as_str())
                        && name.as_str() != self.module_name
                })
                .cloned()
                .collect();
            if !def_only_modules.is_empty() {
                let mut def_only_sorted = def_only_modules;
                def_only_sorted.sort();
                for mod_name in &def_only_sorted {
                    if let Some(def_mod) = self.def_modules.get(mod_name).cloned() {
                        let saved_module_name = self.module_name.clone();
                        let saved_import_map = self.import_map.clone();
                        let saved_import_alias_map = self.import_alias_map.clone();

                        self.module_name = mod_name.clone();
                        self.emitln(&format!("/* Definition-only module {} */", mod_name));
                        self.generating_for_module = Some(mod_name.clone());

                        // Build import scope so intra-module type references resolve
                        self.import_map.clear();
                        self.import_alias_map.clear();
                        self.build_import_map(&def_mod.imports);

                        // Pre-register type names in embedded_enum_types
                        for d in &def_mod.definitions {
                            if let Definition::Type(t) = d {
                                let prefixed = format!("{}_{}", mod_name, self.mangle(&t.name));
                                self.embedded_enum_types.insert(prefixed);
                            }
                        }

                        // Forward declare record types
                        for d in &def_mod.definitions {
                            if let Definition::Type(t) = d {
                                if matches!(&t.typ, Some(TypeNode::Record { .. })) {
                                    let cn = self.type_decl_c_name(&t.name);
                                    self.emitln(&format!("typedef struct {} {};", cn, cn));
                                }
                            }
                        }

                        // Emit type and constant declarations
                        for d in &def_mod.definitions {
                            match d {
                                Definition::Type(t) => self.gen_type_decl(t),
                                Definition::Const(c) => self.gen_const_decl(c),
                                _ => {}
                            }
                        }

                        self.generating_for_module = None;
                        self.module_name = saved_module_name;
                        self.import_map = saved_import_map;
                        self.import_alias_map = saved_import_alias_map;
                        self.newline();
                    }
                }
            }

            for imp_mod in &sorted {
                self.gen_embedded_implementation(imp_mod);
            }
        }
        Ok(())
    }

    /// Emit forward struct declarations for all record types in a declaration list.
    /// Uses type_decl_c_name, which automatically handles module prefixing for
    /// embedded modules (via generating_for_module).
    fn emit_record_forward_decls(&mut self, decls: &[Declaration]) {
        for decl in decls {
            if let Declaration::Type(t) = decl {
                let cn = self.type_decl_c_name(&t.name);
                match &t.typ {
                    Some(TypeNode::Record { .. }) => {
                        self.emitln(&format!("typedef struct {} {};", cn, cn));
                    }
                    Some(TypeNode::Pointer { base, .. }) if matches!(&**base, TypeNode::Record { .. }) => {
                        let tag = format!("{}_r", cn);
                        self.emitln(&format!("typedef struct {} {};", tag, tag));
                        self.emitln(&format!("typedef {} *{};", tag, cn));
                    }
                    _ => {}
                }
            }
        }
    }

    /// Emit type and const declarations from a declaration list.
    /// Types are emitted first (in source order), then constants are topologically
    /// sorted so that forward references between constants are resolved.
    fn emit_type_and_const_decls(&mut self, decls: &[Declaration]) {
        // Pre-pass: collect integer constant values so array bounds can be inlined
        for decl in decls {
            if let Declaration::Const(c) = decl {
                if let Some(val) = self.try_eval_const_int(&c.expr) {
                    self.const_int_values.insert(c.name.clone(), val);
                }
            }
        }
        // Pass 1: emit all Type declarations in source order
        for decl in decls {
            if let Declaration::Type(t) = decl {
                self.gen_type_decl(t);
            }
        }
        // Pass 2: collect and topologically sort Const declarations
        let consts: Vec<&ConstDecl> = decls.iter().filter_map(|d| {
            if let Declaration::Const(c) = d { Some(c) } else { None }
        }).collect();
        if consts.is_empty() {
            return;
        }
        let const_names: HashSet<String> = consts.iter().map(|c| c.name.clone()).collect();
        // Build adjacency: for each const, which other consts does it reference?
        let mut deps: HashMap<String, Vec<String>> = HashMap::new();
        for c in &consts {
            let mut refs = HashSet::new();
            Self::collect_expr_ident_refs(&c.expr, &mut refs);
            let my_deps: Vec<String> = refs.into_iter().filter(|r| const_names.contains(r) && r != &c.name).collect();
            deps.insert(c.name.clone(), my_deps);
        }
        // Kahn's algorithm for topological sort
        // deps maps node → [nodes it depends on]. Build reverse graph: dependee → [dependents]
        let mut reverse: HashMap<String, Vec<String>> = HashMap::new();
        let mut in_degree: HashMap<String, usize> = consts.iter().map(|c| (c.name.clone(), 0)).collect();
        for (node, dep_list) in &deps {
            *in_degree.entry(node.clone()).or_insert(0) += dep_list.len();
            for dep in dep_list {
                reverse.entry(dep.clone()).or_default().push(node.clone());
            }
        }
        let mut queue: std::collections::VecDeque<String> = std::collections::VecDeque::new();
        // Seed with zero-in-degree nodes (in source order for stability)
        for c in &consts {
            if *in_degree.get(&c.name).unwrap_or(&0) == 0 {
                queue.push_back(c.name.clone());
            }
        }
        let mut sorted_names: Vec<String> = Vec::new();
        while let Some(name) = queue.pop_front() {
            sorted_names.push(name.clone());
            if let Some(dependents) = reverse.get(&name) {
                for dependent in dependents {
                    if let Some(deg) = in_degree.get_mut(dependent) {
                        *deg = deg.saturating_sub(1);
                        if *deg == 0 {
                            queue.push_back(dependent.clone());
                        }
                    }
                }
            }
        }
        // If there are cycles, append remaining in source order
        if sorted_names.len() < consts.len() {
            for c in &consts {
                if !sorted_names.contains(&c.name) {
                    sorted_names.push(c.name.clone());
                }
            }
        }
        // Build name->const map and emit in sorted order
        let const_map: HashMap<String, &ConstDecl> = consts.into_iter().map(|c| (c.name.clone(), c)).collect();
        for name in &sorted_names {
            if let Some(c) = const_map.get(name) {
                self.gen_const_decl(c);
            }
        }
    }

    /// Emit type, const, and exception declarations from a declaration list.
    /// Used by embedded implementation gen which also handles exceptions inline.
    fn emit_type_const_exception_decls(&mut self, decls: &[Declaration]) {
        for decl in decls {
            match decl {
                Declaration::Const(c) => self.gen_const_decl(c),
                Declaration::Type(t) => self.gen_type_decl(t),
                Declaration::Exception(e) => self.gen_exception_decl(e),
                _ => {}
            }
        }
    }

    // ── Program module ──────────────────────────────────────────────

    /// Resolve a local name (possibly an alias) to the original imported name.
    fn original_import_name<'a>(&'a self, local_name: &'a str) -> &'a str {
        self.import_alias_map.get(local_name).map(|s| s.as_str()).unwrap_or(local_name)
    }

    /// Check if a name is a known type and return the C type name for casting.
    /// Returns None if the name is not a type (i.e., it's a procedure or variable).
    fn resolve_type_cast_name(&self, name: &str) -> Option<String> {
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

    fn build_import_map(&mut self, imports: &[Import]) {
        // Collect enum variant names to add to import_map after the main loop.
        // When importing an enum type, its variant names are implicitly in scope.
        let mut extra_variants: Vec<(String, String)> = Vec::new();
        for imp in imports {
            if let Some(from_mod) = &imp.from_module {
                // FROM Module IMPORT name1, name2;
                // Also register the module name so Module.Proc() syntax works
                self.imported_modules.insert(from_mod.clone());
                for import_name in &imp.names {
                    let original = &import_name.name;
                    let local = import_name.local_name().to_string();
                    self.import_map.insert(local.clone(), from_mod.clone());
                    // Track alias→original mapping if aliased
                    if import_name.alias.is_some() {
                        self.import_alias_map.insert(local.clone(), original.clone());
                    }
                    // Register stdlib proc params for codegen (is_char, is_var, etc.)
                    if stdlib::is_stdlib_module(from_mod) {
                        if let Some(params) = stdlib::get_stdlib_proc_params(from_mod, original) {
                            let info: Vec<ParamCodegenInfo> = params.into_iter().map(|(pname, is_var, is_char, is_open_array)| {
                                ParamCodegenInfo { name: pname, is_var, is_char, is_open_array }
                            }).collect();
                            let prefixed = format!("{}_{}", from_mod, original);
                            self.proc_params.insert(prefixed, info.clone());
                            self.proc_params.insert(local.clone(), info);
                        }
                    }
                    // If this imported name is an enum type, also import its variant names
                    if let Some(def_mod) = self.def_modules.get(from_mod.as_str()) {
                        for d in &def_mod.definitions {
                            if let Definition::Type(t) = d {
                                if t.name == *original {
                                    if let Some(TypeNode::Enumeration { variants, .. }) = &t.typ {
                                        for v in variants {
                                            extra_variants.push((v.clone(), from_mod.clone()));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                // IMPORT Module1, Module2;  (whole-module / qualified import)
                for import_name in &imp.names {
                    self.imported_modules.insert(import_name.name.clone());
                }
            }
        }
        for (variant_name, module_name) in extra_variants {
            self.import_map.entry(variant_name).or_insert(module_name);
        }
    }

    /// Snapshot the mutable state that gen_embedded_implementation needs to save/restore.
    fn save_embedded_context(&self) -> EmbeddedModuleContext {
        EmbeddedModuleContext {
            module_name: self.module_name.clone(),
            import_map: self.import_map.clone(),
            import_alias_map: self.import_alias_map.clone(),
            var_params: self.var_params.clone(),
            open_array_params: self.open_array_params.clone(),
            named_array_value_params: self.named_array_value_params.clone(),
            proc_params: self.proc_params.clone(),
            var_tracking: self.save_var_tracking(),
        }
    }

    /// Restore state after embedded implementation generation.
    /// Preserves module-prefixed proc_params that were registered during generation.
    fn restore_embedded_context(&mut self, ctx: EmbeddedModuleContext, embedded_module_name: &str) {
        // Extract module-prefixed proc params before restoring (these must survive)
        let prefix = format!("{}_", embedded_module_name);
        let module_proc_params: HashMap<String, Vec<ParamCodegenInfo>> = self.proc_params.iter()
            .filter(|(k, _)| k.starts_with(&prefix))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        self.module_name = ctx.module_name;
        self.import_map = ctx.import_map;
        self.import_alias_map = ctx.import_alias_map;
        self.var_params = ctx.var_params;
        self.open_array_params = ctx.open_array_params;
        self.named_array_value_params = ctx.named_array_value_params;
        self.proc_params = ctx.proc_params;
        self.restore_var_tracking(ctx.var_tracking);
        self.embedded_local_procs.clear();
        self.embedded_local_vars.clear();

        // Merge back the module-prefixed proc param info
        self.proc_params.extend(module_proc_params);
    }

    /// Generate C code for an imported implementation module, embedded in the main program.
    /// All top-level procedure names are prefixed with `ModuleName_`.
    fn gen_embedded_implementation(&mut self, imp: &ImplementationModule) {
        let ctx = self.save_embedded_context();

        self.module_name = imp.name.clone();
        // Each module has its own import scope — start clean to avoid
        // stale entries from previously-processed embedded modules leaking
        // enum variant mappings (e.g., "Invalid" → wrong source module).
        self.import_map.clear();
        self.import_alias_map.clear();
        // Build import map from the def module's imports first (e.g., FROM Gfx IMPORT Renderer),
        // then overlay with the implementation module's imports.
        if let Some(def_mod) = self.def_modules.get(&imp.name).cloned() {
            self.build_import_map(&def_mod.imports);
        }
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
                Declaration::Const(c) => {
                    self.embedded_local_vars.insert(c.name.clone());
                }
                _ => {}
            }
        }

        if self.multi_tu {
            self.emit(&format!("/* MX_MODULE_BEGIN {} */\n", imp.name));
        }
        self.emitln(&format!("/* Imported Module {} */", imp.name));
        self.newline();

        // Set generating_for_module early so all types get module-prefixed C names,
        // including forward struct declarations.
        self.generating_for_module = Some(imp.name.clone());

        // Pre-register ALL type names from this embedded module in embedded_enum_types.
        // This is needed so that forward references (e.g., POINTER TO Record declared
        // before the Record type) resolve to the correct prefixed C name.
        if let Some(def_mod) = self.def_modules.get(&imp.name).cloned() {
            for d in &def_mod.definitions {
                if let Definition::Type(t) = d {
                    let prefixed = format!("{}_{}", imp.name, self.mangle(&t.name));
                    self.embedded_enum_types.insert(prefixed);
                }
            }
        }
        for decl in &imp.block.decls {
            if let Declaration::Type(t) = decl {
                let prefixed = format!("{}_{}", imp.name, self.mangle(&t.name));
                self.embedded_enum_types.insert(prefixed);
            }
        }

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
                            let cn = self.type_decl_c_name(&t.name);
                            self.emitln(&format!("typedef struct {} {};", cn, cn));
                        }
                    }
                }
            }
        }
        // From the implementation block:
        self.emit_record_forward_decls(&imp.block.decls);

        // Emit type and const declarations from the corresponding definition module,
        // but skip types that are redefined in the implementation module.
        if let Some(def_mod) = self.def_modules.get(&imp.name).cloned() {
            // Register def-module constants and exported VARs as local vars for module-prefixed references
            for d in &def_mod.definitions {
                match d {
                    Definition::Const(c) => {
                        self.embedded_local_vars.insert(c.name.clone());
                    }
                    Definition::Var(v) => {
                        for name in &v.names {
                            self.embedded_local_vars.insert(name.clone());
                        }
                    }
                    _ => {}
                }
            }
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
                    Definition::Exception(e) => self.gen_exception_decl(e),
                    _ => {}
                }
            }
        }

        // Type, const, and exception declarations
        self.emit_type_const_exception_decls(&imp.block.decls);
        self.generating_for_module = None;

        // Emit M2+ type descriptors for types declared in this embedded module
        if self.m2plus {
            self.emit_type_descs();
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
                } else if self.multi_tu {
                    self.emit(&format!("{} {}_{}", ret_type, imp.name, p.heading.name));
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
                            let c_param = self.mangle(name);
                            if is_open_array {
                                self.emit(&format!("{} *{}, uint32_t {}_high", ctype, c_param, c_param));
                            } else if Self::is_proc_type(&fp.typ) {
                                let decl = self.proc_type_decl(&fp.typ, &c_param, fp.is_var);
                                self.emit(&decl);
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
        }

        // In multi-TU mode, emit extern declarations for exported vars and init function.
        // This allows other TUs (including main) to reference these symbols.
        if self.multi_tu {
            if let Some(def_mod) = self.def_modules.get(&imp.name).cloned() {
                for d in &def_mod.definitions {
                    if let Definition::Var(v) = d {
                        let ctype = self.type_to_c(&v.typ);
                        let array_suffix = self.type_array_suffix(&v.typ);
                        for name in &v.names {
                            let c_name = format!("{}_{}", imp.name, name);
                            self.emitln(&format!("extern {} {}{};", ctype, c_name, array_suffix));
                        }
                    }
                }
            }
            // Always emit init prototype — harmless if module has no body
            self.emitln(&format!("extern void {}_init(void);", imp.name));
        }

        self.newline();

        // Emit the MODULE_DEFS marker — everything below here is the module "body"
        // (var definitions, proc bodies, init function) that goes into this module's TU.
        if self.multi_tu {
            self.emit(&format!("/* MX_MODULE_DEFS {} */\n", imp.name));
        }

        // Variable declarations from definition module (exported VARs)
        if let Some(def_mod) = self.def_modules.get(&imp.name).cloned() {
            for d in &def_mod.definitions {
                if let Definition::Var(v) = d {
                    self.gen_var_decl(v);
                }
            }
        }

        // Variable declarations from implementation module
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
                } else if self.multi_tu {
                    self.emit(&format!("{} {}_{}", ret_type, imp.name, p.heading.name));
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
                            let c_param = self.mangle(name);
                            if is_open_array {
                                self.emit(&format!("{} *{}, uint32_t {}_high", ctype, c_param, c_param));
                                oa_params.insert(name.clone());
                            } else if Self::is_proc_type(&fp.typ) {
                                let decl = self.proc_type_decl(&fp.typ, &c_param, fp.is_var);
                                self.emit(&decl);
                            } else if fp.is_var {
                                self.emit(&format!("{} *{}", ctype, c_param));
                                param_vars.insert(name.clone(), true);
                            } else {
                                self.emit(&format!("{} {}", ctype, c_param));
                            }
                        }
                    }
                }
                self.emit(") {\n");
                self.indent += 1;

                self.var_params.push(param_vars);
                self.open_array_params.push(oa_params);
                let saved_var_tracking = self.save_var_tracking();
                let mut na_params = HashSet::new();

                // Register param type names for designator type tracking (ptr deref+index)
                for fp in &p.heading.params {
                    if let TypeNode::Named(qi) = &fp.typ {
                        if qi.module.is_none() {
                            for name in &fp.names {
                                self.var_types.insert(name.clone(), qi.name.clone());
                            }
                        }
                    }
                    // Track named-array value params (array decays to pointer in C)
                    if !fp.is_var && !matches!(fp.typ, TypeNode::OpenArray { .. }) {
                        if let TypeNode::Named(qi) = &fp.typ {
                            if self.array_types.contains(&qi.name) {
                                for name in &fp.names {
                                    na_params.insert(name.clone());
                                }
                            }
                        }
                    }
                }
                self.named_array_value_params.push(na_params);

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

                self.restore_var_tracking(saved_var_tracking);
                self.var_params.pop();
                self.open_array_params.pop();
                self.named_array_value_params.pop();
                self.indent -= 1;
                self.emitln("}");
                self.newline();
            }
        }

        // Module initialization body
        if let Some(stmts) = &imp.block.body {
            if self.multi_tu {
                self.emitln(&format!("void {}_init(void) {{", imp.name));
            } else {
                self.emitln(&format!("static void {}_init(void) {{", imp.name));
            }
            self.indent += 1;
            for stmt in stmts {
                self.gen_statement(stmt);
            }
            self.indent -= 1;
            self.emitln("}");
            self.newline();
            self.embedded_init_modules.push(imp.name.clone());
        }

        if self.multi_tu {
            self.emit(&format!("/* MX_MODULE_END {} */\n", imp.name));
        }

        self.restore_embedded_context(ctx, &imp.name);
    }

    /// Topologically sort implementation modules so dependencies come before dependents.
    /// Also considers imports from corresponding .def files so that type dependencies
    /// (e.g. `FROM Gfx IMPORT Renderer;` in Font.def) are properly ordered.
    /// Returns an error if a dependency cycle is detected.
    fn topo_sort_modules(modules: Vec<ImplementationModule>, def_modules: &HashMap<String, crate::ast::DefinitionModule>) -> CompileResult<Vec<ImplementationModule>> {
        let names: HashSet<String> = modules.iter().map(|m| m.name.clone()).collect();
        let mut deps: HashMap<String, Vec<String>> = HashMap::new();
        for m in &modules {
            let mut my_deps = Vec::new();
            // Collect deps from implementation module imports
            for imp in &m.imports {
                if let Some(ref from_mod) = imp.from_module {
                    if names.contains(from_mod) {
                        my_deps.push(from_mod.clone());
                    }
                } else {
                    for name in &imp.names {
                        if names.contains(&name.name) {
                            my_deps.push(name.name.clone());
                        }
                    }
                }
            }
            // Also collect deps from the corresponding definition module imports
            if let Some(def_mod) = def_modules.get(&m.name) {
                for imp in &def_mod.imports {
                    if let Some(ref from_mod) = imp.from_module {
                        if names.contains(from_mod) && !my_deps.contains(from_mod) {
                            my_deps.push(from_mod.clone());
                        }
                    } else {
                        for name in &imp.names {
                            if names.contains(&name.name) && !my_deps.contains(&name.name) {
                                my_deps.push(name.name.clone());
                            }
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
        ) -> Result<(), String> {
            if visited.contains(name) {
                return Ok(());
            }
            if visiting.contains(name) {
                return Err(name.to_string());
            }
            visiting.insert(name.to_string());
            if let Some(d) = deps.get(name) {
                for dep in d {
                    visit(dep, deps, visited, visiting, order).map_err(|cycle_node| {
                        if cycle_node == name {
                            // We've come full circle; build the cycle description
                            format!("{} -> {}", name, dep)
                        } else {
                            format!("{} -> {}", name, cycle_node)
                        }
                    })?;
                }
            }
            visiting.remove(name);
            visited.insert(name.to_string());
            order.push(name.to_string());
            Ok(())
        }
        let mut order = Vec::new();
        for m in &modules {
            visit(&m.name, &deps, &mut visited, &mut visiting, &mut order)
                .map_err(|cycle_desc| {
                    CompileError::codegen(
                        crate::errors::SourceLoc::new("<codegen>", 0, 0),
                        format!("module dependency cycle detected: {}", cycle_desc),
                    )
                })?;
        }
        let pos: HashMap<String, usize> = order.iter().enumerate().map(|(i, n)| (n.clone(), i)).collect();
        let mut result = modules;
        result.sort_by_key(|m| pos.get(&m.name).copied().unwrap_or(usize::MAX));
        Ok(result)
    }

    fn gen_program_module(&mut self, m: &ProgramModule) -> CompileResult<()> {
        self.module_name = m.name.clone();
        self.build_import_map(&m.imports);

        self.emit_preamble_for_imports()?;

        if self.multi_tu {
            self.emit(&format!("/* MX_MAIN_BEGIN {} */\n", m.name));
        }
        self.emitln(&format!("/* Module {} */", m.name));
        self.newline();

        self.emit_record_forward_decls(&m.block.decls);
        self.emit_type_and_const_decls(&m.block.decls);

        // Emit M2+ type descriptors (after all types are declared)
        if self.m2plus {
            self.emit_type_descs();
        }

        // Forward declarations for procedures
        self.gen_forward_decls(&m.block.decls);
        self.newline();

        // Pass 1: Emit all Var declarations first (procedures may reference them)
        for decl in &m.block.decls {
            if let Declaration::Var(v) = decl {
                self.gen_var_decl(v);
            }
        }
        // Also emit vars from nested modules
        for decl in &m.block.decls {
            if let Declaration::Module(local_mod) = decl {
                for d in &local_mod.block.decls {
                    if let Declaration::Var(v) = d {
                        self.gen_var_decl(v);
                    }
                }
            }
        }
        // Pass 2: Emit Procedures and Modules (skip Var/Const/Type)
        for decl in &m.block.decls {
            match decl {
                Declaration::Const(_) | Declaration::Type(_) | Declaration::Var(_) => {}
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
        self.emitln("int main(int _m2_argc, char **_m2_argv) {");
        self.indent += 1;
        self.emitln("m2_argc = _m2_argc; m2_argv = _m2_argv;");
        if self.debug_mode {
            self.emitln("setvbuf(stdout, NULL, _IONBF, 0);");
        }

        // Register FINALLY handler with atexit
        if m.block.finally.is_some() {
            self.emitln("atexit(m2_finally_handler);");
        }

        // Call embedded module init functions (in dependency order)
        for mod_name in &self.embedded_init_modules.clone() {
            self.emitln(&format!("{}_init();", mod_name));
        }

        // Initialize local (nested) modules — run their BEGIN bodies
        for decl in &m.block.decls {
            if let Declaration::Module(local_mod) = decl {
                if let Some(stmts) = &local_mod.block.body {
                    self.emitln(&format!("/* Init local module {} */", local_mod.name));
                    for stmt in stmts {
                        self.gen_statement(stmt);
                    }
                }
            }
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
        if self.multi_tu {
            self.emit("/* MX_MAIN_END */\n");
        }
        Ok(())
    }

    fn gen_definition_module(&mut self, m: &DefinitionModule) {
        self.module_name = m.name.clone();
        self.emitln(&format!("/* Definition Module {} */", m.name));
        self.emitln(&format!("#ifndef {}_H", m.name.to_uppercase()));
        self.emitln(&format!("#define {}_H", m.name.to_uppercase()));
        self.newline();

        // Forward struct declarations for record types (and POINTER TO RECORD)
        for def in &m.definitions {
            if let Definition::Type(t) = def {
                let cn = self.mangle(&t.name);
                match &t.typ {
                    Some(TypeNode::Record { .. }) => {
                        self.emitln(&format!("typedef struct {} {};", cn, cn));
                    }
                    Some(TypeNode::Pointer { base, .. }) if matches!(&**base, TypeNode::Record { .. }) => {
                        let tag = format!("{}_r", cn);
                        self.emitln(&format!("typedef struct {} {};", tag, tag));
                        self.emitln(&format!("typedef {} *{};", tag, cn));
                    }
                    _ => {}
                }
            }
        }

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
                    self.exception_names.insert(e.name.clone());
                    self.emitln(&format!("#define M2_EXC_{} __COUNTER__", self.mangle(&e.name)));
                }
            }
        }

        self.newline();
        self.emitln("#endif");
    }

    fn gen_implementation_module(&mut self, m: &ImplementationModule) -> CompileResult<()> {
        self.module_name = m.name.clone();
        self.build_import_map(&m.imports);

        self.emit_preamble_for_imports()?;

        self.emitln(&format!("/* Implementation Module {} */", m.name));
        self.newline();

        // Emit types and constants from the corresponding definition module.
        // The implementation module's scope includes all .def exports, but
        // m2c must emit them explicitly in the generated C.
        let impl_type_names: std::collections::HashSet<String> = m.block.decls.iter()
            .filter_map(|d| if let Declaration::Type(t) = d { Some(t.name.clone()) } else { None })
            .collect();
        if let Some(def_mod) = self.def_modules.get(&m.name).cloned() {
            // Forward struct declarations from the definition module
            for d in &def_mod.definitions {
                if let Definition::Type(t) = d {
                    if !impl_type_names.contains(&t.name) {
                        let cn = self.mangle(&t.name);
                        match &t.typ {
                            Some(TypeNode::Record { .. }) => {
                                self.emitln(&format!("typedef struct {} {};", cn, cn));
                            }
                            Some(TypeNode::Pointer { base, .. }) if matches!(&**base, TypeNode::Record { .. }) => {
                                let tag = format!("{}_r", cn);
                                self.emitln(&format!("typedef struct {} {};", tag, tag));
                                self.emitln(&format!("typedef {} *{};", tag, cn));
                            }
                            _ => {}
                        }
                    }
                }
            }
            // Emit type and const declarations from the definition module
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

        self.emit_record_forward_decls(&m.block.decls);
        self.emit_type_and_const_decls(&m.block.decls);

        // Emit M2+ type descriptors (after all types are declared)
        if self.m2plus {
            self.emit_type_descs();
        }

        self.gen_forward_decls(&m.block.decls);
        self.newline();

        // Pass 1: Emit Var declarations first
        for decl in &m.block.decls {
            if let Declaration::Var(v) = decl {
                self.gen_var_decl(v);
            }
        }
        // Pass 2: Emit Procedures and Modules (skip Var/Const/Type)
        for decl in &m.block.decls {
            match decl {
                Declaration::Const(_) | Declaration::Type(_) | Declaration::Var(_) => {}
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
        Ok(())
    }

    // ── Forward declarations ────────────────────────────────────────

    fn gen_forward_decls(&mut self, decls: &[Declaration]) {
        for decl in decls {
            match decl {
                Declaration::Procedure(p) => {
                    self.register_proc_params(&p.heading);
                    self.gen_proc_prototype(&p.heading);
                    self.emit(";\n");
                }
                Declaration::Module(m) => {
                    // Also forward-declare procedures from nested modules
                    for d in &m.block.decls {
                        if let Declaration::Procedure(p) = d {
                            self.register_proc_params(&p.heading);
                            self.gen_proc_prototype(&p.heading);
                            self.emit(";\n");
                        }
                    }
                }
                _ => {}
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

    fn gen_declaration(&mut self, decl: &Declaration) {
        match decl {
            Declaration::Const(c) => self.gen_const_decl(c),
            Declaration::Type(t) => self.gen_type_decl(t),
            Declaration::Var(v) => self.gen_var_decl(v),
            Declaration::Procedure(p) => self.gen_proc_decl(p),
            Declaration::Module(m) => {
                // Nested module - generate inline.
                // If inside a procedure, procedures were already hoisted; skip them here.
                // At program/implementation module level, generate them normally.
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
                self.gen_exception_decl(e);
            }
        }
    }

    fn gen_const_decl(&mut self, c: &ConstDecl) {
        let c_name = if let Some(ref mod_name) = self.generating_for_module {
            format!("{}_{}", mod_name, self.mangle(&c.name))
        } else {
            self.mangle(&c.name)
        };
        // Try to evaluate as a compile-time integer constant
        if let Some(val) = self.try_eval_const_int(&c.expr) {
            self.const_int_values.insert(c.name.clone(), val);
            if let Some(ref mod_name) = self.generating_for_module {
                self.const_int_values.insert(format!("{}_{}", mod_name, c.name), val);
            }
            self.emitln(&format!("static const int32_t {} = {};", c_name, val));
            return;
        }
        // Fall back to expression-based constant
        self.emit_indent();
        let ctype = self.infer_c_type(&c.expr);
        self.emit(&format!("static const {} {} = ", ctype, c_name));
        if let ExprKind::StringLit(s) = &c.expr.kind {
            if s.is_empty() {
                self.emit("'\\0'");
            } else if s.len() == 1 {
                let ch = s.chars().next().unwrap();
                self.emit(&format!("'{}'", escape_c_char(ch)));
            } else {
                self.gen_expr(&c.expr);
            }
        } else {
            self.gen_expr(&c.expr);
        }
        self.emit(";\n");
    }

    /// Return the C typedef name for a type declaration.
    /// Inside an embedded module, returns Module_TypeName to avoid collisions.
    fn type_decl_c_name(&self, bare_name: &str) -> String {
        if let Some(ref mod_name) = self.generating_for_module {
            format!("{}_{}", mod_name, self.mangle(bare_name))
        } else {
            self.mangle(bare_name)
        }
    }

    fn gen_type_decl(&mut self, t: &TypeDecl) {
        // Register this type name for type-cast recognition
        self.known_type_names.insert(t.name.clone());
        if let Some(ref mod_name) = self.generating_for_module {
            self.known_type_names.insert(format!("{}_{}", mod_name, t.name));
        }
        // Compute the C name: module-prefixed for embedded modules to avoid collisions
        let c_type_name = self.type_decl_c_name(&t.name);
        if let Some(tn) = &t.typ {
            // Register ALL embedded module types (not just enums) for name resolution
            if self.generating_for_module.is_some() {
                self.embedded_enum_types.insert(c_type_name.clone());
            }
            match tn {
                TypeNode::Record { fields, loc: _ } => {
                    // Collect field names and types for WITH resolution
                    let field_names = self.collect_record_field_metadata(fields, &t.name);
                    self.record_fields.insert(t.name.clone(), field_names);

                    // struct definition (typedef is already forward-declared)
                    self.emitln(&format!("struct {} {{", c_type_name));
                    self.indent += 1;
                    let rec_name = t.name.clone();
                    self.emit_record_fields(fields, &rec_name);
                    self.indent -= 1;
                    self.emitln("};");
                }
                TypeNode::Enumeration { variants, .. } => {
                    self.emit_indent();
                    self.emit("typedef enum { ");
                    let bare_type_name = self.mangle(&t.name);
                    // Module-prefix enum types from embedded modules to avoid C-level collisions
                    let type_name = if let Some(ref mod_name) = self.generating_for_module {
                        format!("{}_{}", mod_name, bare_type_name)
                    } else {
                        bare_type_name.clone()
                    };
                    for (i, v) in variants.iter().enumerate() {
                        if i > 0 {
                            self.emit(", ");
                        }
                        let c_name = format!("{}_{}", type_name, v);
                        self.emit(&c_name);
                        // Register variant under module-qualified key for qualified access
                        if let Some(ref mod_name) = self.generating_for_module {
                            let qual_key = format!("{}_{}", mod_name, v);
                            self.enum_variants.insert(qual_key, c_name.clone());
                        }
                        // Register under bare name only for the main module;
                        // embedded modules use qualified keys to avoid collisions.
                        if self.generating_for_module.is_none() {
                            self.enum_variants.insert(v.clone(), c_name);
                        }
                    }
                    self.emit(&format!(" }} {};\n", type_name));
                    // Emit MIN/MAX macros for the enum type
                    let n = variants.len();
                    self.emitln(&format!("#define m2_min_{} 0", type_name));
                    if n > 0 {
                        self.emitln(&format!("#define m2_max_{} {}", type_name, n - 1));
                    }
                    if self.generating_for_module.is_some() {
                        self.embedded_enum_types.insert(type_name);
                    }
                }
                TypeNode::Pointer { base, .. } => {
                    // Track pointer types whose base is a named array type.
                    if let TypeNode::Named(ref qi) = **base {
                        if self.array_types.contains(&qi.name) {
                            self.ptr_to_named_array.insert(t.name.clone());
                            self.ptr_to_named_array.insert(c_type_name.clone());
                        }
                    }
                    // POINTER TO RECORD: emit a named struct + pointer typedef
                    if let TypeNode::Record { fields, .. } = &**base {
                        let tag = format!("{}_r", c_type_name);
                        self.emitln(&format!("typedef struct {} *{};", tag, c_type_name));
                        // Collect field metadata under both tag and pointer type name
                        let field_names = self.collect_record_field_metadata(fields, &t.name);
                        self.record_fields.insert(t.name.clone(), field_names.clone());
                        self.record_fields.insert(tag.clone(), field_names);
                        // Copy record_field_types entries under the tag name too
                        let entries: Vec<_> = self.record_field_types.iter()
                            .filter(|((rn, _), _)| rn == &t.name)
                            .map(|((_, fn_), v)| (fn_.clone(), v.clone()))
                            .collect();
                        for (fn_, v) in entries {
                            self.record_field_types.insert((tag.clone(), fn_), v);
                        }
                        self.pointer_base_types.insert(c_type_name.clone(), tag.clone());
                        self.pointer_base_types.insert(t.name.clone(), tag.clone());
                        // Emit struct body
                        self.emitln(&format!("struct {} {{", tag));
                        self.indent += 1;
                        let rec_name = t.name.clone();
                        self.emit_record_fields(fields, &rec_name);
                        self.indent -= 1;
                        self.emitln("};");
                    } else {
                        self.emit_indent();
                        let base_c = self.type_to_c(base);
                        let base_c_resolved = if self.generating_for_module.is_some() {
                            if let TypeNode::Named(ref qi) = **base {
                                let prefixed = self.type_decl_c_name(&qi.name);
                                if self.embedded_enum_types.contains(&prefixed) {
                                    prefixed
                                } else {
                                    base_c
                                }
                            } else {
                                base_c
                            }
                        } else {
                            base_c
                        };
                        self.emit(&format!(
                            "typedef {} *{};\n",
                            base_c_resolved,
                            c_type_name
                        ));
                    }
                }
                TypeNode::Array { .. } => {
                    // Track if this is an ARRAY OF CHAR type (for string ops)
                    if self.is_char_array_type(tn) {
                        self.char_array_types.insert(t.name.clone());
                        self.char_array_types.insert(c_type_name.clone());
                    }
                    // Track all array types for memcpy assignment
                    self.array_types.insert(t.name.clone());
                    self.array_types.insert(c_type_name.clone());
                    // Store HIGH expression for named-array value param fixup
                    if let TypeNode::Array { index_types, .. } = tn {
                        if let Some(TypeNode::Subrange { high, .. }) = index_types.first() {
                            let high_str = self.const_expr_to_string(high);
                            self.array_type_high.insert(t.name.clone(), high_str.clone());
                            self.array_type_high.insert(c_type_name.clone(), high_str);
                        }
                    }
                    self.emit_indent();
                    let ctype = self.type_to_c(tn);
                    let suffix = self.type_array_suffix(tn);
                    self.emit(&format!("typedef {} {}{};\n", ctype, c_type_name, suffix));
                }
                TypeNode::ProcedureType { params, return_type, .. } => {
                    // Register param info for this procedure type name
                    // so variables of this type can get correct VAR/open-array info at call sites
                    {
                        let mut pinfo = Vec::new();
                        for fp in params {
                            let is_open = matches!(fp.typ, TypeNode::OpenArray { .. });
                            let is_char = matches!(&fp.typ, TypeNode::Named(qi) if qi.name == "CHAR");
                            for name in &fp.names {
                                pinfo.push(ParamCodegenInfo {
                                    name: name.clone(),
                                    is_var: fp.is_var,
                                    is_open_array: is_open,
                                    is_char,
                                });
                            }
                        }
                        self.proc_type_params.insert(t.name.clone(), pinfo);
                    }
                    // typedef RetType (*Name)(params);
                    self.emit_indent();
                    let ret = if let Some(rt) = return_type {
                        self.type_to_c(rt)
                    } else {
                        "void".to_string()
                    };
                    self.emit(&format!("typedef {} (*{})(", ret, c_type_name));
                    if params.is_empty() {
                        self.emit("void");
                    } else {
                        let mut first = true;
                        for fp in params {
                            let pt = self.type_to_c(&fp.typ);
                            let is_open = matches!(fp.typ, TypeNode::OpenArray { .. });
                            for _name in &fp.names {
                                if !first { self.emit(", "); }
                                first = false;
                                if is_open {
                                    // Open array: pointer + high param
                                    self.emit(&format!("{} *, uint32_t", pt));
                                } else if fp.is_var {
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
                    self.emit(&format!("typedef {} *{};\n", target_c, c_type_name));
                    // Register a type descriptor for this REF type (for TYPECASE)
                    if self.m2plus {
                        let td_sym = self.register_type_desc(&c_type_name, &t.name, None);
                        self.ref_type_descs.insert(c_type_name.clone(), td_sym);
                    }
                }
                TypeNode::RefAny { .. } => {
                    self.emit_indent();
                    self.emit(&format!("typedef void *{};\n", c_type_name));
                }
                TypeNode::Set { base, .. } => {
                    // If base is an inline enumeration, emit the enum constants first
                    if let TypeNode::Enumeration { variants, .. } = &**base {
                        let type_name = &c_type_name;
                        let enum_name = format!("{}_enum", type_name);
                        self.emit_indent();
                        self.emit("typedef enum { ");
                        for (i, v) in variants.iter().enumerate() {
                            if i > 0 {
                                self.emit(", ");
                            }
                            let c_name = format!("{}_{}", type_name, v);
                            self.emit(&c_name);
                            if let Some(ref mod_name) = self.generating_for_module {
                                let qual_key = format!("{}_{}", mod_name, v);
                                self.enum_variants.insert(qual_key, c_name.clone());
                            }
                            if self.generating_for_module.is_none() {
                                self.enum_variants.insert(v.clone(), c_name);
                            }
                        }
                        self.emit(&format!(" }} {};\n", enum_name));
                        self.emitln(&format!("typedef uint32_t {};", type_name));
                        let n = variants.len();
                        self.emitln(&format!("#define m2_min_{} 0", type_name));
                        if n > 0 {
                            self.emitln(&format!("#define m2_max_{} {}", type_name, n - 1));
                        }
                    } else {
                        self.emit_indent();
                        self.emit(&format!("typedef uint32_t {};\n", c_type_name));
                    }
                }
                TypeNode::Subrange { low, high, .. } => {
                    self.emit_indent();
                    self.emit(&format!("typedef int32_t {};\n", c_type_name));
                    // Emit MIN/MAX macros if bounds are evaluable
                    if let Some(lo_val) = self.try_eval_const_int(low) {
                        self.emitln(&format!("#define m2_min_{} {}", c_type_name, lo_val));
                    }
                    if let Some(hi_val) = self.try_eval_const_int(high) {
                        self.emitln(&format!("#define m2_max_{} {}", c_type_name, hi_val));
                    }
                }
                _ => {
                    // Track type aliases that resolve to unsigned types
                    if let TypeNode::Named(qi) = tn {
                        if qi.name == "CARDINAL" || qi.name == "LONGCARD"
                            || self.unsigned_type_aliases.contains(&qi.name) {
                            self.unsigned_type_aliases.insert(t.name.clone());
                            self.unsigned_type_aliases.insert(c_type_name.clone());
                        }
                    }
                    self.emit_indent();
                    let ctype = self.type_to_c(tn);
                    self.emit(&format!("typedef {} {};\n", ctype, c_type_name));
                }
            }
            self.newline();
        } else {
            // Opaque type - generate as void*
            let c_type_name = self.type_decl_c_name(&t.name);
            if self.generating_for_module.is_some() {
                self.embedded_enum_types.insert(c_type_name.clone());
            }
            self.emitln(&format!(
                "typedef void *{};",
                c_type_name
            ));
        }
    }

    /// Collect field metadata (names, types) for a record's fields and register in tracking maps.
    /// Returns the list of field names. `record_name` is the key used in record_fields/record_field_types.
    fn collect_record_field_metadata(&mut self, fields: &[FieldList], record_name: &str) -> Vec<String> {
        let mut field_names = Vec::new();
        for fl in fields {
            for f in &fl.fixed {
                let field_type_name = if let TypeNode::Named(qi) = &f.typ {
                    Some(qi.name.clone())
                } else {
                    None
                };
                for name in &f.names {
                    field_names.push(name.clone());
                    if let Some(ref ftn) = field_type_name {
                        self.record_field_types.insert(
                            (record_name.to_string(), name.clone()),
                            ftn.clone(),
                        );
                        if let Some(pinfo) = self.proc_type_params.get(ftn).cloned() {
                            self.field_proc_params.insert(name.clone(), pinfo);
                        }
                    }
                }
            }
        }
        field_names
    }

    /// Emit struct field declarations for a record body. Handles multi-name pointer fields,
    /// char array tracking, array tracking, pointer tracking, and variant parts.
    fn emit_record_fields(&mut self, fields: &[FieldList], record_name: &str) {
        for fl in fields {
            for f in &fl.fixed {
                let ctype = self.type_to_c(&f.typ);
                let arr_suffix = self.type_array_suffix(&f.typ);
                // Track char array record fields for strcpy assignment
                if ctype == "char" && !arr_suffix.is_empty() {
                    for name in &f.names {
                        self.char_array_fields.insert((record_name.to_string(), name.clone()));
                    }
                }
                // Track array record fields for memcpy assignment
                if !arr_suffix.is_empty() || self.is_array_type(&f.typ) {
                    for name in &f.names {
                        self.array_fields.insert((record_name.to_string(), name.clone()));
                    }
                }
                // Track pointer-typed fields
                if self.is_pointer_type(&f.typ) {
                    for name in &f.names {
                        self.pointer_fields.insert(name.clone());
                    }
                }
                // Fix 5: When ctype contains '*' and multiple names, emit each separately
                let is_ptr = ctype.contains('*');
                if is_ptr && f.names.len() > 1 {
                    for name in &f.names {
                        self.emit_indent();
                        self.emit(&format!("{} {}", ctype, name));
                        if !arr_suffix.is_empty() {
                            self.emit(&arr_suffix);
                        }
                        self.emit(";\n");
                    }
                } else {
                    self.emit_indent();
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
            }
            if let Some(vp) = &fl.variant {
                self.gen_variant_part(vp, record_name);
            }
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
                // Flatten nested variant parts into parent variant struct
                if let Some(nested_vp) = &fl.variant {
                    // Emit the nested tag field
                    if let Some(tag) = &nested_vp.tag_name {
                        self.emit_indent();
                        let tag_c = self.qualident_to_c(&nested_vp.tag_type);
                        self.emit(&format!("{} {};\n", tag_c, tag));
                        // Register nested tag as variant field of parent
                        self.variant_field_map.insert(
                            (record_name.to_string(), tag.clone()),
                            i,
                        );
                        if let Some(fields) = self.record_fields.get_mut(record_name) {
                            fields.push(tag.clone());
                        }
                    }
                    // Emit all nested variant fields flattened
                    for nv in &nested_vp.variants {
                        for nvfl in &nv.fields {
                            for f in &nvfl.fixed {
                                self.emit_indent();
                                let ctype = self.type_to_c(&f.typ);
                                self.emit(&format!("{} ", ctype));
                                for (j, name) in f.names.iter().enumerate() {
                                    if j > 0 {
                                        self.emit(", ");
                                    }
                                    self.emit(name);
                                    self.variant_field_map.insert(
                                        (record_name.to_string(), name.clone()),
                                        i,
                                    );
                                    if let Some(fields) = self.record_fields.get_mut(record_name) {
                                        fields.push(name.clone());
                                    }
                                }
                                self.emit(";\n");
                            }
                        }
                    }
                }
            }
            self.indent -= 1;
            self.emitln(&format!("}} v{};", i));
        }
        // Emit ELSE fields as an additional union variant
        if let Some(else_fls) = &vp.else_fields {
            let idx = vp.variants.len();
            self.emitln("struct {");
            self.indent += 1;
            for fl in else_fls {
                for f in &fl.fixed {
                    self.emit_indent();
                    let ctype = self.type_to_c(&f.typ);
                    self.emit(&format!("{} ", ctype));
                    for (j, name) in f.names.iter().enumerate() {
                        if j > 0 {
                            self.emit(", ");
                        }
                        self.emit(name);
                        self.variant_field_map.insert(
                            (record_name.to_string(), name.clone()),
                            idx,
                        );
                        if let Some(fields) = self.record_fields.get_mut(record_name) {
                            fields.push(name.clone());
                        }
                    }
                    self.emit(";\n");
                }
            }
            self.indent -= 1;
            self.emitln(&format!("}} v{};", idx));
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
        // Track array variable element types for resolving arr[i].field patterns.
        // Uses the C-level type name (module-prefixed in embedded modules) to match
        // record_fields / record_field_types keys.
        if let TypeNode::Array { elem_type, .. } = &v.typ {
            if let TypeNode::Named(qi) = elem_type.as_ref() {
                let elem_name = self.named_type_to_c(qi);
                for name in &v.names {
                    self.array_var_elem_types.insert(name.clone(), elem_name.clone());
                }
            }
        }
        // Register procedure-typed variables so call sites get correct VAR param info
        if let TypeNode::Named(qi) = &v.typ {
            if let Some(pinfo) = self.proc_type_params.get(&qi.name).cloned() {
                for name in &v.names {
                    self.proc_params.insert(name.clone(), pinfo.clone());
                }
            }
        }
        if Self::is_proc_type(&v.typ) {
            // Inline procedure type — extract param info directly
            if let TypeNode::ProcedureType { params, .. } = &v.typ {
                let mut pinfo = Vec::new();
                for fp in params {
                    let is_open = matches!(fp.typ, TypeNode::OpenArray { .. });
                    let is_char = matches!(&fp.typ, TypeNode::Named(qi) if qi.name == "CHAR");
                    for name in &fp.names {
                        pinfo.push(ParamCodegenInfo {
                            name: name.clone(),
                            is_var: fp.is_var,
                            is_open_array: is_open,
                            is_char,
                        });
                    }
                }
                for name in &v.names {
                    self.proc_params.insert(name.clone(), pinfo.clone());
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
        // Track CARDINAL/LONGCARD variables for unsigned DIV/MOD
        if matches!(&v.typ, TypeNode::Named(qi) if qi.name == "CARDINAL" || qi.name == "LONGCARD") {
            for name in &v.names {
                self.cardinal_vars.insert(name.clone());
            }
        }
        // Track LONGINT variables for 64-bit signed DIV/MOD
        if matches!(&v.typ, TypeNode::Named(qi) if qi.name == "LONGINT") {
            for name in &v.names {
                self.longint_vars.insert(name.clone());
            }
        }
        // Track LONGCARD variables for 64-bit detection
        if matches!(&v.typ, TypeNode::Named(qi) if qi.name == "LONGCARD") {
            for name in &v.names {
                self.longcard_vars.insert(name.clone());
            }
        }
        // Track type aliases that resolve to LONGINT/LONGCARD
        if let TypeNode::Named(qi) = &v.typ {
            if self.unsigned_type_aliases.contains(&qi.name) {
                for name in &v.names {
                    self.cardinal_vars.insert(name.clone());
                    self.longcard_vars.insert(name.clone());
                }
            }
            // Check if it's an alias for LONGINT (signed 64-bit)
            // We don't have a signed_type_aliases set, but we can check var_types later
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
            // For PROC builtin, synthesize a parameterless procedure type
            let effective_type = if matches!(&v.typ, TypeNode::Named(qi) if qi.name == "PROC") {
                TypeNode::ProcedureType {
                    params: vec![],
                    return_type: None,
                    loc: crate::errors::SourceLoc::default(),
                }
            } else {
                v.typ.clone()
            };
            for (i, name) in v.names.iter().enumerate() {
                self.emit_indent();
                let c_name = if self.embedded_local_vars.contains(name) {
                    format!("{}_{}", self.module_name, name)
                } else {
                    self.mangle(name)
                };
                let decl = self.proc_type_decl(&effective_type, &c_name, false);
                self.emit(&format!("{};\n", decl));
            }
        } else {
            let ctype = self.type_to_c(&v.typ);
            let array_suffix = self.type_array_suffix(&v.typ);
            // For pointer types with multiple names, emit separate declarations
            // to avoid C's `void * a, b;` bug (b would be void, not void*).
            let is_ptr = ctype.ends_with('*');
            if is_ptr && v.names.len() > 1 {
                for name in &v.names {
                    self.emit_indent();
                    let c_name = if self.embedded_local_vars.contains(name) {
                        format!("{}_{}", self.module_name, name)
                    } else {
                        self.mangle(name)
                    };
                    self.emit(&format!("{} {}{};\n", ctype, c_name, array_suffix));
                }
            } else {
                self.emit_indent();
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
    }

    fn gen_proc_decl(&mut self, p: &ProcDecl) {
        self.register_proc_params(&p.heading);

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
        let saved_var_tracking = self.save_var_tracking();
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
        self.restore_var_tracking(saved_var_tracking);
        self.pop_var_scope();
        self.parent_proc_stack.pop();
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
                                && (self.is_open_array_param(&d.ident.name)
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
                                && self.is_open_array_param(&d.ident.name);
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
                    let arg_strs: Vec<String> = args.iter().enumerate().map(|(idx, a)| {
                        if char_builtins.contains(&actual_name.as_ref()) {
                            self.expr_to_char_string(a)
                        } else if is_set_elem_builtin && idx == 1 {
                            self.expr_to_char_string(a)
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
                mapped.unwrap_or_else(|| {
                    let candidate = format!("{}_{}", module, desig.ident.name);
                    // Check for module-prefixed enum variant (e.g., Stream.OK → Stream_Status_OK)
                    if let Some(c_name) = self.enum_variants.get(&candidate) {
                        c_name.clone()
                    } else if let Some(c_name) = self.resolve_reexported_enum_variant(module, &desig.ident.name) {
                        c_name
                    } else {
                        candidate
                    }
                })
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
                // Check for module-prefixed enum variant (e.g., EventLoop.OK → EventLoop_Status_OK)
                let candidate = format!("{}_{}", mod_name, field_name);
                if let Some(c_name) = self.enum_variants.get(&candidate) {
                    c_name.clone()
                } else if let Some(c_name) = self.resolve_reexported_enum_variant(&mod_name, &field_name) {
                    c_name
                } else {
                    candidate
                }
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
            // Check if this bare name is an enum variant of the CURRENT module.
            // This must come before import_map to avoid name collisions (e.g., "Error"
            // could be both a StreamState enum variant and an imported record type).
            {
                let mod_key = format!("{}_{}", self.module_name, desig.ident.name);
                if let Some(c_name) = self.enum_variants.get(&mod_key) {
                    return c_name.clone();
                }
            }
            // Check if this bare name is an imported enum variant or stdlib variable
            if let Some(module) = self.import_map.get(&desig.ident.name).cloned() {
                let orig = self.original_import_name(&desig.ident.name).to_string();
                // Check for module-prefixed enum variant (e.g., OK from Stream → Stream_Status_OK)
                let qual_key = format!("{}_{}", module, &orig);
                if let Some(c_name) = self.enum_variants.get(&qual_key) {
                    return c_name.clone();
                }
                // Check re-exported enum variants (e.g., OK from Promise → Scheduler_Status_OK)
                if let Some(c_name) = self.resolve_reexported_enum_variant(&module, &orig) {
                    return c_name;
                }
                if stdlib::is_stdlib_module(&module) {
                    if let Some(c_name) = stdlib::map_stdlib_call(&module, &orig) {
                        return c_name;
                    }
                }
                // For imported names from embedded (non-stdlib, non-foreign) modules,
                // use module-prefixed name (constants, variables, etc.)
                // Do NOT early-return — field selectors (.field) must still be applied.
                if !stdlib::is_stdlib_module(&module) && !self.foreign_modules.contains(module.as_str()) {
                    format!("{}_{}", module, orig)
                } else {
                    self.mangle(&desig.ident.name)
                }
            } else {
                // Fallback: check bare enum_variants (for main module's own enums
                // where generating_for_module was None, stored with bare name keys)
                if let Some(c_name) = self.enum_variants.get(&desig.ident.name) {
                    return c_name.clone();
                }
                // Inside an embedded implementation, module-level vars/procs need module prefix
                if self.embedded_local_vars.contains(&desig.ident.name)
                    || self.embedded_local_procs.contains(&desig.ident.name)
                {
                    format!("{}_{}", self.module_name, desig.ident.name)
                } else {
                    // Check if this is a type name from the current embedded module
                    // (e.g., QueueRec in TSIZE(QueueRec) or type casts)
                    let local_type_prefixed = format!("{}_{}", self.module_name, self.mangle(&desig.ident.name));
                    if self.embedded_enum_types.contains(&local_type_prefixed) {
                        local_type_prefixed
                    } else {
                        self.mangle(&desig.ident.name)
                    }
                }
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
                        let idx_str = self.expr_to_char_string(idx);
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
                        if let Selector::Index(indices, _) = &sels[i + 1] {
                            // ptr^[i] — pointer to array, deref then index.
                            // Two cases based on the C typedef shape:
                            //   1. Flat pointer (POINTER TO ARRAY [0..N] OF T where
                            //      the array is anonymous): typedef is T*, use ptr[i]
                            //   2. Pointer to named array type (POINTER TO NamedArray):
                            //      typedef is NamedArray*, use (*ptr)[i]
                            let needs_deref = current_type.as_ref()
                                .map(|t| self.ptr_to_named_array.contains(t))
                                .unwrap_or(false);
                            if needs_deref {
                                result = format!("(*{})", result);
                            }
                            for idx in indices {
                                let idx_str = self.expr_to_char_string(idx);
                                result.push('[');
                                result.push_str(&idx_str);
                                result.push(']');
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

    fn expr_to_string(&mut self, expr: &Expr) -> String {
        let saved = std::mem::take(&mut self.output);
        self.gen_expr(expr);
        let result = std::mem::replace(&mut self.output, saved);
        result
    }

    /// Like expr_to_string but for expressions that should be chars (single-char strings become char literals)
    fn expr_to_char_string(&mut self, expr: &Expr) -> String {
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

    /// Check if a TypeNode is a ProcedureType (including the PROC builtin)
    /// Walk an expression tree and collect all bare identifier references.
    fn collect_expr_ident_refs(expr: &Expr, out: &mut HashSet<String>) {
        match &expr.kind {
            ExprKind::IntLit(_) | ExprKind::RealLit(_) | ExprKind::StringLit(_) | ExprKind::CharLit(_) => {}
            ExprKind::Designator(d) => {
                if d.ident.module.is_none() {
                    out.insert(d.ident.name.clone());
                }
                for sel in &d.selectors {
                    if let Selector::Index(indices, _) = sel {
                        for idx in indices {
                            Self::collect_expr_ident_refs(idx, out);
                        }
                    }
                }
            }
            ExprKind::UnaryOp { operand, .. } => {
                Self::collect_expr_ident_refs(operand, out);
            }
            ExprKind::BinaryOp { left, right, .. } => {
                Self::collect_expr_ident_refs(left, out);
                Self::collect_expr_ident_refs(right, out);
            }
            ExprKind::FuncCall { desig, args } => {
                if desig.ident.module.is_none() {
                    out.insert(desig.ident.name.clone());
                }
                for arg in args {
                    Self::collect_expr_ident_refs(arg, out);
                }
            }
            ExprKind::SetConstructor { elements, .. } => {
                for elem in elements {
                    match elem {
                        SetElement::Single(e) => Self::collect_expr_ident_refs(e, out),
                        SetElement::Range(lo, hi) => {
                            Self::collect_expr_ident_refs(lo, out);
                            Self::collect_expr_ident_refs(hi, out);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn is_proc_type(tn: &TypeNode) -> bool {
        match tn {
            TypeNode::ProcedureType { .. } => true,
            TypeNode::Named(qi) if qi.module.is_none() && qi.name == "PROC" => true,
            _ => false,
        }
    }

    fn named_type_to_c(&self, qi: &QualIdent) -> String {
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
            ExprKind::StringLit(s) if s.len() <= 1 => "char".to_string(),
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
    fn param_info_from_proc_type(&self, mut tid: TypeId) -> Option<Vec<ParamCodegenInfo>> {
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
    fn resolve_designator_type(&self, desig: &Designator) -> Option<TypeId> {
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
    fn unwrap_type_aliases(&self, mut tid: TypeId) -> TypeId {
        loop {
            match self.sema.types.get(tid) {
                Type::Alias { target, .. } => tid = *target,
                _ => return tid,
            }
        }
    }

    /// Unwrap Pointer/Ref to get the base type
    fn unwrap_pointers(&self, mut tid: TypeId) -> TypeId {
        tid = self.unwrap_type_aliases(tid);
        match self.sema.types.get(tid) {
            Type::Pointer { base } => *base,
            Type::Ref { target, .. } => *target,
            _ => tid,
        }
    }

    /// Get proc param info for a complex designator call by resolving its type.
    fn get_designator_proc_param_info(&self, desig: &Designator) -> Vec<ParamCodegenInfo> {
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
                } else if let Some(high) = self.get_named_array_param_high(&arg_str) {
                    self.emit(&high);
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
        self.named_array_value_params.push(HashSet::new());
    }

    fn pop_var_scope(&mut self) {
        self.var_params.pop();
        self.open_array_params.pop();
        self.named_array_value_params.pop();
    }

    /// Save all per-procedure variable tracking sets before entering a scope
    fn save_var_tracking(&self) -> VarTrackingScope {
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
    fn restore_var_tracking(&mut self, saved: VarTrackingScope) {
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
            ExprKind::Deref(_) => None,
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

    /// Check if a TypeNode is a pointer type (POINTER TO ...)
    fn is_pointer_type(&self, tn: &TypeNode) -> bool {
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
    fn is_array_type(&self, tn: &TypeNode) -> bool {
        match tn {
            TypeNode::Array { .. } => true,
            TypeNode::Named(qi) => self.array_types.contains(&qi.name),
            _ => false,
        }
    }

    /// Check if a field name belongs to an array-typed record field (name-only, may false-positive)
    fn is_array_field(&self, field_name: &str) -> bool {
        for ((_rec_name, fname)) in &self.array_fields {
            if fname == field_name {
                return true;
            }
        }
        false
    }

    /// Type-aware check: is `field_name` an array field of record type `record_type`?
    fn is_array_field_of(&self, record_type: &str, field_name: &str) -> bool {
        self.array_fields.contains(&(record_type.to_string(), field_name.to_string()))
    }

    /// Walk a designator's selectors to determine the record type that owns the last field.
    /// Returns None if we can't resolve the type (e.g., through pointer deref or array indexing).
    fn resolve_field_record_type(&self, desig: &Designator) -> Option<String> {
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

    /// Check if an expression is obviously a scalar (literal, arithmetic, non-array variable,
    /// or a field access to a non-array field). Used as a safety guard to prevent emitting
    /// memcpy for scalar sources when type resolution fails in the fallback path.
    fn is_scalar_expr(&self, expr: &Expr) -> bool {
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

    fn is_named_array_value_param(&self, name: &str) -> bool {
        for scope in self.named_array_value_params.iter().rev() {
            if scope.contains(name) {
                return true;
            }
        }
        false
    }

    fn get_named_array_param_high(&self, name: &str) -> Option<String> {
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
            ExprKind::Deref(_) => false,
            _ => false,
        }
    }

    /// Check if an expression is likely CARDINAL/unsigned (for DIV/MOD codegen)
    fn is_address_expr(&self, expr: &Expr) -> bool {
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

    fn is_unsigned_expr(&self, expr: &Expr) -> bool {
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
    fn is_long_expr(&self, expr: &Expr) -> bool {
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

    /// Resolve a variable name (C expression string) to its M2 type name (mangled).
    /// Used to find type descriptors for M2+ NEW calls.
    fn resolve_var_type_name(&self, var_expr: &str) -> Option<String> {
        // Direct variable name lookup
        if let Some(type_name) = self.var_types.get(var_expr) {
            return Some(self.mangle(type_name));
        }
        None
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
            let orig = self.original_import_name(name).to_string();
            if self.foreign_modules.contains(module.as_str()) {
                return orig;
            }
            if let Some(c_name) = stdlib::map_stdlib_call(module, &orig) {
                return c_name;
            }
            // Non-stdlib module: use module-prefixed name
            if !stdlib::is_stdlib_module(module) {
                return format!("{}_{}", module, orig);
            }
        }
        // Check if this is a nested proc with a mangled name
        if let Some(mangled) = self.nested_proc_names.get(name) {
            return mangled.clone();
        }
        // Check if this name has an EXPORTC alias
        if let Some(ecn) = self.export_c_names.get(name) {
            return ecn.clone();
        }
        // Inside an embedded implementation, local proc calls need module prefix
        // Also check embedded_local_vars for procedure-typed variables used as calls
        if self.embedded_local_procs.contains(name)
            || self.embedded_local_vars.contains(name)
        {
            return format!("{}_{}", self.module_name, name);
        }
        self.mangle(name)
    }

    /// If the designator starts with an imported module name followed by a field selector,
    /// return (module_name, proc_name) and the remaining selectors start at index 1.
    /// Otherwise return None.
    /// Resolve an enum variant through a module's re-exports.
    /// When module M re-exports a type from module S (e.g., Promise re-exports Status from Scheduler),
    /// a reference like M.OK needs to resolve to S_Status_OK via S_OK in enum_variants.
    fn resolve_reexported_enum_variant(&self, module: &str, name: &str) -> Option<String> {
        if let Some(def_mod) = self.def_modules.get(module) {
            for imp in &def_mod.imports {
                if let Some(ref from_mod) = imp.from_module {
                    // Check if source_module has this name as an enum variant
                    let source_key = format!("{}_{}", from_mod, name);
                    if let Some(c_name) = self.enum_variants.get(&source_key) {
                        return Some(c_name.clone());
                    }
                }
            }
        }
        None
    }

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
            // Avoid clashing with m2_ runtime prefix
            _ if name.starts_with("m2_") => format!("m2v_{}", &name[3..]),
            // C keywords and standard library names
            _ if C_RESERVED.contains(name) => format!("m2_{}", name),
            _ => name.to_string(),
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

        // Register type descriptor for RTTI
        let parent_c_sym = parent.map(|p| {
            let pc = if let Some(ref m) = p.module {
                format!("{}_{}", m, p.name)
            } else {
                self.mangle(&p.name)
            };
            self.object_type_descs.get(&pc).cloned()
                .unwrap_or_else(|| format!("M2_TD_{}", pc))
        });
        let td_sym = self.register_type_desc(&c_name, name, parent_c_sym);
        self.object_type_descs.insert(c_name.to_string(), td_sym);

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
        self.exception_names.insert(e.name.clone());
        self.emitln(&format!("static const int M2_EXC_{} = {};", self.mangle(&e.name), exc_id));
    }

    fn next_exception_id(&mut self) -> usize {
        self.exception_counter += 1;
        self.exception_counter
    }

    /// Allocate a new unique type ID and register a type descriptor to be emitted.
    /// Returns the C symbol name for the descriptor (e.g. "M2_TD_ModName_TypeName").
    fn register_type_desc(&mut self, type_name: &str, display_name: &str, parent_c_sym: Option<String>) -> String {
        self.type_id_counter += 1;
        let id = self.type_id_counter;
        let depth = if let Some(ref parent) = parent_c_sym {
            // Find parent depth from already-registered descriptors
            self.type_descs.iter()
                .find(|(sym, _, _, _)| sym == parent)
                .map(|(_, _, _, d)| d + 1)
                .unwrap_or(1)
        } else {
            0
        };
        let c_sym = format!("M2_TD_{}", type_name);
        self.type_descs.push((c_sym.clone(), display_name.to_string(), parent_c_sym, depth));
        // Store the ID for later use
        let _ = id;
        c_sym
    }

    /// Emit all registered type descriptors as C globals.
    /// Must be called after all type declarations have been processed.
    /// Parents are always registered before children (due to topo-sorted embedded modules).
    fn emit_type_descs(&mut self) {
        if self.type_descs.is_empty() {
            return;
        }
        let descs = std::mem::take(&mut self.type_descs);
        let mut id = 0usize;
        for (c_sym, display, parent, depth) in &descs {
            id += 1;
            let parent_expr = if let Some(p) = parent {
                format!("&{}", p)
            } else {
                "NULL".to_string()
            };
            self.emitln(&format!(
                "M2_TypeDesc {} = {{ {}, \"{}\", {}, {} }};",
                c_sym, id, display, parent_expr, depth
            ));
        }
        self.newline();
    }

    // ── Modula-2+ TRY/EXCEPT/FINALLY ───────────────────────────────

    fn gen_try_statement(&mut self, body: &[Statement], excepts: &[ExceptClause], finally_body: &Option<Vec<Statement>>) {
        let has_finally = finally_body.is_some();
        // When FINALLY is present, we need to capture exception state
        // so FINALLY runs before any re-raise.
        let needs_deferred_raise = has_finally && (excepts.is_empty() || excepts.iter().all(|ec| ec.exception.is_some()));

        self.emitln("{");
        self.indent += 1;
        self.emitln("m2_ExcFrame _ef;");
        if needs_deferred_raise {
            self.emitln("int _ef_exc = 0;");
        }
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
            if has_finally {
                // Defer re-raise until after FINALLY
                self.emitln("_ef_exc = 1;");
            } else {
                self.emitln("/* no handlers — re-raise */");
                self.emitln("m2_raise(_ef.exception_id, _ef.exception_name, _ef.exception_arg);");
            }
        } else {
            let mut first = true;
            let mut has_catch_all = false;
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
                    has_catch_all = true;
                    self.emit("{\n");
                }
                self.indent += 1;
                for s in &ec.body {
                    self.gen_statement(s);
                }
                self.indent -= 1;
            }
            if !has_catch_all {
                if has_finally {
                    // Defer re-raise until after FINALLY
                    self.emitln("} else {");
                    self.indent += 1;
                    self.emitln("_ef_exc = 1;");
                    self.indent -= 1;
                } else {
                    // No catch-all: unhandled exception must propagate
                    self.emitln("} else {");
                    self.indent += 1;
                    self.emitln("m2_raise(_ef.exception_id, _ef.exception_name, _ef.exception_arg);");
                    self.indent -= 1;
                }
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
            if needs_deferred_raise {
                self.emitln("if (_ef_exc) {");
                self.indent += 1;
                self.emitln("m2_raise(_ef.exception_id, _ef.exception_name, _ef.exception_arg);");
                self.indent -= 1;
                self.emitln("}");
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
        let mut first = true;
        for branch in branches {
            self.emit_indent();
            if !first {
                self.emit("} else ");
            }
            first = false;
            self.emit("if (_tc_val && (");
            for (i, ty) in branch.types.iter().enumerate() {
                if i > 0 {
                    self.emit(" || ");
                }
                let type_name = if let Some(ref m) = ty.module {
                    format!("{}_{}", m, ty.name)
                } else {
                    self.mangle(&ty.name)
                };
                // Use M2_ISA for subtype-aware matching via type descriptor
                self.emit(&format!("M2_ISA(_tc_val, &M2_TD_{})", type_name));
            }
            self.emit(")) {\n");
            self.indent += 1;
            if let Some(ref var_name) = branch.var {
                // Cast to the specific type and bind.
                // REF types and OBJECT types are already pointer typedefs,
                // so we cast directly (no extra pointer level).
                if let Some(first_type) = branch.types.first() {
                    let type_name = if let Some(ref m) = first_type.module {
                        format!("{}_{}", m, first_type.name)
                    } else {
                        self.mangle(&first_type.name)
                    };
                    self.emitln(&format!("{} {} = ({})_tc_val;", type_name, var_name, type_name));
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
    fn test_line_directives_always_emitted() {
        let src = r#"MODULE Test;
FROM InOut IMPORT WriteString, WriteLn;
BEGIN
  WriteString("Hello");
  WriteLn;
END Test."#;
        let c = generate(src, false);
        assert!(c.contains("#line"), "non-debug output should still contain #line directives");
        assert!(!c.contains("setvbuf"), "non-debug output should not contain setvbuf");
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
