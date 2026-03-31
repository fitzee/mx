mod cfg_emit;
mod types;
mod modules;
mod decls;
mod stmts;
mod exprs;
mod designators;
mod closures;
mod stdlib_sigs;
pub(crate) mod llvm_types;
pub(crate) mod type_lowering;
pub(crate) mod debug_info;

use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::builtins;
use crate::errors::{CompileError, CompileResult};
use crate::sema::SemanticAnalyzer;
use crate::stdlib;
use crate::types::*;

// ── LLVM value representation ───────────────────────────────────────

/// An LLVM IR value with its type string.
/// Carries optional semantic TypeId for type-safe lowering.
#[derive(Clone, Debug)]
pub(crate) struct Val {
    /// SSA name ("%3"), constant ("42"), or global ("@.str.0")
    pub(crate) name: String,
    /// LLVM type: "i32", "float", "ptr", "[10 x i8]", etc.
    pub(crate) ty: String,
    /// Semantic TypeId from sema — the source of truth for type identity.
    /// None only for synthetic/constant values where sema identity is irrelevant.
    pub(crate) type_id: Option<crate::types::TypeId>,
}

impl Val {
    pub(crate) fn new(name: impl Into<String>, ty: impl Into<String>) -> Self {
        Self { name: name.into(), ty: ty.into(), type_id: None }
    }

    pub(crate) fn with_tid(name: impl Into<String>, ty: impl Into<String>, tid: crate::types::TypeId) -> Self {
        Self { name: name.into(), ty: ty.into(), type_id: Some(tid) }
    }
}

// ── Semantic type queries (answered from sema only) ────────────────

/// Resolve a TypeId through aliases to the underlying concrete type.
pub(crate) fn resolve_tid(types: &crate::types::TypeRegistry, tid: crate::types::TypeId) -> crate::types::TypeId {
    let mut cur = tid;
    loop {
        match types.get(cur) {
            crate::types::Type::Alias { target, .. } => cur = *target,
            _ => return cur,
        }
    }
}

/// True if the resolved type is an aggregate (record or fixed array)
/// that should stay as an address, not be loaded as an SSA value.
pub(crate) fn is_aggregate(types: &crate::types::TypeRegistry, tid: crate::types::TypeId) -> bool {
    let resolved = resolve_tid(types, tid);
    match types.get(resolved) {
        crate::types::Type::Record { .. } => {
            // COMPLEX/LONGCOMPLEX are small structs treated as LLVM value types
            resolved != crate::types::TY_COMPLEX && resolved != crate::types::TY_LONGCOMPLEX
        }
        crate::types::Type::Array { .. } => true,
        _ => false,
    }
}

/// True if the resolved type is a pointer (POINTER TO X or ADDRESS).
pub(crate) fn is_pointer(types: &crate::types::TypeRegistry, tid: crate::types::TypeId) -> bool {
    matches!(types.get(resolve_tid(types, tid)),
        crate::types::Type::Pointer { .. } |
        crate::types::Type::Address)
}

/// Get the pointer target TypeId (POINTER TO X → X). Returns None for ADDRESS.
pub(crate) fn pointer_target(types: &crate::types::TypeRegistry, tid: crate::types::TypeId) -> Option<crate::types::TypeId> {
    match types.get(resolve_tid(types, tid)) {
        crate::types::Type::Pointer { base } => Some(*base),
        _ => None,
    }
}

/// Get the record fields for a TypeId. Returns None for non-record types.
pub(crate) fn record_fields(types: &crate::types::TypeRegistry, tid: crate::types::TypeId) -> Option<&Vec<crate::types::RecordField>> {
    match types.get(resolve_tid(types, tid)) {
        crate::types::Type::Record { fields, .. } => Some(fields),
        _ => None,
    }
}

/// Look up a field by name in a record type. Returns the field's TypeId.
pub(crate) fn record_field_tid(types: &crate::types::TypeRegistry, record_tid: crate::types::TypeId, field_name: &str) -> Option<crate::types::TypeId> {
    match types.get(resolve_tid(types, record_tid)) {
        crate::types::Type::Record { fields, .. } => {
            fields.iter().find(|f| f.name == field_name).map(|f| f.typ)
        }
        _ => None,
    }
}

/// Get the array element TypeId. Returns None for non-array types.
pub(crate) fn array_element(types: &crate::types::TypeRegistry, tid: crate::types::TypeId) -> Option<crate::types::TypeId> {
    match types.get(resolve_tid(types, tid)) {
        crate::types::Type::Array { elem_type, .. } => Some(*elem_type),
        crate::types::Type::OpenArray { elem_type } => Some(*elem_type),
        _ => None,
    }
}

/// Parameter codegen info (mirrors C backend's ParamCodegenInfo).
#[derive(Clone, Debug)]
pub(crate) struct ParamLLVMInfo {
    pub(crate) name: String,
    pub(crate) is_var: bool,
    pub(crate) is_open_array: bool,
    pub(crate) llvm_type: String,
    /// For open array params: the element type (e.g., "i32", "{ i32, i32 }")
    pub(crate) open_array_elem_type: Option<String>,
}

// ── Main codegen struct ─────────────────────────────────────────────

pub struct LLVMCodeGen {
    /// Global/preamble section (type defs, globals, string constants, declares)
    pub(crate) preamble: String,
    /// Function bodies section
    pub(crate) body: String,

    pub(crate) target: crate::target::TargetInfo,
    pub(crate) sema: SemanticAnalyzer,
    pub(crate) module_name: String,
    pub(crate) m2plus: bool,
    pub(crate) debug_mode: bool,

    // ── SSA naming ──────────────────────────────────────────────────
    pub(crate) tmp_counter: usize,
    pub(crate) label_counter: usize,
    /// Current basic block label (for PHI node predecessor tracking)
    pub(crate) current_block: String,

    // ── String constant pool ────────────────────────────────────────
    /// (content, global_name, byte_length_including_nul)
    pub(crate) string_pool: Vec<(String, String, usize)>,

    // ── Variable tracking ───────────────────────────────────────────
    /// Locals in current function: name → (alloca_name, llvm_type)
    pub(crate) locals: Vec<HashMap<String, (String, String)>>,
    /// Globals: name → (global_name, llvm_type)
    pub(crate) globals: HashMap<String, (String, String)>,

    // ── Import / module tracking ────────────────────────────────────
    pub(crate) import_map: HashMap<String, String>,
    pub(crate) import_alias_map: HashMap<String, String>,
    pub(crate) imported_modules: HashSet<String>,
    pub(crate) pending_module_names: Vec<String>,
    pub(crate) module_imports: HashMap<String, Vec<crate::hir::HirImport>>,
    pub(crate) def_module_names: HashSet<String>,
    pub(crate) def_exception_names: HashMap<String, Vec<String>>,
    pub(crate) foreign_modules: HashSet<String>,
    pub(crate) module_exports: HashMap<String, Vec<(String, Vec<ParamLLVMInfo>)>>,
    // ── RTTI (M2+ REF/OBJECT type descriptors) ────────────────────
    /// Maps type name → LLVM global symbol for M2_TypeDesc
    pub(crate) ref_type_descs: HashMap<String, String>,
    /// Counter for unique type IDs
    pub(crate) rtti_type_id_counter: usize,

    // ── Procedure parameter tracking ────────────────────────────────
    pub(crate) proc_params: HashMap<String, Vec<ParamLLVMInfo>>,
    /// Known return types for functions (populated by declare_stdlib_function and gen_proc_decl)
    pub(crate) fn_return_types: HashMap<String, String>,
    /// Map HIR mangled names → actual LLVM function names (for nested proc calls)
    pub(crate) fn_name_map: HashMap<String, String>,
    /// String constant lengths (for CONST s = "..." passed to open array params)
    pub(crate) string_const_lengths: HashMap<String, usize>,
    /// VAR params in current scope (passed as pointers)
    pub(crate) var_params: Vec<HashSet<String>>,
    /// Open array params in current scope (have _high companion)
    pub(crate) open_array_params: Vec<HashSet<String>>,
    /// Named array params in current scope (passed as ptr to array, need load before GEP)
    pub(crate) named_array_params: Vec<HashSet<String>>,

    // ── Declared external functions (avoid duplicates) ──────────────
    pub(crate) declared_fns: HashSet<String>,

    // ── Control flow stacks ─────────────────────────────────────────
    /// Loop exit labels for EXIT statements
    // loop_exit_stack removed — EXIT is resolved to Goto by CFG builder

    // ── Current function context ────────────────────────────────────
    pub(crate) current_return_type: Option<String>,
    /// Stack frame alloca for stack trace support (None if not in a function)
    pub(crate) stack_frame_alloca: Option<String>,
    pub(crate) in_function: bool,

    // ── Enum / const tracking ───────────────────────────────────────
    pub(crate) enum_variants: HashMap<String, i64>,
    pub(crate) const_values: HashMap<String, i64>,

    // ── Type tracking ───────────────────────────────────────────────
    /// Type name → LLVM type (for user-defined types: records, arrays, etc.)
    pub(crate) type_map: HashMap<String, String>,
    /// Record type name → vec of (field_name, llvm_type, field_index)
    pub(crate) record_fields: HashMap<String, Vec<(String, String, usize)>>,
    /// Variable name → M2 type name (for record field resolution)
    pub(crate) var_type_names: HashMap<String, String>,
    /// Array type tracking: type_name → (elem_llvm_type, size)
    pub(crate) array_types: HashMap<String, (String, usize)>,
    /// Variables that are array types
    pub(crate) array_vars: HashSet<String>,
    /// Array variable → element M2 type name (for record field resolution after indexing)
    pub(crate) array_elem_type_names: HashMap<String, String>,
    /// Variables that are char arrays (ARRAY OF CHAR)
    pub(crate) char_array_vars: HashSet<String>,
    /// Pointer type → target type name (e.g., "NodePtr" → "Node")
    pub(crate) pointer_target_types: HashMap<String, String>,
    /// Anonymous record counter for synthetic type names
    pub(crate) anon_record_counter: usize,

    // ── Stdlib name mapping (InOut_WriteString → m2_WriteString) ───
    pub(crate) stdlib_name_map: HashMap<String, String>,

    // ── Parent proc name stack for nested proc mangling ───────────
    pub(crate) parent_proc_stack: Vec<String>,
    /// Label stack for TRY entry points (used by RETRY)
    pub(crate) try_entry_label: Vec<String>,
    /// When inside a TRY body, the unwind destination label for invoke
    pub(crate) try_unwind_dest: Option<String>,
    /// When inside a SjLj-guarded procedure body (ISO EXCEPT), RAISE uses m2_raise
    pub(crate) in_sjlj_context: bool,

    // ── WITH statement alias stack ───
    // (record_var_name, type_name_legacy, field_names, has_deref, type_id)
    pub(crate) with_stack: Vec<(String, String, Vec<String>, bool, Option<crate::types::TypeId>)>,

    // ── Source file for metadata ────────────────────────────────────
    pub(crate) source_file: String,

    // ── Embedded module init functions ──────────────────────────────
    pub(crate) embedded_init_modules: Vec<String>,

    // ── New type system (Phase 1 refactor) ──────────────────────────
    /// Canonical type lowering table — built from sema TypeRegistry.
    /// Single source of truth for M2 TypeId → LLVM type mapping.
    pub(crate) type_lowering: Option<type_lowering::TypeLowering>,
    /// Variable name → semantic TypeId (for type-safe field resolution)
    pub(crate) var_types: HashMap<String, TypeId>,

    // ── Debug info ──────────────────────────────────────────────────
    /// DWARF debug metadata builder. Only active when debug_mode is true.
    pub(crate) di: Option<debug_info::DebugInfoBuilder>,
    /// Prebuilt HIR module (Phase 4). When set, procedure body codegen
    /// uses prebuilt HirProc.body instead of building HIR on demand.
    pub(crate) prebuilt_hir: Option<crate::hir::HirModule>,
}

impl LLVMCodeGen {
    pub fn new(target: crate::target::TargetInfo) -> Self {
        Self {
            preamble: String::new(),
            body: String::new(),
            target,
            sema: SemanticAnalyzer::new(),
            module_name: String::new(),
            m2plus: false,
            debug_mode: false,
            tmp_counter: 0,
            label_counter: 0,
            current_block: "bb.entry".to_string(),
            string_pool: Vec::new(),
            locals: vec![HashMap::new()],
            globals: HashMap::new(),
            import_map: HashMap::new(),
            import_alias_map: HashMap::new(),
            imported_modules: HashSet::new(),
            pending_module_names: Vec::new(),
            module_imports: HashMap::new(),
            def_module_names: HashSet::new(),
            def_exception_names: HashMap::new(),
            foreign_modules: HashSet::new(),
            module_exports: HashMap::new(),
            ref_type_descs: HashMap::new(),
            rtti_type_id_counter: 0,
            proc_params: HashMap::new(),
            fn_return_types: HashMap::new(),
            fn_name_map: HashMap::new(),
            string_const_lengths: HashMap::new(),
            var_params: vec![HashSet::new()],
            open_array_params: vec![HashSet::new()],
            named_array_params: vec![HashSet::new()],
            declared_fns: HashSet::new(),
            current_return_type: None,
            stack_frame_alloca: None,
            in_function: false,
            enum_variants: HashMap::new(),
            const_values: HashMap::new(),
            type_map: HashMap::new(),
            record_fields: HashMap::new(),
            var_type_names: HashMap::new(),
            array_types: HashMap::new(),
            array_vars: HashSet::new(),
            array_elem_type_names: HashMap::new(),
            char_array_vars: HashSet::new(),
            pointer_target_types: HashMap::new(),
            anon_record_counter: 0,
            stdlib_name_map: HashMap::new(),
            parent_proc_stack: Vec::new(),
            try_entry_label: Vec::new(),
            try_unwind_dest: None,
            in_sjlj_context: false,
            with_stack: Vec::new(),
            source_file: String::new(),
            embedded_init_modules: Vec::new(),
            type_lowering: None,
            var_types: HashMap::new(),
            di: None,
            prebuilt_hir: None,
        }
    }

    // ── Public interface (mirrors CodeGen) ───────────────────────────

    pub fn set_m2plus(&mut self, enabled: bool) {
        self.m2plus = enabled;
        self.sema.m2plus = enabled;
    }

    pub fn set_debug(&mut self, enabled: bool) {
        self.debug_mode = enabled;
    }

    /// Replace the LLVM backend's sema with a pre-populated one.
    /// Used to share sema between C and LLVM backends so .def modules
    /// are registered once.
    pub fn set_sema(&mut self, sema: SemanticAnalyzer) {
        self.sema = sema;
    }

    /// Store import list for a module (for topo sorting and import map building).
    pub fn register_module_imports(&mut self, name: &str, imports: Vec<crate::hir::HirImport>) {
        self.module_imports.entry(name.to_string())
            .and_modify(|existing| existing.extend(imports.clone()))
            .or_insert(imports);
    }

    pub fn register_def_by_name(&mut self, name: &str, is_foreign: bool) {
        if !is_foreign {
            self.def_module_names.insert(name.to_string());
        }
        if is_foreign {
            self.foreign_modules.insert(name.to_string());
            // Build exports from sema
            let mut exports = Vec::new();
            if let Some(scope_id) = self.sema.symtab.lookup_module_scope(name) {
                let syms: Vec<(String, Vec<crate::symtab::ParamInfo>)> =
                    self.sema.symtab.symbols_in_scope(scope_id).iter()
                        .filter_map(|sym| {
                            if let crate::symtab::SymbolKind::Procedure { params, .. } = &sym.kind {
                                Some((sym.name.clone(), params.clone()))
                            } else { None }
                        })
                        .collect();
                for (proc_name, params) in syms {
                    let param_info: Vec<ParamLLVMInfo> = params.iter().map(|p| {
                        let resolved = {
                            let mut id = p.typ;
                            loop {
                                if let crate::types::Type::Alias { target, .. } = self.sema.types.get(id) { id = *target; } else { break id; }
                            }
                        };
                        let is_open = matches!(self.sema.types.get(resolved), crate::types::Type::OpenArray { .. });
                        let llvm_ty = self.tl_type_str(p.typ);
                        let elem_ty = if is_open {
                            if let crate::types::Type::OpenArray { elem_type } = self.sema.types.get(resolved) {
                                Some(self.tl_type_str(*elem_type))
                            } else { None }
                        } else { None };
                        ParamLLVMInfo {
                            name: p.name.clone(),
                            is_var: p.is_var,
                            is_open_array: is_open,
                            llvm_type: if p.is_var { "ptr".to_string() } else { llvm_ty },
                            open_array_elem_type: elem_ty,
                        }
                    }).collect();
                    exports.push((proc_name, param_info));
                }
            }
            self.module_exports.insert(name.to_string(), exports);
        }
    }

    pub fn add_imported_module_by_name(&mut self, name: &str) {
        self.imported_modules.insert(name.to_string());
        self.pending_module_names.push(name.to_string());
    }

    pub fn is_foreign_module(&self, name: &str) -> bool {
        self.foreign_modules.contains(name)
    }

    pub fn generate_or_errors(&mut self, unit: &CompilationUnit) -> Result<String, Vec<CompileError>> {
        self.sema.fixup_record_field_types();
        self.build_type_lowering();
        self.post_sema_generate(unit).map_err(|e| vec![e])?;
        Ok(self.finalize())
    }

    pub fn generate(&mut self, unit: &CompilationUnit) -> CompileResult<String> {
        self.sema.fixup_record_field_types();
        self.build_type_lowering();
        self.post_sema_generate(unit)?;
        Ok(self.finalize())
    }

    /// Register a type descriptor for a REF/OBJECT type.
    /// Returns the LLVM global symbol name.
    pub(crate) fn register_type_desc(&mut self, type_name: &str, parent: Option<&str>) -> String {
        self.rtti_type_id_counter += 1;
        let id = self.rtti_type_id_counter;
        let sym = format!("@M2_TD_{}", type_name);
        let depth = if let Some(p) = parent {
            if let Some(psym) = self.ref_type_descs.get(p) {
                // TODO: track depths properly
                1
            } else { 0 }
        } else { 0 };

        // Emit type name string
        let name_global = format!("@.td_name.{}", id);
        self.emit_preambleln(&format!(
            "{} = private unnamed_addr constant [{} x i8] c\"{}\\00\"",
            name_global, type_name.len() + 1, type_name));

        // Emit M2_TypeDesc global: { i32 type_id, ptr name, ptr parent, i32 depth }
        let parent_ref = if let Some(p) = parent {
            if let Some(psym) = self.ref_type_descs.get(p) {
                psym.clone()
            } else { "null".to_string() }
        } else { "null".to_string() };

        self.emit_preambleln(&format!(
            "{} = global {{ i32, ptr, ptr, i32 }} {{ i32 {}, ptr {}, ptr {}, i32 {} }}",
            sym, id, name_global, parent_ref, depth));

        self.ref_type_descs.insert(type_name.to_string(), sym.clone());
        sym
    }

    /// Declare RTTI runtime functions if not already declared.
    pub(crate) fn declare_rtti_runtime(&mut self) {
        if !self.declared_fns.contains("M2_ref_alloc") {
            self.emit_preambleln("declare noalias ptr @M2_ref_alloc(i64, ptr) nounwind");
            self.emit_preambleln("declare i32 @M2_ISA(ptr nocapture, ptr nocapture) nounwind readonly");
            self.emit_preambleln("declare void @M2_ref_free(ptr) nounwind");
            self.declared_fns.insert("M2_ref_alloc".to_string());
            self.declared_fns.insert("M2_ISA".to_string());
            self.declared_fns.insert("M2_ref_free".to_string());
        }
    }

    fn build_type_lowering(&mut self) {
        self.type_lowering = Some(type_lowering::TypeLowering::build(&self.sema.types));

        // Dump specific TypeIds for debugging

        // Initialize debug info builder if debug mode is on
        if self.debug_mode {
            let producer = format!("{} {}", crate::identity::COMPILER_ID, crate::identity::VERSION);
            self.di = Some(debug_info::DebugInfoBuilder::new(&producer));
        }
    }

    // ── Core generation ─────────────────────────────────────────────

    fn post_sema_generate(&mut self, unit: &CompilationUnit) -> CompileResult<()> {
        match unit {
            CompilationUnit::ProgramModule(_) => self.gen_program_module(),
            CompilationUnit::DefinitionModule(_) => {
                // Definition modules don't produce output
                Ok(())
            }
            CompilationUnit::ImplementationModule(_) => self.gen_implementation_module(),
        }
    }

    fn finalize(&self) -> String {
        let mut out = String::new();

        // Header
        out.push_str(&format!("; ModuleID = '{}'\n", self.module_name));
        out.push_str(&format!("source_filename = \"{}\"\n", self.source_file));

        // Target triple and data layout from TargetInfo
        out.push_str(&format!("target datalayout = \"{}\"\n", self.target.llvm_datalayout()));
        out.push_str(&format!("target triple = \"{}\"\n", self.target.llvm_triple()));
        out.push('\n');

        // String constants
        for (content, name, _len) in &self.string_pool {
            let bytes = content.as_bytes();
            let total_len = bytes.len() + 1; // +1 for NUL
            out.push_str(&format!(
                "{} = private unnamed_addr constant [{} x i8] c\"{}\\00\"\n",
                name,
                total_len,
                escape_llvm_string(content)
            ));
        }
        if !self.string_pool.is_empty() {
            out.push('\n');
        }

        // Preamble (globals, type defs, extern declares)
        out.push_str(&self.preamble);
        if !self.preamble.is_empty() {
            out.push('\n');
        }

        // Function bodies
        out.push_str(&self.body);

        // Debug metadata (if debug mode)
        if let Some(ref di) = self.di {
            out.push_str(&di.finalize());
        }

        out
    }

    // ── Emission helpers ────────────────────────────────────────────

    pub(crate) fn emit_preamble(&mut self, s: &str) {
        self.preamble.push_str(s);
    }

    pub(crate) fn emit_preambleln(&mut self, s: &str) {
        self.preamble.push_str(s);
        self.preamble.push('\n');
    }

    pub(crate) fn emit(&mut self, s: &str) {
        self.body.push_str(s);
    }

    pub(crate) fn emitln(&mut self, s: &str) {
        // Attach !dbg metadata to instructions when debug mode is active
        if s.starts_with("  ") && !s.contains("!dbg") && !s.contains("!DILocation") {
            if let Some(ref di) = self.di {
                if let Some(loc_id) = di.current_location() {
                    let trimmed = s.trim();
                    // Only attach to actual LLVM instructions, not labels/comments/directives
                    let is_instruction = !trimmed.is_empty()
                        && !trimmed.starts_with(';')
                        && !trimmed.ends_with(':')
                        && (trimmed.starts_with('%')      // %t = ...
                            || trimmed.starts_with("store")
                            || trimmed.starts_with("call")
                            || trimmed.starts_with("ret")
                            || trimmed.starts_with("br ")    // br label / br i1
                            || trimmed.starts_with("switch")
                            || trimmed.starts_with("unreachable"));
                    if is_instruction {
                        self.body.push_str(s);
                        self.body.push_str(&format!(", !dbg !{}", loc_id));
                        self.body.push('\n');
                        return;
                    }
                }
            }
        }
        // Track current basic block for PHI predecessor tracking
        let trimmed = s.trim();
        if trimmed.ends_with(':') && !trimmed.starts_with(';') && !trimmed.contains("  ") {
            self.current_block = trimmed.trim_end_matches(':').to_string();
        }
        self.body.push_str(s);
        self.body.push('\n');
    }

    pub(crate) fn next_tmp(&mut self) -> String {
        let n = self.tmp_counter;
        self.tmp_counter += 1;
        format!("%t{}", n)
    }

    pub(crate) fn next_label(&mut self, prefix: &str) -> String {
        let n = self.label_counter;
        self.label_counter += 1;
        format!("{}.{}", prefix, n)
    }

    /// Emit a basic block label and update current_block for PHI tracking
    pub(crate) fn emit_label(&mut self, label: &str) {
        self.emitln(&format!("{}:", label));
        self.current_block = label.to_string();
    }

    pub(crate) fn mangle(&self, name: &str) -> String {
        format!("{}_{}", self.module_name, name)
    }


    /// Set the current debug source location from a SourceLoc.
    pub(crate) fn set_debug_loc(&mut self, loc: &crate::errors::SourceLoc) {
        if let Some(ref mut di) = self.di {
            if loc.line > 0 && !loc.file.is_empty() {
                di.set_location(loc.line, loc.col, &loc.file);
            }
        }
    }

    /// Emit a #dbg_declare record for a local variable or parameter.
    /// `alloca` is the SSA alloca name (e.g., "%t0"), `var_id` is the !DILocalVariable metadata ID.
    pub(crate) fn emit_dbg_declare(&mut self, alloca: &str, var_id: usize) {
        if let Some(loc_id) = self.di.as_ref().and_then(|di| di.current_location()) {
            self.body.push_str(&format!(
                "    #dbg_declare(ptr {}, !{}, !DIExpression(), !{})\n",
                alloca, var_id, loc_id
            ));
        }
    }


    /// Create debug type for a named type — checks if it's a record, pointer, array, or builtin.
    fn debug_type_for_named(&mut self, name: &str) -> usize {
        // Check if it's a builtin type first
        match name {
            "INTEGER" | "CARDINAL" | "LONGINT" | "LONGCARD" | "REAL" | "LONGREAL"
            | "BOOLEAN" | "CHAR" | "BITSET" | "ADDRESS" | "BYTE" => {
                return self.di.as_mut().unwrap().get_m2_type(name);
            }
            _ => {}
        }

        // Check if it's a record type
        if let Some(fields) = self.record_fields.get(name).cloned() {
            let file = self.source_file.clone();
            // Build member list
            let mut members = Vec::new();
            let mut offset_bits = 0usize;
            for (fname, ftype_str, _idx) in &fields {
                let fsize = self.debug_size_bits_for_llvm_type(ftype_str);
                let ftype_id = self.debug_type_for_llvm_type_str(ftype_str);
                members.push((fname.clone(), ftype_id, fsize, offset_bits));
                offset_bits += fsize;
            }
            let total_size = offset_bits;
            return self.di.as_mut().unwrap().create_record_type(
                name, &file, 0, total_size, members,
            );
        }

        // Check if it's a pointer type
        if let Some(target) = self.pointer_target_types.get(name).cloned() {
            let base_id = self.debug_type_for_named(&target);
            return self.di.as_mut().unwrap().create_pointer_type(base_id, 64);
        }

        // Check if it's an array type
        if let Some((elem_ty, size)) = self.array_types.get(name).cloned() {
            let elem_id = self.debug_type_for_llvm_type_str(&elem_ty);
            let elem_size = self.debug_size_bits_for_llvm_type(&elem_ty);
            return self.di.as_mut().unwrap().create_array_type(elem_id, size, elem_size);
        }

        // Fallback: treat as opaque i32
        self.di.as_mut().unwrap().get_m2_type(name)
    }


    /// Map an LLVM type string to a debug type ID.
    fn debug_type_for_llvm_type_str(&mut self, ty: &str) -> usize {
        match ty {
            "i1" => self.di.as_mut().unwrap().get_m2_type("BOOLEAN"),
            "i8" => self.di.as_mut().unwrap().get_m2_type("CHAR"),
            "i16" => self.di.as_mut().unwrap().create_basic_type("SHORTINT", 16, "DW_ATE_signed"),
            "i32" => self.di.as_mut().unwrap().get_m2_type("INTEGER"),
            "i64" => self.di.as_mut().unwrap().get_m2_type("LONGINT"),
            "float" => self.di.as_mut().unwrap().get_m2_type("REAL"),
            "double" => self.di.as_mut().unwrap().get_m2_type("LONGREAL"),
            "ptr" => self.di.as_mut().unwrap().get_m2_type("ADDRESS"),
            _ => self.di.as_mut().unwrap().get_m2_type("INTEGER"),
        }
    }

    /// Get size in bits for an LLVM type string.
    fn debug_size_bits_for_llvm_type(&self, ty: &str) -> usize {
        match ty {
            "i1" | "i8" => 8,
            "i16" => 16,
            "i32" | "float" => 32,
            "i64" | "double" | "ptr" => 64,
            _ if ty.starts_with('{') => {
                // Rough struct size — count fields
                ty.matches("i32").count() * 32
                    + ty.matches("i64").count() * 64
                    + ty.matches("i8").count() * 8
                    + ty.matches("float").count() * 32
                    + ty.matches("double").count() * 64
                    + ty.matches("ptr").count() * 64
            }
            _ => 32,
        }
    }


    pub(crate) fn intern_string(&mut self, s: &str) -> (String, usize) {
        // Check if already interned
        for (content, name, len) in &self.string_pool {
            if content == s {
                return (name.clone(), *len);
            }
        }
        let idx = self.string_pool.len();
        let name = format!("@.str.{}", idx);
        let len = s.len(); // without NUL
        self.string_pool.push((s.to_string(), name.clone(), len));
        (name, len)
    }
}

// ── Helper types ────────────────────────────────────────────────────

pub(crate) struct FnSig {
    pub(crate) return_type: String,
    pub(crate) params_str: String,
    pub(crate) param_infos: Vec<ParamLLVMInfo>,
}

impl FnSig {
    pub(crate) fn new(ret: &str, params: &str) -> Self {
        Self {
            return_type: ret.to_string(),
            params_str: params.to_string(),
            param_infos: Vec::new(),
        }
    }

    pub(crate) fn with_params(ret: &str, params: &str, infos: Vec<ParamLLVMInfo>) -> Self {
        Self {
            return_type: ret.to_string(),
            params_str: params.to_string(),
            param_infos: infos,
        }
    }
}

// ── String escaping for LLVM IR ─────────────────────────────────────

pub(crate) fn escape_llvm_string(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'\\' => out.push_str("\\5C"),
            b'"' => out.push_str("\\22"),
            0x20..=0x7E => out.push(b as char),
            _ => out.push_str(&format!("\\{:02X}", b)),
        }
    }
    out
}
