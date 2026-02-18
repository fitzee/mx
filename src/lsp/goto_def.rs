use std::path::{Path, PathBuf};
use crate::json::Json;
use crate::symtab::{SymbolTable, SymbolKind};
use super::hover::word_at_position;

/// Handle textDocument/definition request.
pub fn goto_definition(
    source: &str,
    uri: &str,
    line: usize,
    col: usize,
    symtab: &SymbolTable,
    include_paths: &[PathBuf],
) -> Option<Json> {
    let word = word_at_position(source, line, col)?;

    // Check if it's a module name — try to find its .def file
    if let Some(sym) = symtab.lookup_all(&word) {
        if let SymbolKind::Module { .. } = &sym.kind {
            // Try to find the definition file for this module
            let file_path = super::documents::uri_to_path(uri);
            let input_path = Path::new(&file_path);
            if let Some(def_path) = find_def_file(&word, input_path, include_paths) {
                let def_uri = super::documents::path_to_uri(&def_path.to_string_lossy());
                return Some(Json::obj(vec![
                    ("uri", Json::str_val(&def_uri)),
                    ("range", make_range(0, 0, 0, 0)),
                ]));
            }
        }

        // For symbols with a known module, try to find the source location
        if let Some(ref module) = sym.module {
            let file_path = super::documents::uri_to_path(uri);
            let input_path = Path::new(&file_path);
            if let Some(def_path) = find_def_file(module, input_path, include_paths) {
                // Search the def file for the symbol name
                if let Ok(def_source) = std::fs::read_to_string(&def_path) {
                    if let Some((def_line, def_col)) = find_name_in_source(&def_source, &sym.name) {
                        let def_uri = super::documents::path_to_uri(&def_path.to_string_lossy());
                        return Some(Json::obj(vec![
                            ("uri", Json::str_val(&def_uri)),
                            ("range", make_range(def_line, def_col, def_line, def_col + sym.name.len())),
                        ]));
                    }
                }
            }
        }

        // Same-file: search for the declaration in the current source
        if let Some((def_line, def_col)) = find_declaration_in_source(source, &word) {
            return Some(Json::obj(vec![
                ("uri", Json::str_val(uri)),
                ("range", make_range(def_line, def_col, def_line, def_col + word.len())),
            ]));
        }
    }

    None
}

fn find_def_file(module_name: &str, input_path: &Path, include_paths: &[PathBuf]) -> Option<PathBuf> {
    let dir = input_path.parent().unwrap_or(Path::new("."));
    for ext in &["def", "DEF"] {
        let p = dir.join(format!("{}.{}", module_name, ext));
        if p.exists() { return Some(p); }
    }
    for inc_dir in include_paths {
        for ext in &["def", "DEF"] {
            let p = inc_dir.join(format!("{}.{}", module_name, ext));
            if p.exists() { return Some(p); }
        }
    }
    None
}

fn find_name_in_source(source: &str, name: &str) -> Option<(usize, usize)> {
    for (line_idx, line) in source.lines().enumerate() {
        if let Some(col) = line.find(name) {
            // Verify it's a whole word
            let before_ok = col == 0 || !line.as_bytes()[col - 1].is_ascii_alphanumeric();
            let after_ok = col + name.len() >= line.len()
                || !line.as_bytes()[col + name.len()].is_ascii_alphanumeric();
            if before_ok && after_ok {
                return Some((line_idx, col));
            }
        }
    }
    None
}

fn find_declaration_in_source(source: &str, name: &str) -> Option<(usize, usize)> {
    // Look for PROCEDURE name, VAR/CONST/TYPE followed by name
    let keywords = ["PROCEDURE ", "TYPE ", "CONST ", "VAR "];
    for (line_idx, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        for kw in &keywords {
            if trimmed.starts_with(kw) {
                let rest = &trimmed[kw.len()..];
                if rest.starts_with(name) {
                    let after = rest.get(name.len()..name.len() + 1).unwrap_or("");
                    if after == "" || after == "(" || after == ";" || after == " " || after == ":" || after == "=" {
                        if let Some(col) = line.find(name) {
                            return Some((line_idx, col));
                        }
                    }
                }
            }
        }
    }
    None
}

fn make_range(sl: usize, sc: usize, el: usize, ec: usize) -> Json {
    Json::obj(vec![
        ("start", Json::obj(vec![
            ("line", Json::int_val(sl as i64)),
            ("character", Json::int_val(sc as i64)),
        ])),
        ("end", Json::obj(vec![
            ("line", Json::int_val(el as i64)),
            ("character", Json::int_val(ec as i64)),
        ])),
    ])
}
