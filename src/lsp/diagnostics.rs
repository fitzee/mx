use crate::errors::CompileError;
use crate::json::Json;

/// Convert CompileErrors to LSP Diagnostic array.
pub fn errors_to_diagnostics(errors: &[CompileError]) -> Vec<Json> {
    errors.iter().map(|e| {
        let line = if e.loc.line > 0 { e.loc.line - 1 } else { 0 };
        let col = if e.loc.col > 0 { e.loc.col - 1 } else { 0 };
        Json::obj(vec![
            ("range", Json::obj(vec![
                ("start", Json::obj(vec![
                    ("line", Json::int_val(line as i64)),
                    ("character", Json::int_val(col as i64)),
                ])),
                ("end", Json::obj(vec![
                    ("line", Json::int_val(line as i64)),
                    ("character", Json::int_val((col + 1) as i64)),
                ])),
            ])),
            ("severity", Json::int_val(1)), // 1 = Error
            ("source", Json::str_val("m2c")),
            ("message", Json::str_val(&e.message)),
        ])
    }).collect()
}

/// Build publishDiagnostics notification params.
pub fn publish_diagnostics(uri: &str, diagnostics: Vec<Json>) -> Json {
    Json::obj(vec![
        ("uri", Json::str_val(uri)),
        ("diagnostics", Json::arr(diagnostics)),
    ])
}
