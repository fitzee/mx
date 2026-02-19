use crate::analyze::{self, ScopeMap};
use crate::json::Json;
use crate::symtab::{SymbolTable, SymbolKind, ConstValue};
use crate::types::TypeRegistry;
use super::hover::word_at_position;
use super::index::WorkspaceIndex;

/// Handle textDocument/completion request.
/// Uses ScopeMap for scope-aware completion.
/// Returns lightweight items with `data` for lazy resolve.
pub fn completion(
    source: &str,
    uri: &str,
    line: usize,
    col: usize,
    symtab: &SymbolTable,
    types: &TypeRegistry,
    scope_map: &ScopeMap,
    workspace_index: Option<&WorkspaceIndex>,
) -> Json {
    let prefix = word_at_position(source, line, col).unwrap_or_default();
    let prefix_upper = prefix.to_uppercase();

    let mut items = Vec::new();

    let lines: Vec<&str> = source.lines().collect();
    let line_text = if line < lines.len() { lines[line] } else { "" };

    // Detect import context
    let import_ctx = detect_import_context(line_text, col);

    match import_ctx {
        ImportContext::ModuleName => {
            // After FROM or IMPORT — suggest module names
            add_module_name_completions(&prefix_upper, symtab, workspace_index, uri, &mut items);
            sort_items(&mut items);
            return Json::obj(vec![
                ("isIncomplete", Json::Bool(false)),
                ("items", Json::arr(items)),
            ]);
        }
        ImportContext::SymbolFromModule(ref mod_name) => {
            // After FROM X IMPORT — suggest exports from that module
            let mod_upper = mod_name.to_uppercase();
            add_module_exports(symtab, &mod_upper, &prefix_upper, uri, &mut items);
            // Also search workspace index for symbols from that module
            if let Some(idx) = workspace_index {
                add_workspace_module_exports(idx, mod_name, &prefix_upper, uri, &mut items);
            }
            sort_items(&mut items);
            return Json::obj(vec![
                ("isIncomplete", Json::Bool(false)),
                ("items", Json::arr(items)),
            ]);
        }
        ImportContext::None => {}
    }

    // Check if we're completing a qualified name (after a dot)
    let mut qualified_module: Option<String> = None;
    if line < lines.len() && col > 0 {
        let chars: Vec<char> = line_text.chars().collect();
        let mut pos = if col < chars.len() { col } else { chars.len() };
        while pos > 0 && (chars[pos - 1].is_ascii_alphanumeric() || chars[pos - 1] == '_') {
            pos -= 1;
        }
        if pos > 0 && chars[pos - 1] == '.' {
            pos -= 1;
            let mod_end = pos;
            while pos > 0 && (chars[pos - 1].is_ascii_alphanumeric() || chars[pos - 1] == '_') {
                pos -= 1;
            }
            let mod_name: String = chars[pos..mod_end].iter().collect();
            if !mod_name.is_empty() {
                qualified_module = Some(mod_name);
            }
        }
    }

    if let Some(ref mod_name) = qualified_module {
        // Qualified completion: module.name
        let scope_id = scope_map.scope_at(line + 1, col + 1);
        if let Some(sym) = symtab.lookup_in_scope(scope_id, mod_name)
            .or_else(|| symtab.lookup_all(mod_name))
        {
            if let SymbolKind::Module { scope_id: mod_scope } = &sym.kind {
                for s in symtab.scope_symbols(*mod_scope) {
                    if s.exported && (prefix.is_empty() || s.name.to_uppercase().starts_with(&prefix_upper)) {
                        items.push(make_completion_item(&s.name, &s.kind, types, uri));
                    }
                }
            }
        }
    } else {
        // Unqualified: scope-visible symbols + keywords
        let scope_id = scope_map.scope_at(line + 1, col + 1);
        add_scope_completions(symtab, types, scope_id, &prefix_upper, uri, &mut items);

        // Add keywords if prefix looks like a keyword start
        if !prefix.is_empty() {
            add_keyword_completions(&prefix_upper, &mut items);

            // Suggest imports for unknown identifiers from workspace
            if let Some(idx) = workspace_index {
                add_import_completions(idx, &prefix_upper, uri, source, &mut items);
            }
        }
    }

    sort_items(&mut items);

    Json::obj(vec![
        ("isIncomplete", Json::Bool(false)),
        ("items", Json::arr(items)),
    ])
}

/// Handle completionItem/resolve — fill detail and documentation from analysis.
/// Uses lang_docs for builtins/keywords; semantic info for user-defined symbols.
pub fn resolve_completion(
    name: &str,
    symtab: &SymbolTable,
    types: &TypeRegistry,
    item: Json,
) -> Json {
    let label = item.get("label").and_then(|l| l.as_str()).unwrap_or(name);
    let kind = item.get("kind").and_then(|k| k.as_i64()).unwrap_or(1);

    if let Some(sym) = symtab.lookup_all(name) {
        let detail = symbol_detail(sym, types);

        let mut fields = vec![
            ("label", Json::str_val(label)),
            ("kind", Json::int_val(kind)),
            ("detail", Json::str_val(&detail)),
        ];

        // Tier 1: Doc comment from source
        if let Some(ref doc_comment) = sym.doc {
            fields.push(("documentation", Json::obj(vec![
                ("kind", Json::str_val("markdown")),
                ("value", Json::str_val(doc_comment)),
            ])));
        }
        // Tier 2: Embedded lang docs (rich markdown)
        else if let Some(entry) = crate::lang_docs::get_doc(name) {
            fields.push(("documentation", Json::obj(vec![
                ("kind", Json::str_val("markdown")),
                ("value", Json::str_val(&crate::lang_docs::format_hover(entry))),
            ])));
        }
        // Tier 2 fallback: inline lang_docs for builtins
        else if sym.loc.file.is_empty() && sym.loc.line == 0 {
            if let Some(doc) = super::lang_docs::lookup(name) {
                let mut doc_text = doc.summary.to_string();
                if let Some(details) = doc.details {
                    doc_text.push_str("\n\n");
                    doc_text.push_str(details);
                }
                fields.push(("documentation", Json::obj(vec![
                    ("kind", Json::str_val("markdown")),
                    ("value", Json::str_val(&doc_text)),
                ])));
            }
        }

        if let Some(data) = item.get("data") {
            fields.push(("data", data.clone()));
        }
        // Preserve insertText/insertTextFormat if present (snippets)
        if let Some(it) = item.get("insertText") {
            fields.push(("insertText", it.clone()));
        }
        if let Some(itf) = item.get("insertTextFormat") {
            fields.push(("insertTextFormat", itf.clone()));
        }
        return Json::obj(fields);
    }

    // Not in symtab — try embedded lang docs first, then inline lang_docs
    if let Some(entry) = crate::lang_docs::get_doc(name) {
        let mut fields = vec![
            ("label", Json::str_val(label)),
            ("kind", Json::int_val(kind)),
            ("detail", Json::str_val(entry.key)),
            ("documentation", Json::obj(vec![
                ("kind", Json::str_val("markdown")),
                ("value", Json::str_val(&crate::lang_docs::format_hover(entry))),
            ])),
        ];
        if let Some(data) = item.get("data") {
            fields.push(("data", data.clone()));
        }
        if let Some(it) = item.get("insertText") {
            fields.push(("insertText", it.clone()));
        }
        if let Some(itf) = item.get("insertTextFormat") {
            fields.push(("insertTextFormat", itf.clone()));
        }
        return Json::obj(fields);
    }
    if let Some(doc) = super::lang_docs::lookup(name) {
        let detail = doc.signature.unwrap_or(doc.name);
        let mut doc_text = doc.summary.to_string();
        if let Some(details) = doc.details {
            doc_text.push_str("\n\n");
            doc_text.push_str(details);
        }
        let mut fields = vec![
            ("label", Json::str_val(label)),
            ("kind", Json::int_val(kind)),
            ("detail", Json::str_val(detail)),
            ("documentation", Json::obj(vec![
                ("kind", Json::str_val("markdown")),
                ("value", Json::str_val(&doc_text)),
            ])),
        ];
        if let Some(data) = item.get("data") {
            fields.push(("data", data.clone()));
        }
        if let Some(it) = item.get("insertText") {
            fields.push(("insertText", it.clone()));
        }
        if let Some(itf) = item.get("insertTextFormat") {
            fields.push(("insertTextFormat", itf.clone()));
        }
        return Json::obj(fields);
    }

    item
}

// ── Import context detection ────────────────────────────────────────

enum ImportContext {
    None,
    ModuleName,               // After FROM or IMPORT keyword
    SymbolFromModule(String), // After FROM X IMPORT
}

fn detect_import_context(line_text: &str, col: usize) -> ImportContext {
    let before = if col <= line_text.len() { &line_text[..col] } else { line_text };
    let trimmed = before.trim();

    // FROM ModuleName IMPORT <cursor>
    // Match: FROM <word> IMPORT (possibly with more words after)
    if let Some(rest) = strip_prefix_ci(trimmed, "FROM") {
        let rest = rest.trim_start();
        // Get module name
        let mod_end = rest.find(|c: char| !c.is_ascii_alphanumeric() && c != '_').unwrap_or(rest.len());
        let mod_name = &rest[..mod_end];
        if mod_name.is_empty() {
            return ImportContext::ModuleName;
        }
        let after_mod = rest[mod_end..].trim_start();
        // Check for IMPORT keyword after module name
        if starts_with_ci(after_mod, "IMPORT") {
            return ImportContext::SymbolFromModule(mod_name.to_string());
        }
        // Still typing module name (no IMPORT yet)
        return ImportContext::ModuleName;
    }

    // IMPORT <cursor>
    if starts_with_ci(trimmed, "IMPORT") {
        let rest = trimmed[6..].trim_start();
        if rest.is_empty() || rest.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return ImportContext::ModuleName;
        }
    }

    ImportContext::None
}

fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    if s.len() >= prefix.len() && s[..prefix.len()].eq_ignore_ascii_case(prefix) {
        let rest = &s[prefix.len()..];
        // Must be followed by whitespace or end
        if rest.is_empty() || rest.starts_with(char::is_whitespace) {
            return Some(rest);
        }
    }
    None
}

fn starts_with_ci(s: &str, prefix: &str) -> bool {
    s.len() >= prefix.len() && s[..prefix.len()].eq_ignore_ascii_case(prefix)
}

// ── Module name completions ─────────────────────────────────────────

fn add_module_name_completions(
    prefix_upper: &str,
    symtab: &SymbolTable,
    workspace_index: Option<&WorkspaceIndex>,
    uri: &str,
    items: &mut Vec<Json>,
) {
    let mut seen = std::collections::HashSet::new();

    // Stdlib modules
    for name in crate::stdlib::stdlib_module_names() {
        if (prefix_upper.is_empty() || name.to_uppercase().starts_with(prefix_upper))
            && seen.insert(name.to_uppercase())
        {
            items.push(Json::obj(vec![
                ("label", Json::str_val(name)),
                ("kind", Json::int_val(9)), // Module
                ("sortText", Json::str_val(&format!("0_{}", name))),
                ("data", Json::obj(vec![
                    ("uri", Json::str_val(uri)),
                    ("name", Json::str_val(name)),
                ])),
            ]));
        }
    }

    // Modules from symbol table
    for scope_id in 0..symtab.scope_count() {
        for sym in symtab.scope_symbols(scope_id) {
            if let SymbolKind::Module { .. } = &sym.kind {
                if (prefix_upper.is_empty() || sym.name.to_uppercase().starts_with(prefix_upper))
                    && seen.insert(sym.name.to_uppercase())
                {
                    items.push(Json::obj(vec![
                        ("label", Json::str_val(&sym.name)),
                        ("kind", Json::int_val(9)),
                        ("sortText", Json::str_val(&format!("1_{}", sym.name))),
                        ("data", Json::obj(vec![
                            ("uri", Json::str_val(uri)),
                            ("name", Json::str_val(&sym.name)),
                        ])),
                    ]));
                }
            }
        }
    }

    // Modules from workspace index
    if let Some(idx) = workspace_index {
        for ws in idx.search("", 200) {
            if ws.kind == super::index::SymbolKindTag::Module
                && (prefix_upper.is_empty() || ws.name.to_uppercase().starts_with(prefix_upper))
                && seen.insert(ws.name.to_uppercase())
            {
                items.push(Json::obj(vec![
                    ("label", Json::str_val(&ws.name)),
                    ("kind", Json::int_val(9)),
                    ("sortText", Json::str_val(&format!("2_{}", ws.name))),
                    ("data", Json::obj(vec![
                        ("uri", Json::str_val(uri)),
                        ("name", Json::str_val(&ws.name)),
                    ])),
                ]));
            }
        }
    }
}

// ── Module export completions ───────────────────────────────────────

fn add_module_exports(
    symtab: &SymbolTable,
    mod_name_upper: &str,
    prefix_upper: &str,
    uri: &str,
    items: &mut Vec<Json>,
) {
    // Find the module in symtab and list its exported symbols
    if let Some(sym) = symtab.lookup_all(mod_name_upper) {
        if let SymbolKind::Module { scope_id } = &sym.kind {
            for s in symtab.scope_symbols(*scope_id) {
                if s.exported && (prefix_upper.is_empty() || s.name.to_uppercase().starts_with(prefix_upper)) {
                    items.push(Json::obj(vec![
                        ("label", Json::str_val(&s.name)),
                        ("kind", Json::int_val(symbol_kind_to_lsp(&s.kind))),
                        ("sortText", Json::str_val(&format!("0_{}", s.name))),
                        ("data", Json::obj(vec![
                            ("uri", Json::str_val(uri)),
                            ("name", Json::str_val(&s.name)),
                        ])),
                    ]));
                }
            }
        }
    }
}

fn add_workspace_module_exports(
    idx: &WorkspaceIndex,
    mod_name: &str,
    prefix_upper: &str,
    uri: &str,
    items: &mut Vec<Json>,
) {
    let mut seen = std::collections::HashSet::new();
    // Collect items from the module
    for ws in idx.search("", 500) {
        if let Some(ref container) = ws.container {
            if container.eq_ignore_ascii_case(mod_name)
                && (prefix_upper.is_empty() || ws.name.to_uppercase().starts_with(prefix_upper))
                && seen.insert(ws.name.to_uppercase())
            {
                let lsp_kind = match ws.kind {
                    super::index::SymbolKindTag::Procedure => 3,
                    super::index::SymbolKindTag::Type => 7,
                    super::index::SymbolKindTag::Variable => 6,
                    super::index::SymbolKindTag::Constant => 21,
                    super::index::SymbolKindTag::Module => 9,
                };
                items.push(Json::obj(vec![
                    ("label", Json::str_val(&ws.name)),
                    ("kind", Json::int_val(lsp_kind)),
                    ("sortText", Json::str_val(&format!("1_{}", ws.name))),
                    ("data", Json::obj(vec![
                        ("uri", Json::str_val(uri)),
                        ("name", Json::str_val(&ws.name)),
                    ])),
                ]));
            }
        }
    }
}

// ── Scope completions ───────────────────────────────────────────────

fn add_scope_completions(
    symtab: &SymbolTable,
    types: &TypeRegistry,
    scope_id: usize,
    prefix_upper: &str,
    uri: &str,
    items: &mut Vec<Json>,
) {
    let mut seen = std::collections::HashSet::new();

    let mut current = Some(scope_id);
    while let Some(sid) = current {
        for sym in symtab.scope_symbols(sid) {
            if (prefix_upper.is_empty() || sym.name.to_uppercase().starts_with(prefix_upper))
                && seen.insert(sym.name.clone())
            {
                items.push(make_completion_item(&sym.name, &sym.kind, types, uri));
            }
        }
        current = symtab.scope_parent(sid);
    }
}

// ── Import suggestion completions ───────────────────────────────────

fn add_import_completions(
    idx: &WorkspaceIndex,
    prefix_upper: &str,
    uri: &str,
    _source: &str,
    items: &mut Vec<Json>,
) {
    // Search workspace for symbols matching prefix that could be imported
    let matches = idx.search(prefix_upper, 10);
    for ws in &matches {
        if let Some(ref module) = ws.container {
            if ws.name.to_uppercase().starts_with(prefix_upper) {
                let label = format!("{} (import from {})", ws.name, module);
                items.push(Json::obj(vec![
                    ("label", Json::str_val(&label)),
                    ("kind", Json::int_val(1)), // Text
                    ("sortText", Json::str_val(&format!("9_{}", ws.name))),
                    ("detail", Json::str_val(&format!("FROM {} IMPORT {}", module, ws.name))),
                    ("data", Json::obj(vec![
                        ("uri", Json::str_val(uri)),
                        ("name", Json::str_val(&ws.name)),
                    ])),
                ]));
            }
        }
    }
}

// ── Item construction ───────────────────────────────────────────────

fn make_completion_item(name: &str, kind: &SymbolKind, types: &TypeRegistry, uri: &str) -> Json {
    let lsp_kind = symbol_kind_to_lsp(kind);
    let mut fields = vec![
        ("label", Json::str_val(name)),
        ("kind", Json::int_val(lsp_kind)),
        ("sortText", Json::str_val(&format!("1_{}", name))),
        ("data", Json::obj(vec![
            ("uri", Json::str_val(uri)),
            ("name", Json::str_val(name)),
        ])),
    ];

    // Add procedure snippet
    if let SymbolKind::Procedure { params, return_type, .. } = kind {
        if !params.is_empty() {
            let mut snippet = format!("{}(", name);
            for (i, p) in params.iter().enumerate() {
                if i > 0 { snippet.push_str(", "); }
                snippet.push_str(&format!("${{{}:{}}}", i + 1, p.name));
            }
            snippet.push(')');
            fields.push(("insertText", Json::str_val(&snippet)));
            fields.push(("insertTextFormat", Json::int_val(2))); // Snippet
        }

        // Add detail (signature)
        let detail = proc_signature(params, return_type.as_ref(), types);
        fields.push(("detail", Json::str_val(&detail)));
    }

    Json::obj(fields)
}

fn symbol_kind_to_lsp(kind: &SymbolKind) -> i64 {
    match kind {
        SymbolKind::Constant(_) => 21, // Constant
        SymbolKind::Variable => 6,     // Variable
        SymbolKind::Type => 7,         // Class
        SymbolKind::Procedure { .. } => 3, // Function
        SymbolKind::Module { .. } => 9,    // Module
        SymbolKind::Field => 5,            // Field
        SymbolKind::EnumVariant(_) => 20,  // EnumMember
    }
}

fn proc_signature(params: &[crate::symtab::ParamInfo], return_type: Option<&crate::types::TypeId>, types: &TypeRegistry) -> String {
    let mut sig = String::from("(");
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

fn symbol_detail(sym: &crate::symtab::Symbol, types: &TypeRegistry) -> String {
    match &sym.kind {
        SymbolKind::Variable => analyze::type_to_string(types, sym.typ),
        SymbolKind::Constant(cv) => {
            let val = match cv {
                ConstValue::Integer(n) => format!("{}", n),
                ConstValue::Real(r) => format!("{}", r),
                ConstValue::Boolean(b) => if *b { "TRUE".into() } else { "FALSE".into() },
                ConstValue::Char(c) => format!("'{}'", c),
                ConstValue::String(s) => format!("\"{}\"", s),
                ConstValue::Set(_) => "SET".into(),
                ConstValue::Nil => "NIL".into(),
            };
            format!("= {}", val)
        }
        SymbolKind::Type => analyze::type_to_string(types, sym.typ),
        SymbolKind::Procedure { params, return_type, .. } => {
            proc_signature(params, return_type.as_ref(), types)
        }
        SymbolKind::Module { .. } => "MODULE".into(),
        SymbolKind::Field => analyze::type_to_string(types, sym.typ),
        SymbolKind::EnumVariant(v) => format!("= {}", v),
    }
}

fn add_keyword_completions(prefix_upper: &str, items: &mut Vec<Json>) {
    let keywords = [
        "AND", "ARRAY", "BEGIN", "BY", "CASE", "CONST", "DEFINITION", "DIV",
        "DO", "ELSE", "ELSIF", "END", "EXIT", "EXPORT", "FOR", "FROM", "IF",
        "IMPLEMENTATION", "IMPORT", "IN", "LOOP", "MOD", "MODULE", "NOT",
        "OF", "OR", "POINTER", "PROCEDURE", "QUALIFIED", "RECORD", "REPEAT",
        "RETURN", "SET", "THEN", "TO", "TYPE", "UNTIL", "VAR", "WHILE", "WITH",
    ];
    for kw in &keywords {
        if kw.starts_with(prefix_upper) {
            items.push(Json::obj(vec![
                ("label", Json::str_val(kw)),
                ("kind", Json::int_val(14)), // Keyword
                ("sortText", Json::str_val(&format!("2_{}", kw))),
            ]));
        }
    }
}

fn sort_items(items: &mut Vec<Json>) {
    items.sort_by(|a, b| {
        let sa = a.get("sortText").and_then(|s| s.as_str()).unwrap_or("");
        let sb = b.get("sortText").and_then(|s| s.as_str()).unwrap_or("");
        sa.cmp(sb)
    });
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_import_context_from() {
        assert!(matches!(detect_import_context("FROM ", 5), ImportContext::ModuleName));
        assert!(matches!(detect_import_context("FROM In", 7), ImportContext::ModuleName));
        assert!(matches!(detect_import_context("from ", 5), ImportContext::ModuleName));
    }

    #[test]
    fn test_detect_import_context_from_module_import() {
        match detect_import_context("FROM InOut IMPORT ", 18) {
            ImportContext::SymbolFromModule(m) => assert_eq!(m, "InOut"),
            _ => panic!("expected SymbolFromModule"),
        }
        match detect_import_context("FROM InOut IMPORT Write", 23) {
            ImportContext::SymbolFromModule(m) => assert_eq!(m, "InOut"),
            _ => panic!("expected SymbolFromModule"),
        }
    }

    #[test]
    fn test_detect_import_context_import() {
        assert!(matches!(detect_import_context("IMPORT ", 7), ImportContext::ModuleName));
        assert!(matches!(detect_import_context("IMPORT In", 9), ImportContext::ModuleName));
    }

    #[test]
    fn test_detect_import_context_none() {
        assert!(matches!(detect_import_context("VAR x: INTEGER;", 5), ImportContext::None));
        assert!(matches!(detect_import_context("  WriteString(", 14), ImportContext::None));
    }

    #[test]
    fn test_keyword_completion_sorts() {
        let mut items = Vec::new();
        add_keyword_completions("FO", &mut items);
        // Only FOR starts with FO (FROM starts with FR)
        assert_eq!(items.len(), 1);
        let label = items[0].get("label").and_then(|l| l.as_str()).unwrap();
        assert_eq!(label, "FOR");
    }

    #[test]
    fn test_proc_snippet_format() {
        use crate::symtab::ParamInfo;
        use crate::types::{TypeRegistry, TY_INTEGER, TY_CHAR};
        let types = TypeRegistry::new();
        let kind = SymbolKind::Procedure {
            params: vec![
                ParamInfo { name: "s".to_string(), typ: TY_INTEGER, is_var: false },
                ParamInfo { name: "ch".to_string(), typ: TY_CHAR, is_var: false },
            ],
            return_type: None,
            is_builtin: false,
        };
        let item = make_completion_item("WriteChar", &kind, &types, "file:///test.mod");
        let insert = item.get("insertText").and_then(|t| t.as_str()).unwrap();
        assert_eq!(insert, "WriteChar(${1:s}, ${2:ch})");
        let fmt = item.get("insertTextFormat").and_then(|f| f.as_i64()).unwrap();
        assert_eq!(fmt, 2); // Snippet format
    }

    #[test]
    fn test_completion_resolve_builtin_doc() {
        // Resolve a builtin symbol — should get lang_docs documentation
        use crate::analyze;
        let source = "MODULE Test;\nFROM InOut IMPORT WriteString;\nBEGIN\n  WriteString(\"hi\");\nEND Test.\n";
        let result = analyze::analyze_source(source, "test.mod", &[]);

        // Create a minimal completion item for "WriteString"
        let item = Json::obj(vec![
            ("label", Json::str_val("WriteString")),
            ("kind", Json::int_val(3)),
            ("data", Json::obj(vec![
                ("uri", Json::str_val("file:///test.mod")),
                ("name", Json::str_val("WriteString")),
            ])),
        ]);

        let resolved = resolve_completion("WriteString", &result.symtab, &result.types, item);
        // Should have detail (semantic signature)
        let detail = resolved.get("detail").and_then(|d| d.as_str());
        assert!(detail.is_some(), "expected detail field");
    }

    #[test]
    fn test_completion_resolve_keyword_doc() {
        // Resolve a keyword — no symtab entry, should get lang_docs
        use crate::analyze;
        let source = "MODULE Test;\nBEGIN\nEND Test.\n";
        let result = analyze::analyze_source(source, "test.mod", &[]);

        let item = Json::obj(vec![
            ("label", Json::str_val("WHILE")),
            ("kind", Json::int_val(14)),
        ]);

        let resolved = resolve_completion("WHILE", &result.symtab, &result.types, item);
        let doc = resolved.get("documentation");
        assert!(doc.is_some(), "expected documentation for keyword");
        let doc_value = doc.unwrap().get("value").and_then(|v| v.as_str()).unwrap();
        assert!(doc_value.contains("loop"), "expected loop description, got: {}", doc_value);
    }
}
