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

    fn get_or_parse(&mut self, path: &Path) -> Option<&DefinitionModule> {
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
    _m2plus: bool,
    include_paths: &[PathBuf],
    def_cache: &mut DefCache,
) -> AnalysisResult {
    // Collect def modules for imports
    let def_modules = collect_def_modules(source, filename, include_paths, def_cache);
    let def_refs: Vec<&DefinitionModule> = def_modules.iter().collect();

    analyze::analyze_source(source, filename, &def_refs)
}

/// Pre-parse the source to extract imports, then load .def files from disk/cache.
fn collect_def_modules(
    source: &str,
    filename: &str,
    include_paths: &[PathBuf],
    def_cache: &mut DefCache,
) -> Vec<DefinitionModule> {
    let mut result = Vec::new();

    // Quick lex+parse just to get imports (reuse the parser)
    let mut lexer = Lexer::new(source, filename);
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
    for imp in imports {
        if let Some(ref from_mod) = imp.from_module {
            if !crate::stdlib::is_stdlib_module(from_mod) {
                if let Some(def_path) = find_def_file(from_mod, input_path, include_paths) {
                    if let Some(def_mod) = def_cache.get_or_parse(&def_path) {
                        result.push(def_mod.clone());
                    }
                }
            }
        } else {
            for mod_name in &imp.names {
                if !crate::stdlib::is_stdlib_module(mod_name) {
                    if let Some(def_path) = find_def_file(mod_name, input_path, include_paths) {
                        if let Some(def_mod) = def_cache.get_or_parse(&def_path) {
                            result.push(def_mod.clone());
                        }
                    }
                }
            }
        }
    }

    result
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
