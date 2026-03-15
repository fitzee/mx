use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::analyze::{self, AnalysisResult, ScopeMap};
use crate::ast::{CompilationUnit, DefinitionModule};
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::symtab::SymbolTable;
use crate::types::TypeRegistry;

/// Cache entry for a parsed .def file, with mtime for invalidation.
struct DefCacheEntry {
    def_mod: DefinitionModule,
    mtime: SystemTime,
}

/// Cache for parsed .def files. Keyed by canonical path.
/// Invalidates entries when the file's mtime changes.
pub struct DefCache {
    entries: HashMap<PathBuf, DefCacheEntry>,
}

impl DefCache {
    pub fn new() -> Self {
        Self { entries: HashMap::new() }
    }

    fn get_or_parse(&mut self, path: &Path, m2plus: bool) -> Option<&DefinitionModule> {
        // Canonicalize path for consistent cache keys
        let key = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

        // Check if cached entry is still valid (mtime hasn't changed)
        let current_mtime = std::fs::metadata(&key)
            .and_then(|m| m.modified())
            .ok();

        let needs_reparse = if let Some(entry) = self.entries.get(&key) {
            match current_mtime {
                Some(mt) => mt != entry.mtime,
                None => true, // can't read mtime, re-parse
            }
        } else {
            true // not cached
        };

        if needs_reparse {
            let source = std::fs::read_to_string(&key).ok()?;
            let name = key.to_string_lossy().to_string();
            let mut lexer = Lexer::new(&source, &name);
            lexer.set_m2plus(m2plus);
            let tokens = lexer.tokenize().ok()?;
            let mut parser = Parser::new(tokens);
            if let Ok(CompilationUnit::DefinitionModule(def_mod)) = parser.parse_compilation_unit() {
                let mtime = current_mtime.unwrap_or(SystemTime::UNIX_EPOCH);
                self.entries.insert(key.clone(), DefCacheEntry { def_mod, mtime });
            }
        }

        self.entries.get(&key).map(|e| &e.def_mod)
    }
}

/// Analyze a source file: lex, parse, run sema (no codegen).
/// Returns the full AnalysisResult with symtab, types, scope_map, and diagnostics.
pub fn analyze(
    source: &str,
    filename: &str,
    m2plus: bool,
    include_paths: &[PathBuf],
    def_cache: &mut DefCache,
) -> AnalysisResult {
    // Collect def modules for imports.
    // Reverse so transitive deps (discovered later by BFS) are registered
    // before the modules that depend on them — poor man's topo sort.
    let mut def_modules = collect_def_modules(source, filename, m2plus, include_paths, def_cache);
    def_modules.reverse();
    let def_refs: Vec<&DefinitionModule> = def_modules.iter().collect();

    analyze::analyze_source(source, filename, m2plus, &def_refs)
}

/// Pre-parse the source to extract imports, then load .def files from disk/cache.
fn collect_def_modules(
    source: &str,
    filename: &str,
    m2plus: bool,
    include_paths: &[PathBuf],
    def_cache: &mut DefCache,
) -> Vec<DefinitionModule> {
    let mut result = Vec::new();

    // Quick lex+parse just to get imports (reuse the parser)
    let mut lexer = Lexer::new(source, filename);
    lexer.set_m2plus(m2plus);
    let tokens = match lexer.tokenize() {
        Ok(t) => t,
        Err(_) => return result,
    };
    let mut parser = Parser::new(tokens);
    let unit = match parser.parse_compilation_unit() {
        Ok(u) => u,
        Err(_) => return result,
    };

    let imports = match &unit {
        CompilationUnit::ProgramModule(m) => &m.imports,
        CompilationUnit::ImplementationModule(m) => &m.imports,
        _ => return result,
    };

    let input_path = Path::new(filename);

    // Track loaded paths to avoid registering the same .def twice.
    let mut loaded: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();

    // For implementation modules, load the corresponding .def file first.
    // Types/constants/procedures declared in the .def must be visible in the .mod.
    // Only look in the SAME directory — a .def found via include paths with the
    // same module name is a different module that happens to share the name.
    if let CompilationUnit::ImplementationModule(m) = &unit {
        if let Some(def_path) = find_def_file_same_dir(&m.name, input_path) {
            if let Some(def_mod) = def_cache.get_or_parse(&def_path, m2plus) {
                let canon = def_path.canonicalize().unwrap_or(def_path.clone());
                loaded.insert(canon);
                result.push(def_mod.clone());
            }
        }
    }

    // Collect module names to process in a queue for transitive loading
    let mut queue: std::collections::VecDeque<String> = std::collections::VecDeque::new();
    for imp in imports {
        if let Some(ref from_mod) = imp.from_module {
            queue.push_back(from_mod.clone());
        } else {
            for mod_name in &imp.names {
                queue.push_back(mod_name.name.clone());
            }
        }
    }

    // Process the queue, transitively loading imports from each .def file
    while let Some(mod_name) = queue.pop_front() {
        if crate::stdlib::is_stdlib_module(&mod_name) {
            continue;
        }
        if let Some(def_path) = find_def_file(&mod_name, input_path, include_paths) {
            let canon = def_path.canonicalize().unwrap_or(def_path.clone());
            if !loaded.contains(&canon) {
                if let Some(def_mod) = def_cache.get_or_parse(&def_path, m2plus) {
                    loaded.insert(canon);
                    // Enqueue transitive imports from this .def
                    for imp in &def_mod.imports {
                        if let Some(ref from_mod) = imp.from_module {
                            queue.push_back(from_mod.clone());
                        } else {
                            for name in &imp.names {
                                queue.push_back(name.name.clone());
                            }
                        }
                    }
                    result.push(def_mod.clone());
                }
            }
        }
    }

    result
}

/// Find a .def file ONLY in the same directory as the input file.
/// Used for implementation module's own .def — a .def from a different directory
/// with the same module name is a different module, not the corresponding definition.
fn find_def_file_same_dir(module_name: &str, input_path: &Path) -> Option<PathBuf> {
    let dir = input_path.parent().unwrap_or(Path::new("."));
    let candidates = [
        dir.join(format!("{}.def", module_name)),
        dir.join(format!("{}.DEF", module_name)),
        dir.join(format!("{}.def", module_name.to_lowercase())),
    ];
    for c in &candidates {
        if c.exists() { return Some(c.clone()); }
    }
    None
}

/// Unified find_def_file: searches same dir as input, then include paths.
pub fn find_def_file(module_name: &str, input_path: &Path, include_paths: &[PathBuf]) -> Option<PathBuf> {
    let dir = input_path.parent().unwrap_or(Path::new("."));
    let candidates = [
        dir.join(format!("{}.def", module_name)),
        dir.join(format!("{}.DEF", module_name)),
        dir.join(format!("{}.def", module_name.to_lowercase())),
    ];
    for c in &candidates {
        if c.exists() { return Some(c.clone()); }
    }
    for inc_dir in include_paths {
        let candidates = [
            inc_dir.join(format!("{}.def", module_name)),
            inc_dir.join(format!("{}.DEF", module_name)),
            inc_dir.join(format!("{}.def", module_name.to_lowercase())),
        ];
        for c in &candidates {
            if c.exists() { return Some(c.clone()); }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lsp_analyze_pim4_rejects_m2plus_syntax() {
        // Through the LSP analysis path, M2+ syntax should produce parse errors
        // in PIM4 mode (m2plus=false)
        let source = "MODULE Test; EXCEPTION Foo; BEGIN RAISE Foo END Test.";
        let mut def_cache = DefCache::new();
        let result = analyze(source, "test.mod", false, &[], &mut def_cache);
        // In PIM4 mode, EXCEPTION is an identifier — the parse should fail or
        // produce diagnostics because "EXCEPTION Foo;" doesn't match any declaration
        assert!(!result.diagnostics.is_empty(),
            "M2+ syntax should produce diagnostics in PIM4 mode via LSP path");
    }

    #[test]
    fn test_lsp_analyze_m2plus_accepts_m2plus_syntax() {
        // Same source should parse clean in M2+ mode
        let source = "MODULE Test; EXCEPTION Foo; BEGIN RAISE Foo END Test.";
        let mut def_cache = DefCache::new();
        let result = analyze(source, "test.mod", true, &[], &mut def_cache);
        // Should parse successfully — EXCEPTION is a keyword, RAISE is a statement
        let has_parse_error = result.diagnostics.iter().any(|e|
            format!("{}", e).contains("expected") || format!("{}", e).contains("parse"));
        assert!(!has_parse_error,
            "M2+ syntax should parse cleanly in M2+ mode via LSP path, got: {:?}",
            result.diagnostics);
    }

    #[test]
    fn test_lsp_analyze_pim4_allows_m2plus_identifiers() {
        // In PIM4 mode via LSP, M2+ keywords should work as identifiers
        let source = "MODULE Test; VAR OBJECT: INTEGER; BEGIN OBJECT := 42 END Test.";
        let mut def_cache = DefCache::new();
        let result = analyze(source, "test.mod", false, &[], &mut def_cache);
        assert!(result.diagnostics.is_empty(),
            "M2+ keywords as identifiers should work in PIM4 LSP mode, got: {:?}",
            result.diagnostics);
    }
}
