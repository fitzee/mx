use std::io::{self, BufRead, Read, Write};
use crate::json::Json;

/// Read a JSON-RPC message from stdin using Content-Length framing.
pub fn read_message() -> Option<Json> {
    let stdin = io::stdin();
    let mut reader = stdin.lock();

    // Read headers
    let mut content_length: usize = 0;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).ok()? == 0 {
            return None; // EOF
        }
        let header = header.trim_end();
        if header.is_empty() {
            break; // End of headers
        }
        if let Some(len_str) = header.strip_prefix("Content-Length: ") {
            content_length = len_str.parse().ok()?;
        }
    }

    if content_length == 0 {
        return None;
    }

    // Read body
    let mut body = vec![0u8; content_length];
    reader.read_exact(&mut body).ok()?;
    let body_str = String::from_utf8(body).ok()?;

    Json::parse(&body_str).ok()
}

/// Write a JSON-RPC message to stdout with Content-Length framing.
pub fn write_message(msg: &Json) {
    let body = msg.serialize();
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let _ = out.write_all(header.as_bytes());
    let _ = out.write_all(body.as_bytes());
    let _ = out.flush();
}

/// Send a JSON-RPC response.
pub fn send_response(id: &Json, result: Json) {
    let msg = Json::obj(vec![
        ("jsonrpc", Json::str_val("2.0")),
        ("id", id.clone()),
        ("result", result),
    ]);
    write_message(&msg);
}

/// Send a JSON-RPC error response.
pub fn send_error(id: &Json, code: i64, message: &str) {
    let msg = Json::obj(vec![
        ("jsonrpc", Json::str_val("2.0")),
        ("id", id.clone()),
        ("error", Json::obj(vec![
            ("code", Json::int_val(code)),
            ("message", Json::str_val(message)),
        ])),
    ]);
    write_message(&msg);
}

/// Send a JSON-RPC notification (no id).
pub fn send_notification(method: &str, params: Json) {
    let msg = Json::obj(vec![
        ("jsonrpc", Json::str_val("2.0")),
        ("method", Json::str_val(method)),
        ("params", params),
    ]);
    write_message(&msg);
}

/// Send a JSON-RPC request from server to client (e.g. window/workDoneProgress/create).
/// Returns the request id used.
pub fn send_request(id: i64, method: &str, params: Json) {
    let msg = Json::obj(vec![
        ("jsonrpc", Json::str_val("2.0")),
        ("id", Json::int_val(id)),
        ("method", Json::str_val(method)),
        ("params", params),
    ]);
    write_message(&msg);
}
