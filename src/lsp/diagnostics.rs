use crate::errors::{CompileError, ErrorKind};
use crate::json::Json;

/// Convert CompileErrors to LSP Diagnostic array.
/// Uses identifier-length spans and maps ErrorKind to LSP severity.
pub fn errors_to_diagnostics(errors: &[CompileError]) -> Vec<Json> {
    errors.iter().map(|e| {
        let line = if e.loc.line > 0 { e.loc.line - 1 } else { 0 };
        let col = if e.loc.col > 0 { e.loc.col - 1 } else { 0 };

        // Estimate end column from the error message (extract identifier if present)
        let end_col = col + estimate_token_length(&e.message);

        // Map ErrorKind to LSP DiagnosticSeverity
        let severity = match e.kind {
            ErrorKind::Warning => 2, // Warning
            ErrorKind::Lexer | ErrorKind::Parser | ErrorKind::Semantic
            | ErrorKind::CodeGen | ErrorKind::Driver => 1, // Error
        };

        // Include warning code (W01, W10, etc.) or error kind as LSP code
        let code = if let Some(wc) = &e.code {
            (*wc).to_string()
        } else {
            match e.kind {
                ErrorKind::Lexer => "lexer".to_string(),
                ErrorKind::Parser => "parser".to_string(),
                ErrorKind::Semantic => "semantic".to_string(),
                ErrorKind::CodeGen => "codegen".to_string(),
                ErrorKind::Driver => "driver".to_string(),
                ErrorKind::Warning => "warning".to_string(),
            }
        };

        Json::obj(vec![
            ("range", Json::obj(vec![
                ("start", Json::obj(vec![
                    ("line", Json::int_val(line as i64)),
                    ("character", Json::int_val(col as i64)),
                ])),
                ("end", Json::obj(vec![
                    ("line", Json::int_val(line as i64)),
                    ("character", Json::int_val(end_col as i64)),
                ])),
            ])),
            ("severity", Json::int_val(severity)),
            ("code", Json::str_val(&code)),
            ("source", Json::str_val(crate::identity::COMPILER_ID)),
            ("message", Json::str_val(&e.message)),
        ])
    }).collect()
}

/// Estimate the token length from an error message.
/// Extracts quoted identifiers like 'foo' or "bar" from the message.
fn estimate_token_length(message: &str) -> usize {
    // Look for 'identifier' patterns in the error message
    if let Some(start) = message.find('\'') {
        if let Some(end) = message[start + 1..].find('\'') {
            if end > 0 && end < 64 {
                return end;
            }
        }
    }
    // Default: highlight a reasonable span
    1
}

/// Build publishDiagnostics notification params.
pub fn publish_diagnostics(uri: &str, diagnostics: Vec<Json>) -> Json {
    Json::obj(vec![
        ("uri", Json::str_val(uri)),
        ("diagnostics", Json::arr(diagnostics)),
    ])
}
