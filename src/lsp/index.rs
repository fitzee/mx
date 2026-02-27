use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::analyze::AnalysisResult;
use crate::ast::{CompilationUnit, Declaration, Definition};
use crate::symtab::{SymbolTable, SymbolKind};
use super::analysis::DefCache;
use super::documents::path_to_uri;

// ── SymbolIdentity ──────────────────────────────────────────────────

/// A stable identity for a symbol, strong enough to disambiguate two local
/// procedures with the same name in different scopes of the same module.
///
/// Fields:
///   file      — canonical defining file path (empty if unknown)
///   scope_id  — defining scope ID (0 for global, module scope otherwise)
///   module    — module name (for cross-file matching)
///   name      — symbol name
///   kind      — symbol kind
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct SymbolIdentity {
    pub file: String,
    pub scope_id: usize,
    pub module: String,
    pub name: String,
    pub kind: IdentityKind,
}

/// String key for HashMap lookups.
pub type IdentityKey = String;

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub enum IdentityKind {
    Procedure,
    Type,
    Variable,
    Constant,
    Module,
}

impl IdentityKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            IdentityKind::Procedure => "proc",
            IdentityKind::Type => "type",
            IdentityKind::Variable => "var",
            IdentityKind::Constant => "const",
            IdentityKind::Module => "mod",
        }
    }

    pub fn from_symbol_kind(kind: &SymbolKind) -> Self {
        match kind {
            SymbolKind::Procedure { .. } => IdentityKind::Procedure,
            SymbolKind::Type => IdentityKind::Type,
            SymbolKind::Variable => IdentityKind::Variable,
            SymbolKind::Constant(_) => IdentityKind::Constant,
            SymbolKind::Module { .. } => IdentityKind::Module,
            SymbolKind::Field => IdentityKind::Variable,
            SymbolKind::EnumVariant(_) => IdentityKind::Constant,
        }
    }
}

impl SymbolIdentity {
    /// Cross-file key: "Module::Name::kind". Used for inverted indexes.
    pub fn make_key(module: &str, name: &str, kind: IdentityKind) -> IdentityKey {
        format!("{}::{}::{}", module, name, kind.as_str())
    }

    /// Cross-file key (backwards-compatible, used by workspace index).
    pub fn key(&self) -> IdentityKey {
        Self::make_key(&self.module, &self.name, self.kind)
    }

    /// Local key including file + scope_id for intra-module disambiguation.
    /// Two local procs with the same name in different scopes get different keys.
    pub fn local_key(&self) -> IdentityKey {
        format!("{}::{}::{}::{}", self.file, self.scope_id, self.name, self.kind.as_str())
    }
}

/// Resolve a symbol name to a SymbolIdentity using the given symtab.
/// `filename` is the canonical file path where the symbol is being resolved.
pub fn resolve_identity(symtab: &SymbolTable, name: &str, filename: &str) -> Option<SymbolIdentity> {
    let (scope_id, sym) = symtab.lookup_with_scope(name)?;
    let module = sym.module.as_ref()?;
    Some(SymbolIdentity {
        file: filename.to_string(),
        scope_id,
        module: module.clone(),
        name: name.to_string(),
        kind: IdentityKind::from_symbol_kind(&sym.kind),
    })
}

// ── WorkspaceSymbol ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct WorkspaceSymbol {
    pub name: String,
    pub kind: SymbolKindTag,
    pub file_uri: String,
    pub line: usize,   // 1-based (from SourceLoc)
    pub col: usize,    // 1-based (from SourceLoc)
    pub container: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKindTag {
    Module,
    Procedure,
    Type,
    Constant,
    Variable,
}

impl SymbolKindTag {
    pub fn to_lsp_kind(self) -> i64 {
        match self {
            SymbolKindTag::Module => 2,
            SymbolKindTag::Procedure => 12,
            SymbolKindTag::Type => 5,
            SymbolKindTag::Constant => 14,
            SymbolKindTag::Variable => 13,
        }
    }
}

// ── CrossFileRef (name-based fallback) ──────────────────────────────

#[derive(Debug, Clone)]
pub struct CrossFileRef {
    pub file_uri: String,
    pub line: usize,
    pub col: usize,
    pub len: usize,
}

// ── IdentityRef (identity-based, primary) ───────────────────────────

#[derive(Debug, Clone)]
pub struct IdentityRef {
    pub file_uri: String,
    pub line: usize,
    pub col: usize,
    pub len: usize,
    pub is_definition: bool,
}

#[derive(Debug, Clone)]
pub struct IdentityLocation {
    pub file_uri: String,
    pub line: usize,
    pub col: usize,
    pub len: usize,
}

// ── FileStamp ───────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
struct FileStamp {
    mtime: SystemTime,
    size: u64,
    content_hash: u64,
}

impl FileStamp {
    fn from_path(path: &Path) -> Option<FileStamp> {
        let meta = std::fs::metadata(path).ok()?;
        let mtime = meta.modified().ok()?;
        let size = meta.len();
        let content = std::fs::read(path).ok()?;
        let content_hash = fnv1a(&content);
        Some(FileStamp { mtime, size, content_hash })
    }

    fn from_content(content: &[u8]) -> FileStamp {
        FileStamp {
            mtime: SystemTime::UNIX_EPOCH,
            size: content.len() as u64,
            content_hash: fnv1a(content),
        }
    }
}

fn fnv1a(data: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

// ── IndexedRef (name-based fallback) ────────────────────────────────

#[derive(Debug, Clone)]
struct IndexedRef {
    file_uri: String,
    module_name: Option<String>,
    line: usize,
    col: usize,
    len: usize,
}

// ── IndexedFile ─────────────────────────────────────────────────────

struct IndexedFile {
    uri: String,
    stamp: FileStamp,
    analysis: AnalysisResult,
}

// ── WsCallEdge ─────────────────────────────────────────────────────

/// A call edge in the workspace call graph. Connects a caller identity to a
/// callee identity (for outgoing) or vice versa (for incoming).
#[derive(Debug, Clone)]
pub struct WsCallEdge {
    /// Identity key of the other end (callee for calls_out, caller for calls_in).
    pub other_key: IdentityKey,
    /// Display name of the other end.
    pub other_name: String,
    /// URI of the file where the call occurs.
    pub site_uri: String,
    /// Call site line (1-based).
    pub site_line: usize,
    /// Call site column of the callee identifier (1-based).
    pub site_col: usize,
    /// Call site end column of the callee identifier (1-based, exclusive).
    pub site_end_col: usize,
}

// ── WorkspaceIndex ──────────────────────────────────────────────────

pub struct WorkspaceIndex {
    files: HashMap<PathBuf, IndexedFile>,

    // Flat symbol list (rebuilt on dirty)
    symbols: Vec<WorkspaceSymbol>,

    // Inverted indexes — name-based (fallback + workspace/symbol search)
    symbols_by_name: HashMap<String, Vec<usize>>,
    refs_by_name: HashMap<String, Vec<IndexedRef>>,

    // Inverted indexes — identity-based (primary for cross-file refs/rename)
    refs_by_identity: HashMap<IdentityKey, Vec<IdentityRef>>,
    defs_by_identity: HashMap<IdentityKey, IdentityLocation>,

    // Workspace call graph — identity-based
    calls_out: HashMap<IdentityKey, Vec<WsCallEdge>>,
    calls_in: HashMap<IdentityKey, Vec<WsCallEdge>>,
    /// Per-file contribution tracking: canonical path → list of (caller_key, callee_key).
    file_call_edges: HashMap<PathBuf, Vec<(IdentityKey, IdentityKey)>>,

    dirty: bool,
}

impl WorkspaceIndex {
    pub fn new() -> Self {
        Self {
            files: HashMap::new(),
            symbols: Vec::new(),
            symbols_by_name: HashMap::new(),
            refs_by_name: HashMap::new(),
            refs_by_identity: HashMap::new(),
            defs_by_identity: HashMap::new(),
            calls_out: HashMap::new(),
            calls_in: HashMap::new(),
            file_call_edges: HashMap::new(),
            dirty: false,
        }
    }

    /// Scan directories for .def/.mod files and index each one.
    /// Returns the number of files indexed (for progress reporting).
    pub fn index_directories(
        &mut self,
        dirs: &[PathBuf],
        m2plus: bool,
        include_paths: &[PathBuf],
        def_cache: &mut DefCache,
    ) -> usize {
        let mut count = 0;
        for dir in dirs {
            if !dir.is_dir() {
                continue;
            }
            let entries = match std::fs::read_dir(dir) {
                Ok(e) => e,
                Err(_) => continue,
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    match ext {
                        "def" | "DEF" | "mod" | "MOD" => {
                            if self.index_file(&path, m2plus, include_paths, def_cache) {
                                count += 1;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        count
    }

    /// Index a single file from disk. Skip if FileStamp unchanged.
    pub fn index_file(
        &mut self,
        path: &Path,
        m2plus: bool,
        include_paths: &[PathBuf],
        def_cache: &mut DefCache,
    ) -> bool {
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

        let new_stamp = match FileStamp::from_path(&canonical) {
            Some(s) => s,
            None => return false,
        };

        if let Some(existing) = self.files.get(&canonical) {
            if existing.stamp == new_stamp {
                return false;
            }
        }

        let source = match std::fs::read_to_string(&canonical) {
            Ok(s) => s,
            Err(_) => return false,
        };

        let filename = canonical.to_string_lossy().to_string();
        let uri = path_to_uri(&filename);
        let result = super::analysis::analyze(&source, &filename, m2plus, include_paths, def_cache);

        self.files.insert(canonical, IndexedFile { uri, stamp: new_stamp, analysis: result });
        self.dirty = true;
        true
    }

    /// Index a file from in-memory content (for open documents).
    pub fn index_from_analysis(
        &mut self,
        path: &Path,
        uri: &str,
        source: &str,
        analysis: AnalysisResult,
    ) {
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let stamp = FileStamp::from_content(source.as_bytes());
        self.files.insert(canonical, IndexedFile {
            uri: uri.to_string(),
            stamp,
            analysis,
        });
        self.dirty = true;
    }

    /// Force reindex: clear all files and stamps.
    pub fn force_clear(&mut self) {
        self.files.clear();
        self.symbols.clear();
        self.symbols_by_name.clear();
        self.refs_by_name.clear();
        self.refs_by_identity.clear();
        self.defs_by_identity.clear();
        self.calls_out.clear();
        self.calls_in.clear();
        self.file_call_edges.clear();
        self.dirty = false;
    }

    /// Rebuild flat symbol list and all inverted indexes from indexed files if dirty.
    pub fn rebuild_if_dirty(&mut self) {
        if !self.dirty {
            return;
        }

        self.symbols.clear();
        self.symbols_by_name.clear();
        self.refs_by_name.clear();
        self.refs_by_identity.clear();
        self.defs_by_identity.clear();
        self.calls_out.clear();
        self.calls_in.clear();
        self.file_call_edges.clear();

        for indexed in self.files.values() {
            let ast = match &indexed.analysis.ast {
                Some(a) => a,
                None => continue,
            };

            let module_name = match ast {
                CompilationUnit::ProgramModule(m) => &m.name,
                CompilationUnit::DefinitionModule(m) => &m.name,
                CompilationUnit::ImplementationModule(m) => &m.name,
            };

            let module_loc = match ast {
                CompilationUnit::ProgramModule(m) => &m.loc,
                CompilationUnit::DefinitionModule(m) => &m.loc,
                CompilationUnit::ImplementationModule(m) => &m.loc,
            };

            // Module symbol
            let idx = self.symbols.len();
            self.symbols.push(WorkspaceSymbol {
                name: module_name.clone(),
                kind: SymbolKindTag::Module,
                file_uri: indexed.uri.clone(),
                line: module_loc.line,
                col: module_loc.col,
                container: None,
            });
            self.symbols_by_name
                .entry(module_name.to_lowercase())
                .or_default()
                .push(idx);

            // Declarations/definitions
            match ast {
                CompilationUnit::DefinitionModule(m) => {
                    for def in &m.definitions {
                        add_definition_symbols(
                            &mut self.symbols,
                            &mut self.symbols_by_name,
                            def, &indexed.uri, module_name,
                        );
                    }
                }
                CompilationUnit::ProgramModule(m) => {
                    for decl in &m.block.decls {
                        add_declaration_symbols(
                            &mut self.symbols,
                            &mut self.symbols_by_name,
                            decl, &indexed.uri, module_name,
                        );
                    }
                }
                CompilationUnit::ImplementationModule(m) => {
                    for decl in &m.block.decls {
                        add_declaration_symbols(
                            &mut self.symbols,
                            &mut self.symbols_by_name,
                            decl, &indexed.uri, module_name,
                        );
                    }
                }
            }

            // Build refs from this file's ref_index
            for r in indexed.analysis.ref_index.refs() {
                let sym = indexed.analysis.symtab.lookup_all(&r.name);
                let ref_module = sym.and_then(|s| s.module.clone());
                let sym_kind = sym.map(|s| IdentityKind::from_symbol_kind(&s.kind));

                // Identity-based index (primary)
                if let (Some(ref module), Some(kind)) = (&ref_module, sym_kind) {
                    let key = SymbolIdentity::make_key(module, &r.name, kind);

                    self.refs_by_identity.entry(key.clone()).or_default().push(IdentityRef {
                        file_uri: indexed.uri.clone(),
                        line: r.line,
                        col: r.col,
                        len: r.len,
                        is_definition: r.is_definition,
                    });

                    if r.is_definition {
                        self.defs_by_identity.entry(key).or_insert(IdentityLocation {
                            file_uri: indexed.uri.clone(),
                            line: r.line,
                            col: r.col,
                            len: r.len,
                        });
                    }
                }

                // Name-based fallback
                self.refs_by_name.entry(r.name.clone()).or_default().push(IndexedRef {
                    file_uri: indexed.uri.clone(),
                    module_name: ref_module,
                    line: r.line,
                    col: r.col,
                    len: r.len,
                });
            }
        }

        // Second pass: build workspace call graph from per-file call_graph maps.
        // We need defs_by_identity to be fully populated first (done above).
        let file_data: Vec<(PathBuf, String, String, HashMap<String, Vec<crate::analyze::CallEdge>>, crate::symtab::SymbolTable)> =
            self.files.iter().map(|(path, indexed)| {
                let module_name = indexed.analysis.ast.as_ref().map(|ast| match ast {
                    CompilationUnit::ProgramModule(m) => m.name.clone(),
                    CompilationUnit::DefinitionModule(m) => m.name.clone(),
                    CompilationUnit::ImplementationModule(m) => m.name.clone(),
                }).unwrap_or_default();
                (
                    path.clone(),
                    indexed.uri.clone(),
                    module_name,
                    indexed.analysis.call_graph.clone(),
                    indexed.analysis.symtab.clone(),
                )
            }).collect();

        for (canonical_path, file_uri, module_name, call_graph, symtab) in &file_data {
            if module_name.is_empty() {
                continue;
            }

            let canonical_str = canonical_path.to_string_lossy();

            let mut file_edges = Vec::new();

            for (caller_name, edges) in call_graph {
                // Use strong key for nested/local procedures (scope_id > 1)
                let caller_key = make_proc_key(&canonical_str, module_name, caller_name, symtab);

                for edge in edges {
                    // Resolve callee module: explicit module qualifier, symtab lookup, or same module
                    let callee_module = match &edge.callee_module {
                        Some(m) => m.clone(),
                        None => {
                            symtab.lookup_all(&edge.callee)
                                .and_then(|s| s.module.clone())
                                .unwrap_or_else(|| module_name.clone())
                        }
                    };
                    let callee_key = make_proc_key(&canonical_str, &callee_module, &edge.callee, symtab);

                    // Outgoing edge: caller → callee
                    self.calls_out.entry(caller_key.clone()).or_default().push(WsCallEdge {
                        other_key: callee_key.clone(),
                        other_name: edge.callee.clone(),
                        site_uri: file_uri.clone(),
                        site_line: edge.line,
                        site_col: edge.col,
                        site_end_col: edge.end_col,
                    });

                    // Incoming edge: callee ← caller
                    self.calls_in.entry(callee_key.clone()).or_default().push(WsCallEdge {
                        other_key: caller_key.clone(),
                        other_name: caller_name.clone(),
                        site_uri: file_uri.clone(),
                        site_line: edge.line,
                        site_col: edge.col,
                        site_end_col: edge.end_col,
                    });

                    file_edges.push((caller_key.clone(), callee_key));
                }
            }

            self.file_call_edges.insert(canonical_path.clone(), file_edges);
        }

        self.dirty = false;
    }

    /// Case-insensitive substring search.
    pub fn search(&self, query: &str, limit: usize) -> Vec<&WorkspaceSymbol> {
        let cap = if limit == 0 { 200 } else { limit };
        if query.is_empty() {
            return self.symbols.iter().take(cap).collect();
        }
        let query_lower = query.to_lowercase();
        self.symbols.iter()
            .filter(|s| s.name.to_lowercase().contains(&query_lower))
            .take(cap)
            .collect()
    }

    /// Find cross-file references using identity key. O(k) in matching refs.
    pub fn find_refs_by_identity(&self, key: &str) -> &[IdentityRef] {
        self.refs_by_identity.get(key).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Find the definition location for a symbol identity.
    pub fn find_def_by_identity(&self, key: &str) -> Option<&IdentityLocation> {
        self.defs_by_identity.get(key)
    }

    /// Find cross-file refs using name-based fallback. O(k) in matching refs.
    pub fn find_cross_file_refs(&self, module_name: &str, symbol_name: &str) -> Vec<CrossFileRef> {
        let refs = match self.refs_by_name.get(symbol_name) {
            Some(r) => r,
            None => return Vec::new(),
        };

        refs.iter()
            .filter(|r| r.module_name.as_deref() == Some(module_name))
            .map(|r| CrossFileRef {
                file_uri: r.file_uri.clone(),
                line: r.line,
                col: r.col,
                len: r.len,
            })
            .collect()
    }

    /// Query workspace outgoing calls for a given caller identity key. O(k).
    pub fn outgoing_calls_for(&self, key: &str) -> &[WsCallEdge] {
        self.calls_out.get(key).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Query workspace incoming calls for a given callee identity key. O(k).
    pub fn incoming_calls_for(&self, key: &str) -> &[WsCallEdge] {
        self.calls_in.get(key).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    pub fn symbol_count(&self) -> usize {
        self.symbols.len()
    }

    /// Test helper: inject a symbol directly for unit testing.
    #[cfg(test)]
    pub fn inject_symbol_for_test(
        &mut self, name: &str, kind: SymbolKindTag,
        file_uri: &str, line: usize, col: usize,
        container: Option<&str>,
    ) {
        self.symbols.push(WorkspaceSymbol {
            name: name.to_string(),
            kind,
            file_uri: file_uri.to_string(),
            line, col,
            container: container.map(|s| s.to_string()),
        });
    }
}

/// Build a procedure identity key. If the call graph name contains `@` (indicating
/// a nested procedure, e.g. `helper@Outer1`), uses `Module::name@parent::proc` as
/// a strong key to disambiguate same-named nested procedures.
/// Otherwise uses the standard `Module::Name::proc`.
fn make_proc_key(_file: &str, module: &str, name: &str, _symtab: &SymbolTable) -> IdentityKey {
    if name.contains('@') {
        // Nested procedure: use the full qualified name
        format!("{}::{}::proc", module, name)
    } else {
        SymbolIdentity::make_key(module, name, IdentityKind::Procedure)
    }
}

// ── Free functions for symbol extraction ────────────────────────────

fn push_symbol(
    symbols: &mut Vec<WorkspaceSymbol>,
    by_name: &mut HashMap<String, Vec<usize>>,
    sym: WorkspaceSymbol,
) {
    let idx = symbols.len();
    let key = sym.name.to_lowercase();
    symbols.push(sym);
    by_name.entry(key).or_default().push(idx);
}

fn add_definition_symbols(
    symbols: &mut Vec<WorkspaceSymbol>,
    by_name: &mut HashMap<String, Vec<usize>>,
    def: &Definition,
    uri: &str,
    module_name: &str,
) {
    match def {
        Definition::Const(c) => {
            push_symbol(symbols, by_name, WorkspaceSymbol {
                name: c.name.clone(), kind: SymbolKindTag::Constant,
                file_uri: uri.to_string(), line: c.loc.line, col: c.loc.col,
                container: Some(module_name.to_string()),
            });
        }
        Definition::Type(t) => {
            push_symbol(symbols, by_name, WorkspaceSymbol {
                name: t.name.clone(), kind: SymbolKindTag::Type,
                file_uri: uri.to_string(), line: t.loc.line, col: t.loc.col,
                container: Some(module_name.to_string()),
            });
        }
        Definition::Var(v) => {
            for (i, name) in v.names.iter().enumerate() {
                let loc = v.name_locs.get(i).unwrap_or(&v.loc);
                push_symbol(symbols, by_name, WorkspaceSymbol {
                    name: name.clone(), kind: SymbolKindTag::Variable,
                    file_uri: uri.to_string(), line: loc.line, col: loc.col,
                    container: Some(module_name.to_string()),
                });
            }
        }
        Definition::Procedure(p) => {
            push_symbol(symbols, by_name, WorkspaceSymbol {
                name: p.name.clone(), kind: SymbolKindTag::Procedure,
                file_uri: uri.to_string(), line: p.loc.line, col: p.loc.col,
                container: Some(module_name.to_string()),
            });
        }
        _ => {}
    }
}

fn add_declaration_symbols(
    symbols: &mut Vec<WorkspaceSymbol>,
    by_name: &mut HashMap<String, Vec<usize>>,
    decl: &Declaration,
    uri: &str,
    module_name: &str,
) {
    match decl {
        Declaration::Const(c) => {
            push_symbol(symbols, by_name, WorkspaceSymbol {
                name: c.name.clone(), kind: SymbolKindTag::Constant,
                file_uri: uri.to_string(), line: c.loc.line, col: c.loc.col,
                container: Some(module_name.to_string()),
            });
        }
        Declaration::Type(t) => {
            push_symbol(symbols, by_name, WorkspaceSymbol {
                name: t.name.clone(), kind: SymbolKindTag::Type,
                file_uri: uri.to_string(), line: t.loc.line, col: t.loc.col,
                container: Some(module_name.to_string()),
            });
        }
        Declaration::Var(v) => {
            for (i, name) in v.names.iter().enumerate() {
                let loc = v.name_locs.get(i).unwrap_or(&v.loc);
                push_symbol(symbols, by_name, WorkspaceSymbol {
                    name: name.clone(), kind: SymbolKindTag::Variable,
                    file_uri: uri.to_string(), line: loc.line, col: loc.col,
                    container: Some(module_name.to_string()),
                });
            }
        }
        Declaration::Procedure(p) => {
            push_symbol(symbols, by_name, WorkspaceSymbol {
                name: p.heading.name.clone(), kind: SymbolKindTag::Procedure,
                file_uri: uri.to_string(), line: p.heading.loc.line, col: p.heading.loc.col,
                container: Some(module_name.to_string()),
            });
        }
        _ => {}
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_kind_to_lsp() {
        assert_eq!(SymbolKindTag::Module.to_lsp_kind(), 2);
        assert_eq!(SymbolKindTag::Procedure.to_lsp_kind(), 12);
        assert_eq!(SymbolKindTag::Type.to_lsp_kind(), 5);
        assert_eq!(SymbolKindTag::Constant.to_lsp_kind(), 14);
        assert_eq!(SymbolKindTag::Variable.to_lsp_kind(), 13);
    }

    #[test]
    fn test_workspace_index_search_empty() {
        let idx = WorkspaceIndex::new();
        assert!(idx.search("anything", 100).is_empty());
    }

    #[test]
    fn test_workspace_index_search_case_insensitive() {
        let mut idx = WorkspaceIndex::new();
        idx.symbols.push(WorkspaceSymbol {
            name: "MyProcedure".to_string(),
            kind: SymbolKindTag::Procedure,
            file_uri: "file:///test.mod".to_string(),
            line: 5, col: 1,
            container: Some("TestModule".to_string()),
        });
        idx.symbols.push(WorkspaceSymbol {
            name: "OtherProc".to_string(),
            kind: SymbolKindTag::Procedure,
            file_uri: "file:///test.mod".to_string(),
            line: 10, col: 1,
            container: Some("TestModule".to_string()),
        });

        let results = idx.search("myproc", 100);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "MyProcedure");

        let all = idx.search("", 100);
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_workspace_index_search_limit() {
        let mut idx = WorkspaceIndex::new();
        for i in 0..150 {
            idx.symbols.push(WorkspaceSymbol {
                name: format!("sym{}", i),
                kind: SymbolKindTag::Variable,
                file_uri: "file:///test.mod".to_string(),
                line: i + 1, col: 1,
                container: None,
            });
        }
        let results = idx.search("", 100);
        assert_eq!(results.len(), 100);
    }

    #[test]
    fn test_workspace_index_index_file() {
        let tmp = std::env::temp_dir().join("m2_lsp_test_idx7");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let def_path = tmp.join("Stack.def");
        std::fs::write(&def_path, "\
DEFINITION MODULE Stack;
TYPE Stack;
PROCEDURE Create(): Stack;
PROCEDURE Push(s: Stack; val: INTEGER);
END Stack.
").unwrap();

        let mut idx = WorkspaceIndex::new();
        let mut def_cache = DefCache::new();
        assert!(idx.index_file(&def_path, false, &[], &mut def_cache));
        assert!(idx.dirty);

        idx.rebuild_if_dirty();
        assert!(!idx.dirty);

        assert!(idx.symbols.len() >= 4, "expected >= 4 symbols, got {}", idx.symbols.len());
        let procs: Vec<_> = idx.symbols.iter()
            .filter(|s| matches!(s.kind, SymbolKindTag::Procedure))
            .collect();
        assert!(procs.len() >= 2);

        assert!(idx.symbols_by_name.contains_key("stack"));
        assert!(idx.symbols_by_name.contains_key("create"));
        assert!(idx.symbols_by_name.contains_key("push"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_workspace_index_stamp_skip() {
        let tmp = std::env::temp_dir().join("m2_lsp_test_stamp7");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let mod_path = tmp.join("Test.mod");
        std::fs::write(&mod_path, "MODULE Test;\nVAR x: INTEGER;\nBEGIN\nEND Test.\n").unwrap();

        let mut idx = WorkspaceIndex::new();
        let mut def_cache = DefCache::new();
        assert!(idx.index_file(&mod_path, false, &[], &mut def_cache));
        assert!(idx.dirty);
        idx.dirty = false;

        assert!(!idx.index_file(&mod_path, false, &[], &mut def_cache));
        assert!(!idx.dirty);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_inverted_refs_no_scan_all() {
        let mut idx = WorkspaceIndex::new();
        idx.refs_by_name.entry("Create".to_string()).or_default().push(IndexedRef {
            file_uri: "file:///a.mod".to_string(),
            module_name: Some("Stack".to_string()),
            line: 5, col: 3, len: 6,
        });
        idx.refs_by_name.entry("Create".to_string()).or_default().push(IndexedRef {
            file_uri: "file:///b.mod".to_string(),
            module_name: Some("Queue".to_string()),
            line: 10, col: 5, len: 6,
        });

        let refs = idx.find_cross_file_refs("Stack", "Create");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].file_uri, "file:///a.mod");

        let refs2 = idx.find_cross_file_refs("Queue", "Create");
        assert_eq!(refs2.len(), 1);
        assert_eq!(refs2[0].file_uri, "file:///b.mod");

        let refs3 = idx.find_cross_file_refs("Stack", "Pop");
        assert!(refs3.is_empty());
    }

    #[test]
    fn test_file_stamp_content_hash() {
        let s1 = FileStamp::from_content(b"hello");
        let s2 = FileStamp::from_content(b"hello");
        let s3 = FileStamp::from_content(b"world");
        assert_eq!(s1, s2);
        assert_ne!(s1, s3);
    }

    #[test]
    fn test_force_clear() {
        let mut idx = WorkspaceIndex::new();
        idx.symbols.push(WorkspaceSymbol {
            name: "Foo".to_string(), kind: SymbolKindTag::Procedure,
            file_uri: "file:///test.mod".to_string(), line: 1, col: 1,
            container: None,
        });
        idx.refs_by_name.entry("Foo".to_string()).or_default().push(IndexedRef {
            file_uri: "file:///test.mod".to_string(),
            module_name: None, line: 1, col: 1, len: 3,
        });
        idx.refs_by_identity.entry("Test::Foo::proc".to_string()).or_default().push(IdentityRef {
            file_uri: "file:///test.mod".to_string(),
            line: 1, col: 1, len: 3, is_definition: true,
        });

        idx.force_clear();
        assert!(idx.files.is_empty());
        assert!(idx.symbols.is_empty());
        assert!(idx.symbols_by_name.is_empty());
        assert!(idx.refs_by_name.is_empty());
        assert!(idx.refs_by_identity.is_empty());
        assert!(idx.defs_by_identity.is_empty());
        assert!(idx.calls_out.is_empty());
        assert!(idx.calls_in.is_empty());
        assert!(idx.file_call_edges.is_empty());
    }

    #[test]
    fn test_index_from_analysis() {
        let source = "MODULE Test;\nVAR x: INTEGER;\nBEGIN\nEND Test.\n";
        let result = crate::analyze::analyze_source(source, "test.mod", &[]);
        let path = PathBuf::from("/tmp/m2_test_open/Test.mod");

        let mut idx = WorkspaceIndex::new();
        idx.index_from_analysis(&path, "file:///tmp/m2_test_open/Test.mod", source, result);
        assert!(idx.dirty);

        idx.rebuild_if_dirty();
        assert!(idx.symbols.len() >= 2, "expected >= 2 symbols, got {}", idx.symbols.len());
    }

    #[test]
    fn test_identity_no_collision_across_modules() {
        // Two modules both define 'x' — identity keys must differ.
        let mut idx = WorkspaceIndex::new();

        idx.refs_by_identity.entry("ModA::x::var".to_string()).or_default().push(IdentityRef {
            file_uri: "file:///a.mod".to_string(),
            line: 5, col: 3, len: 1, is_definition: false,
        });
        idx.refs_by_identity.entry("ModA::x::var".to_string()).or_default().push(IdentityRef {
            file_uri: "file:///c.mod".to_string(),
            line: 2, col: 7, len: 1, is_definition: false,
        });
        idx.refs_by_identity.entry("ModB::x::var".to_string()).or_default().push(IdentityRef {
            file_uri: "file:///b.mod".to_string(),
            line: 10, col: 3, len: 1, is_definition: false,
        });

        let refs_a = idx.find_refs_by_identity("ModA::x::var");
        assert_eq!(refs_a.len(), 2);
        assert_eq!(refs_a[0].file_uri, "file:///a.mod");
        assert_eq!(refs_a[1].file_uri, "file:///c.mod");

        let refs_b = idx.find_refs_by_identity("ModB::x::var");
        assert_eq!(refs_b.len(), 1);
        assert_eq!(refs_b[0].file_uri, "file:///b.mod");

        let refs_none = idx.find_refs_by_identity("ModC::x::var");
        assert!(refs_none.is_empty());
    }

    #[test]
    fn test_identity_key_format() {
        let ident = SymbolIdentity {
            file: "/test/Stack.def".to_string(),
            scope_id: 1,
            module: "Stack".to_string(),
            name: "Create".to_string(),
            kind: IdentityKind::Procedure,
        };
        assert_eq!(ident.key(), "Stack::Create::proc");

        let key = SymbolIdentity::make_key("Queue", "Push", IdentityKind::Procedure);
        assert_eq!(key, "Queue::Push::proc");

        assert_ne!(
            SymbolIdentity::make_key("M", "x", IdentityKind::Variable),
            SymbolIdentity::make_key("M", "x", IdentityKind::Type),
        );
    }

    #[test]
    fn test_identity_local_key_disambiguates_scopes() {
        let ident_a = SymbolIdentity {
            file: "/test/Mod.mod".to_string(),
            scope_id: 2,
            module: "Mod".to_string(),
            name: "helper".to_string(),
            kind: IdentityKind::Procedure,
        };
        let ident_b = SymbolIdentity {
            file: "/test/Mod.mod".to_string(),
            scope_id: 3,
            module: "Mod".to_string(),
            name: "helper".to_string(),
            kind: IdentityKind::Procedure,
        };
        // Cross-file keys are the same (both are Mod::helper::proc)
        assert_eq!(ident_a.key(), ident_b.key());
        // Local keys differ because scope_id differs
        assert_ne!(ident_a.local_key(), ident_b.local_key());
    }

    #[test]
    fn test_identity_kind_from_symbol_kind() {
        assert!(matches!(IdentityKind::from_symbol_kind(&SymbolKind::Variable), IdentityKind::Variable));
        assert!(matches!(IdentityKind::from_symbol_kind(&SymbolKind::Type), IdentityKind::Type));
        assert!(matches!(
            IdentityKind::from_symbol_kind(&SymbolKind::Procedure { params: vec![], return_type: None, is_builtin: false }),
            IdentityKind::Procedure
        ));
    }

    #[test]
    fn test_identity_cross_file_unaffected() {
        // Two different files defining the same module-level symbol
        // should produce the same cross-file key.
        let ident_def = SymbolIdentity {
            file: "/project/Stack.def".to_string(),
            scope_id: 1,
            module: "Stack".to_string(),
            name: "Push".to_string(),
            kind: IdentityKind::Procedure,
        };
        let ident_mod = SymbolIdentity {
            file: "/project/Stack.mod".to_string(),
            scope_id: 1,
            module: "Stack".to_string(),
            name: "Push".to_string(),
            kind: IdentityKind::Procedure,
        };
        assert_eq!(ident_def.key(), ident_mod.key());
        // But local keys differ (different files)
        assert_ne!(ident_def.local_key(), ident_mod.local_key());
    }

    // ── Workspace call graph tests ──────────────────────────────────

    #[test]
    fn test_workspace_call_graph_same_module() {
        // Single module with Foo calling Bar — workspace index should capture the edge.
        let source = "MODULE M;\nPROCEDURE Bar;\nBEGIN END Bar;\nPROCEDURE Foo;\nBEGIN\n  Bar\nEND Foo;\nBEGIN\nEND M.\n";
        let result = crate::analyze::analyze_source(source, "m.mod", &[]);

        let mut idx = WorkspaceIndex::new();
        let path = PathBuf::from("/tmp/m2_wscg/M.mod");
        idx.index_from_analysis(&path, "file:///tmp/m2_wscg/M.mod", source, result);
        idx.rebuild_if_dirty();

        // Outgoing: Foo → Bar
        let out = idx.outgoing_calls_for("M::Foo::proc");
        assert!(!out.is_empty(), "Foo should have outgoing calls");
        assert!(out.iter().any(|e| e.other_key == "M::Bar::proc"), "Foo should call Bar");

        // Incoming: Bar ← Foo
        let inc = idx.incoming_calls_for("M::Bar::proc");
        assert!(!inc.is_empty(), "Bar should have incoming calls");
        assert!(inc.iter().any(|e| e.other_key == "M::Foo::proc"), "Bar should be called by Foo");
    }

    #[test]
    fn test_workspace_call_graph_cross_file() {
        // Three files: A calls B.ProcB, B calls C.ProcC, C defines ProcC.
        // Uses qualified calls (B.ProcB) so callee_module is set in AST.
        let tmp = std::env::temp_dir().join("m2_wscg_cross");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        // Write .def and .mod files
        std::fs::write(tmp.join("B.def"), "DEFINITION MODULE B;\nPROCEDURE ProcB;\nEND B.\n").unwrap();
        std::fs::write(tmp.join("C.def"), "DEFINITION MODULE C;\nPROCEDURE ProcC;\nEND C.\n").unwrap();
        std::fs::write(tmp.join("A.mod"),
            "MODULE A;\nIMPORT B;\nPROCEDURE ProcA;\nBEGIN\n  B.ProcB\nEND ProcA;\nBEGIN\nEND A.\n"
        ).unwrap();
        std::fs::write(tmp.join("B.mod"),
            "IMPLEMENTATION MODULE B;\nIMPORT C;\nPROCEDURE ProcB;\nBEGIN\n  C.ProcC\nEND ProcB;\nEND B.\n"
        ).unwrap();
        std::fs::write(tmp.join("C.mod"),
            "IMPLEMENTATION MODULE C;\nPROCEDURE ProcC;\nBEGIN END ProcC;\nEND C.\n"
        ).unwrap();

        // Index all files (without opening them — pure disk indexing)
        let mut idx = WorkspaceIndex::new();
        let mut def_cache = DefCache::new();
        let inc = vec![tmp.clone()];
        let count = idx.index_directories(&[tmp.clone()], false, &inc, &mut def_cache);
        assert!(count >= 3, "should index at least A.mod, B.mod, C.mod (got {})", count);
        idx.rebuild_if_dirty();

        // A::ProcA outgoing → B::ProcB
        let out_a = idx.outgoing_calls_for("A::ProcA::proc");
        assert!(!out_a.is_empty(), "ProcA should have outgoing calls, calls_out keys: {:?}",
            idx.calls_out.keys().collect::<Vec<_>>());
        assert!(out_a.iter().any(|e| e.other_key == "B::ProcB::proc"),
            "ProcA should call ProcB, got: {:?}", out_a.iter().map(|e| &e.other_key).collect::<Vec<_>>());

        // B::ProcB outgoing → C::ProcC
        let out_b = idx.outgoing_calls_for("B::ProcB::proc");
        assert!(!out_b.is_empty(), "ProcB should have outgoing calls");
        assert!(out_b.iter().any(|e| e.other_key == "C::ProcC::proc"),
            "ProcB should call ProcC");

        // C::ProcC incoming ← B::ProcB
        let inc_c = idx.incoming_calls_for("C::ProcC::proc");
        assert!(!inc_c.is_empty(), "ProcC should have incoming calls");
        assert!(inc_c.iter().any(|e| e.other_key == "B::ProcB::proc"),
            "ProcC should be called by ProcB");

        // B::ProcB incoming ← A::ProcA
        let inc_b = idx.incoming_calls_for("B::ProcB::proc");
        assert!(!inc_b.is_empty(), "ProcB should have incoming calls");
        assert!(inc_b.iter().any(|e| e.other_key == "A::ProcA::proc"),
            "ProcB should be called by ProcA");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_workspace_call_graph_incremental_reindex() {
        // Index a file, modify it, reindex — old edges removed, new edges present.
        let source_v1 = "MODULE M;\nPROCEDURE Old;\nBEGIN END Old;\nPROCEDURE Caller;\nBEGIN\n  Old\nEND Caller;\nBEGIN\nEND M.\n";
        let source_v2 = "MODULE M;\nPROCEDURE New;\nBEGIN END New;\nPROCEDURE Caller;\nBEGIN\n  New\nEND Caller;\nBEGIN\nEND M.\n";

        let result_v1 = crate::analyze::analyze_source(source_v1, "m.mod", &[]);
        let result_v2 = crate::analyze::analyze_source(source_v2, "m.mod", &[]);

        let mut idx = WorkspaceIndex::new();
        let path = PathBuf::from("/tmp/m2_wscg_inc/M.mod");

        // V1: Caller → Old
        idx.index_from_analysis(&path, "file:///tmp/m2_wscg_inc/M.mod", source_v1, result_v1);
        idx.rebuild_if_dirty();
        assert!(!idx.outgoing_calls_for("M::Caller::proc").is_empty());
        assert!(idx.outgoing_calls_for("M::Caller::proc").iter().any(|e| e.other_key == "M::Old::proc"));

        // V2: Caller → New (Old is gone)
        idx.index_from_analysis(&path, "file:///tmp/m2_wscg_inc/M.mod", source_v2, result_v2);
        idx.rebuild_if_dirty();
        let out = idx.outgoing_calls_for("M::Caller::proc");
        assert!(out.iter().any(|e| e.other_key == "M::New::proc"), "should call New now");
        assert!(!out.iter().any(|e| e.other_key == "M::Old::proc"), "should NOT call Old anymore");

        // Old should have no incoming calls
        assert!(idx.incoming_calls_for("M::Old::proc").is_empty(), "Old should have no callers");
    }

    #[test]
    fn test_workspace_call_graph_multi_root_isolation() {
        // Two roots, each with ProcX calling ProcY — calls should not leak.
        let source_root1 = "MODULE R1;\nPROCEDURE ProcY;\nBEGIN END ProcY;\nPROCEDURE ProcX;\nBEGIN\n  ProcY\nEND ProcX;\nBEGIN\nEND R1.\n";
        let source_root2 = "MODULE R2;\nPROCEDURE ProcY;\nBEGIN END ProcY;\nPROCEDURE ProcX;\nBEGIN\n  ProcY\nEND ProcX;\nBEGIN\nEND R2.\n";

        let result1 = crate::analyze::analyze_source(source_root1, "r1.mod", &[]);
        let result2 = crate::analyze::analyze_source(source_root2, "r2.mod", &[]);

        let mut idx = WorkspaceIndex::new();
        let path1 = PathBuf::from("/root1/R1.mod");
        let path2 = PathBuf::from("/root2/R2.mod");
        idx.index_from_analysis(&path1, "file:///root1/R1.mod", source_root1, result1);
        idx.index_from_analysis(&path2, "file:///root2/R2.mod", source_root2, result2);
        idx.rebuild_if_dirty();

        // R1::ProcX → R1::ProcY (not R2::ProcY)
        let out_r1 = idx.outgoing_calls_for("R1::ProcX::proc");
        assert!(out_r1.iter().any(|e| e.other_key == "R1::ProcY::proc"), "R1 ProcX should call R1 ProcY");
        assert!(!out_r1.iter().any(|e| e.other_key == "R2::ProcY::proc"), "R1 ProcX should NOT call R2 ProcY");

        // R2::ProcX → R2::ProcY (not R1::ProcY)
        let out_r2 = idx.outgoing_calls_for("R2::ProcX::proc");
        assert!(out_r2.iter().any(|e| e.other_key == "R2::ProcY::proc"), "R2 ProcX should call R2 ProcY");
        assert!(!out_r2.iter().any(|e| e.other_key == "R1::ProcY::proc"), "R2 ProcX should NOT call R1 ProcY");

        // R1::ProcY incoming — only from R1::ProcX
        let inc_r1 = idx.incoming_calls_for("R1::ProcY::proc");
        assert!(inc_r1.iter().all(|e| e.other_key.starts_with("R1::")), "R1 ProcY callers should all be from R1");

        // R2::ProcY incoming — only from R2::ProcX
        let inc_r2 = idx.incoming_calls_for("R2::ProcY::proc");
        assert!(inc_r2.iter().all(|e| e.other_key.starts_with("R2::")), "R2 ProcY callers should all be from R2");
    }

    #[test]
    fn test_workspace_call_graph_per_file_tracking() {
        // Verify file_call_edges is populated correctly.
        let source = "MODULE M;\nPROCEDURE Bar;\nBEGIN END Bar;\nPROCEDURE Foo;\nBEGIN\n  Bar\nEND Foo;\nBEGIN\nEND M.\n";
        let result = crate::analyze::analyze_source(source, "m.mod", &[]);

        let mut idx = WorkspaceIndex::new();
        let path = PathBuf::from("/tmp/m2_wscg_ft/M.mod");
        idx.index_from_analysis(&path, "file:///tmp/m2_wscg_ft/M.mod", source, result);
        idx.rebuild_if_dirty();

        // file_call_edges should have an entry for this file
        assert!(idx.file_call_edges.values().any(|edges| !edges.is_empty()),
            "file_call_edges should track edges");
        let all_edges: Vec<_> = idx.file_call_edges.values().flat_map(|v| v.iter()).collect();
        assert!(all_edges.iter().any(|(caller, callee)| caller == "M::Foo::proc" && callee == "M::Bar::proc"),
            "should track Foo→Bar edge");
    }

    #[test]
    fn test_workspace_call_graph_nested_proc_no_collision() {
        // Two procedures with the same local name in different scopes should get different keys.
        // Module M has Outer1.helper and Outer2.helper — they should not collide.
        let source = "\
MODULE M;
PROCEDURE Outer1;
  PROCEDURE helper;
  BEGIN END helper;
BEGIN
  helper
END Outer1;
PROCEDURE Outer2;
  PROCEDURE helper;
  BEGIN END helper;
BEGIN
  helper
END Outer2;
BEGIN
END M.
";
        let result = crate::analyze::analyze_source(source, "m.mod", &[]);

        let mut idx = WorkspaceIndex::new();
        let path = PathBuf::from("/tmp/m2_wscg_nest/M.mod");
        idx.index_from_analysis(&path, "file:///tmp/m2_wscg_nest/M.mod", source, result);
        idx.rebuild_if_dirty();

        // Outer1 and Outer2 should have outgoing calls to their respective helpers
        let out1 = idx.outgoing_calls_for("M::Outer1::proc");
        let out2 = idx.outgoing_calls_for("M::Outer2::proc");
        assert!(!out1.is_empty(), "Outer1 should have outgoing calls");
        assert!(!out2.is_empty(), "Outer2 should have outgoing calls");

        // The callee keys for the two helpers should differ (nested strong keys)
        let helper1_key = &out1[0].other_key;
        let helper2_key = &out2[0].other_key;
        assert_ne!(helper1_key, helper2_key,
            "Nested helpers should have different identity keys, got: {} vs {}",
            helper1_key, helper2_key);

        // Neither should be the flat "M::helper::proc" key (they should be strong local keys)
        assert_ne!(helper1_key, "M::helper::proc",
            "Nested helper should use strong key, not flat module key");
        assert_ne!(helper2_key, "M::helper::proc",
            "Nested helper should use strong key, not flat module key");
    }
}
