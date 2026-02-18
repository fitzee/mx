use crate::json::Json;
use crate::symtab::{SymbolTable, SymbolKind};
use crate::token::TokenKind;
use super::hover::word_at_position;

/// Handle textDocument/completion request.
pub fn completion(
    source: &str,
    line: usize,
    col: usize,
    symtab: &SymbolTable,
) -> Json {
    let prefix = word_at_position(source, line, col).unwrap_or_default();
    let prefix_upper = prefix.to_uppercase();

    let mut items = Vec::new();

    // Check if we're completing a qualified name (after a dot)
    let lines: Vec<&str> = source.lines().collect();
    let mut qualified_module: Option<String> = None;
    if line < lines.len() && col > 0 {
        let line_text = lines[line];
        let chars: Vec<char> = line_text.chars().collect();
        // Look for a dot before the current word
        let mut pos = if col < chars.len() { col } else { chars.len() };
        // Skip back past current word
        while pos > 0 && (chars[pos - 1].is_ascii_alphanumeric() || chars[pos - 1] == '_') {
            pos -= 1;
        }
        if pos > 0 && chars[pos - 1] == '.' {
            // Found a dot — find the module name before it
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
        if let Some(sym) = symtab.lookup(mod_name) {
            if let SymbolKind::Module { scope_id } = &sym.kind {
                for s in symtab.scope_symbols(*scope_id) {
                    if s.exported && (prefix.is_empty() || s.name.to_uppercase().starts_with(&prefix_upper)) {
                        items.push(make_completion_item(&s.name, &s.kind));
                    }
                }
            }
        }
    } else {
        // Unqualified: scope-visible symbols + keywords
        // Add symbols from current scope
        // We'll iterate through what we can from the symtab
        // For a simple approach, look up all names that start with prefix
        add_scope_completions(symtab, &prefix_upper, &mut items);

        // Add keywords if prefix looks like a keyword start
        if !prefix.is_empty() {
            add_keyword_completions(&prefix_upper, &mut items);
        }
    }

    Json::obj(vec![
        ("isIncomplete", Json::Bool(false)),
        ("items", Json::arr(items)),
    ])
}

fn add_scope_completions(symtab: &SymbolTable, prefix_upper: &str, items: &mut Vec<Json>) {
    // Iterate all scopes for symbols visible to the module
    let mut seen = std::collections::HashSet::new();
    for scope_id in 0..symtab.scope_count() {
        for sym in symtab.scope_symbols(scope_id) {
            if (prefix_upper.is_empty() || sym.name.to_uppercase().starts_with(prefix_upper))
                && seen.insert(sym.name.clone())
            {
                items.push(make_completion_item(&sym.name, &sym.kind));
            }
        }
    }
}

fn make_completion_item(name: &str, kind: &SymbolKind) -> Json {
    let lsp_kind = match kind {
        SymbolKind::Constant(_) => 21, // Constant
        SymbolKind::Variable => 6,     // Variable
        SymbolKind::Type => 25,        // TypeParameter -> better: Class=7 for types
        SymbolKind::Procedure { .. } => 3, // Function
        SymbolKind::Module { .. } => 9,    // Module
        SymbolKind::Field => 5,            // Field
        SymbolKind::EnumVariant(_) => 20,  // EnumMember
    };
    Json::obj(vec![
        ("label", Json::str_val(name)),
        ("kind", Json::int_val(lsp_kind)),
    ])
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
            ]));
        }
    }
}
