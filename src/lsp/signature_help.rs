use crate::analyze::{self, ScopeMap};
use crate::json::Json;
use crate::symtab::{SymbolTable, SymbolKind};
use crate::types::TypeRegistry;

/// Handle textDocument/signatureHelp request.
/// Supports multiline calls and nested calls.
/// Uses balanced parenthesis parsing with string/comment awareness.
pub fn signature_help(
    source: &str,
    line: usize,
    col: usize,
    symtab: &SymbolTable,
    types: &TypeRegistry,
    scope_map: &ScopeMap,
) -> Option<Json> {
    let lines: Vec<&str> = source.lines().collect();
    if line >= lines.len() {
        return None;
    }

    // Flatten all chars up to cursor position with (char, line, col) tracking.
    let mut all_chars: Vec<(char, usize, usize)> = Vec::new();
    for (li, line_text) in lines.iter().enumerate() {
        if li > line {
            break;
        }
        for (ci, ch) in line_text.chars().enumerate() {
            if li == line && ci >= col {
                break;
            }
            all_chars.push((ch, li, ci));
        }
    }

    if all_chars.is_empty() {
        return None;
    }

    // Forward pass: mark which char positions are code (not in strings or comments).
    let mut is_code = vec![true; all_chars.len()];
    let mut i = 0;
    while i < all_chars.len() {
        let ch = all_chars[i].0;
        if ch == '"' || ch == '\'' {
            let quote = ch;
            is_code[i] = false;
            i += 1;
            while i < all_chars.len() && all_chars[i].0 != quote {
                is_code[i] = false;
                i += 1;
            }
            if i < all_chars.len() {
                is_code[i] = false;
                i += 1;
            }
        } else if ch == '(' && i + 1 < all_chars.len() && all_chars[i + 1].0 == '*' {
            // Modula-2 nested comment (* ... *)
            let mut depth = 1;
            is_code[i] = false;
            is_code[i + 1] = false;
            i += 2;
            while i < all_chars.len() && depth > 0 {
                if all_chars[i].0 == '(' && i + 1 < all_chars.len() && all_chars[i + 1].0 == '*' {
                    depth += 1;
                    is_code[i] = false;
                    is_code[i + 1] = false;
                    i += 2;
                } else if all_chars[i].0 == '*' && i + 1 < all_chars.len() && all_chars[i + 1].0 == ')' {
                    depth -= 1;
                    is_code[i] = false;
                    is_code[i + 1] = false;
                    i += 2;
                } else {
                    is_code[i] = false;
                    i += 1;
                }
            }
        } else {
            i += 1;
        }
    }

    // Walk backwards through code chars to find the nearest unmatched '('.
    let mut paren_depth = 0i32;
    let mut comma_count = 0usize;
    let mut open_paren_idx = None;

    let mut pos = all_chars.len();
    while pos > 0 {
        pos -= 1;
        if !is_code[pos] {
            continue;
        }
        let ch = all_chars[pos].0;
        match ch {
            ')' => paren_depth += 1,
            '(' => {
                if paren_depth == 0 {
                    open_paren_idx = Some(pos);
                    break;
                }
                paren_depth -= 1;
            }
            ',' if paren_depth == 0 => comma_count += 1,
            _ => {}
        }
    }

    let paren_idx = open_paren_idx?;

    // Find the procedure name before the '(' — skip whitespace (same line only),
    // then collect identifier chars (same line only to avoid crossing line boundaries).
    let paren_line_no = all_chars[paren_idx].1;
    let mut end = paren_idx;
    while end > 0
        && all_chars[end - 1].1 == paren_line_no
        && all_chars[end - 1].0.is_ascii_whitespace()
    {
        end -= 1;
    }
    if end == 0 || all_chars[end - 1].1 != paren_line_no {
        return None;
    }
    let name_end = end;
    while end > 0
        && all_chars[end - 1].1 == paren_line_no
        && (all_chars[end - 1].0.is_ascii_alphanumeric() || all_chars[end - 1].0 == '_')
    {
        end -= 1;
    }
    let proc_name: String = all_chars[end..name_end].iter().map(|(ch, _, _)| ch).collect();
    if proc_name.is_empty() {
        return None;
    }

    // Look up the procedure in symtab using scope at the paren position.
    let (paren_line, paren_col) = (all_chars[paren_idx].1, all_chars[paren_idx].2);
    let scope_id = scope_map.scope_at(paren_line + 1, paren_col + 1);
    let sym = symtab.lookup_in_scope(scope_id, &proc_name)
        .or_else(|| symtab.lookup_all(&proc_name))?;

    let (params, return_type) = match &sym.kind {
        SymbolKind::Procedure { params, return_type, .. } => (params, return_type),
        _ => return None,
    };

    // Build signature label with parameter offset tracking.
    let mut label = format!("PROCEDURE {}(", proc_name);
    let mut param_labels = Vec::new();
    for (i, p) in params.iter().enumerate() {
        if i > 0 {
            label.push_str("; ");
        }
        let start = label.len();
        if p.is_var {
            label.push_str("VAR ");
        }
        label.push_str(&p.name);
        label.push_str(": ");
        label.push_str(&analyze::type_to_string(types, p.typ));
        let end = label.len();
        param_labels.push((start, end));
    }
    label.push(')');
    if let Some(rt) = return_type {
        label.push_str(": ");
        label.push_str(&analyze::type_to_string(types, *rt));
    }

    let parameters: Vec<Json> = param_labels.iter().map(|(start, end)| {
        Json::obj(vec![
            ("label", Json::arr(vec![
                Json::int_val(*start as i64),
                Json::int_val(*end as i64),
            ])),
        ])
    }).collect();

    let active_param = if comma_count < params.len() {
        comma_count
    } else {
        params.len().saturating_sub(1)
    };

    // Add documentation: doc comment > embedded docs > inline lang_docs
    let mut sig_fields = vec![
        ("label", Json::str_val(&label)),
        ("parameters", Json::arr(parameters)),
    ];
    if let Some(ref doc_comment) = sym.doc {
        sig_fields.push(("documentation", Json::obj(vec![
            ("kind", Json::str_val("markdown")),
            ("value", Json::str_val(doc_comment)),
        ])));
    } else if let Some(entry) = crate::lang_docs::get_doc(&proc_name) {
        sig_fields.push(("documentation", Json::obj(vec![
            ("kind", Json::str_val("markdown")),
            ("value", Json::str_val(&crate::lang_docs::format_hover(entry))),
        ])));
    } else if let Some(doc) = super::lang_docs::lookup(&proc_name) {
        let mut doc_text = doc.summary.to_string();
        if let Some(details) = doc.details {
            doc_text.push_str("\n\n");
            doc_text.push_str(details);
        }
        sig_fields.push(("documentation", Json::obj(vec![
            ("kind", Json::str_val("markdown")),
            ("value", Json::str_val(&doc_text)),
        ])));
    }

    let signature = Json::obj(sig_fields);

    Some(Json::obj(vec![
        ("signatures", Json::arr(vec![signature])),
        ("activeSignature", Json::int_val(0)),
        ("activeParameter", Json::int_val(active_param as i64)),
    ]))
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyze;

    #[test]
    fn test_multiline_signature_help() {
        // NOTE: avoid Rust \ line-continuation which eats leading whitespace.
        let source = "MODULE Test;\nPROCEDURE Foo(a: INTEGER; b: CARDINAL);\nBEGIN END Foo;\nBEGIN\n  Foo(\n    10,\n    20\n  );\nEND Test.\n";
        let result = analyze::analyze_source(source, "test.mod", false, &[]);
        // Line 5 = "    10," — cursor at col 5 (on '1')
        // Walking back: '(' is on line 4 col 5 but extraction needs name on same line.
        // '(' is at line 4 "  Foo(" col 5. Name extraction on same line: "Foo".
        let sig = signature_help(source, 5, 5, &result.symtab, &result.types, &result.scope_map);
        assert!(sig.is_some(), "expected signature help for first param");
        let sig = sig.unwrap();
        let active = sig.get("activeParameter").and_then(|p| p.as_i64()).unwrap();
        assert_eq!(active, 0);

        // Line 6 = "    20" — cursor at col 5 (on '2'), comma_count=1
        let sig2 = signature_help(source, 6, 5, &result.symtab, &result.types, &result.scope_map);
        assert!(sig2.is_some(), "expected signature help for second param");
        let sig2 = sig2.unwrap();
        let active2 = sig2.get("activeParameter").and_then(|p| p.as_i64()).unwrap();
        assert_eq!(active2, 1);
    }

    #[test]
    fn test_nested_signature_help() {
        let source = "MODULE Test;\nPROCEDURE Bar(x: INTEGER; y: INTEGER): INTEGER;\nBEGIN RETURN x + y END Bar;\nPROCEDURE Foo(a: INTEGER; b: INTEGER);\nBEGIN END Foo;\nBEGIN\n  Foo(Bar(1, 2), 3);\nEND Test.\n";
        // Line 6 = "  Foo(Bar(1, 2), 3);"
        // positions: 0=' ' 1=' ' 2='F' 3='o' 4='o' 5='(' 6='B' 7='a' 8='r' 9='(' 10='1' 11=',' 12=' ' 13='2' 14=')' 15=',' 16=' ' 17='3' 18=')' 19=';'
        let result = analyze::analyze_source(source, "test.mod", false, &[]);

        // Cursor at col 11: include chars 0..10. '(' at col 9 is innermost unmatched → Bar.
        let sig = signature_help(source, 6, 11, &result.symtab, &result.types, &result.scope_map);
        assert!(sig.is_some(), "expected Bar signature");
        let sig = sig.unwrap();
        let sigs = sig.get("signatures").and_then(|s| s.as_array()).unwrap();
        let label = sigs[0].get("label").and_then(|l| l.as_str()).unwrap();
        assert!(label.contains("Bar"), "expected Bar in label, got: {}", label);
        let active = sig.get("activeParameter").and_then(|p| p.as_i64()).unwrap();
        assert_eq!(active, 0); // first param of Bar

        // Cursor at col 14: include chars 0..13. Walking back: '2',' ',','(comma=1),'1','(' at 9 → Bar param 1.
        let sig2 = signature_help(source, 6, 14, &result.symtab, &result.types, &result.scope_map);
        assert!(sig2.is_some(), "expected Bar signature at second param");
        let sig2 = sig2.unwrap();
        let sigs2 = sig2.get("signatures").and_then(|s| s.as_array()).unwrap();
        let label2 = sigs2[0].get("label").and_then(|l| l.as_str()).unwrap();
        assert!(label2.contains("Bar"), "expected Bar, got: {}", label2);
        let active2 = sig2.get("activeParameter").and_then(|p| p.as_i64()).unwrap();
        assert_eq!(active2, 1); // second param of Bar

        // Cursor at col 18: Walking back: '3',' ',','(comma=1), ')'(depth=1), '2',' ',','(skip), '1', '('(depth→0),
        // 'r','a','B', '(' at 5 → Foo, comma=1 → param 1.
        let sig3 = signature_help(source, 6, 18, &result.symtab, &result.types, &result.scope_map);
        assert!(sig3.is_some(), "expected Foo signature at second param");
        let sig3 = sig3.unwrap();
        let sigs3 = sig3.get("signatures").and_then(|s| s.as_array()).unwrap();
        let label3 = sigs3[0].get("label").and_then(|l| l.as_str()).unwrap();
        assert!(label3.contains("Foo"), "expected Foo, got: {}", label3);
        let active3 = sig3.get("activeParameter").and_then(|p| p.as_i64()).unwrap();
        assert_eq!(active3, 1); // second param of Foo
    }

    #[test]
    fn test_signature_help_single_line() {
        let source = "MODULE Test;\nPROCEDURE Add(a: INTEGER; b: INTEGER): INTEGER;\nBEGIN RETURN a + b END Add;\nBEGIN\n  Add(1, 2);\nEND Test.\n";
        let result = analyze::analyze_source(source, "test.mod", false, &[]);
        // Line 4 = "  Add(1, 2);"
        // col 7 → include chars 0..6 = "  Add(1". '(' at col 5. Name = "Add".
        let sig = signature_help(source, 4, 7, &result.symtab, &result.types, &result.scope_map);
        assert!(sig.is_some(), "expected signature help");
        let sig = sig.unwrap();
        let active = sig.get("activeParameter").and_then(|p| p.as_i64()).unwrap();
        assert_eq!(active, 0);

        // col 10 → include chars 0..9 = "  Add(1, ". Walking back: ' ',','(comma=1),'1','(' → Add param 1.
        let sig2 = signature_help(source, 4, 10, &result.symtab, &result.types, &result.scope_map);
        assert!(sig2.is_some());
        let sig2 = sig2.unwrap();
        let active2 = sig2.get("activeParameter").and_then(|p| p.as_i64()).unwrap();
        assert_eq!(active2, 1);
    }

    #[test]
    fn test_signature_help_builtin_doc() {
        // INC is a builtin procedure — should get lang_docs documentation
        let source = "MODULE Test;\nVAR n: INTEGER;\nBEGIN\n  INC(n);\nEND Test.\n";
        let result = analyze::analyze_source(source, "test.mod", false, &[]);
        // Line 3 = "  INC(n);" — cursor at col 6 (on 'n')
        let sig = signature_help(source, 3, 6, &result.symtab, &result.types, &result.scope_map);
        assert!(sig.is_some(), "expected signature help for INC");
        let sig = sig.unwrap();
        let sigs = sig.get("signatures").and_then(|s| s.as_array()).unwrap();
        assert!(!sigs.is_empty());
        // Should have documentation from lang_docs
        let doc = sigs[0].get("documentation");
        assert!(doc.is_some(), "expected documentation for builtin INC");
        let doc_value = doc.unwrap().get("value").and_then(|v| v.as_str()).unwrap();
        assert!(doc_value.contains("Increment"), "expected increment description, got: {}", doc_value);
    }
}
