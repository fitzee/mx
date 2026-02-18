use crate::json::Json;
use crate::symtab::SymbolTable;
use super::hover::word_at_position;

/// Handle textDocument/rename request. Single-file rename.
pub fn rename(
    source: &str,
    uri: &str,
    line: usize,
    col: usize,
    new_name: &str,
    symtab: &SymbolTable,
) -> Option<Json> {
    let old_name = word_at_position(source, line, col)?;

    // Verify the name exists in the symbol table
    symtab.lookup_all(&old_name)?;

    // Find all occurrences of old_name in source (whole-word only)
    let mut edits = Vec::new();
    for (line_idx, line_text) in source.lines().enumerate() {
        let chars: Vec<char> = line_text.chars().collect();
        let mut pos = 0;
        while pos < chars.len() {
            if let Some(idx) = line_text[pos..].find(&old_name) {
                let abs_pos = pos + idx;
                // Check whole-word boundary
                let before_ok = abs_pos == 0
                    || !(chars[abs_pos - 1].is_ascii_alphanumeric() || chars[abs_pos - 1] == '_');
                let after_pos = abs_pos + old_name.len();
                let after_ok = after_pos >= chars.len()
                    || !(chars[after_pos].is_ascii_alphanumeric() || chars[after_pos] == '_');

                if before_ok && after_ok {
                    edits.push(Json::obj(vec![
                        ("range", Json::obj(vec![
                            ("start", Json::obj(vec![
                                ("line", Json::int_val(line_idx as i64)),
                                ("character", Json::int_val(abs_pos as i64)),
                            ])),
                            ("end", Json::obj(vec![
                                ("line", Json::int_val(line_idx as i64)),
                                ("character", Json::int_val(after_pos as i64)),
                            ])),
                        ])),
                        ("newText", Json::str_val(new_name)),
                    ]));
                }
                pos = after_pos;
            } else {
                break;
            }
        }
    }

    if edits.is_empty() {
        return None;
    }

    // WorkspaceEdit
    Some(Json::obj(vec![
        ("changes", Json::obj(vec![
            (uri, Json::arr(edits)),
        ])),
    ]))
}
