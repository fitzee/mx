use crate::json::Json;
use crate::symtab::{SymbolTable, SymbolKind, ConstValue};

/// Handle textDocument/hover request.
/// Finds the token at the given position and looks it up in the symbol table.
pub fn hover(source: &str, line: usize, col: usize, symtab: &SymbolTable) -> Option<Json> {
    let word = word_at_position(source, line, col)?;
    let sym = symtab.lookup_all(&word)?;

    let typ_info = match &sym.kind {
        SymbolKind::Variable => format!("VAR {}: ...", sym.name),
        SymbolKind::Constant(cv) => {
            let val = match cv {
                ConstValue::Integer(n) => format!("{}", n),
                ConstValue::Real(r) => format!("{}", r),
                ConstValue::Boolean(b) => if *b { "TRUE".to_string() } else { "FALSE".to_string() },
                ConstValue::Char(c) => format!("'{}'", c),
                ConstValue::String(s) => format!("\"{}\"", s),
                ConstValue::Set(_) => "SET".to_string(),
                ConstValue::Nil => "NIL".to_string(),
            };
            format!("CONST {} = {}", sym.name, val)
        }
        SymbolKind::Type => format!("TYPE {}", sym.name),
        SymbolKind::Procedure { params, return_type, .. } => {
            let mut sig = format!("PROCEDURE {}(", sym.name);
            for (i, p) in params.iter().enumerate() {
                if i > 0 { sig.push_str("; "); }
                if p.is_var { sig.push_str("VAR "); }
                sig.push_str(&p.name);
            }
            sig.push(')');
            if return_type.is_some() {
                sig.push_str(": ...");
            }
            sig
        }
        SymbolKind::Module { .. } => format!("MODULE {}", sym.name),
        SymbolKind::Field => format!("field {}", sym.name),
        SymbolKind::EnumVariant(v) => format!("{} = {}", sym.name, v),
    };

    let hover_line = if line > 0 { line - 1 } else { 0 };
    let hover_col = if col > 0 { col - 1 } else { 0 };

    Some(Json::obj(vec![
        ("contents", Json::obj(vec![
            ("kind", Json::str_val("markdown")),
            ("value", Json::str_val(&format!("```modula2\n{}\n```", typ_info))),
        ])),
    ]))
}

/// Extract the word at a given (1-based) line and column in the source text.
pub fn word_at_position(source: &str, line: usize, col: usize) -> Option<String> {
    let lines: Vec<&str> = source.lines().collect();
    // LSP positions are 0-based, but our SourceLoc is 1-based
    let line_idx = line;
    if line_idx >= lines.len() { return None; }
    let line_text = lines[line_idx];
    let col_idx = col;
    if col_idx >= line_text.len() { return None; }

    let chars: Vec<char> = line_text.chars().collect();
    if !chars[col_idx].is_ascii_alphanumeric() && chars[col_idx] != '_' {
        return None;
    }

    // Find word boundaries
    let mut start = col_idx;
    while start > 0 && (chars[start - 1].is_ascii_alphanumeric() || chars[start - 1] == '_') {
        start -= 1;
    }
    let mut end = col_idx;
    while end < chars.len() && (chars[end].is_ascii_alphanumeric() || chars[end] == '_') {
        end += 1;
    }

    Some(chars[start..end].iter().collect())
}
