use crate::analyze::{self, ScopeMap};
use crate::json::Json;
use crate::symtab::{SymbolTable, SymbolKind, ConstValue};
use crate::types::TypeRegistry;

/// Handle textDocument/hover request.
/// Finds the token at the given position and looks it up in the symbol table,
/// using the ScopeMap for visibility-correct lookup and TypeRegistry for type display.
/// Falls back to centralized language documentation for builtins and keywords.
pub fn hover(
    source: &str,
    line: usize,
    col: usize,
    symtab: &SymbolTable,
    types: &TypeRegistry,
    scope_map: &ScopeMap,
) -> Option<Json> {
    let word = word_at_position(source, line, col)?;

    // Use scope-aware lookup: convert LSP 0-based line to 1-based for ScopeMap
    let scope_id = scope_map.scope_at(line + 1, col + 1);
    let sym = symtab.lookup_in_scope(scope_id, &word)
        .or_else(|| symtab.lookup_all(&word));

    if let Some(sym) = sym {
        // Tier 1: Doc comment from source (highest priority)
        if let Some(ref doc_comment) = sym.doc {
            let mut md = format!("```modula2\n{}\n```\n", semantic_hover(sym, types));
            md.push_str(doc_comment);
            return Some(make_hover_response(&md));
        }

        // Tier 2: Embedded language documentation (rich markdown from docs/lang/)
        if let Some(entry) = crate::lang_docs::get_doc(&word) {
            let md = crate::lang_docs::format_hover(entry);
            return Some(make_hover_response(&md));
        }

        // Tier 2 fallback: inline lang_docs registry
        if let Some(doc) = super::lang_docs::lookup(&word) {
            return Some(make_hover_response(&super::lang_docs::format_hover(doc)));
        }

        // Tier 3: Semantic signature only
        let sig = semantic_hover(sym, types);
        if !sym.loc.file.is_empty() && sym.loc.line > 0 {
            return Some(make_hover_response(&format!("```modula2\n{}\n```", sig)));
        }
        return Some(make_hover_response(&format!("```modula2\n{}\n```", sig)));
    }

    // No symbol found — try embedded lang docs, then inline lang_docs for keywords
    if let Some(entry) = crate::lang_docs::get_doc(&word) {
        let md = crate::lang_docs::format_hover(entry);
        return Some(make_hover_response(&md));
    }
    if let Some(doc) = super::lang_docs::lookup(&word) {
        return Some(make_hover_response(&super::lang_docs::format_hover(doc)));
    }

    None
}

fn make_hover_response(markdown: &str) -> Json {
    Json::obj(vec![
        ("contents", Json::obj(vec![
            ("kind", Json::str_val("markdown")),
            ("value", Json::str_val(markdown)),
        ])),
    ])
}

fn semantic_hover(sym: &crate::symtab::Symbol, types: &TypeRegistry) -> String {
    match &sym.kind {
        SymbolKind::Variable => {
            let type_str = analyze::type_to_string(types, sym.typ);
            format!("VAR {}: {}", sym.name, type_str)
        }
        SymbolKind::Constant(cv) => {
            let val = match cv {
                ConstValue::Integer(n) => format!("{}", n),
                ConstValue::Real(r) => format!("{}", r),
                ConstValue::Boolean(b) => if *b { "TRUE".to_string() } else { "FALSE".to_string() },
                ConstValue::Char(c) => format!("'{}'", c),
                ConstValue::String(s) => format!("\"{}\"", s),
                ConstValue::Set(_) => "SET".to_string(),
                ConstValue::Nil => "NIL".to_string(),
            };
            format!("CONST {} = {}", sym.name, val)
        }
        SymbolKind::Type => {
            let type_str = analyze::type_to_string(types, sym.typ);
            format!("TYPE {} = {}", sym.name, type_str)
        }
        SymbolKind::Procedure { params, return_type, .. } => {
            let mut sig = format!("PROCEDURE {}(", sym.name);
            for (i, p) in params.iter().enumerate() {
                if i > 0 { sig.push_str("; "); }
                if p.is_var { sig.push_str("VAR "); }
                sig.push_str(&p.name);
                sig.push_str(": ");
                sig.push_str(&analyze::type_to_string(types, p.typ));
            }
            sig.push(')');
            if let Some(rt) = return_type {
                sig.push_str(": ");
                sig.push_str(&analyze::type_to_string(types, *rt));
            }
            sig
        }
        SymbolKind::Module { .. } => format!("MODULE {}", sym.name),
        SymbolKind::Field => {
            let type_str = analyze::type_to_string(types, sym.typ);
            format!("field {}: {}", sym.name, type_str)
        }
        SymbolKind::EnumVariant(v) => format!("{} = {}", sym.name, v),
    }
}

/// Extract the word at a given (0-based) line and column in the source text.
pub fn word_at_position(source: &str, line: usize, col: usize) -> Option<String> {
    let lines: Vec<&str> = source.lines().collect();
    if line >= lines.len() { return None; }
    let line_text = lines[line];
    if col >= line_text.len() { return None; }

    let chars: Vec<char> = line_text.chars().collect();
    if !chars[col].is_ascii_alphanumeric() && chars[col] != '_' {
        return None;
    }

    // Find word boundaries
    let mut start = col;
    while start > 0 && (chars[start - 1].is_ascii_alphanumeric() || chars[start - 1] == '_') {
        start -= 1;
    }
    let mut end = col;
    while end < chars.len() && (chars[end].is_ascii_alphanumeric() || chars[end] == '_') {
        end += 1;
    }

    Some(chars[start..end].iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyze;

    #[test]
    fn test_hover_builtin_type_doc() {
        // INTEGER is a builtin type with SourceLoc::default()
        let source = "MODULE Test;\nVAR x: INTEGER;\nBEGIN\n  x := 1;\nEND Test.\n";
        let result = analyze::analyze_source(source, "test.mod", false, &[]);
        // Hover over "INTEGER" at line 1, col 7
        let h = hover(source, 1, 8, &result.symtab, &result.types, &result.scope_map);
        assert!(h.is_some(), "expected hover for INTEGER");
        let h = h.unwrap();
        let value = h.get("contents")
            .and_then(|c| c.get("value"))
            .and_then(|v| v.as_str())
            .unwrap();
        // Rich embedded docs contain "# INTEGER" header and description
        assert!(value.contains("INTEGER"), "expected INTEGER in hover, got: {}", value);
        assert!(value.contains("Signed whole number"), "expected summary, got: {}", value);
    }

    #[test]
    fn test_hover_keyword_doc() {
        // MODULE is a keyword — not in symtab as a resolvable word at position
        // but we test with a word that IS a keyword and not a symbol
        let source = "MODULE Test;\nBEGIN\nEND Test.\n";
        let result = analyze::analyze_source(source, "test.mod", false, &[]);
        // Hover over "REPEAT" (not present in source but we can test lookup directly)
        // Instead test via lang_docs lookup
        let doc = super::super::lang_docs::lookup("WHILE");
        assert!(doc.is_some());
        let doc = doc.unwrap();
        assert!(doc.summary.contains("loop"));
    }

    #[test]
    fn test_hover_user_symbol_not_overridden() {
        // A user-defined variable should show semantic info, not lang_docs
        let source = "MODULE Test;\nVAR x: INTEGER;\nBEGIN\n  x := 42;\nEND Test.\n";
        let result = analyze::analyze_source(source, "test.mod", false, &[]);
        // Hover over "x" at line 1, col 4
        let h = hover(source, 1, 4, &result.symtab, &result.types, &result.scope_map);
        assert!(h.is_some());
        let h = h.unwrap();
        let value = h.get("contents")
            .and_then(|c| c.get("value"))
            .and_then(|v| v.as_str())
            .unwrap();
        assert!(value.contains("VAR x: INTEGER"), "expected semantic info, got: {}", value);
    }

    #[test]
    fn test_hover_doc_comment() {
        // A procedure with a doc comment should show the doc comment in hover
        let source = "MODULE Test;\n(** Adds two integers. *)\nPROCEDURE Add(a, b: INTEGER): INTEGER;\nBEGIN\n  RETURN a + b;\nEND Add;\nBEGIN\nEND Test.\n";
        let result = analyze::analyze_source(source, "test.mod", false, &[]);
        // Hover over "Add" at line 2, col 10
        let h = hover(source, 2, 10, &result.symtab, &result.types, &result.scope_map);
        assert!(h.is_some(), "expected hover for Add");
        let h = h.unwrap();
        let value = h.get("contents")
            .and_then(|c| c.get("value"))
            .and_then(|v| v.as_str())
            .unwrap();
        assert!(value.contains("Adds two integers"), "expected doc comment in hover, got: {}", value);
        assert!(value.contains("PROCEDURE Add"), "expected signature in hover, got: {}", value);
    }

    #[test]
    fn test_hover_embedded_doc_for_keyword() {
        // Keywords should get rich embedded docs from docs/lang/
        let doc = crate::lang_docs::get_doc("IF");
        assert!(doc.is_some(), "expected embedded doc for IF keyword");
        let entry = doc.unwrap();
        assert!(entry.markdown.contains("IF"), "expected IF in markdown");
    }

    #[test]
    fn test_hover_stdlib_proc_doc() {
        // Stdlib procedures imported via FROM InOut IMPORT should show their doc strings
        let source = "MODULE Test;\nFROM InOut IMPORT WriteLn, WriteString;\nBEGIN\n  WriteString(\"hi\");\n  WriteLn;\nEND Test.\n";
        let result = analyze::analyze_source(source, "test.mod", false, &[]);
        // Hover over "WriteLn" at line 4, col 2
        let h = hover(source, 4, 3, &result.symtab, &result.types, &result.scope_map);
        assert!(h.is_some(), "expected hover for WriteLn");
        let h = h.unwrap();
        let value = h.get("contents")
            .and_then(|c| c.get("value"))
            .and_then(|v| v.as_str())
            .unwrap();
        assert!(value.contains("newline"), "expected doc about newline, got: {}", value);

        // Hover over "WriteString" at line 3, col 2
        let h2 = hover(source, 3, 3, &result.symtab, &result.types, &result.scope_map);
        assert!(h2.is_some(), "expected hover for WriteString");
        let h2 = h2.unwrap();
        let value2 = h2.get("contents")
            .and_then(|c| c.get("value"))
            .and_then(|v| v.as_str())
            .unwrap();
        assert!(value2.contains("string"), "expected doc about string, got: {}", value2);
    }
}
