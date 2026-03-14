use crate::analyze::AnalysisResult;
use crate::json::Json;
use crate::lexer::Lexer;
use crate::symtab::SymbolKind;
use crate::token::TokenKind;

// LSP semantic token type indices (must match legend in initialize).
const TT_KEYWORD: u32 = 0;
const TT_TYPE: u32 = 1;
const TT_FUNCTION: u32 = 2;
const TT_VARIABLE: u32 = 3;
const TT_PARAMETER: u32 = 4;
const TT_PROPERTY: u32 = 5;
const TT_NAMESPACE: u32 = 6;
const TT_ENUM_MEMBER: u32 = 7;
const TT_NUMBER: u32 = 8;
const TT_STRING: u32 = 9;

/// Token legend for the LSP semanticTokensProvider capability.
pub fn token_types_legend() -> Vec<Json> {
    [
        "keyword", "type", "function", "variable", "parameter",
        "property", "namespace", "enumMember", "number", "string",
    ].iter().map(|s| Json::str_val(s)).collect()
}

/// Collect semantic tokens for a source file, delta-encoded per LSP spec.
/// Returns the `data` array (flat Vec of u32 quintuplets).
pub fn collect_semantic_tokens(
    source: &str,
    filename: &str,
    m2plus: bool,
    result: &AnalysisResult,
) -> Vec<u32> {
    // Re-lex to get tokens with exact positions.
    let mut lexer = Lexer::new(source, filename);
    lexer.set_m2plus(m2plus);
    let tokens = match lexer.tokenize() {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };

    // Collect raw tokens: (line_1based, col_1based, length, token_type, modifiers)
    let mut raw: Vec<(u32, u32, u32, u32, u32)> = Vec::new();

    for tok in &tokens {
        let line = tok.loc.line as u32;
        let col = tok.loc.col as u32;

        match &tok.kind {
            // Keywords
            TokenKind::And | TokenKind::Array | TokenKind::Begin |
            TokenKind::By | TokenKind::Case | TokenKind::Const |
            TokenKind::Definition | TokenKind::Div | TokenKind::Do |
            TokenKind::Else | TokenKind::Elsif | TokenKind::End |
            TokenKind::Except | TokenKind::Exit | TokenKind::Export |
            TokenKind::Finally | TokenKind::For | TokenKind::From |
            TokenKind::If | TokenKind::Implementation | TokenKind::Import |
            TokenKind::In | TokenKind::Loop | TokenKind::Mod |
            TokenKind::Module | TokenKind::Not | TokenKind::Of |
            TokenKind::Or | TokenKind::Pointer | TokenKind::Procedure |
            TokenKind::Qualified | TokenKind::Raise | TokenKind::Record |
            TokenKind::Repeat | TokenKind::Retry | TokenKind::Return |
            TokenKind::Set | TokenKind::Then | TokenKind::To |
            TokenKind::Type | TokenKind::Until | TokenKind::Var |
            TokenKind::While | TokenKind::With |
            // M2+ keywords
            TokenKind::Branded | TokenKind::Exception | TokenKind::Lock |
            TokenKind::Methods | TokenKind::Object | TokenKind::Override |
            TokenKind::Ref | TokenKind::Refany | TokenKind::Reveal |
            TokenKind::Safe | TokenKind::Try | TokenKind::Typecase |
            TokenKind::Unsafe => {
                let len = keyword_len(&tok.kind);
                raw.push((line, col, len, TT_KEYWORD, 0));
            }

            // Numbers
            TokenKind::IntLit(_) | TokenKind::RealLit(_) => {
                let len = token_source_len(source, line as usize, col as usize);
                raw.push((line, col, len, TT_NUMBER, 0));
            }

            // Strings
            TokenKind::StringLit(s) => {
                // +2 for surrounding quotes
                let len = s.len() as u32 + 2;
                raw.push((line, col, len, TT_STRING, 0));
            }
            TokenKind::CharLit(_) => {
                let len = token_source_len(source, line as usize, col as usize);
                raw.push((line, col, len, TT_STRING, 0));
            }

            // Identifiers — classify using ref_index + symtab
            TokenKind::Ident(name) => {
                let tt = classify_ident(name, line as usize, col as usize, result);
                raw.push((line, col, name.len() as u32, tt, 0));
            }

            _ => {} // operators, punctuation — no semantic tokens
        }
    }

    // Delta-encode: convert absolute (line, col) to deltas.
    let mut data = Vec::with_capacity(raw.len() * 5);
    let mut prev_line: u32 = 0;
    let mut prev_col: u32 = 0;

    for (line, col, len, tt, mods) in &raw {
        // Convert 1-based to 0-based for LSP
        let lsp_line = line.saturating_sub(1);
        let lsp_col = col.saturating_sub(1);
        let delta_line = lsp_line - prev_line;
        let delta_col = if delta_line == 0 { lsp_col - prev_col } else { lsp_col };
        data.push(delta_line);
        data.push(delta_col);
        data.push(*len);
        data.push(*tt);
        data.push(*mods);
        prev_line = lsp_line;
        prev_col = lsp_col;
    }

    data
}

/// Build the LSP response for textDocument/semanticTokens/full.
pub fn semantic_tokens_response(data: Vec<u32>) -> Json {
    Json::obj(vec![
        ("data", Json::arr(data.iter().map(|n| Json::int_val(*n as i64)).collect())),
    ])
}

// ── Helpers ─────────────────────────────────────────────────────────

fn classify_ident(name: &str, line_1: usize, col_1: usize, result: &AnalysisResult) -> u32 {
    // ref_index uses 0-based LSP coordinates
    let lsp_line = line_1.saturating_sub(1);
    let lsp_col = col_1.saturating_sub(1);

    let debug = name.contains("skip");

    // Primary path: use ref_index for exact position lookup
    if let Some(reference) = result.ref_index.at_position(lsp_line, lsp_col) {
        if debug {
            eprintln!("[semtok] {} @{}:{} → ref_index hit: def_scope={} name={}",
                name, line_1, col_1, reference.def_scope, reference.name);
        }
        if let Some(sym) = result.symtab.lookup_in_scope(reference.def_scope, &reference.name) {
            let tt = symbol_to_token_type(sym, &result.symtab, reference.def_scope, &reference.name);
            if debug {
                eprintln!("[semtok] {} → kind={:?} tt={}", name, sym.kind, tt);
            }
            return tt;
        }
    } else if debug {
        eprintln!("[semtok] {} @{}:{} → ref_index MISS (lsp {}:{})",
            name, line_1, col_1, lsp_line, lsp_col);
    }

    // Fallback: scope-aware lookup (handles imports and other identifiers
    // not in ref_index, same approach as hover/completion)
    let scope_id = result.scope_map.scope_at(line_1, col_1);
    if let Some(sym) = result.symtab.lookup_in_scope(scope_id, name) {
        let tt = symbol_to_token_type(sym, &result.symtab, scope_id, name);
        if debug {
            eprintln!("[semtok] {} @{}:{} → fallback scope={} kind={:?} tt={}",
                name, line_1, col_1, scope_id, sym.kind, tt);
        }
        return tt;
    }

    // Last resort: check if it's a well-known builtin type
    if is_builtin_type(name) {
        return TT_TYPE;
    }

    if debug {
        eprintln!("[semtok] {} @{}:{} → NOT FOUND, defaulting to TT_VARIABLE", name, line_1, col_1);
    }
    TT_VARIABLE // unknown identifiers default to variable
}

fn symbol_to_token_type(
    sym: &crate::symtab::Symbol,
    symtab: &crate::symtab::SymbolTable,
    def_scope: usize,
    name: &str,
) -> u32 {
    match &sym.kind {
        SymbolKind::Procedure { .. } => TT_FUNCTION,
        SymbolKind::Type => TT_TYPE,
        SymbolKind::Variable => {
            if is_parameter(symtab, def_scope, name) {
                TT_PARAMETER
            } else {
                TT_VARIABLE
            }
        }
        SymbolKind::Constant(_) => TT_VARIABLE,
        SymbolKind::Module { .. } => TT_NAMESPACE,
        SymbolKind::Field => TT_PROPERTY,
        SymbolKind::EnumVariant(_) => TT_ENUM_MEMBER,
    }
}

fn is_parameter(symtab: &crate::symtab::SymbolTable, def_scope: usize, name: &str) -> bool {
    // A parameter is a Variable defined in a procedure's body scope.
    // The body scope is named after its procedure, so find the matching
    // procedure in the parent scope and check its param list.
    let scope_name = match symtab.scope_name(def_scope) {
        Some(n) => n,
        None => return false,
    };
    if let Some(parent_id) = symtab.scope_parent(def_scope) {
        if let Some(sym) = symtab.lookup_in_scope_direct(parent_id, scope_name) {
            if let SymbolKind::Procedure { params, .. } = &sym.kind {
                return params.iter().any(|p| p.name == name);
            }
        }
    }
    false
}

fn is_builtin_type(name: &str) -> bool {
    matches!(name,
        "INTEGER" | "CARDINAL" | "REAL" | "LONGREAL" | "BOOLEAN" |
        "CHAR" | "BITSET" | "PROC" | "WORD" | "BYTE" | "ADDRESS" |
        "LONGINT" | "LONGCARD" | "COMPLEX" | "LONGCOMPLEX"
    )
}

fn keyword_len(kind: &TokenKind) -> u32 {
    match kind {
        TokenKind::And => 3,
        TokenKind::Array => 5,
        TokenKind::Begin => 5,
        TokenKind::By => 2,
        TokenKind::Case => 4,
        TokenKind::Const => 5,
        TokenKind::Definition => 10,
        TokenKind::Div => 3,
        TokenKind::Do => 2,
        TokenKind::Else => 4,
        TokenKind::Elsif => 5,
        TokenKind::End => 3,
        TokenKind::Except => 6,
        TokenKind::Exit => 4,
        TokenKind::Export => 6,
        TokenKind::Finally => 7,
        TokenKind::For => 3,
        TokenKind::From => 4,
        TokenKind::If => 2,
        TokenKind::Implementation => 14,
        TokenKind::Import => 6,
        TokenKind::In => 2,
        TokenKind::Loop => 4,
        TokenKind::Mod => 3,
        TokenKind::Module => 6,
        TokenKind::Not => 3,
        TokenKind::Of => 2,
        TokenKind::Or => 2,
        TokenKind::Pointer => 7,
        TokenKind::Procedure => 9,
        TokenKind::Qualified => 9,
        TokenKind::Raise => 5,
        TokenKind::Record => 6,
        TokenKind::Repeat => 6,
        TokenKind::Retry => 5,
        TokenKind::Return => 6,
        TokenKind::Set => 3,
        TokenKind::Then => 4,
        TokenKind::To => 2,
        TokenKind::Type => 4,
        TokenKind::Until => 5,
        TokenKind::Var => 3,
        TokenKind::While => 5,
        TokenKind::With => 4,
        TokenKind::Branded => 7,
        TokenKind::Exception => 9,
        TokenKind::Lock => 4,
        TokenKind::Methods => 7,
        TokenKind::Object => 6,
        TokenKind::Override => 8,
        TokenKind::Ref => 3,
        TokenKind::Refany => 6,
        TokenKind::Reveal => 6,
        TokenKind::Safe => 4,
        TokenKind::Try => 3,
        TokenKind::Typecase => 8,
        TokenKind::Unsafe => 6,
        _ => 0,
    }
}

/// Estimate the source-level length of a token at (1-based line, 1-based col).
fn token_source_len(source: &str, line_1: usize, col_1: usize) -> u32 {
    let lines: Vec<&str> = source.lines().collect();
    if line_1 == 0 || line_1 > lines.len() {
        return 1;
    }
    let line_text = lines[line_1 - 1];
    let start = if col_1 > 0 { col_1 - 1 } else { 0 };
    let chars: Vec<char> = line_text.chars().collect();
    let mut end = start;
    while end < chars.len() && (chars[end].is_ascii_alphanumeric() || chars[end] == '_' || chars[end] == '.' || chars[end] == 'H' || chars[end] == 'C' || chars[end] == 'B') {
        end += 1;
    }
    let len = end - start;
    if len == 0 { 1 } else { len as u32 }
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyze;

    #[test]
    fn test_semantic_token_classification() {
        let source = "MODULE Test;\nVAR x: INTEGER;\nPROCEDURE Foo(a: INTEGER);\nBEGIN\n  x := a\nEND Foo;\nBEGIN\nEND Test.\n";
        let result = analyze::analyze_source(source, "test.mod", false, &[]);
        let data = collect_semantic_tokens(source, "test.mod", false, &result);

        // Verify we got some tokens
        assert!(!data.is_empty(), "expected non-empty semantic tokens");
        assert_eq!(data.len() % 5, 0, "data should be multiple of 5");

        // Decode first token: should be "MODULE" keyword at line 0, col 0
        assert_eq!(data[0], 0); // delta_line
        assert_eq!(data[1], 0); // delta_col
        assert_eq!(data[2], 6); // length of "MODULE"
        assert_eq!(data[3], TT_KEYWORD); // type: keyword
    }

    #[test]
    fn test_semantic_tokens_nested_scopes() {
        let source = "MODULE Test;\nPROCEDURE Outer;\n  VAR a: INTEGER;\n  PROCEDURE Inner;\n    VAR b: INTEGER;\n  BEGIN\n    b := 1\n  END Inner;\nBEGIN\n  a := 2\nEND Outer;\nBEGIN\nEND Test.\n";
        let result = analyze::analyze_source(source, "test.mod", false, &[]);
        let data = collect_semantic_tokens(source, "test.mod", false, &result);
        assert!(!data.is_empty());
        assert_eq!(data.len() % 5, 0);
    }

    #[test]
    fn test_semantic_tokens_malformed() {
        let source = "MODULE Broken;\nVAR x: ;\nBEGIN\nEND Broken.\n";
        let result = analyze::analyze_source(source, "broken.mod", false, &[]);
        // Should not crash, may produce partial tokens
        let data = collect_semantic_tokens(source, "broken.mod", false, &result);
        assert_eq!(data.len() % 5, 0);
    }

    #[test]
    fn test_semantic_tokens_parameter_vs_variable() {
        let source = "MODULE Test;\nVAR g: INTEGER;\nPROCEDURE Foo(p: INTEGER);\nBEGIN\n  g := p\nEND Foo;\nBEGIN\nEND Test.\n";
        let result = analyze::analyze_source(source, "test.mod", false, &[]);
        let data = collect_semantic_tokens(source, "test.mod", false, &result);

        // Find tokens — decode to absolute positions for analysis
        let mut tokens_abs: Vec<(u32, u32, u32, u32)> = Vec::new(); // (line, col, len, type)
        let mut pl = 0u32;
        let mut pc = 0u32;
        let mut i = 0;
        while i + 4 < data.len() {
            let dl = data[i]; let dc = data[i+1]; let len = data[i+2]; let tt = data[i+3];
            let line = pl + dl;
            let col = if dl == 0 { pc + dc } else { dc };
            tokens_abs.push((line, col, len, tt));
            pl = line; pc = col;
            i += 5;
        }

        // "p" on line 4 (0-based) col 7 should be TT_PARAMETER
        // "g" on line 4 col 2 should be TT_VARIABLE
        let p_token = tokens_abs.iter().find(|t| t.0 == 4 && t.2 == 1 && t.1 > 4);
        let g_token = tokens_abs.iter().find(|t| t.0 == 4 && t.2 == 1 && t.1 < 4);

        if let Some(pt) = p_token {
            assert_eq!(pt.3, TT_PARAMETER, "p should be parameter, got {}", pt.3);
        }
        if let Some(gt) = g_token {
            assert_eq!(gt.3, TT_VARIABLE, "g should be variable, got {}", gt.3);
        }
    }

    #[test]
    fn test_semantic_tokens_multi_var_declaration() {
        // Multi-var: a, b: INTEGER should both be TT_VARIABLE
        let source = "MODULE Test;\nVAR\n  a, b: INTEGER;\nBEGIN\nEND Test.\n";
        let result = analyze::analyze_source(source, "test.mod", false, &[]);
        let data = collect_semantic_tokens(source, "test.mod", false, &result);

        // Decode to absolute positions
        let mut tokens_abs: Vec<(u32, u32, u32, u32, String)> = Vec::new();
        let mut pl = 0u32;
        let mut pc = 0u32;
        let mut i = 0;
        // Also re-lex to get token text
        let mut lexer = crate::lexer::Lexer::new(source, "test.mod");
        let all_tokens = lexer.tokenize().unwrap();
        let _tok_idx = 0;
        while i + 4 < data.len() {
            let dl = data[i]; let dc = data[i+1]; let len = data[i+2]; let tt = data[i+3];
            let line = pl + dl;
            let col = if dl == 0 { pc + dc } else { dc };
            // Find token name from source
            let name = source.lines().nth(line as usize)
                .map(|l| &l[col as usize..(col as usize + len as usize).min(l.len())])
                .unwrap_or("?");
            tokens_abs.push((line, col, len, tt, name.to_string()));
            pl = line; pc = col;
            i += 5;
        }

        // Find "a" and "b" tokens on line 2 (0-based)
        let a_token = tokens_abs.iter().find(|t| t.4 == "a" && t.0 == 2);
        let b_token = tokens_abs.iter().find(|t| t.4 == "b" && t.0 == 2);

        assert!(a_token.is_some(), "token 'a' not found; tokens: {:?}", tokens_abs);
        assert!(b_token.is_some(), "token 'b' not found; tokens: {:?}", tokens_abs);
        assert_eq!(a_token.unwrap().3, TT_VARIABLE,
            "'a' should be TT_VARIABLE({}), got {}. All tokens: {:?}",
            TT_VARIABLE, a_token.unwrap().3, tokens_abs);
        assert_eq!(b_token.unwrap().3, TT_VARIABLE,
            "'b' should be TT_VARIABLE({}), got {}. All tokens: {:?}",
            TT_VARIABLE, b_token.unwrap().3, tokens_abs);
    }

    #[test]
    fn test_semantic_tokens_impl_module_multi_var() {
        // Replicate exact scenario: IMPLEMENTATION MODULE with module-level multi-var
        let source = "IMPLEMENTATION MODULE Scan;\n\nVAR\n  storeEntries: BOOLEAN;\n  skipVendored, skipGenerated: BOOLEAN;\n\nPROCEDURE Foo;\nBEGIN\n  skipVendored := TRUE;\n  skipGenerated := FALSE\nEND Foo;\n\nBEGIN\nEND Scan.\n";
        let result = analyze::analyze_source(source, "Scan.mod", false, &[]);
        let data = collect_semantic_tokens(source, "Scan.mod", false, &result);

        // Decode to absolute positions with names
        let mut tokens_abs: Vec<(u32, u32, u32, u32, String)> = Vec::new();
        let mut pl = 0u32;
        let mut pc = 0u32;
        let mut i = 0;
        while i + 4 < data.len() {
            let dl = data[i]; let dc = data[i+1]; let len = data[i+2]; let tt = data[i+3];
            let line = pl + dl;
            let col = if dl == 0 { pc + dc } else { dc };
            let name = source.lines().nth(line as usize)
                .map(|l| {
                    let start = col as usize;
                    let end = (start + len as usize).min(l.len());
                    if start <= l.len() { &l[start..end] } else { "?" }
                })
                .unwrap_or("?");
            tokens_abs.push((line, col, len, tt, name.to_string()));
            pl = line; pc = col;
            i += 5;
        }

        // Check both vars get TT_VARIABLE at definition sites (line 4, 0-based)
        let sv_def = tokens_abs.iter().find(|t| t.4 == "skipVendored" && t.0 == 4);
        let sg_def = tokens_abs.iter().find(|t| t.4 == "skipGenerated" && t.0 == 4);

        assert!(sv_def.is_some(), "skipVendored def not found; tokens: {:?}", tokens_abs);
        assert!(sg_def.is_some(), "skipGenerated def not found; tokens: {:?}", tokens_abs);
        assert_eq!(sv_def.unwrap().3, TT_VARIABLE,
            "skipVendored def: expected TT_VARIABLE({}), got {}.\nAll tokens: {:?}",
            TT_VARIABLE, sv_def.unwrap().3, tokens_abs);
        assert_eq!(sg_def.unwrap().3, TT_VARIABLE,
            "skipGenerated def: expected TT_VARIABLE({}), got {}.\nAll tokens: {:?}",
            TT_VARIABLE, sg_def.unwrap().3, tokens_abs);

        // Check use sites
        let sv_use = tokens_abs.iter().find(|t| t.4 == "skipVendored" && t.0 == 8);
        let sg_use = tokens_abs.iter().find(|t| t.4 == "skipGenerated" && t.0 == 9);
        if let Some(t) = sv_use {
            assert_eq!(t.3, TT_VARIABLE, "skipVendored use: expected TT_VARIABLE, got {}", t.3);
        }
        if let Some(t) = sg_use {
            assert_eq!(t.3, TT_VARIABLE, "skipGenerated use: expected TT_VARIABLE, got {}", t.3);
        }
    }

    #[test]
    fn test_semantic_tokens_multi_var_in_procedure() {
        // Multi-var inside a procedure: both should be TT_VARIABLE, not TT_PARAMETER
        let source = "MODULE Test;\nPROCEDURE Foo;\nVAR\n  skipVendored, skipGenerated: BOOLEAN;\nBEGIN\n  skipVendored := TRUE;\n  skipGenerated := FALSE\nEND Foo;\nBEGIN\nEND Test.\n";
        let result = analyze::analyze_source(source, "test.mod", false, &[]);
        let data = collect_semantic_tokens(source, "test.mod", false, &result);

        // Decode to absolute positions with names
        let mut tokens_abs: Vec<(u32, u32, u32, u32, String)> = Vec::new();
        let mut pl = 0u32;
        let mut pc = 0u32;
        let mut i = 0;
        while i + 4 < data.len() {
            let dl = data[i]; let dc = data[i+1]; let len = data[i+2]; let tt = data[i+3];
            let line = pl + dl;
            let col = if dl == 0 { pc + dc } else { dc };
            let name = source.lines().nth(line as usize)
                .map(|l| &l[col as usize..(col as usize + len as usize).min(l.len())])
                .unwrap_or("?");
            tokens_abs.push((line, col, len, tt, name.to_string()));
            pl = line; pc = col;
            i += 5;
        }

        // Check definition sites (line 3, 0-based)
        let sv_def = tokens_abs.iter().find(|t| t.4 == "skipVendored" && t.0 == 3);
        let sg_def = tokens_abs.iter().find(|t| t.4 == "skipGenerated" && t.0 == 3);

        assert!(sv_def.is_some(), "skipVendored def not found; tokens: {:?}", tokens_abs);
        assert!(sg_def.is_some(), "skipGenerated def not found; tokens: {:?}", tokens_abs);
        assert_eq!(sv_def.unwrap().3, TT_VARIABLE,
            "skipVendored def should be TT_VARIABLE({}), got {}. All: {:?}",
            TT_VARIABLE, sv_def.unwrap().3, tokens_abs);
        assert_eq!(sg_def.unwrap().3, TT_VARIABLE,
            "skipGenerated def should be TT_VARIABLE({}), got {}. All: {:?}",
            TT_VARIABLE, sg_def.unwrap().3, tokens_abs);

        // Check use sites (lines 5 and 6, 0-based)
        let sv_use = tokens_abs.iter().find(|t| t.4 == "skipVendored" && t.0 == 5);
        let sg_use = tokens_abs.iter().find(|t| t.4 == "skipGenerated" && t.0 == 6);

        assert!(sv_use.is_some(), "skipVendored use not found; tokens: {:?}", tokens_abs);
        assert!(sg_use.is_some(), "skipGenerated use not found; tokens: {:?}", tokens_abs);
        assert_eq!(sv_use.unwrap().3, TT_VARIABLE,
            "skipVendored use should be TT_VARIABLE({}), got {}",
            TT_VARIABLE, sv_use.unwrap().3);
        assert_eq!(sg_use.unwrap().3, TT_VARIABLE,
            "skipGenerated use should be TT_VARIABLE({}), got {}",
            TT_VARIABLE, sg_use.unwrap().3);
    }

    #[test]
    fn test_semantic_tokens_import_site_classification() {
        // Imported procedures should be classified as TT_FUNCTION at the import site,
        // not TT_VARIABLE (the default fallback).
        let source = "MODULE Test;\nFROM InOut IMPORT WriteString, WriteLn;\nBEGIN\n  WriteString(\"hi\");\n  WriteLn\nEND Test.\n";
        let result = analyze::analyze_source(source, "test.mod", false, &[]);
        let data = collect_semantic_tokens(source, "test.mod", false, &result);

        // Decode to absolute positions
        let mut tokens_abs: Vec<(u32, u32, u32, u32)> = Vec::new();
        let mut pl = 0u32;
        let mut pc = 0u32;
        let mut i = 0;
        while i + 4 < data.len() {
            let dl = data[i]; let dc = data[i+1]; let len = data[i+2]; let tt = data[i+3];
            let line = pl + dl;
            let col = if dl == 0 { pc + dc } else { dc };
            tokens_abs.push((line, col, len, tt));
            pl = line; pc = col;
            i += 5;
        }

        // "WriteString" on line 1 (0-based), length 11, should be TT_FUNCTION
        let ws_import = tokens_abs.iter().find(|t| t.0 == 1 && t.2 == 11);
        assert!(ws_import.is_some(), "WriteString token not found on import line");
        assert_eq!(ws_import.unwrap().3, TT_FUNCTION,
            "WriteString at import site should be function, got {}", ws_import.unwrap().3);

        // "WriteLn" on line 1 (0-based), length 7, should be TT_FUNCTION
        let wl_import = tokens_abs.iter().find(|t| t.0 == 1 && t.2 == 7);
        assert!(wl_import.is_some(), "WriteLn token not found on import line");
        assert_eq!(wl_import.unwrap().3, TT_FUNCTION,
            "WriteLn at import site should be function, got {}", wl_import.unwrap().3);

        // "InOut" on line 1, length 5, should be TT_NAMESPACE
        let inout = tokens_abs.iter().find(|t| t.0 == 1 && t.2 == 5);
        assert!(inout.is_some(), "InOut token not found on import line");
        assert_eq!(inout.unwrap().3, TT_NAMESPACE,
            "InOut at import site should be namespace, got {}", inout.unwrap().3);
    }

    #[test]
    fn test_semantic_tokens_def_module_imports() {
        // Imports from pre-registered .def modules should be classified by
        // their actual kind (type, function, constant), not all as functions.
        use crate::ast::DefinitionModule;
        use crate::lexer::Lexer;
        use crate::parser::Parser;
        use crate::ast::CompilationUnit;

        // Parse a .def module
        let gfx_def_src = "DEFINITION MODULE Gfx;\nTYPE Renderer = ADDRESS;\nCONST WIN_CENTERED = 1;\nPROCEDURE Init(): BOOLEAN;\nEND Gfx.\n";
        let mut lex = Lexer::new(gfx_def_src, "Gfx.def");
        let tokens = lex.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let gfx_unit = parser.parse_compilation_unit().unwrap();
        let gfx_def = match gfx_unit {
            CompilationUnit::DefinitionModule(d) => d,
            _ => panic!("expected def module"),
        };

        // Analyze the main module with the .def pre-registered
        let main_src = "MODULE Test;\nFROM Gfx IMPORT Renderer, WIN_CENTERED, Init;\nBEGIN\nEND Test.\n";
        let result = analyze::analyze_source(main_src, "test.mod", false, &[&gfx_def]);
        let data = collect_semantic_tokens(main_src, "test.mod", false, &result);

        // Decode tokens
        let mut tokens_abs: Vec<(u32, u32, u32, u32, String)> = Vec::new();
        let mut pl = 0u32;
        let mut pc = 0u32;
        let mut i = 0;
        while i + 4 < data.len() {
            let dl = data[i]; let dc = data[i+1]; let len = data[i+2]; let tt = data[i+3];
            let line = pl + dl;
            let col = if dl == 0 { pc + dc } else { dc };
            let name = main_src.lines().nth(line as usize)
                .map(|l| {
                    let s = col as usize;
                    let e = (s + len as usize).min(l.len());
                    if s <= l.len() { &l[s..e] } else { "?" }
                })
                .unwrap_or("?");
            tokens_abs.push((line, col, len, tt, name.to_string()));
            pl = line; pc = col;
            i += 5;
        }

        // "Renderer" on line 1 should be TT_TYPE
        let renderer = tokens_abs.iter().find(|t| t.4 == "Renderer" && t.0 == 1);
        assert!(renderer.is_some(), "Renderer not found; tokens: {:?}", tokens_abs);
        assert_eq!(renderer.unwrap().3, TT_TYPE,
            "Renderer should be TT_TYPE({}), got {}. All: {:?}",
            TT_TYPE, renderer.unwrap().3, tokens_abs);

        // "WIN_CENTERED" on line 1 should be TT_VARIABLE (constant maps to variable)
        let wc = tokens_abs.iter().find(|t| t.4 == "WIN_CENTERED" && t.0 == 1);
        assert!(wc.is_some(), "WIN_CENTERED not found; tokens: {:?}", tokens_abs);
        assert_eq!(wc.unwrap().3, TT_VARIABLE,
            "WIN_CENTERED should be TT_VARIABLE({}), got {}",
            TT_VARIABLE, wc.unwrap().3);

        // "Init" on line 1 should be TT_FUNCTION
        let init = tokens_abs.iter().find(|t| t.4 == "Init" && t.0 == 1);
        assert!(init.is_some(), "Init not found; tokens: {:?}", tokens_abs);
        assert_eq!(init.unwrap().3, TT_FUNCTION,
            "Init should be TT_FUNCTION({}), got {}",
            TT_FUNCTION, init.unwrap().3);

    }

    #[test]
    fn test_semantic_tokens_impl_module_imports() {
        // Same as above but for IMPLEMENTATION MODULE (hexed files are impl modules)
        use crate::ast::DefinitionModule;
        use crate::lexer::Lexer;
        use crate::parser::Parser;
        use crate::ast::CompilationUnit;

        // Parse Events.def with constants and procedures
        let events_def_src = "DEFINITION MODULE Events;\nCONST KEYDOWN = 2; QUIT_EVENT = 1;\nPROCEDURE KeyCode(): INTEGER;\nPROCEDURE KeyMod(): INTEGER;\nEND Events.\n";
        let mut lex = Lexer::new(events_def_src, "Events.def");
        let tokens = lex.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let events_def = match parser.parse_compilation_unit().unwrap() {
            CompilationUnit::DefinitionModule(d) => d,
            _ => panic!("expected def module"),
        };

        // Parse Keymap.def (the own .def for the impl module)
        let keymap_def_src = "DEFINITION MODULE Keymap;\nTYPE Action = (ActNone, ActQuit);\nPROCEDURE HandleEvent(evType: INTEGER): Action;\nEND Keymap.\n";
        let mut lex2 = Lexer::new(keymap_def_src, "Keymap.def");
        let tokens2 = lex2.tokenize().unwrap();
        let mut parser2 = Parser::new(tokens2);
        let keymap_def = match parser2.parse_compilation_unit().unwrap() {
            CompilationUnit::DefinitionModule(d) => d,
            _ => panic!("expected def module"),
        };

        // Analyze an IMPLEMENTATION MODULE that imports from Events
        let impl_src = "IMPLEMENTATION MODULE Keymap;\nFROM Events IMPORT KeyCode, KeyMod, KEYDOWN, QUIT_EVENT;\nBEGIN\nEND Keymap.\n";
        let result = analyze::analyze_source(impl_src, "Keymap.mod", false, &[&keymap_def, &events_def]);
        let data = collect_semantic_tokens(impl_src, "Keymap.mod", false, &result);

        // Decode tokens
        let mut tokens_abs: Vec<(u32, u32, u32, u32, String)> = Vec::new();
        let mut pl = 0u32;
        let mut pc = 0u32;
        let mut i = 0;
        while i + 4 < data.len() {
            let dl = data[i]; let dc = data[i+1]; let len = data[i+2]; let tt = data[i+3];
            let line = pl + dl;
            let col = if dl == 0 { pc + dc } else { dc };
            let name = impl_src.lines().nth(line as usize)
                .map(|l| {
                    let s = col as usize;
                    let e = (s + len as usize).min(l.len());
                    if s <= l.len() { &l[s..e] } else { "?" }
                })
                .unwrap_or("?");
            tokens_abs.push((line, col, len, tt, name.to_string()));
            pl = line; pc = col;
            i += 5;
        }

        // "KeyCode" on line 1 should be TT_FUNCTION
        let kc = tokens_abs.iter().find(|t| t.4 == "KeyCode" && t.0 == 1);
        assert!(kc.is_some(), "KeyCode not found; tokens: {:?}", tokens_abs);
        assert_eq!(kc.unwrap().3, TT_FUNCTION,
            "KeyCode should be TT_FUNCTION({}), got {}. All: {:?}",
            TT_FUNCTION, kc.unwrap().3, tokens_abs);

        // "KEYDOWN" on line 1 should be TT_VARIABLE (constant)
        let kd = tokens_abs.iter().find(|t| t.4 == "KEYDOWN" && t.0 == 1);
        assert!(kd.is_some(), "KEYDOWN not found; tokens: {:?}", tokens_abs);
        assert_eq!(kd.unwrap().3, TT_VARIABLE,
            "KEYDOWN should be TT_VARIABLE({}), got {}",
            TT_VARIABLE, kd.unwrap().3);

        // "QUIT_EVENT" on line 1 should be TT_VARIABLE (constant)
        let qe = tokens_abs.iter().find(|t| t.4 == "QUIT_EVENT" && t.0 == 1);
        assert!(qe.is_some(), "QUIT_EVENT not found; tokens: {:?}", tokens_abs);
        assert_eq!(qe.unwrap().3, TT_VARIABLE,
            "QUIT_EVENT should be TT_VARIABLE({}), got {}",
            TT_VARIABLE, qe.unwrap().3);
    }
}
