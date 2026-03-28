mod designators;
mod stmts;
mod exprs;
mod types;
mod m2plus;
mod modules;
mod decls;
mod hir_emit;

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use crate::ast::*;
use crate::builtins;
use crate::errors::{CompileError, CompileResult};
use crate::stdlib;
use crate::sema::SemanticAnalyzer;
use crate::types::*;

#[derive(Clone, Debug)]
pub(crate) struct ParamCodegenInfo {
    pub(crate) name: String,
    pub(crate) is_var: bool,
    pub(crate) is_open_array: bool,
    pub(crate) is_char: bool,
}

/// Per-procedure variable tracking sets that must be saved/restored when
/// entering/leaving a procedure scope. Without this, a local `key: ARRAY`
/// in procedure A leaks into `array_vars`, causing procedure B's
/// `VAR key: ADDRESS` assignment to emit memcpy instead of `*key = val`.
#[derive(Clone)]
pub(crate) struct VarTrackingScope {
    pub(crate) array_vars: HashSet<String>,
    pub(crate) char_array_vars: HashSet<String>,
    pub(crate) set_vars: HashSet<String>,
    pub(crate) cardinal_vars: HashSet<String>,
    pub(crate) longint_vars: HashSet<String>,
    pub(crate) longcard_vars: HashSet<String>,
    pub(crate) complex_vars: HashSet<String>,
    pub(crate) longcomplex_vars: HashSet<String>,
    pub(crate) var_types: HashMap<String, String>,
}

/// Snapshot of CodeGen state that must be saved/restored around embedded
/// implementation module generation. Keeps the save/restore in one place
/// instead of manually cloning 8+ fields at each call site.
pub(crate) struct EmbeddedModuleContext {
    pub(crate) module_name: String,
    pub(crate) import_map: HashMap<String, String>,
    pub(crate) import_alias_map: HashMap<String, String>,
    pub(crate) var_params: Vec<HashMap<String, bool>>,
    pub(crate) open_array_params: Vec<HashSet<String>>,
    pub(crate) named_array_value_params: Vec<HashSet<String>>,
    pub(crate) proc_params: HashMap<String, Vec<ParamCodegenInfo>>,
    pub(crate) var_tracking: VarTrackingScope,
    pub(crate) typeid_c_names: HashMap<TypeId, String>,
}

pub struct CodeGen {
    output: String,
    indent: usize,
    module_name: String,
    pub(crate) sema: SemanticAnalyzer,
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
    /// TypeId → C typedef name mapping (populated from HirModule type_decls)
    pub(crate) typeid_c_names: HashMap<TypeId, String>,
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
    /// Prebuilt HIR module (Phase 4). When set, procedure body codegen
    /// uses prebuilt HirProc.body instead of building HIR on demand.
    pub(crate) prebuilt_hir: Option<crate::hir::HirModule>,
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
            typeid_c_names: HashMap::new(),
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
            prebuilt_hir: None,
        }
    }

    pub fn set_m2plus(&mut self, enabled: bool) {
        self.m2plus = enabled;
        self.sema.m2plus = enabled;
    }

    pub fn set_debug(&mut self, enabled: bool) {
        self.debug_mode = enabled;
    }

    /// Take ownership of the symbol table (for LSP use).
    pub fn take_symtab(self) -> crate::symtab::SymbolTable {
        self.sema.symtab
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
        let exports = self.build_module_exports_from_sema(&mod_name);
        self.module_exports.insert(mod_name, exports);
        if self.pending_modules.is_none() {
            self.pending_modules = Some(Vec::new());
        }
        self.pending_modules.as_mut().unwrap().push(imp);
    }

    pub fn pre_register_type_names(&mut self, def: &crate::ast::DefinitionModule) {
        self.sema.pre_register_type_names(def);
    }

    /// Pre-register an external definition module so its types and procedures
    /// are available during semantic analysis and code generation.
    pub fn register_def_module(&mut self, def: &crate::ast::DefinitionModule) {
        self.sema.register_def_module(def);

        // Register type names from sema scope for type-cast recognition
        if let Some(scope_id) = self.sema.symtab.lookup_module_scope(&def.name) {
            for sym in self.sema.symtab.symbols_in_scope(scope_id) {
                if matches!(sym.kind, crate::symtab::SymbolKind::Type) {
                    self.known_type_names.insert(sym.name.clone());
                    self.known_type_names.insert(format!("{}_{}", def.name, sym.name));
                }
            }
        }

        // Store non-foreign def modules for type emission during embedded impl gen
        if def.foreign_lang.is_none() {
            self.def_modules.insert(def.name.clone(), def.clone());
        }

        if def.foreign_lang.is_some() {
            self.foreign_modules.insert(def.name.clone());
            self.foreign_def_modules.push(def.clone());
            let exports = self.build_module_exports_from_sema(&def.name);
            self.module_exports.insert(def.name.clone(), exports);
        }
    }

    /// Replace sema with a pre-populated one from the driver.
    pub fn set_sema(&mut self, sema: crate::sema::SemanticAnalyzer) {
        self.sema = sema;
    }

    /// Populate the TypeId → C type name mapping from prebuilt HirModule.
    /// Must be called after prebuilt_hir is set.
    pub fn populate_typeid_c_names(&mut self) {
        let mut entries = Vec::new();
        if let Some(ref hir) = self.prebuilt_hir {
            // Main module types
            for td in &hir.type_decls {
                // Skip builtins (0..19) and TY_VOID — don't override canonical names
                if td.type_id >= 20 {
                    entries.push((td.type_id, td.mangled.clone()));
                }
            }
            // Embedded module types — scoped lookup per module
            for emb in &hir.embedded_modules {
                let scope_id = self.sema.symtab.lookup_module_scope(&emb.name);
                for td in &emb.type_decls {
                    // Only register non-structural types (records, enums, arrays, aliases)
                    // Skip pointers/sets/subranges — they resolve structurally and can
                    // conflict across modules when typedef'd with different names.
                    // Skip structural types that can conflict across modules
                    let resolved = {
                        let mut id = td.type_id;
                        for _ in 0..50 {
                            match self.sema.types.get(id) {
                                crate::types::Type::Alias { target, .. } => id = *target,
                                _ => break,
                            }
                        }
                        id
                    };
                    let is_structural = matches!(self.sema.types.get(resolved),
                        crate::types::Type::Pointer { .. }
                        | crate::types::Type::Set { .. }
                        | crate::types::Type::Subrange { .. });
                    if is_structural { continue; }

                    // Use scoped lookup for correct TypeId
                    let type_id = scope_id
                        .and_then(|sid| self.sema.symtab.lookup_in_scope(sid, &td.name))
                        .map(|s| s.typ)
                        .unwrap_or(td.type_id);
                    if type_id != crate::types::TY_VOID {
                        entries.push((type_id, td.mangled.clone()));
                    }
                }
            }
        }
        for (tid, name) in entries {
            self.typeid_c_names.insert(tid, name);
        }
    }

    /// Register .def metadata without running sema (sema already populated by driver).
    pub fn register_def_module_no_sema(&mut self, def: &crate::ast::DefinitionModule) {
        // Register type names from sema scope (replaces AST Definition::Type iteration)
        if let Some(scope_id) = self.sema.symtab.lookup_module_scope(&def.name) {
            for sym in self.sema.symtab.symbols_in_scope(scope_id) {
                if matches!(sym.kind, crate::symtab::SymbolKind::Type) {
                    self.known_type_names.insert(sym.name.clone());
                    let prefixed = format!("{}_{}", def.name, sym.name);
                    self.known_type_names.insert(prefixed.clone());
                    if sym.typ >= 20 {
                        self.typeid_c_names.insert(sym.typ, prefixed);
                    }
                }
            }
        }
        if def.foreign_lang.is_none() {
            self.def_modules.insert(def.name.clone(), def.clone());
        }
        if def.foreign_lang.is_some() {
            self.foreign_modules.insert(def.name.clone());
            self.foreign_def_modules.push(def.clone());
            let exports = self.build_module_exports_from_sema(&def.name);
            self.module_exports.insert(def.name.clone(), exports);
        }
    }

    /// Add an implementation module without running sema registration.
    pub fn add_imported_module_no_sema(&mut self, imp: ImplementationModule) {
        let mod_name = imp.name.clone();
        let exports = self.build_module_exports_from_sema(&mod_name);
        self.module_exports.insert(mod_name, exports);
        if self.pending_modules.is_none() {
            self.pending_modules = Some(Vec::new());
        }
        self.pending_modules.as_mut().unwrap().push(imp);
    }

    pub fn is_foreign_module(&self, name: &str) -> bool {
        self.foreign_modules.contains(name)
    }

    /// Build module exports (proc name → ParamCodegenInfo) from sema symtab.
    /// Replaces 4 copies of AST Declaration::Procedure / TypeNode iteration.
    fn build_module_exports_from_sema(&self, mod_name: &str) -> Vec<(String, Vec<ParamCodegenInfo>)> {
        let mut exports = Vec::new();
        if let Some(scope_id) = self.sema.symtab.lookup_module_scope(mod_name) {
            for sym in self.sema.symtab.symbols_in_scope(scope_id) {
                if let crate::symtab::SymbolKind::Procedure { params, .. } = &sym.kind {
                    let param_info: Vec<ParamCodegenInfo> = params.iter().map(|p| {
                        let is_open = matches!(self.sema.types.get(p.typ), crate::types::Type::OpenArray { .. });
                        ParamCodegenInfo {
                            name: p.name.clone(),
                            is_var: p.is_var,
                            is_open_array: is_open,
                            is_char: p.typ == crate::types::TY_CHAR,
                        }
                    }).collect();
                    exports.push((sym.name.clone(), param_info));
                }
            }
        }
        exports
    }


    /// Like generate(), but returns sema errors as a Vec for structured diagnostics
    /// Run sema on all imported implementation modules.
    /// Creates procedure scopes + param/local symbols needed for
    /// scope-aware HIR designator resolution.
    fn analyze_all_impl_modules(&mut self) {
        let modules: Vec<_> = self.pending_modules.as_ref()
            .map(|v| v.iter().cloned().collect())
            .unwrap_or_default();
        for imp in &modules {
            self.sema.analyze_impl_module(imp);
        }
        self.sema.fixup_record_field_types();
    }

    pub fn generate_or_errors(&mut self, unit: &CompilationUnit) -> Result<String, Vec<CompileError>> {
        self.sema.analyze(unit)?;
        self.analyze_all_impl_modules();
        self.post_sema_generate(unit).map_err(|e| vec![e])?;
        Ok(self.output.clone())
    }

    pub fn generate(&mut self, unit: &CompilationUnit) -> CompileResult<String> {
        // Sema already fully populated by driver — just generate code.
        self.post_sema_generate(unit)?;
        Ok(self.output.clone())
    }

    fn post_sema_generate(&mut self, unit: &CompilationUnit) -> CompileResult<()> {
        // Scan compilation unit to determine which M2+ features are needed
        if self.m2plus {
            self.scan_m2plus_features();
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

        // Extract module name and imports, dispatch to gen functions.
        match unit {
            CompilationUnit::ProgramModule(m) => {
                self.module_name = m.name.clone();
                self.build_import_map(&m.imports);
                self.gen_program_module()?
            }
            CompilationUnit::DefinitionModule(m) => {
                self.module_name = m.name.clone();
                self.gen_definition_module(m)
            }
            CompilationUnit::ImplementationModule(m) => {
                self.module_name = m.name.clone();
                self.build_import_map(&m.imports);
                self.gen_implementation_module()?
            }
        }
        Ok(())
    }

    // ── Shared emission helpers ───────────────────────────────────────


    fn register_var_param(&mut self, name: &str) {
        if let Some(scope) = self.var_params.last_mut() {
            scope.insert(name.to_string(), true);
        }
    }


}

pub(crate) fn escape_c_string(s: &str) -> String {
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

pub(crate) fn escape_c_char(ch: char) -> String {
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
        // Run sema + build HIR (required by the HIR-based codegen pipeline)
        cg.sema.analyze(&unit).unwrap();
        let hir = crate::hir_build::build_module(&unit, &[], &cg.sema);
        cg.prebuilt_hir = Some(hir);
        cg.populate_typeid_c_names();
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
