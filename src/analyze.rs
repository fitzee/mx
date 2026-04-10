use std::collections::HashMap;

use crate::ast::{CompilationUnit, Declaration, DefinitionModule, ExprKind, Statement, StatementKind};
use crate::errors::CompileError;
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::sema::SemanticAnalyzer;
use crate::symtab::SymbolTable;
use crate::types::{Type, TypeId, TypeRegistry};

// ── ScopeMap ────────────────────────────────────────────────────────

/// A span mapping a source region to a scope.
#[derive(Debug, Clone)]
pub struct ScopeSpan {
    pub scope_id: usize,
    pub start_line: usize,
    pub start_col: usize,
    pub end_line: usize,
    pub end_col: usize,
}

/// Maps source positions to scope IDs for visibility-correct lookups.
#[derive(Debug, Clone)]
pub struct ScopeMap {
    spans: Vec<ScopeSpan>,
}

impl ScopeMap {
    pub fn new() -> Self {
        Self { spans: Vec::new() }
    }

    pub fn push(
        &mut self,
        scope_id: usize,
        start_line: usize,
        start_col: usize,
        end_line: usize,
        end_col: usize,
    ) {
        self.spans.push(ScopeSpan {
            scope_id,
            start_line,
            start_col,
            end_line,
            end_col,
        });
    }

    /// Find the innermost scope containing the given position (1-based line and col).
    pub fn scope_at(&self, line: usize, col: usize) -> usize {
        let mut best_scope = 0usize; // global scope
        let mut best_area = u64::MAX;
        for span in &self.spans {
            let contains = (line > span.start_line
                || (line == span.start_line && col >= span.start_col))
                && (line < span.end_line
                    || (line == span.end_line && col <= span.end_col));
            if contains {
                let area = ((span.end_line - span.start_line) as u64) * 10000
                    + (span.end_col.saturating_sub(span.start_col)) as u64;
                if area < best_area {
                    best_area = area;
                    best_scope = span.scope_id;
                }
            }
        }
        best_scope
    }

    pub fn spans(&self) -> &[ScopeSpan] {
        &self.spans
    }
}

impl Default for ScopeMap {
    fn default() -> Self {
        Self::new()
    }
}

// ── ReferenceIndex ──────────────────────────────────────────────────

/// A reference to a symbol at a specific source location.
#[derive(Debug, Clone)]
pub struct Reference {
    /// Source line (1-based).
    pub line: usize,
    /// Source column (1-based).
    pub col: usize,
    /// Length of the identifier token.
    pub len: usize,
    /// Scope ID where the symbol is *defined* (the symbol's identity).
    pub def_scope: usize,
    /// Symbol name.
    pub name: String,
    /// Whether this is the definition site.
    pub is_definition: bool,
}

/// Index of all symbol references in a source file.
/// Built during semantic analysis. Enables semantic rename and find-references.
#[derive(Debug, Clone)]
pub struct ReferenceIndex {
    refs: Vec<Reference>,
}

impl ReferenceIndex {
    pub fn new() -> Self {
        Self { refs: Vec::new() }
    }

    pub fn push(&mut self, r: Reference) {
        self.refs.push(r);
    }

    /// Find the reference at the given position (0-based line and col, as from LSP).
    pub fn at_position(&self, line: usize, col: usize) -> Option<&Reference> {
        let line1 = line + 1;
        let col1 = col + 1;
        self.refs.iter().find(|r| {
            r.line == line1 && col1 >= r.col && col1 < r.col + r.len
        })
    }

    /// Find all references to the same symbol as the given identity.
    pub fn find_all(&self, def_scope: usize, name: &str) -> Vec<&Reference> {
        self.refs
            .iter()
            .filter(|r| r.def_scope == def_scope && r.name == name)
            .collect()
    }

    pub fn refs(&self) -> &[Reference] {
        &self.refs
    }
}

impl Default for ReferenceIndex {
    fn default() -> Self {
        Self::new()
    }
}

// ── AnalysisResult ──────────────────────────────────────────────────

/// A call edge: caller → callee at a source location.
#[derive(Debug, Clone)]
pub struct CallEdge {
    pub callee: String,
    pub callee_module: Option<String>,
    pub line: usize,
    pub col: usize,
    /// End column of the callee identifier token (1-based, exclusive).
    pub end_col: usize,
}

/// Result of analyzing a source file: AST, symbol table, type registry,
/// scope map, reference index, call graph, and diagnostics.
#[derive(Clone)]
pub struct AnalysisResult {
    pub ast: Option<CompilationUnit>,
    pub symtab: SymbolTable,
    pub types: TypeRegistry,
    pub scope_map: ScopeMap,
    pub ref_index: ReferenceIndex,
    /// Call graph: procedure name → list of callees.
    pub call_graph: HashMap<String, Vec<CallEdge>>,
    pub diagnostics: Vec<CompileError>,
}

// ── analyze_source ──────────────────────────────────────────────────

/// Run lex → parse → sema on source text, returning all semantic artifacts
/// needed by the LSP. No C code generation is performed.
pub fn analyze_source(
    source: &str,
    filename: &str,
    m2plus: bool,
    def_modules: &[&DefinitionModule],
) -> AnalysisResult {
    let mut diagnostics = Vec::new();

    // Lex
    let mut lexer = Lexer::new(source, filename);
    lexer.set_m2plus(m2plus);
    let tokens = match lexer.tokenize() {
        Ok(t) => t,
        Err(e) => {
            diagnostics.push(e);
            return AnalysisResult {
                ast: None,
                symtab: SymbolTable::new(),
                types: TypeRegistry::new(),
                scope_map: ScopeMap::new(),
                ref_index: ReferenceIndex::new(),
                call_graph: HashMap::new(),
                diagnostics,
            };
        }
    };

    // Parse
    let mut parser = Parser::new(tokens);
    let unit = match parser.parse_compilation_unit() {
        Ok(u) => u,
        Err(e) => {
            let accumulated = parser.get_errors();
            if !accumulated.is_empty() {
                diagnostics.extend_from_slice(accumulated);
            } else {
                diagnostics.push(e);
            }
            return AnalysisResult {
                ast: None,
                symtab: SymbolTable::new(),
                types: TypeRegistry::new(),
                scope_map: ScopeMap::new(),
                ref_index: ReferenceIndex::new(),
                call_graph: HashMap::new(),
                diagnostics,
            };
        }
    };

    // Semantic analysis (without codegen)
    let mut sema = SemanticAnalyzer::new();
    // Two-pass registration: pre-register type names first so cross-module
    // qualified type references (e.g., Scheduler.Scheduler) resolve correctly.
    for def in def_modules {
        sema.pre_register_type_names(def);
    }
    for def in def_modules {
        sema.register_def_module(def);
    }
    // Clear scope_map/ref_index from .def registration — their position data
    // refers to other files and would shadow the main file's scopes.
    if !def_modules.is_empty() {
        sema.reset_position_artifacts();
    }
    let sema_ok = sema.analyze(&unit).is_ok(); // errors are captured inside sema

    // Tier 2 lint: build HIR + CFG and run dataflow-based checks.
    // Only attempt if sema succeeded (no hard errors) — HIR builder
    // assumes a well-typed AST.
    if sema_ok {
        let hir_lint_warnings = run_cfg_lint(&unit, &sema);
        diagnostics.extend(hir_lint_warnings);
    }

    let (symtab, types, scope_map, ref_index, errors) = sema.into_results();
    diagnostics.extend(errors);

    // Build call graph by walking AST procedure declarations.
    let call_graph = build_call_graph(&unit);

    // ── Warning suppression ─────────────────────────────────────────
    // Scan source for (*!Wxx*) pragmas.
    // Line-level: suppress that warning code on that line only.
    // File-level (line 1 or before MODULE): suppress globally.
    let suppressions = collect_suppressions(source);
    diagnostics.retain(|d| {
        if let Some(code) = d.code {
            !suppressions.is_suppressed(code, d.loc.line)
        } else {
            true
        }
    });

    AnalysisResult {
        ast: Some(unit),
        symtab,
        types,
        scope_map,
        ref_index,
        call_graph,
        diagnostics,
    }
}

// ── Warning suppression ─────────────────────────────────────────────

pub struct Suppressions {
    /// Warning codes suppressed for specific lines: (line_number, code).
    pub by_line: std::collections::HashSet<(usize, String)>,
    /// Warning codes suppressed for the entire file.
    pub file_wide: std::collections::HashSet<String>,
}

impl Suppressions {
    /// Check if a warning should be suppressed.
    pub fn is_suppressed(&self, code: &str, line: usize) -> bool {
        self.file_wide.contains(code) || self.by_line.contains(&(line, code.to_string()))
    }
}

/// Scan source for (*!Wxx*) suppression pragmas.
/// `(*!W06*)` on a line suppresses W06 for that line.
/// `(*!W06*)` before the MODULE keyword suppresses W06 file-wide.
pub fn collect_suppressions(source: &str) -> Suppressions {
    let mut by_line = std::collections::HashSet::new();
    let mut file_wide = std::collections::HashSet::new();
    let mut seen_module = false;

    for (line_idx, line) in source.lines().enumerate() {
        let line_num = line_idx + 1; // 1-based
        if !seen_module && line.contains("MODULE") {
            seen_module = true;
        }

        // Find all (*!Wxx*) patterns in this line
        let mut pos = 0;
        while let Some(start) = line[pos..].find("(*!") {
            let abs_start = pos + start + 3; // skip (*!
            if let Some(end) = line[abs_start..].find("*)") {
                let code = line[abs_start..abs_start + end].trim();
                // Accept W followed by digits (W01..W99)
                if code.starts_with('W') && code.len() >= 2 && code[1..].chars().all(|c| c.is_ascii_digit()) {
                    if !seen_module {
                        file_wide.insert(code.to_string());
                    } else {
                        by_line.insert((line_num, code.to_string()));
                    }
                }
                pos = abs_start + end + 2;
            } else {
                break;
            }
        }
    }

    Suppressions { by_line, file_wide }
}

// ── Tier 2 CFG lint ─────────────────────────────────────────────────

/// Build HIR + CFG for the given AST and run dataflow-based lint checks.
/// Returns warnings only — never errors.
fn run_cfg_lint(unit: &CompilationUnit, sema: &SemanticAnalyzer) -> Vec<CompileError> {
    use std::panic;

    // Build HIR from AST + sema. Catch panics since HIR builder may
    // not handle all edge cases gracefully on partially-valid ASTs.
    let hir_result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        crate::hir_build::build_module(unit, &[], sema)
    }));
    let mut hir_module = match hir_result {
        Ok(m) => m,
        Err(_) => return Vec::new(),
    };

    // Build CFGs for all procedures
    fn build_proc_cfgs(procs: &mut [crate::hir::HirProcDecl]) {
        for pd in procs.iter_mut() {
            if let Some(ref body) = pd.body {
                pd.cfg = Some(crate::cfg::build_cfg(body));
            }
            build_proc_cfgs(&mut pd.nested_procs);
        }
    }
    build_proc_cfgs(&mut hir_module.proc_decls);

    // Run lint on all procedures
    let mut warnings = Vec::new();
    fn lint_procs(procs: &[crate::hir::HirProcDecl], types: &TypeRegistry, warnings: &mut Vec<CompileError>) {
        for pd in procs {
            warnings.extend(crate::cfg::lint::lint_procedure(pd, types));
            lint_procs(&pd.nested_procs, types, warnings);
        }
    }
    lint_procs(&hir_module.proc_decls, &sema.types, &mut warnings);
    warnings
}

// ── Call graph builder ──────────────────────────────────────────────

fn build_call_graph(unit: &CompilationUnit) -> HashMap<String, Vec<CallEdge>> {
    let mut graph: HashMap<String, Vec<CallEdge>> = HashMap::new();

    let decls = match unit {
        CompilationUnit::ProgramModule(m) => &m.block.decls,
        CompilationUnit::ImplementationModule(m) => &m.block.decls,
        CompilationUnit::DefinitionModule(_) => return graph,
    };

    // Also collect calls from module body
    let body = match unit {
        CompilationUnit::ProgramModule(m) => m.block.body.as_ref(),
        CompilationUnit::ImplementationModule(m) => m.block.body.as_ref(),
        _ => None,
    };
    let module_name = match unit {
        CompilationUnit::ProgramModule(m) => &m.name,
        CompilationUnit::ImplementationModule(m) => &m.name,
        CompilationUnit::DefinitionModule(m) => &m.name,
    };

    collect_calls_from_decls(decls, &mut graph, "");

    // Module body calls (attributed to module name)
    if let Some(stmts) = body {
        let mut calls = Vec::new();
        collect_calls_from_stmts(Some(stmts), &mut calls);
        if !calls.is_empty() {
            graph.insert(module_name.clone(), calls);
        }
    }

    graph
}

/// Recursively walk procedure declarations, tracking the parent scope path.
/// Top-level procedures use their bare name.
/// Nested procedures get `name@parent` to disambiguate from same-named procs
/// in sibling scopes.
fn collect_calls_from_decls(
    decls: &[Declaration],
    graph: &mut HashMap<String, Vec<CallEdge>>,
    parent_path: &str,
) {
    for decl in decls {
        if let Declaration::Procedure(p) = decl {
            let key = if parent_path.is_empty() {
                p.heading.name.clone()
            } else {
                format!("{}@{}", p.heading.name, parent_path)
            };

            // Collect nested procedure names for callee resolution
            let nested_names: HashMap<String, String> = p.block.decls.iter()
                .filter_map(|d| {
                    if let Declaration::Procedure(np) = d {
                        Some((np.heading.name.clone(), format!("{}@{}", np.heading.name, key)))
                    } else {
                        None
                    }
                })
                .collect();

            let mut calls = Vec::new();
            collect_calls_from_stmts(p.block.body.as_ref(), &mut calls);

            // Rewrite callee names for nested procedures
            for call in &mut calls {
                if let Some(nested_key) = nested_names.get(&call.callee) {
                    call.callee = nested_key.clone();
                }
            }

            graph.insert(key.clone(), calls);
            // Recurse into nested declarations
            collect_calls_from_decls(&p.block.decls, graph, &key);
        }
    }
}

fn collect_calls_from_stmts(stmts: Option<&Vec<Statement>>, calls: &mut Vec<CallEdge>) {
    let stmts = match stmts {
        Some(s) => s,
        None => return,
    };
    for stmt in stmts {
        collect_calls_from_stmt(stmt, calls);
    }
}

/// Extract callee name, module, and callee identifier span from a designator.
/// Returns (name, module, callee_col, callee_end_col) where cols are 1-based.
/// Handles both `QualIdent { module: Some("B"), name: "ProcB" }` and
/// the case where `ident.name = "B"` with `selectors = [Field("ProcB")]`.
fn extract_call_target(desig: &crate::ast::Designator) -> (String, Option<String>, usize, usize) {
    if let Some(ref module) = desig.ident.module {
        // Qualified: module.name — the callee ident starts after "Module."
        let callee_col = desig.loc.col + module.len() + 1; // skip "Module."
        let callee_end_col = callee_col + desig.ident.name.len();
        (desig.ident.name.clone(), Some(module.clone()), callee_col, callee_end_col)
    } else if !desig.selectors.is_empty() {
        // Check if the first selector is a field access (module.proc pattern)
        if let crate::ast::Selector::Field(ref field_name, ref field_loc) = desig.selectors[0] {
            // ident.name is the module, field_name is the procedure
            let callee_col = field_loc.col;
            let callee_end_col = callee_col + field_name.len();
            (field_name.clone(), Some(desig.ident.name.clone()), callee_col, callee_end_col)
        } else {
            let callee_col = desig.loc.col;
            let callee_end_col = callee_col + desig.ident.name.len();
            (desig.ident.name.clone(), None, callee_col, callee_end_col)
        }
    } else {
        let callee_col = desig.loc.col;
        let callee_end_col = callee_col + desig.ident.name.len();
        (desig.ident.name.clone(), None, callee_col, callee_end_col)
    }
}

fn collect_calls_from_stmt(stmt: &Statement, calls: &mut Vec<CallEdge>) {
    match &stmt.kind {
        StatementKind::ProcCall { desig, args } => {
            let (name, module, callee_col, callee_end_col) = extract_call_target(desig);
            calls.push(CallEdge {
                callee: name,
                callee_module: module,
                line: desig.loc.line,
                col: callee_col,
                end_col: callee_end_col,
            });
            for arg in args {
                collect_calls_from_expr(arg, calls);
            }
        }
        StatementKind::Assign { expr, .. } => {
            collect_calls_from_expr(expr, calls);
        }
        StatementKind::If { cond, then_body, elsifs, else_body } => {
            collect_calls_from_expr(cond, calls);
            collect_calls_from_stmts(Some(then_body), calls);
            for (ec, eb) in elsifs {
                collect_calls_from_expr(ec, calls);
                collect_calls_from_stmts(Some(eb), calls);
            }
            if let Some(eb) = else_body {
                collect_calls_from_stmts(Some(eb), calls);
            }
        }
        StatementKind::While { cond, body } => {
            collect_calls_from_expr(cond, calls);
            collect_calls_from_stmts(Some(body), calls);
        }
        StatementKind::Repeat { body, cond } => {
            collect_calls_from_stmts(Some(body), calls);
            collect_calls_from_expr(cond, calls);
        }
        StatementKind::For { start, end, step, body, .. } => {
            collect_calls_from_expr(start, calls);
            collect_calls_from_expr(end, calls);
            if let Some(s) = step { collect_calls_from_expr(s, calls); }
            collect_calls_from_stmts(Some(body), calls);
        }
        StatementKind::Loop { body } => {
            collect_calls_from_stmts(Some(body), calls);
        }
        StatementKind::Case { expr, branches, else_body } => {
            collect_calls_from_expr(expr, calls);
            for b in branches {
                collect_calls_from_stmts(Some(&b.body), calls);
            }
            if let Some(eb) = else_body {
                collect_calls_from_stmts(Some(eb), calls);
            }
        }
        StatementKind::With { body, .. } => {
            collect_calls_from_stmts(Some(body), calls);
        }
        StatementKind::Return { expr } => {
            if let Some(e) = expr { collect_calls_from_expr(e, calls); }
        }
        StatementKind::Try { body, excepts, finally_body } => {
            collect_calls_from_stmts(Some(body), calls);
            for ec in excepts {
                collect_calls_from_stmts(Some(&ec.body), calls);
            }
            if let Some(fb) = finally_body {
                collect_calls_from_stmts(Some(fb), calls);
            }
        }
        StatementKind::Lock { body, .. } => {
            collect_calls_from_stmts(Some(body), calls);
        }
        StatementKind::TypeCase { branches, else_body, .. } => {
            for b in branches {
                collect_calls_from_stmts(Some(&b.body), calls);
            }
            if let Some(eb) = else_body {
                collect_calls_from_stmts(Some(eb), calls);
            }
        }
        _ => {}
    }
}

fn collect_calls_from_expr(expr: &crate::ast::Expr, calls: &mut Vec<CallEdge>) {
    match &expr.kind {
        ExprKind::FuncCall { desig, args } => {
            let (name, module, callee_col, callee_end_col) = extract_call_target(desig);
            calls.push(CallEdge {
                callee: name,
                callee_module: module,
                line: desig.loc.line,
                col: callee_col,
                end_col: callee_end_col,
            });
            for arg in args {
                collect_calls_from_expr(arg, calls);
            }
        }
        ExprKind::UnaryOp { operand, .. } | ExprKind::Not(operand) => {
            collect_calls_from_expr(operand, calls);
        }
        ExprKind::BinaryOp { left, right, .. } => {
            collect_calls_from_expr(left, calls);
            collect_calls_from_expr(right, calls);
        }
        _ => {}
    }
}

// ── type_to_string ──────────────────────────────────────────────────

/// Render a TypeId to a human-readable Modula-2 type string.
pub fn type_to_string(types: &TypeRegistry, id: TypeId) -> String {
    match types.get(id) {
        Type::Integer => "INTEGER".to_string(),
        Type::Cardinal => "CARDINAL".to_string(),
        Type::Real => "REAL".to_string(),
        Type::LongReal => "LONGREAL".to_string(),
        Type::Boolean => "BOOLEAN".to_string(),
        Type::Char => "CHAR".to_string(),
        Type::Bitset => "BITSET".to_string(),
        Type::Void => "VOID".to_string(),
        Type::Nil => "NIL".to_string(),
        Type::Word => "WORD".to_string(),
        Type::Byte => "BYTE".to_string(),
        Type::Address => "ADDRESS".to_string(),
        Type::LongInt => "LONGINT".to_string(),
        Type::LongCard => "LONGCARD".to_string(),
        Type::Complex => "COMPLEX".to_string(),
        Type::LongComplex => "LONGCOMPLEX".to_string(),
        Type::StringLit(n) => {
            if *n == 1 {
                "CHAR".to_string()
            } else {
                format!("ARRAY [0..{}] OF CHAR", n.saturating_sub(1))
            }
        }
        Type::Array {
            elem_type,
            low,
            high,
            ..
        } => {
            let elem = type_to_string(types, *elem_type);
            format!("ARRAY [{}..{}] OF {}", low, high, elem)
        }
        Type::OpenArray { elem_type } => {
            format!("ARRAY OF {}", type_to_string(types, *elem_type))
        }
        Type::Record { fields, .. } => {
            if fields.is_empty() {
                "RECORD END".to_string()
            } else {
                let field_strs: Vec<String> = fields
                    .iter()
                    .take(4)
                    .map(|f| format!("{}: {}", f.name, type_to_string(types, f.typ)))
                    .collect();
                let suffix = if fields.len() > 4 { "; ..." } else { "" };
                format!("RECORD {} {}END", field_strs.join("; "), suffix)
            }
        }
        Type::Pointer { base } => {
            format!("POINTER TO {}", type_to_string(types, *base))
        }
        Type::Set { base } => {
            format!("SET OF {}", type_to_string(types, *base))
        }
        Type::Enumeration { variants, .. } => {
            if variants.len() <= 6 {
                format!("({})", variants.join(", "))
            } else {
                format!(
                    "({}, ...) [{} values]",
                    variants[..3].join(", "),
                    variants.len()
                )
            }
        }
        Type::Subrange { low, high, .. } => format!("[{}..{}]", low, high),
        Type::ProcedureType {
            params,
            return_type,
        } => {
            let param_strs: Vec<String> = params
                .iter()
                .map(|p| {
                    let prefix = if p.is_var { "VAR " } else { "" };
                    format!("{}{}", prefix, type_to_string(types, p.typ))
                })
                .collect();
            let ret = match return_type {
                Some(rt) => format!(": {}", type_to_string(types, *rt)),
                None => String::new(),
            };
            format!("PROCEDURE({}){}", param_strs.join(", "), ret)
        }
        Type::Opaque { name, module } => {
            if module.is_empty() {
                name.clone()
            } else {
                format!("{}.{}", module, name)
            }
        }
        Type::Alias { name, target } => {
            format!("{} (= {})", name, type_to_string(types, *target))
        }
        Type::Ref { target, branded } => {
            let prefix = branded
                .as_ref()
                .map(|b| format!("BRANDED \"{}\" ", b))
                .unwrap_or_default();
            format!("{}REF {}", prefix, type_to_string(types, *target))
        }
        Type::RefAny => "REFANY".to_string(),
        Type::Object { name, .. } => {
            if name.is_empty() {
                "OBJECT".to_string()
            } else {
                format!("{} (OBJECT)", name)
            }
        }
        Type::Exception { name } => format!("EXCEPTION {}", name),
        Type::Error => "<error>".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scope_map_global_scope() {
        let map = ScopeMap::new();
        assert_eq!(map.scope_at(1, 1), 0);
        assert_eq!(map.scope_at(100, 1), 0);
    }

    #[test]
    fn test_scope_map_nested() {
        let mut map = ScopeMap::new();
        map.push(1, 1, 1, 20, 999); // module scope
        map.push(2, 5, 1, 15, 999); // procedure scope

        assert_eq!(map.scope_at(1, 1), 1);
        assert_eq!(map.scope_at(5, 1), 2);
        assert_eq!(map.scope_at(10, 1), 2);
        assert_eq!(map.scope_at(15, 1), 2);
        assert_eq!(map.scope_at(16, 1), 1);
        assert_eq!(map.scope_at(20, 1), 1);
        assert_eq!(map.scope_at(21, 1), 0);
    }

    #[test]
    fn test_scope_map_siblings() {
        let mut map = ScopeMap::new();
        map.push(1, 1, 1, 30, 999);
        map.push(2, 3, 1, 10, 999);
        map.push(3, 15, 1, 25, 999);

        assert_eq!(map.scope_at(5, 1), 2);
        assert_eq!(map.scope_at(12, 1), 1);
        assert_eq!(map.scope_at(20, 1), 3);
    }

    #[test]
    fn test_scope_map_column_precision() {
        let mut map = ScopeMap::new();
        map.push(1, 1, 1, 10, 999); // outer
        map.push(2, 5, 10, 5, 30); // inner (same line, cols 10-30)

        assert_eq!(map.scope_at(5, 5), 1); // before inner start col
        assert_eq!(map.scope_at(5, 15), 2); // inside inner
        assert_eq!(map.scope_at(5, 35), 1); // after inner end col
    }

    #[test]
    fn test_ref_index_at_position() {
        let mut idx = ReferenceIndex::new();
        idx.push(Reference {
            line: 3, col: 5, len: 4, def_scope: 1, name: "foo".into(), is_definition: true,
        });
        idx.push(Reference {
            line: 7, col: 3, len: 4, def_scope: 1, name: "foo".into(), is_definition: false,
        });

        // 0-based LSP positions → at_position converts
        assert!(idx.at_position(2, 4).is_some()); // line 3, col 5
        assert!(idx.at_position(2, 7).is_some()); // line 3, col 8 (still within "foo_")
        assert!(idx.at_position(2, 8).is_none()); // line 3, col 9 (past end)
        assert!(idx.at_position(6, 2).is_some()); // line 7, col 3
    }

    #[test]
    fn test_ref_index_find_all() {
        let mut idx = ReferenceIndex::new();
        idx.push(Reference {
            line: 1, col: 5, len: 1, def_scope: 1, name: "x".into(), is_definition: true,
        });
        idx.push(Reference {
            line: 5, col: 3, len: 1, def_scope: 1, name: "x".into(), is_definition: false,
        });
        idx.push(Reference {
            line: 3, col: 5, len: 1, def_scope: 2, name: "x".into(), is_definition: true,
        });

        let refs = idx.find_all(1, "x");
        assert_eq!(refs.len(), 2); // only the scope-1 "x" refs
        let refs2 = idx.find_all(2, "x");
        assert_eq!(refs2.len(), 1); // the scope-2 "x"
    }

    #[test]
    fn test_analyze_source_ref_index() {
        let source = "MODULE Test;\nVAR x: INTEGER;\nBEGIN\n  x := 42\nEND Test.\n";
        let result = analyze_source(source, "test.mod", false, &[]);
        assert!(result.diagnostics.is_empty());
        // Should have at least a definition ref for x and a use ref for x
        let x_refs: Vec<_> = result.ref_index.refs().iter()
            .filter(|r| r.name == "x")
            .collect();
        assert!(x_refs.len() >= 2, "expected at least 2 refs to 'x', got {}", x_refs.len());
        assert!(x_refs.iter().any(|r| r.is_definition), "should have a definition ref");
        assert!(x_refs.iter().any(|r| !r.is_definition), "should have a use ref");
        // Both should have the same def_scope (same symbol identity)
        let scope = x_refs[0].def_scope;
        assert!(x_refs.iter().all(|r| r.def_scope == scope));
    }

    #[test]
    fn test_type_to_string_builtins() {
        let types = TypeRegistry::new();
        assert_eq!(type_to_string(&types, 0), "INTEGER");
        assert_eq!(type_to_string(&types, 4), "BOOLEAN");
        assert_eq!(type_to_string(&types, 5), "CHAR");
    }

    #[test]
    fn test_type_to_string_array() {
        let mut types = TypeRegistry::new();
        let arr = types.register(Type::Array {
            index_type: 0,
            elem_type: 0,
            low: 0,
            high: 9,
        });
        assert_eq!(type_to_string(&types, arr), "ARRAY [0..9] OF INTEGER");
    }

    #[test]
    fn test_type_to_string_pointer() {
        let mut types = TypeRegistry::new();
        let ptr = types.register(Type::Pointer { base: 0 });
        assert_eq!(type_to_string(&types, ptr), "POINTER TO INTEGER");
    }

    #[test]
    fn test_type_to_string_procedure() {
        let mut types = TypeRegistry::new();
        use crate::types::ParamType;
        let proc_ty = types.register(Type::ProcedureType {
            params: vec![
                ParamType { is_var: false, typ: 0 },
                ParamType { is_var: true, typ: 4 },
            ],
            return_type: Some(0),
        });
        assert_eq!(
            type_to_string(&types, proc_ty),
            "PROCEDURE(INTEGER, VAR BOOLEAN): INTEGER"
        );
    }

    #[test]
    fn test_analyze_source_basic() {
        let source = "MODULE Test;\nVAR x: INTEGER;\nBEGIN\nEND Test.\n";
        let result = analyze_source(source, "test.mod", false, &[]);
        assert!(result.ast.is_some());
        assert!(result.diagnostics.is_empty());
        assert!(result.symtab.lookup_all("x").is_some());
    }

    #[test]
    fn test_analyze_source_with_error() {
        let source = "MODULE Broken;\nVAR x: ;\nBEGIN\nEND Broken.\n";
        let result = analyze_source(source, "broken.mod", false, &[]);
        assert!(!result.diagnostics.is_empty());
    }

    #[test]
    fn test_analyze_source_scope_map() {
        let source = "MODULE Test;\nPROCEDURE Foo;\nBEGIN\nEND Foo;\nBEGIN\nEND Test.\n";
        let result = analyze_source(source, "test.mod", false, &[]);
        assert!(result.ast.is_some());
        assert!(result.scope_map.spans().len() >= 1);
    }

    // ── RETURN validation tests ─────────────────────────────────────

    fn has_error(result: &AnalysisResult, substring: &str) -> bool {
        result.diagnostics.iter().any(|e| format!("{}", e).contains(substring))
    }

    #[test]
    fn test_return_with_expr_in_function_procedure() {
        // Function procedure with RETURN expr — should be fine
        let source = "MODULE Test;\nPROCEDURE Add(a, b: INTEGER): INTEGER;\nBEGIN\n  RETURN a + b\nEND Add;\nBEGIN\nEND Test.\n";
        let result = analyze_source(source, "test.mod", false, &[]);
        assert!(!has_error(&result, "RETURN"), "valid function RETURN should not error");
    }

    #[test]
    fn test_bare_return_in_function_procedure() {
        // Function procedure with bare RETURN — should error
        let source = "MODULE Test;\nPROCEDURE GetVal(): INTEGER;\nBEGIN\n  RETURN\nEND GetVal;\nBEGIN\nEND Test.\n";
        let result = analyze_source(source, "test.mod", false, &[]);
        assert!(has_error(&result, "function procedure requires RETURN"),
            "bare RETURN in function procedure should error, got: {:?}",
            result.diagnostics);
    }

    #[test]
    fn test_return_with_expr_in_proper_procedure() {
        // Proper procedure with RETURN expr — should error
        let source = "MODULE Test;\nPROCEDURE DoIt;\nBEGIN\n  RETURN 42\nEND DoIt;\nBEGIN\nEND Test.\n";
        let result = analyze_source(source, "test.mod", false, &[]);
        assert!(has_error(&result, "proper procedure must not return"),
            "RETURN expr in proper procedure should error, got: {:?}",
            result.diagnostics);
    }

    #[test]
    fn test_bare_return_in_proper_procedure() {
        // Proper procedure with bare RETURN — should be fine
        let source = "MODULE Test;\nPROCEDURE DoIt;\nBEGIN\n  RETURN\nEND DoIt;\nBEGIN\nEND Test.\n";
        let result = analyze_source(source, "test.mod", false, &[]);
        assert!(!has_error(&result, "RETURN"), "bare RETURN in proper procedure should be fine");
    }

    #[test]
    fn test_return_at_module_level() {
        // RETURN at module level (no expression) — should be fine
        let source = "MODULE Test;\nBEGIN\n  RETURN\nEND Test.\n";
        let result = analyze_source(source, "test.mod", false, &[]);
        assert!(!has_error(&result, "RETURN"), "module-level RETURN should be fine");
    }

    // ── Set constructor typing tests ────────────────────────────────

    #[test]
    fn test_set_constructor_bare_is_bitset() {
        // Bare {1, 2, 3} should type as BITSET
        let source = "MODULE Test;\nVAR s: BITSET;\nBEGIN\n  s := {1, 2, 3}\nEND Test.\n";
        let result = analyze_source(source, "test.mod", false, &[]);
        assert!(!has_error(&result, "incompatible"),
            "bare set constructor should be BITSET-compatible");
    }

    #[test]
    fn test_set_constructor_typed() {
        // BITSET{1, 2} with explicit type
        let source = "MODULE Test;\nVAR s: BITSET;\nBEGIN\n  s := BITSET{1, 2}\nEND Test.\n";
        let result = analyze_source(source, "test.mod", false, &[]);
        assert!(!has_error(&result, "incompatible"),
            "typed BITSET constructor should be compatible");
    }

    #[test]
    fn test_set_constructor_with_range() {
        // Set with range elements
        let source = "MODULE Test;\nVAR s: BITSET;\nBEGIN\n  s := {0..7}\nEND Test.\n";
        let result = analyze_source(source, "test.mod", false, &[]);
        assert!(!has_error(&result, "incompatible"),
            "set constructor with range should work");
    }

    #[test]
    fn test_set_constructor_empty() {
        // Empty set {}
        let source = "MODULE Test;\nVAR s: BITSET;\nBEGIN\n  s := {}\nEND Test.\n";
        let result = analyze_source(source, "test.mod", false, &[]);
        assert!(!has_error(&result, "incompatible"),
            "empty set constructor should be BITSET-compatible");
    }

    // ── CARDINAL underflow warning tests ─────────────────────────────

    #[test]
    fn test_for_cardinal_underflow_warning() {
        // FOR i := 0 TO n - 1 with CARDINAL variable should warn
        let source = "MODULE Test;\nVAR i, n: CARDINAL;\nBEGIN\n  FOR i := 0 TO n - 1 DO END\nEND Test.\n";
        let result = analyze_source(source, "test.mod", false, &[]);
        assert!(result.diagnostics.iter().any(|e| e.kind == crate::errors::ErrorKind::Warning && e.message.contains("underflow")),
            "FOR with CARDINAL subtraction in upper bound should warn, got: {:?}", result.diagnostics);
    }

    #[test]
    fn test_for_integer_no_warning() {
        // FOR i := 0 TO n - 1 with INTEGER variable should NOT warn
        let source = "MODULE Test;\nVAR i, n: INTEGER;\nBEGIN\n  FOR i := 0 TO n - 1 DO END\nEND Test.\n";
        let result = analyze_source(source, "test.mod", false, &[]);
        assert!(!result.diagnostics.iter().any(|e| e.kind == crate::errors::ErrorKind::Warning),
            "FOR with INTEGER subtraction should not warn");
    }

    #[test]
    fn test_for_cardinal_no_sub_no_warning() {
        // FOR i := 0 TO n with CARDINAL variable should NOT warn (no subtraction)
        let source = "MODULE Test;\nVAR i, n: CARDINAL;\nBEGIN\n  FOR i := 0 TO n DO END\nEND Test.\n";
        let result = analyze_source(source, "test.mod", false, &[]);
        assert!(!result.diagnostics.iter().any(|e| e.kind == crate::errors::ErrorKind::Warning),
            "FOR with CARDINAL but no subtraction should not warn");
    }

    // ── W01: Unsigned comparison against zero ────────────────────────

    fn has_warning(result: &AnalysisResult, substring: &str) -> bool {
        result.diagnostics.iter().any(|e|
            e.kind == crate::errors::ErrorKind::Warning && e.message.contains(substring))
    }

    #[test]
    fn test_cardinal_ge_zero_always_true() {
        let source = "MODULE Test;\nVAR c: CARDINAL;\nBEGIN\n  IF c >= 0 THEN END\nEND Test.\n";
        let result = analyze_source(source, "test.mod", false, &[]);
        assert!(has_warning(&result, "always true"),
            "CARDINAL >= 0 should warn always true, got: {:?}", result.diagnostics);
    }

    #[test]
    fn test_cardinal_lt_zero_always_false() {
        let source = "MODULE Test;\nVAR c: CARDINAL;\nBEGIN\n  IF c < 0 THEN END\nEND Test.\n";
        let result = analyze_source(source, "test.mod", false, &[]);
        assert!(has_warning(&result, "always false"),
            "CARDINAL < 0 should warn always false, got: {:?}", result.diagnostics);
    }

    #[test]
    fn test_integer_ge_zero_no_warning() {
        let source = "MODULE Test;\nVAR i: INTEGER;\nBEGIN\n  IF i >= 0 THEN END\nEND Test.\n";
        let result = analyze_source(source, "test.mod", false, &[]);
        assert!(!has_warning(&result, "always"),
            "INTEGER >= 0 should not warn");
    }

    // ── W02: Unsigned countdown loop ─────────────────────────────────

    #[test]
    fn test_unsigned_countdown_loop() {
        let source = "MODULE Test;\nVAR i: CARDINAL;\nBEGIN\n  i := 10;\n  WHILE i >= 0 DO DEC(i) END\nEND Test.\n";
        let result = analyze_source(source, "test.mod", false, &[]);
        assert!(has_warning(&result, "infinite loop"),
            "WHILE CARDINAL >= 0 with DEC should warn infinite loop, got: {:?}", result.diagnostics);
    }

    #[test]
    fn test_signed_countdown_no_warning() {
        let source = "MODULE Test;\nVAR i: INTEGER;\nBEGIN\n  i := 10;\n  WHILE i >= 0 DO DEC(i) END\nEND Test.\n";
        let result = analyze_source(source, "test.mod", false, &[]);
        assert!(!has_warning(&result, "infinite loop"),
            "WHILE INTEGER >= 0 with DEC should not warn");
    }

    // ── W03: Short-circuit safety ────────────────────────────────────

    #[test]
    fn test_short_circuit_deref_after_nil_check() {
        let source = "MODULE Test;\nTYPE Ptr = POINTER TO RECORD x: INTEGER END;\nVAR p: Ptr;\nBEGIN\n  IF (p # NIL) AND (p^.x = 1) THEN END\nEND Test.\n";
        let result = analyze_source(source, "test.mod", false, &[]);
        assert!(has_warning(&result, "short-circuit"),
            "AND with NIL check + deref should warn, got: {:?}", result.diagnostics);
    }

    #[test]
    fn test_no_short_circuit_warning_without_nil() {
        let source = "MODULE Test;\nVAR a, b: BOOLEAN;\nBEGIN\n  IF a AND b THEN END\nEND Test.\n";
        let result = analyze_source(source, "test.mod", false, &[]);
        assert!(!has_warning(&result, "short-circuit"),
            "plain AND without NIL/deref should not warn");
    }

    // ── W05: INC/DEC on bounded subrange ─────────────────────────────

    #[test]
    fn test_inc_on_subrange() {
        let source = "MODULE Test;\nTYPE Idx = [0..15];\nVAR i: Idx;\nBEGIN\n  INC(i)\nEND Test.\n";
        let result = analyze_source(source, "test.mod", false, &[]);
        assert!(has_warning(&result, "overflow"),
            "INC on subrange should warn about overflow, got: {:?}", result.diagnostics);
    }

    #[test]
    fn test_dec_on_subrange() {
        let source = "MODULE Test;\nTYPE Idx = [0..15];\nVAR i: Idx;\nBEGIN\n  DEC(i)\nEND Test.\n";
        let result = analyze_source(source, "test.mod", false, &[]);
        assert!(has_warning(&result, "underflow"),
            "DEC on subrange should warn about underflow, got: {:?}", result.diagnostics);
    }

    #[test]
    fn test_inc_on_integer_no_warning() {
        let source = "MODULE Test;\nVAR i: INTEGER;\nBEGIN\n  INC(i)\nEND Test.\n";
        let result = analyze_source(source, "test.mod", false, &[]);
        assert!(!has_warning(&result, "overflow"),
            "INC on INTEGER should not warn");
    }

    // ── W06: SYSTEM import warning ───────────────────────────────────

    #[test]
    fn test_system_import_warning() {
        let source = "MODULE Test;\nFROM SYSTEM IMPORT ADDRESS;\nBEGIN\nEND Test.\n";
        let result = analyze_source(source, "test.mod", false, &[]);
        assert!(has_warning(&result, "SYSTEM"),
            "FROM SYSTEM IMPORT should warn, got: {:?}", result.diagnostics);
    }

    #[test]
    fn test_non_system_import_no_warning() {
        let source = "MODULE Test;\nFROM Storage IMPORT ALLOCATE;\nBEGIN\nEND Test.\n";
        let result = analyze_source(source, "test.mod", false, &[]);
        assert!(!has_warning(&result, "SYSTEM"),
            "FROM Storage IMPORT should not warn");
    }

    // ── W08: Mixed signed/unsigned arithmetic ────────────────────────

    #[test]
    fn test_mixed_signed_unsigned() {
        let source = "MODULE Test;\nVAR i: INTEGER; c: CARDINAL;\nBEGIN\n  i := i + c\nEND Test.\n";
        let result = analyze_source(source, "test.mod", false, &[]);
        assert!(has_warning(&result, "mixed signed/unsigned"),
            "INTEGER + CARDINAL should warn, got: {:?}", result.diagnostics);
    }

    #[test]
    fn test_same_signedness_no_warning() {
        let source = "MODULE Test;\nVAR a, b: INTEGER;\nBEGIN\n  a := a + b\nEND Test.\n";
        let result = analyze_source(source, "test.mod", false, &[]);
        assert!(!has_warning(&result, "mixed signed/unsigned"),
            "INTEGER + INTEGER should not warn");
    }

    // ── W09: FOR unsigned subtraction (already tested above) ─────────

    // ── Warnings don't block compilation ─────────────────────────────

    // ── W10: Uninitialized variable (CFG-based, via LSP path) ─────────

    #[test]
    fn test_uninit_var_via_lsp_path() {
        // Variable used before assignment should produce a warning
        // through the LSP analysis path (analyze_source → HIR → CFG → lint)
        let source = "MODULE Test;\nPROCEDURE Foo(): INTEGER;\nVAR x, y: INTEGER;\nBEGIN\n  y := x + 1;\n  RETURN y\nEND Foo;\nBEGIN\nEND Test.\n";
        let result = analyze_source(source, "test.mod", false, &[]);
        assert!(has_warning(&result, "used before being assigned"),
            "uninit var should produce warning via LSP path, got: {:?}", result.diagnostics);
    }

    #[test]
    fn test_uninit_var_path_sensitive_via_lsp() {
        // Variable assigned in THEN but not ELSE — should warn
        let source = "MODULE Test;\nPROCEDURE Foo(flag: BOOLEAN): INTEGER;\nVAR x: INTEGER;\nBEGIN\n  IF flag THEN x := 10 END;\n  RETURN x\nEND Foo;\nBEGIN\nEND Test.\n";
        let result = analyze_source(source, "test.mod", false, &[]);
        assert!(has_warning(&result, "used before being assigned"),
            "path-sensitive uninit should warn via LSP path, got: {:?}", result.diagnostics);
    }

    #[test]
    fn test_no_uninit_warning_when_assigned() {
        // All variables properly assigned before use — no warning
        let source = "MODULE Test;\nPROCEDURE Foo(): INTEGER;\nVAR a, b: INTEGER;\nBEGIN\n  a := 1;\n  b := a + 2;\n  RETURN b\nEND Foo;\nBEGIN\nEND Test.\n";
        let result = analyze_source(source, "test.mod", false, &[]);
        assert!(!has_warning(&result, "used before being assigned"),
            "properly assigned vars should not warn");
    }

    // ── W11: Pointer NIL safety ────────────────────────────────────────

    #[test]
    fn test_nil_deref_uninitialized_pointer() {
        let source = "MODULE Test;\nTYPE Node = POINTER TO RECORD val: INTEGER END;\nPROCEDURE Bad(): INTEGER;\nVAR p: Node;\nBEGIN\n  RETURN p^.val\nEND Bad;\nBEGIN\nEND Test.\n";
        let result = analyze_source(source, "test.mod", false, &[]);
        assert!(has_warning(&result, "may be NIL"),
            "deref of uninitialized pointer should warn W11, got: {:?}", result.diagnostics);
    }

    #[test]
    fn test_no_nil_deref_after_new() {
        let source = "MODULE Test;\nTYPE Node = POINTER TO RECORD val: INTEGER END;\nPROCEDURE Good(): INTEGER;\nVAR p: Node;\nBEGIN\n  NEW(p);\n  p^.val := 42;\n  RETURN p^.val\nEND Good;\nBEGIN\nEND Test.\n";
        let result = analyze_source(source, "test.mod", false, &[]);
        assert!(!has_warning(&result, "may be NIL"),
            "pointer after NEW should not warn W11");
    }

    #[test]
    fn test_nil_deref_after_nil_assign() {
        let source = "MODULE Test;\nTYPE Node = POINTER TO RECORD val: INTEGER END;\nPROCEDURE Bad(): INTEGER;\nVAR p: Node;\nBEGIN\n  NEW(p);\n  p := NIL;\n  RETURN p^.val\nEND Bad;\nBEGIN\nEND Test.\n";
        let result = analyze_source(source, "test.mod", false, &[]);
        assert!(has_warning(&result, "may be NIL"),
            "deref after NIL assignment should warn W11, got: {:?}", result.diagnostics);
    }

    // ── Warnings don't block compilation ─────────────────────────────

    #[test]
    fn test_warnings_dont_block_analysis() {
        // Code with a warning (CARDINAL >= 0) should still analyze successfully
        let source = "MODULE Test;\nVAR c: CARDINAL;\nBEGIN\n  IF c >= 0 THEN c := 1 END\nEND Test.\n";
        let result = analyze_source(source, "test.mod", false, &[]);
        assert!(result.ast.is_some(), "AST should be present despite warnings");
        // Should have warning but no errors
        let warnings: Vec<_> = result.diagnostics.iter().filter(|e| e.kind == crate::errors::ErrorKind::Warning).collect();
        let errors: Vec<_> = result.diagnostics.iter().filter(|e| e.kind != crate::errors::ErrorKind::Warning).collect();
        assert!(!warnings.is_empty(), "should have warnings");
        assert!(errors.is_empty(), "should have no errors");
    }

    // ── Warning suppression tests ────────────────────────────────────

    #[test]
    fn test_line_level_suppression() {
        // (*!W06*) on the SYSTEM import line should suppress the warning
        let source = "MODULE Test;\nFROM SYSTEM IMPORT ADDRESS; (*!W06*)\nBEGIN\nEND Test.\n";
        let result = analyze_source(source, "test.mod", false, &[]);
        assert!(!has_warning(&result, "SYSTEM"),
            "W06 should be suppressed by line-level pragma");
    }

    #[test]
    fn test_file_wide_suppression() {
        // (*!W06*) before MODULE should suppress file-wide
        let source = "(*!W06*)\nMODULE Test;\nFROM SYSTEM IMPORT ADDRESS;\nFROM SYSTEM IMPORT ADR;\nBEGIN\nEND Test.\n";
        let result = analyze_source(source, "test.mod", false, &[]);
        assert!(!has_warning(&result, "SYSTEM"),
            "W06 should be suppressed file-wide");
    }

    #[test]
    fn test_suppression_does_not_affect_other_codes() {
        // Suppressing W06 should not suppress W01
        let source = "MODULE Test;\nFROM SYSTEM IMPORT ADDRESS; (*!W06*)\nVAR c: CARDINAL;\nBEGIN\n  IF c >= 0 THEN END\nEND Test.\n";
        let result = analyze_source(source, "test.mod", false, &[]);
        assert!(!has_warning(&result, "SYSTEM"), "W06 should be suppressed");
        assert!(has_warning(&result, "always true"), "W01 should NOT be suppressed");
    }

    #[test]
    fn test_warning_codes_present() {
        // Warnings should carry their code
        let source = "MODULE Test;\nVAR c: CARDINAL;\nBEGIN\n  IF c >= 0 THEN END\nEND Test.\n";
        let result = analyze_source(source, "test.mod", false, &[]);
        let w01 = result.diagnostics.iter().find(|e|
            e.kind == crate::errors::ErrorKind::Warning && e.message.contains("always true"));
        assert!(w01.is_some(), "should have W01 warning");
        assert_eq!(w01.unwrap().code, Some("W01"), "warning should carry code W01");
    }
}
