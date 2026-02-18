use std::collections::HashMap;
use std::path::{Path, PathBuf};
use crate::ast::{CompilationUnit, DefinitionModule};
use crate::codegen::CodeGen;
use crate::errors::CompileError;
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::symtab::SymbolTable;

/// Cache for parsed .def files. Keyed by canonical path.
pub struct DefCache {
    entries: HashMap<PathBuf, DefinitionModule>,
}

impl DefCache {
    pub fn new() -> Self {
        Self { entries: HashMap::new() }
    }

    fn get_or_parse(&mut self, path: &Path) -> Option<&DefinitionModule> {
        let key = path.to_path_buf();
        if !self.entries.contains_key(&key) {
            let source = std::fs::read_to_string(path).ok()?;
            let name = path.to_string_lossy().to_string();
            let mut lexer = Lexer::new(&source, &name);
            let tokens = lexer.tokenize().ok()?;
            let mut parser = Parser::new(tokens);
            if let Ok(CompilationUnit::DefinitionModule(def_mod)) = parser.parse_compilation_unit() {
                self.entries.insert(key.clone(), def_mod);
            }
        }
        self.entries.get(&key)
    }
}

/// Result of analyzing a document.
pub struct AnalysisResult {
    pub errors: Vec<CompileError>,
    pub unit: Option<CompilationUnit>,
    pub symtab: Option<SymbolTable>,
}

/// Analyze a source file: lex, parse, run sema. Returns errors and AST.
pub fn analyze(source: &str, filename: &str, m2plus: bool, include_paths: &[PathBuf], def_cache: &mut DefCache) -> AnalysisResult {
    let mut errors = Vec::new();

    // Lex
    let mut lexer = Lexer::new(source, filename);
    let tokens = match lexer.tokenize() {
        Ok(t) => t,
        Err(e) => {
            errors.push(e);
            return AnalysisResult { errors, unit: None, symtab: None };
        }
    };

    // Parse
    let mut parser = Parser::new(tokens);
    let unit = match parser.parse_compilation_unit() {
        Ok(u) => u,
        Err(e) => {
            let accumulated = parser.get_errors();
            if !accumulated.is_empty() {
                errors.extend_from_slice(accumulated);
            } else {
                errors.push(e);
            }
            return AnalysisResult { errors, unit: None, symtab: None };
        }
    };

    // Run codegen (which includes sema) to collect semantic errors
    let mut codegen = CodeGen::new();
    codegen.set_m2plus(m2plus);

    // Load imported module definitions if available
    let imports = match &unit {
        CompilationUnit::ProgramModule(m) => m.imports.clone(),
        CompilationUnit::ImplementationModule(m) => m.imports.clone(),
        _ => Vec::new(),
    };

    let input_path = Path::new(filename);
    for imp in &imports {
        if let Some(ref from_mod) = imp.from_module {
            if !crate::stdlib::is_stdlib_module(from_mod) {
                if let Some(def_path) = find_def_file(from_mod, input_path, include_paths) {
                    if let Some(def_mod) = def_cache.get_or_parse(&def_path) {
                        codegen.register_def_module(def_mod);
                    }
                }
            }
        } else {
            for mod_name in &imp.names {
                if !crate::stdlib::is_stdlib_module(mod_name) {
                    if let Some(def_path) = find_def_file(mod_name, input_path, include_paths) {
                        if let Some(def_mod) = def_cache.get_or_parse(&def_path) {
                            codegen.register_def_module(def_mod);
                        }
                    }
                }
            }
        }
    }

    match codegen.generate_or_errors(&unit) {
        Ok(_) => {}
        Err(errs) => {
            errors.extend(errs);
        }
    }

    let symtab = Some(codegen.take_symtab());

    AnalysisResult {
        errors,
        unit: Some(unit),
        symtab,
    }
}

fn find_def_file(module_name: &str, input_path: &Path, include_paths: &[PathBuf]) -> Option<PathBuf> {
    let dir = input_path.parent().unwrap_or(Path::new("."));
    let candidates = vec![
        dir.join(format!("{}.def", module_name)),
        dir.join(format!("{}.DEF", module_name)),
        dir.join(format!("{}.def", module_name.to_lowercase())),
    ];
    for c in &candidates {
        if c.exists() { return Some(c.clone()); }
    }
    for inc_dir in include_paths {
        let candidates = vec![
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
