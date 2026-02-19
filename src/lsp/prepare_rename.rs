use crate::analyze::ReferenceIndex;
use crate::json::Json;

/// Handle textDocument/prepareRename request.
/// Returns the range and placeholder text if the cursor is on a renameable symbol.
pub fn prepare_rename(
    line: usize,
    col: usize,
    ref_index: &ReferenceIndex,
) -> Option<Json> {
    // Find the reference at cursor position (0-based LSP coords)
    let target = ref_index.at_position(line, col)?;

    // Return the range and placeholder text
    Some(Json::obj(vec![
        ("range", Json::obj(vec![
            ("start", Json::obj(vec![
                ("line", Json::int_val((target.line - 1) as i64)),
                ("character", Json::int_val((target.col - 1) as i64)),
            ])),
            ("end", Json::obj(vec![
                ("line", Json::int_val((target.line - 1) as i64)),
                ("character", Json::int_val((target.col - 1 + target.len) as i64)),
            ])),
        ])),
        ("placeholder", Json::str_val(&target.name)),
    ]))
}
