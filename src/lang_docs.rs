//! Tier 1: Embedded language documentation for Modula-2 / M2+.
//!
//! Provides O(1) lookup of comprehensive documentation for types, builtins,
//! keywords, stdlib modules, and M2+ extensions. Documentation is sourced
//! from `docs/lang/` markdown files via `include_str!`.

use std::collections::HashMap;
use std::sync::LazyLock;

/// A documentation entry for a language element.
pub struct DocEntry {
    pub key: &'static str,
    pub category: DocCategory,
    pub markdown: &'static str,
}

/// Category of documentation entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocCategory {
    Language,
    Builtin,
    Stdlib,
    Extension,
}

/// O(1) lookup of embedded documentation by key.
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

/// Return all registered documentation keys (deduplicated by canonical key).
pub fn all_keys() -> Vec<&'static str> {
    let mut seen = std::collections::HashSet::new();
    REGISTRY.values()
        .filter(|entry| seen.insert(entry.key))
        .map(|entry| entry.key)
        .collect()
}

// ── Registry ────────────────────────────────────────────────────────

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
    // FileSystem not yet documented
    // m.insert("FileSystem", DocEntry { ... });

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
    fn test_registry_size() {
        // Verify we have a reasonable number of entries
        assert!(REGISTRY.len() >= 80, "expected at least 80 entries, got {}", REGISTRY.len());
    }
}
