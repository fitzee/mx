//! Documentation for Modula-2 / M2+.
//!
//! Two tiers:
//!   1. Core docs (keywords, builtins, stdlib, M2+ extensions) — embedded via
//!      `include_str!` at compile time. These rarely change.
//!   2. Library docs (m2gfx, m2log, m2bytes, etc.) — loaded from disk at
//!      runtime by the LSP via `LibraryDocs`. Adding new library docs does
//!      NOT require recompiling m2c.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

/// A documentation entry for a core language element (embedded at compile time).
pub struct DocEntry {
    pub key: &'static str,
    pub category: DocCategory,
    pub markdown: &'static str,
}

/// Category of core documentation entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocCategory {
    Language,
    Builtin,
    Stdlib,
    Extension,
}

/// O(1) lookup of embedded core documentation by key.
/// Keys: keywords uppercase, builtins uppercase, module names exact case.
pub fn get_doc(key: &str) -> Option<&'static DocEntry> {
    // Try exact match first (for module names like "InOut")
    if let Some(entry) = REGISTRY.get(key) {
        return Some(entry);
    }
    // Try uppercase (for keywords and builtins)
    let upper = key.to_uppercase();
    REGISTRY.get(upper.as_str())
}

/// Format a DocEntry for hover display.
pub fn format_hover(entry: &DocEntry) -> String {
    entry.markdown.to_string()
}

/// Return all registered core documentation keys (deduplicated by canonical key).
pub fn all_keys() -> Vec<&'static str> {
    let mut seen = std::collections::HashSet::new();
    REGISTRY.values()
        .filter(|entry| seen.insert(entry.key))
        .map(|entry| entry.key)
        .collect()
}

// ── Core Registry (embedded at compile time) ────────────────────────

static REGISTRY: LazyLock<HashMap<&'static str, DocEntry>> = LazyLock::new(|| {
    let mut m = HashMap::new();

    // Built-in types
    m.insert("INTEGER", DocEntry { key: "INTEGER", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/types/INTEGER.md") });
    m.insert("CARDINAL", DocEntry { key: "CARDINAL", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/types/CARDINAL.md") });
    m.insert("REAL", DocEntry { key: "REAL", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/types/REAL.md") });
    m.insert("LONGREAL", DocEntry { key: "LONGREAL", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/types/LONGREAL.md") });
    m.insert("BOOLEAN", DocEntry { key: "BOOLEAN", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/types/BOOLEAN.md") });
    m.insert("CHAR", DocEntry { key: "CHAR", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/types/CHAR.md") });
    m.insert("BITSET", DocEntry { key: "BITSET", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/types/BITSET.md") });
    m.insert("WORD", DocEntry { key: "WORD", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/types/WORD.md") });
    m.insert("BYTE", DocEntry { key: "BYTE", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/types/BYTE.md") });
    m.insert("ADDRESS", DocEntry { key: "ADDRESS", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/types/ADDRESS.md") });
    m.insert("LONGINT", DocEntry { key: "LONGINT", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/types/LONGINT.md") });
    m.insert("LONGCARD", DocEntry { key: "LONGCARD", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/types/LONGCARD.md") });
    m.insert("PROC", DocEntry { key: "PROC", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/types/PROC.md") });

    // Built-in procedures
    m.insert("NEW", DocEntry { key: "NEW", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/builtins/NEW.md") });
    m.insert("DISPOSE", DocEntry { key: "DISPOSE", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/builtins/DISPOSE.md") });
    m.insert("INC", DocEntry { key: "INC", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/builtins/INC.md") });
    m.insert("DEC", DocEntry { key: "DEC", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/builtins/DEC.md") });
    m.insert("INCL", DocEntry { key: "INCL", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/builtins/INCL.md") });
    m.insert("EXCL", DocEntry { key: "EXCL", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/builtins/EXCL.md") });
    m.insert("HALT", DocEntry { key: "HALT", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/builtins/HALT.md") });
    m.insert("ABS", DocEntry { key: "ABS", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/builtins/ABS.md") });
    m.insert("ODD", DocEntry { key: "ODD", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/builtins/ODD.md") });
    m.insert("CAP", DocEntry { key: "CAP", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/builtins/CAP.md") });
    m.insert("ORD", DocEntry { key: "ORD", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/builtins/ORD.md") });
    m.insert("CHR", DocEntry { key: "CHR", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/builtins/CHR.md") });
    m.insert("VAL", DocEntry { key: "VAL", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/builtins/VAL.md") });
    m.insert("HIGH", DocEntry { key: "HIGH", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/builtins/HIGH.md") });
    m.insert("SIZE", DocEntry { key: "SIZE", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/builtins/SIZE.md") });
    m.insert("TSIZE", DocEntry { key: "TSIZE", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/builtins/TSIZE.md") });
    m.insert("ADR", DocEntry { key: "ADR", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/builtins/ADR.md") });
    m.insert("MAX", DocEntry { key: "MAX", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/builtins/MAX.md") });
    m.insert("MIN", DocEntry { key: "MIN", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/builtins/MIN.md") });
    m.insert("FLOAT", DocEntry { key: "FLOAT", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/builtins/FLOAT.md") });
    m.insert("TRUNC", DocEntry { key: "TRUNC", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/builtins/TRUNC.md") });
    m.insert("LONG", DocEntry { key: "LONG", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/builtins/LONG.md") });
    m.insert("SHORT", DocEntry { key: "SHORT", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/builtins/SHORT.md") });
    m.insert("LFLOAT", DocEntry { key: "LFLOAT", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/builtins/LFLOAT.md") });
    m.insert("CMPLX", DocEntry { key: "CMPLX", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/builtins/CMPLX.md") });
    m.insert("RE", DocEntry { key: "RE", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/builtins/RE.md") });
    m.insert("IM", DocEntry { key: "IM", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/builtins/IM.md") });

    // Bitwise operations
    m.insert("SHL", DocEntry { key: "SHL", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/builtins/SHL.md") });
    m.insert("SHR", DocEntry { key: "SHR", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/builtins/SHR.md") });
    m.insert("BAND", DocEntry { key: "BAND", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/builtins/BAND.md") });
    m.insert("BOR", DocEntry { key: "BOR", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/builtins/BOR.md") });
    m.insert("BXOR", DocEntry { key: "BXOR", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/builtins/BXOR.md") });
    m.insert("BNOT", DocEntry { key: "BNOT", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/builtins/BNOT.md") });
    m.insert("SHIFT", DocEntry { key: "SHIFT", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/builtins/SHIFT.md") });
    m.insert("ROTATE", DocEntry { key: "ROTATE", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/builtins/ROTATE.md") });

    // Coroutines
    m.insert("NEWPROCESS", DocEntry { key: "NEWPROCESS", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/builtins/NEWPROCESS.md") });
    m.insert("TRANSFER", DocEntry { key: "TRANSFER", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/builtins/TRANSFER.md") });
    m.insert("IOTRANSFER", DocEntry { key: "IOTRANSFER", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/builtins/IOTRANSFER.md") });

    // Built-in constants
    m.insert("TRUE", DocEntry { key: "TRUE", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/builtins/TRUE.md") });
    m.insert("FALSE", DocEntry { key: "FALSE", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/builtins/FALSE.md") });
    m.insert("NIL", DocEntry { key: "NIL", category: DocCategory::Builtin, markdown: include_str!("../docs/lang/builtins/NIL.md") });

    // Keywords
    m.insert("MODULE", DocEntry { key: "MODULE", category: DocCategory::Language, markdown: include_str!("../docs/lang/keywords/MODULE.md") });
    m.insert("PROCEDURE", DocEntry { key: "PROCEDURE", category: DocCategory::Language, markdown: include_str!("../docs/lang/keywords/PROCEDURE.md") });
    m.insert("IF", DocEntry { key: "IF", category: DocCategory::Language, markdown: include_str!("../docs/lang/keywords/IF.md") });
    m.insert("WHILE", DocEntry { key: "WHILE", category: DocCategory::Language, markdown: include_str!("../docs/lang/keywords/WHILE.md") });
    m.insert("REPEAT", DocEntry { key: "REPEAT", category: DocCategory::Language, markdown: include_str!("../docs/lang/keywords/REPEAT.md") });
    m.insert("FOR", DocEntry { key: "FOR", category: DocCategory::Language, markdown: include_str!("../docs/lang/keywords/FOR.md") });
    m.insert("LOOP", DocEntry { key: "LOOP", category: DocCategory::Language, markdown: include_str!("../docs/lang/keywords/LOOP.md") });
    m.insert("CASE", DocEntry { key: "CASE", category: DocCategory::Language, markdown: include_str!("../docs/lang/keywords/CASE.md") });
    m.insert("WITH", DocEntry { key: "WITH", category: DocCategory::Language, markdown: include_str!("../docs/lang/keywords/WITH.md") });
    m.insert("RETURN", DocEntry { key: "RETURN", category: DocCategory::Language, markdown: include_str!("../docs/lang/keywords/RETURN.md") });
    m.insert("EXIT", DocEntry { key: "EXIT", category: DocCategory::Language, markdown: include_str!("../docs/lang/keywords/EXIT.md") });
    m.insert("IMPORT", DocEntry { key: "IMPORT", category: DocCategory::Language, markdown: include_str!("../docs/lang/keywords/IMPORT.md") });
    m.insert("FROM", DocEntry { key: "FROM", category: DocCategory::Language, markdown: include_str!("../docs/lang/keywords/FROM.md") });
    m.insert("EXPORT", DocEntry { key: "EXPORT", category: DocCategory::Language, markdown: include_str!("../docs/lang/keywords/EXPORT.md") });
    m.insert("VAR", DocEntry { key: "VAR", category: DocCategory::Language, markdown: include_str!("../docs/lang/keywords/VAR.md") });
    m.insert("CONST", DocEntry { key: "CONST", category: DocCategory::Language, markdown: include_str!("../docs/lang/keywords/CONST.md") });
    m.insert("TYPE", DocEntry { key: "TYPE", category: DocCategory::Language, markdown: include_str!("../docs/lang/keywords/TYPE.md") });
    m.insert("BEGIN", DocEntry { key: "BEGIN", category: DocCategory::Language, markdown: include_str!("../docs/lang/keywords/BEGIN.md") });
    m.insert("END", DocEntry { key: "END", category: DocCategory::Language, markdown: include_str!("../docs/lang/keywords/END.md") });
    m.insert("DEFINITION", DocEntry { key: "DEFINITION", category: DocCategory::Language, markdown: include_str!("../docs/lang/keywords/DEFINITION.md") });
    m.insert("IMPLEMENTATION", DocEntry { key: "IMPLEMENTATION", category: DocCategory::Language, markdown: include_str!("../docs/lang/keywords/IMPLEMENTATION.md") });
    m.insert("QUALIFIED", DocEntry { key: "QUALIFIED", category: DocCategory::Language, markdown: include_str!("../docs/lang/keywords/QUALIFIED.md") });
    m.insert("ARRAY", DocEntry { key: "ARRAY", category: DocCategory::Language, markdown: include_str!("../docs/lang/keywords/ARRAY.md") });
    m.insert("RECORD", DocEntry { key: "RECORD", category: DocCategory::Language, markdown: include_str!("../docs/lang/keywords/RECORD.md") });
    m.insert("SET", DocEntry { key: "SET", category: DocCategory::Language, markdown: include_str!("../docs/lang/keywords/SET.md") });
    m.insert("POINTER", DocEntry { key: "POINTER", category: DocCategory::Language, markdown: include_str!("../docs/lang/keywords/POINTER.md") });
    m.insert("AND", DocEntry { key: "AND", category: DocCategory::Language, markdown: include_str!("../docs/lang/keywords/AND.md") });
    m.insert("OR", DocEntry { key: "OR", category: DocCategory::Language, markdown: include_str!("../docs/lang/keywords/OR.md") });
    m.insert("NOT", DocEntry { key: "NOT", category: DocCategory::Language, markdown: include_str!("../docs/lang/keywords/NOT.md") });
    m.insert("DIV", DocEntry { key: "DIV", category: DocCategory::Language, markdown: include_str!("../docs/lang/keywords/DIV.md") });
    m.insert("MOD", DocEntry { key: "MOD", category: DocCategory::Language, markdown: include_str!("../docs/lang/keywords/MOD.md") });
    m.insert("IN", DocEntry { key: "IN", category: DocCategory::Language, markdown: include_str!("../docs/lang/keywords/IN.md") });

    // Standard library modules (exact case keys + uppercase aliases)
    m.insert("InOut", DocEntry { key: "InOut", category: DocCategory::Stdlib, markdown: include_str!("../docs/lang/stdlib/InOut.md") });
    m.insert("INOUT", DocEntry { key: "InOut", category: DocCategory::Stdlib, markdown: include_str!("../docs/lang/stdlib/InOut.md") });
    m.insert("RealInOut", DocEntry { key: "RealInOut", category: DocCategory::Stdlib, markdown: include_str!("../docs/lang/stdlib/RealInOut.md") });
    m.insert("REALINOUT", DocEntry { key: "RealInOut", category: DocCategory::Stdlib, markdown: include_str!("../docs/lang/stdlib/RealInOut.md") });
    m.insert("MathLib0", DocEntry { key: "MathLib0", category: DocCategory::Stdlib, markdown: include_str!("../docs/lang/stdlib/MathLib0.md") });
    m.insert("MATHLIB0", DocEntry { key: "MathLib0", category: DocCategory::Stdlib, markdown: include_str!("../docs/lang/stdlib/MathLib0.md") });
    m.insert("Strings", DocEntry { key: "Strings", category: DocCategory::Stdlib, markdown: include_str!("../docs/lang/stdlib/Strings.md") });
    m.insert("STRINGS", DocEntry { key: "Strings", category: DocCategory::Stdlib, markdown: include_str!("../docs/lang/stdlib/Strings.md") });
    m.insert("Terminal", DocEntry { key: "Terminal", category: DocCategory::Stdlib, markdown: include_str!("../docs/lang/stdlib/Terminal.md") });
    m.insert("TERMINAL", DocEntry { key: "Terminal", category: DocCategory::Stdlib, markdown: include_str!("../docs/lang/stdlib/Terminal.md") });
    m.insert("Storage", DocEntry { key: "Storage", category: DocCategory::Stdlib, markdown: include_str!("../docs/lang/stdlib/Storage.md") });
    m.insert("STORAGE", DocEntry { key: "Storage", category: DocCategory::Stdlib, markdown: include_str!("../docs/lang/stdlib/Storage.md") });
    m.insert("SYSTEM", DocEntry { key: "SYSTEM", category: DocCategory::Stdlib, markdown: include_str!("../docs/lang/stdlib/SYSTEM.md") });
    m.insert("Conversions", DocEntry { key: "Conversions", category: DocCategory::Stdlib, markdown: include_str!("../docs/lang/stdlib/Conversions.md") });
    m.insert("CONVERSIONS", DocEntry { key: "Conversions", category: DocCategory::Stdlib, markdown: include_str!("../docs/lang/stdlib/Conversions.md") });
    m.insert("Args", DocEntry { key: "Args", category: DocCategory::Stdlib, markdown: include_str!("../docs/lang/stdlib/Args.md") });
    m.insert("ARGS", DocEntry { key: "Args", category: DocCategory::Stdlib, markdown: include_str!("../docs/lang/stdlib/Args.md") });
    m.insert("STextIO", DocEntry { key: "STextIO", category: DocCategory::Stdlib, markdown: include_str!("../docs/lang/stdlib/STextIO.md") });
    m.insert("STEXTIO", DocEntry { key: "STextIO", category: DocCategory::Stdlib, markdown: include_str!("../docs/lang/stdlib/STextIO.md") });
    m.insert("SWholeIO", DocEntry { key: "SWholeIO", category: DocCategory::Stdlib, markdown: include_str!("../docs/lang/stdlib/SWholeIO.md") });
    m.insert("SWHOLEIO", DocEntry { key: "SWholeIO", category: DocCategory::Stdlib, markdown: include_str!("../docs/lang/stdlib/SWholeIO.md") });
    m.insert("SRealIO", DocEntry { key: "SRealIO", category: DocCategory::Stdlib, markdown: include_str!("../docs/lang/stdlib/SRealIO.md") });
    m.insert("SREALIO", DocEntry { key: "SRealIO", category: DocCategory::Stdlib, markdown: include_str!("../docs/lang/stdlib/SRealIO.md") });
    m.insert("Thread", DocEntry { key: "Thread", category: DocCategory::Stdlib, markdown: include_str!("../docs/lang/stdlib/Thread.md") });
    m.insert("THREAD", DocEntry { key: "Thread", category: DocCategory::Stdlib, markdown: include_str!("../docs/lang/stdlib/Thread.md") });
    m.insert("Mutex", DocEntry { key: "Mutex", category: DocCategory::Stdlib, markdown: include_str!("../docs/lang/stdlib/Mutex.md") });
    m.insert("MUTEX", DocEntry { key: "Mutex", category: DocCategory::Stdlib, markdown: include_str!("../docs/lang/stdlib/Mutex.md") });
    m.insert("Condition", DocEntry { key: "Condition", category: DocCategory::Stdlib, markdown: include_str!("../docs/lang/stdlib/Condition.md") });
    m.insert("CONDITION", DocEntry { key: "Condition", category: DocCategory::Stdlib, markdown: include_str!("../docs/lang/stdlib/Condition.md") });

    // M2+ extensions
    m.insert("TRY", DocEntry { key: "TRY", category: DocCategory::Extension, markdown: include_str!("../docs/lang/m2plus/TRY.md") });
    m.insert("EXCEPT", DocEntry { key: "EXCEPT", category: DocCategory::Extension, markdown: include_str!("../docs/lang/m2plus/EXCEPT.md") });
    m.insert("FINALLY", DocEntry { key: "FINALLY", category: DocCategory::Extension, markdown: include_str!("../docs/lang/m2plus/FINALLY.md") });
    m.insert("RAISE", DocEntry { key: "RAISE", category: DocCategory::Extension, markdown: include_str!("../docs/lang/m2plus/RAISE.md") });
    m.insert("EXCEPTION", DocEntry { key: "EXCEPTION", category: DocCategory::Extension, markdown: include_str!("../docs/lang/m2plus/EXCEPTION.md") });
    m.insert("RETRY", DocEntry { key: "RETRY", category: DocCategory::Extension, markdown: include_str!("../docs/lang/m2plus/RETRY.md") });
    m.insert("REF", DocEntry { key: "REF", category: DocCategory::Extension, markdown: include_str!("../docs/lang/m2plus/REF.md") });
    m.insert("REFANY", DocEntry { key: "REFANY", category: DocCategory::Extension, markdown: include_str!("../docs/lang/m2plus/REFANY.md") });
    m.insert("BRANDED", DocEntry { key: "BRANDED", category: DocCategory::Extension, markdown: include_str!("../docs/lang/m2plus/BRANDED.md") });
    m.insert("OBJECT", DocEntry { key: "OBJECT", category: DocCategory::Extension, markdown: include_str!("../docs/lang/m2plus/OBJECT.md") });
    m.insert("METHODS", DocEntry { key: "METHODS", category: DocCategory::Extension, markdown: include_str!("../docs/lang/m2plus/METHODS.md") });
    m.insert("OVERRIDES", DocEntry { key: "OVERRIDES", category: DocCategory::Extension, markdown: include_str!("../docs/lang/m2plus/OVERRIDES.md") });
    m.insert("LOCK", DocEntry { key: "LOCK", category: DocCategory::Extension, markdown: include_str!("../docs/lang/m2plus/LOCK.md") });
    m.insert("TYPECASE", DocEntry { key: "TYPECASE", category: DocCategory::Extension, markdown: include_str!("../docs/lang/m2plus/TYPECASE.md") });
    m.insert("SAFE", DocEntry { key: "SAFE", category: DocCategory::Extension, markdown: include_str!("../docs/lang/m2plus/SAFE.md") });
    m.insert("UNSAFE", DocEntry { key: "UNSAFE", category: DocCategory::Extension, markdown: include_str!("../docs/lang/m2plus/UNSAFE.md") });

    m
});

// ── Library Docs (loaded from disk at runtime) ──────────────────────

/// A library documentation entry loaded from disk.
pub struct LibraryDocEntry {
    pub key: String,
    pub category: String,     // "LibGraphics", "LibNetworking", etc.
    pub markdown: String,
}

/// Runtime-loaded library documentation.
/// Populated by scanning `docs/libs/` using `libraries.toml` as the category index.
pub struct LibraryDocs {
    entries: HashMap<String, LibraryDocEntry>,
}

impl LibraryDocs {
    /// Create an empty LibraryDocs (used when docs path is not found).
    pub fn empty() -> Self {
        Self { entries: HashMap::new() }
    }

    /// Load library docs from the given docs root directory.
    /// Reads `docs_root/libs/libraries.toml` for category mappings,
    /// then scans each library subdirectory for module .md files.
    pub fn load(docs_root: &Path) -> Self {
        let manifest_path = docs_root.join("libs").join("libraries.toml");
        let manifest_str = match std::fs::read_to_string(&manifest_path) {
            Ok(s) => s,
            Err(_) => return Self::empty(),
        };

        // Parse the TOML manifest (minimal parser — no external deps)
        let categories = parse_libraries_toml(&manifest_str);

        let mut entries = HashMap::new();
        let libs_dir = docs_root.join("libs");

        for (lib_dir_name, category_name) in &categories {
            let lib_path = libs_dir.join(lib_dir_name);
            if !lib_path.is_dir() {
                continue;
            }
            let category = format!("Lib{}", category_name);

            let dir_entries = match std::fs::read_dir(&lib_path) {
                Ok(d) => d,
                Err(_) => continue,
            };

            for entry in dir_entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("md") {
                    continue;
                }
                let stem = match path.file_stem().and_then(|s| s.to_str()) {
                    Some(s) => s.to_string(),
                    None => continue,
                };
                // Only include files whose stem looks like a module name:
                // starts with uppercase, no hyphens or underscores
                if !is_module_name(&stem) {
                    continue;
                }
                let markdown = match std::fs::read_to_string(&path) {
                    Ok(s) => s,
                    Err(_) => continue,
                };

                // Insert exact-case entry
                let doc = LibraryDocEntry {
                    key: stem.clone(),
                    category: category.clone(),
                    markdown: markdown.clone(),
                };
                entries.insert(stem.clone(), doc);

                // Insert uppercase alias (if different from exact case)
                let upper = stem.to_uppercase();
                if upper != stem {
                    let alias = LibraryDocEntry {
                        key: stem.clone(),
                        category: category.clone(),
                        markdown,
                    };
                    entries.insert(upper, alias);
                }
            }
        }

        Self { entries }
    }

    /// Lookup a library doc entry by key (case-insensitive).
    pub fn get(&self, key: &str) -> Option<&LibraryDocEntry> {
        if let Some(entry) = self.entries.get(key) {
            return Some(entry);
        }
        self.entries.get(&key.to_uppercase())
    }

    /// Return all canonical library doc keys (deduplicated).
    pub fn all_keys(&self) -> Vec<&str> {
        let mut seen = std::collections::HashSet::new();
        self.entries.values()
            .filter(|entry| seen.insert(entry.key.as_str()))
            .map(|entry| entry.key.as_str())
            .collect()
    }
}

/// Check if a filename stem looks like a Modula-2 module name.
/// Must start with uppercase A-Z, rest alphanumeric only.
fn is_module_name(stem: &str) -> bool {
    let mut chars = stem.chars();
    match chars.next() {
        Some(c) if c.is_ascii_uppercase() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric())
}

/// Minimal TOML parser for libraries.toml.
/// Extracts [section] + category = "value" pairs.
fn parse_libraries_toml(content: &str) -> Vec<(String, String)> {
    let mut result = Vec::new();
    let mut current_section = String::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            current_section = trimmed[1..trimmed.len() - 1].trim().to_string();
        } else if let Some(eq_pos) = trimmed.find('=') {
            let key = trimmed[..eq_pos].trim();
            let val = trimmed[eq_pos + 1..].trim().trim_matches('"');
            if key == "category" && !current_section.is_empty() {
                result.push((current_section.clone(), val.to_string()));
            }
        }
    }
    result
}

/// Resolve the docs root directory.
/// Checks (in order):
///   1. M2C_DOCS_PATH environment variable
///   2. Walk up from the binary's real path looking for docs/libs/libraries.toml
pub fn resolve_docs_root() -> Option<PathBuf> {
    // 1. Environment variable
    if let Ok(path) = std::env::var("M2C_DOCS_PATH") {
        let p = PathBuf::from(&path);
        if p.join("libs").join("libraries.toml").exists() {
            return Some(p);
        }
    }

    // 2. Walk up from binary location
    if let Ok(exe) = std::env::current_exe() {
        // Resolve symlinks to get the real path
        let real = match std::fs::canonicalize(&exe) {
            Ok(p) => p,
            Err(_) => exe,
        };
        let mut dir = real.parent();
        while let Some(d) = dir {
            let candidate = d.join("docs").join("libs").join("libraries.toml");
            if candidate.exists() {
                return Some(d.join("docs"));
            }
            dir = d.parent();
        }
    }

    // 3. Check install prefix (M2C_HOME or ~/.m2c)
    let prefix = std::env::var_os("M2C_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".m2c")));
    if let Some(p) = prefix {
        let candidate = p.join("docs");
        if candidate.join("libs").join("libraries.toml").exists() {
            return Some(candidate);
        }
    }

    None
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_doc_builtin_type() {
        let entry = get_doc("INTEGER").unwrap();
        assert_eq!(entry.key, "INTEGER");
        assert_eq!(entry.category, DocCategory::Builtin);
        assert!(entry.markdown.contains("INTEGER"));
    }

    #[test]
    fn test_get_doc_case_insensitive() {
        let entry = get_doc("integer").unwrap();
        assert_eq!(entry.key, "INTEGER");
    }

    #[test]
    fn test_get_doc_stdlib_exact_case() {
        let entry = get_doc("InOut").unwrap();
        assert_eq!(entry.key, "InOut");
        assert_eq!(entry.category, DocCategory::Stdlib);
    }

    #[test]
    fn test_get_doc_stdlib_uppercase() {
        let entry = get_doc("INOUT").unwrap();
        assert_eq!(entry.key, "InOut");
    }

    #[test]
    fn test_get_doc_keyword() {
        let entry = get_doc("WHILE").unwrap();
        assert_eq!(entry.category, DocCategory::Language);
    }

    #[test]
    fn test_get_doc_extension() {
        let entry = get_doc("TRY").unwrap();
        assert_eq!(entry.category, DocCategory::Extension);
    }

    #[test]
    fn test_get_doc_missing() {
        assert!(get_doc("NONEXISTENT").is_none());
    }

    #[test]
    fn test_core_registry_size() {
        // Core entries only (no library entries)
        assert!(REGISTRY.len() >= 80, "expected at least 80 core entries, got {}", REGISTRY.len());
    }

    #[test]
    fn test_is_module_name() {
        assert!(is_module_name("Gfx"));
        assert!(is_module_name("HTTPClient"));
        assert!(is_module_name("TLS"));
        assert!(is_module_name("ByteBuf"));
        // NOTE: README passes is_module_name() since it starts uppercase and is all alpha.
        // This is fine — "README" won't conflict with any real M2 module name.
        assert!(!is_module_name("api"));         // lowercase start
        assert!(!is_module_name("design"));      // lowercase start
        assert!(!is_module_name("Stream-Architecture")); // has hyphen
        assert!(!is_module_name("http_get_example"));    // has underscore, lowercase
    }

    #[test]
    fn test_parse_libraries_toml() {
        let toml = r#"
# comment
[m2gfx]
category = "Graphics"

[m2log]
category = "Helpers"
"#;
        let result = parse_libraries_toml(toml);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], ("m2gfx".to_string(), "Graphics".to_string()));
        assert_eq!(result[1], ("m2log".to_string(), "Helpers".to_string()));
    }

    #[test]
    fn test_library_docs_load() {
        // Test loading from the actual docs directory
        if let Some(docs_root) = resolve_docs_root() {
            let lib_docs = LibraryDocs::load(&docs_root);
            // Should find at least some library entries
            assert!(!lib_docs.entries.is_empty(), "expected library docs to be loaded");
            // Check a known module
            let gfx = lib_docs.get("Gfx");
            assert!(gfx.is_some(), "expected Gfx doc to be loaded");
            assert_eq!(gfx.unwrap().category, "LibGraphics");
            // Check case-insensitive lookup
            assert!(lib_docs.get("GFX").is_some());
            // Check m2bytes
            assert!(lib_docs.get("ByteBuf").is_some());
            assert_eq!(lib_docs.get("ByteBuf").unwrap().category, "LibHelpers");
        }
    }

    #[test]
    fn test_library_docs_empty() {
        let lib_docs = LibraryDocs::empty();
        assert!(lib_docs.get("Gfx").is_none());
        assert!(lib_docs.all_keys().is_empty());
    }
}
