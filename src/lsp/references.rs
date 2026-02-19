use crate::analyze::ReferenceIndex;
use crate::json::Json;

/// Handle textDocument/references request.
/// Uses the ReferenceIndex for semantic, identity-based reference finding.
pub fn references(
    uri: &str,
    line: usize,
    col: usize,
    ref_index: &ReferenceIndex,
) -> Option<Json> {
    // Find the reference at cursor position (0-based LSP coords)
    let target = ref_index.at_position(line, col)?;

    // Find all references to the same symbol (same def_scope + name)
    let all_refs = ref_index.find_all(target.def_scope, &target.name);

    let locations: Vec<Json> = all_refs
        .iter()
        .map(|r| {
            Json::obj(vec![
                ("uri", Json::str_val(uri)),
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
            ])
        })
        .collect();

    Some(Json::arr(locations))
}
