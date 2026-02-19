use crate::analyze::ReferenceIndex;
use crate::json::Json;

/// Handle textDocument/documentHighlight request.
/// Returns all references to the symbol at cursor within the current file.
pub fn document_highlight(
    line: usize,
    col: usize,
    ref_index: &ReferenceIndex,
) -> Option<Json> {
    let target = ref_index.at_position(line, col)?;
    let all_refs = ref_index.find_all(target.def_scope, &target.name);

    let highlights: Vec<Json> = all_refs
        .iter()
        .map(|r| {
            // kind: 1 = Text (default), 2 = Read, 3 = Write
            // Use Write for definition sites, Read for use sites
            let kind = if r.is_definition { 3 } else { 2 };
            Json::obj(vec![
                ("range", Json::obj(vec![
                    ("start", Json::obj(vec![
                        ("line", Json::int_val((r.line - 1) as i64)),
                        ("character", Json::int_val((r.col - 1) as i64)),
                    ])),
                    ("end", Json::obj(vec![
                        ("line", Json::int_val((r.line - 1) as i64)),
                        ("character", Json::int_val((r.col - 1 + r.len) as i64)),
                    ])),
                ])),
                ("kind", Json::int_val(kind)),
            ])
        })
        .collect();

    if highlights.is_empty() {
        None
    } else {
        Some(Json::arr(highlights))
    }
}
