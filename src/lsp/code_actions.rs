use crate::analyze::AnalysisResult;
use crate::json::Json;
use super::index::WorkspaceIndex;

/// Handle textDocument/codeAction request.
/// Returns a list of CodeAction objects for available quick fixes.
pub fn code_actions(
    uri: &str,
    source: &str,
    params: &Json,
    result: &AnalysisResult,
    workspace_index: &WorkspaceIndex,
) -> Vec<Json> {
    let mut actions = Vec::new();

    let range = match params.get("range") {
        Some(r) => r,
        None => return actions,
    };

    let start_line = range.get("start")
        .and_then(|s| s.get("line"))
        .and_then(|l| l.as_i64())
        .unwrap_or(0) as usize;

    // Check diagnostics in range for quick-fix opportunities
    for diag in &result.diagnostics {
        let diag_line = if diag.loc.line > 0 { diag.loc.line - 1 } else { 0 };
        if diag_line != start_line {
            continue;
        }

        let msg = &diag.message;

        // Missing import: "undefined identifier 'Foo'"
        if msg.contains("undefined") || msg.contains("undeclared") {
            if let Some(name) = extract_undefined_name(msg) {
                // Search workspace index for a module that exports this symbol
                if let Some(action) = suggest_import(uri, source, &name, workspace_index) {
                    actions.push(action);
                }
            }
        }
    }

    // Stub procedure: compare def exports vs mod declarations
    if let Some(ast) = &result.ast {
        if let crate::ast::CompilationUnit::ImplementationModule(m) = ast {
            // Check if there are missing procedure implementations
            // (This would need the .def to be loaded — skip for now if not available)
            let _ = m;
        }
    }

    actions
}

/// Extract the undefined identifier name from an error message.
fn extract_undefined_name(msg: &str) -> Option<String> {
    // Match patterns: "undefined identifier 'Name'" or "undeclared identifier: Name"
    if let Some(start) = msg.find('\'') {
        let rest = &msg[start + 1..];
        if let Some(end) = rest.find('\'') {
            let name = &rest[..end];
            if !name.is_empty() && name.chars().next().map_or(false, |c| c.is_ascii_uppercase()) {
                return Some(name.to_string());
            }
        }
    }
    // Also try: "undeclared identifier: Name" (no quotes)
    if let Some(idx) = msg.find(": ") {
        let rest = msg[idx + 2..].trim();
        let name: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
        if !name.is_empty() && name.chars().next().map_or(false, |c| c.is_ascii_uppercase()) {
            return Some(name);
        }
    }
    None
}

/// Suggest adding an import for an undefined name.
fn suggest_import(uri: &str, source: &str, name: &str, workspace_index: &WorkspaceIndex) -> Option<Json> {
    // Search workspace symbols for matching name
    let matches = workspace_index.search(name, 10);
    let exact: Vec<_> = matches.iter()
        .filter(|s| s.name == name && s.container.is_some())
        .collect();

    if exact.is_empty() {
        return None;
    }

    // Use the first match's container module
    let module_name = exact[0].container.as_ref()?;

    // Find insertion point: after last IMPORT/FROM line, or after MODULE line
    let lines: Vec<&str> = source.lines().collect();
    let mut insert_line = 1; // default: after MODULE line
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("IMPORT") || trimmed.starts_with("FROM") {
            insert_line = i + 1;
        }
    }

    let import_text = format!("FROM {} IMPORT {};\n", module_name, name);

    let edit = Json::obj(vec![
        ("range", Json::obj(vec![
            ("start", Json::obj(vec![
                ("line", Json::int_val(insert_line as i64)),
                ("character", Json::int_val(0)),
            ])),
            ("end", Json::obj(vec![
                ("line", Json::int_val(insert_line as i64)),
                ("character", Json::int_val(0)),
            ])),
        ])),
        ("newText", Json::str_val(&import_text)),
    ]);

    Some(Json::obj(vec![
        ("title", Json::str_val(&format!("Import '{}' from {}", name, module_name))),
        ("kind", Json::str_val("quickfix")),
        ("edit", Json::obj(vec![
            ("changes", Json::obj(vec![
                (uri, Json::arr(vec![edit])),
            ])),
        ])),
    ]))
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_undefined_name() {
        assert_eq!(extract_undefined_name("undefined identifier 'Foo'"), Some("Foo".to_string()));
        assert_eq!(extract_undefined_name("undeclared identifier: Bar"), Some("Bar".to_string()));
        assert_eq!(extract_undefined_name("something else"), None);
        // Lowercase names should not match (not module-level symbols)
        assert_eq!(extract_undefined_name("undefined identifier 'x'"), None);
    }

    #[test]
    fn test_suggest_import_no_match() {
        let idx = super::super::index::WorkspaceIndex::new();
        let result = suggest_import("file:///test.mod", "MODULE Test;\nBEGIN\nEND Test.\n", "Unknown", &idx);
        assert!(result.is_none());
    }

    #[test]
    fn test_suggest_import_with_match() {
        let mut idx = super::super::index::WorkspaceIndex::new();
        // Manually push a symbol
        idx.inject_symbol_for_test("Push", super::super::index::SymbolKindTag::Procedure,
            "file:///Stack.def", 5, 1, Some("Stack"));

        let result = suggest_import(
            "file:///test.mod",
            "MODULE Test;\nBEGIN\n  Push(s, 1);\nEND Test.\n",
            "Push", &idx,
        );
        assert!(result.is_some());
        let action = result.unwrap();
        let title = action.get("title").and_then(|t| t.as_str()).unwrap();
        assert!(title.contains("Push"));
        assert!(title.contains("Stack"));
    }
}
