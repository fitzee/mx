use std::collections::HashSet;
use crate::errors::{CompileError, CompileResult, SourceLoc};
use crate::token::{Token, TokenKind};

pub struct Lexer {
    source: Vec<char>,
    pos: usize,
    line: usize,
    col: usize,
    file: String,
    features: HashSet<String>,
}

impl Lexer {
    pub fn new(source: &str, file: &str) -> Self {
        Self {
            source: source.chars().collect(),
            pos: 0,
            line: 1,
            col: 1,
            file: file.to_string(),
            features: HashSet::new(),
        }
    }

    pub fn set_features(&mut self, features: &[String]) {
        self.features = features.iter().cloned().collect();
    }

    fn loc(&self) -> SourceLoc {
        SourceLoc::new(&self.file, self.line, self.col)
    }

    fn peek(&self) -> Option<char> {
        self.source.get(self.pos).copied()
    }

    fn peek_ahead(&self, offset: usize) -> Option<char> {
        self.source.get(self.pos + offset).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.source.get(self.pos).copied()?;
        self.pos += 1;
        if ch == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(ch)
    }

    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.peek() {
            if ch.is_ascii_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn skip_comment(&mut self) -> CompileResult<Option<Token>> {
        let loc = self.loc();
        // consume '('
        self.advance();
        // consume '*'
        self.advance();

        // Check for pragma: (*$DIRECTIVE ...*)
        if self.peek() == Some('$') {
            self.advance(); // consume '$'
            return self.read_pragma(loc);
        }

        let mut depth = 1;
        while depth > 0 {
            match self.peek() {
                None => {
                    return Err(CompileError::lexer(loc, "unterminated comment"));
                }
                Some('(') => {
                    self.advance();
                    if self.peek() == Some('*') {
                        self.advance();
                        depth += 1;
                    }
                }
                Some('*') => {
                    self.advance();
                    if self.peek() == Some(')') {
                        self.advance();
                        depth -= 1;
                    }
                }
                _ => {
                    self.advance();
                }
            }
        }
        Ok(None)
    }

    fn read_pragma(&mut self, loc: SourceLoc) -> CompileResult<Option<Token>> {
        // Read directive name (alphanumeric)
        let mut directive = String::new();
        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                directive.push(ch);
                self.advance();
            } else {
                break;
            }
        }
        if directive.is_empty() {
            return Err(CompileError::lexer(loc, "expected pragma directive name after (*$"));
        }

        // Handle conditional compilation pragmas
        if directive == "IF" {
            // Read feature name
            while let Some(ch) = self.peek() {
                if ch.is_ascii_whitespace() { self.advance(); } else { break; }
            }
            let mut feature_name = String::new();
            while let Some(ch) = self.peek() {
                if ch.is_ascii_alphanumeric() || ch == '_' {
                    feature_name.push(ch);
                    self.advance();
                } else { break; }
            }
            // Skip to closing *)
            self.skip_pragma_close(&loc)?;

            if self.features.contains(&feature_name) {
                // Feature enabled: include tokens until ELSE or END
                // ELSE/END handled when encountered as separate pragmas
                return Ok(None);
            } else {
                // Feature disabled: skip tokens until ELSE or END
                self.skip_disabled_block(true)?;
                return Ok(None);
            }
        } else if directive == "ELSE" {
            // Reached ELSE from enabled IF block — skip until END
            self.skip_pragma_close(&loc)?;
            self.skip_disabled_block(false)?;
            return Ok(None);
        } else if directive == "END" {
            // End of a conditional block — just consume
            self.skip_pragma_close(&loc)?;
            return Ok(None);
        }

        // Skip whitespace
        while let Some(ch) = self.peek() {
            if ch.is_ascii_whitespace() {
                self.advance();
            } else {
                break;
            }
        }

        // Optionally read a string argument
        let arg = if self.peek() == Some('"') {
            self.advance(); // consume opening "
            let mut s = String::new();
            loop {
                match self.peek() {
                    None => {
                        return Err(CompileError::lexer(loc, "unterminated string in pragma"));
                    }
                    Some('"') => {
                        self.advance();
                        break;
                    }
                    Some(ch) => {
                        s.push(ch);
                        self.advance();
                    }
                }
            }
            Some(s)
        } else {
            None
        };

        // Skip to closing *)
        self.skip_pragma_close(&loc)?;

        Ok(Some(Token::new(TokenKind::Pragma(directive, arg), loc)))
    }

    /// Skip to closing *) of current pragma
    fn skip_pragma_close(&mut self, loc: &SourceLoc) -> CompileResult<()> {
        loop {
            match self.peek() {
                None => return Err(CompileError::lexer(loc.clone(), "unterminated pragma")),
                Some('*') => {
                    self.advance();
                    if self.peek() == Some(')') {
                        self.advance();
                        return Ok(());
                    }
                }
                _ => { self.advance(); }
            }
        }
    }

    /// Skip content in a disabled conditional block.
    /// If stop_at_else is true, also stops at (*$ELSE*) at depth 0.
    /// Handles nested (*$IF*)...(*$END*) blocks.
    fn skip_disabled_block(&mut self, stop_at_else: bool) -> CompileResult<()> {
        let loc = self.loc();
        let mut depth = 0usize;
        loop {
            match self.peek() {
                None => return Err(CompileError::lexer(loc, "unterminated (*$IF*) block")),
                Some('(') => {
                    self.advance();
                    if self.peek() == Some('*') {
                        self.advance();
                        if self.peek() == Some('$') {
                            self.advance();
                            // Read directive name
                            let mut dir = String::new();
                            while let Some(ch) = self.peek() {
                                if ch.is_ascii_alphanumeric() || ch == '_' {
                                    dir.push(ch);
                                    self.advance();
                                } else { break; }
                            }
                            // Skip rest of pragma to closing *)
                            let ploc = self.loc();
                            self.skip_pragma_close(&ploc)?;

                            if dir == "IF" {
                                depth += 1;
                            } else if dir == "ELSE" && depth == 0 && stop_at_else {
                                return Ok(());
                            } else if dir == "END" {
                                if depth == 0 { return Ok(()); }
                                depth -= 1;
                            }
                        } else {
                            // Regular nested comment — skip it
                            let mut cdepth = 1usize;
                            while cdepth > 0 {
                                match self.peek() {
                                    None => return Err(CompileError::lexer(loc, "unterminated comment in disabled block")),
                                    Some('(') => {
                                        self.advance();
                                        if self.peek() == Some('*') { self.advance(); cdepth += 1; }
                                    }
                                    Some('*') => {
                                        self.advance();
                                        if self.peek() == Some(')') { self.advance(); cdepth -= 1; }
                                    }
                                    _ => { self.advance(); }
                                }
                            }
                        }
                    }
                }
                _ => { self.advance(); }
            }
        }
    }

    fn read_string(&mut self, quote: char) -> CompileResult<Token> {
        let loc = self.loc();
        self.advance(); // consume opening quote
        let mut s = String::new();
        loop {
            match self.peek() {
                None | Some('\n') => {
                    return Err(CompileError::lexer(loc, "unterminated string literal"));
                }
                Some(ch) if ch == quote => {
                    self.advance();
                    break;
                }
                Some(ch) => {
                    s.push(ch);
                    self.advance();
                }
            }
        }
        // In PIM4, single-char strings are assignment-compatible with CHAR,
        // but we keep them as StringLit and handle coercion in sema/codegen.
        Ok(Token::new(TokenKind::StringLit(s), loc))
    }

    fn read_number(&mut self) -> CompileResult<Token> {
        let loc = self.loc();
        let mut digits = String::new();

        // Collect all hex-valid digits and potential suffix
        while let Some(ch) = self.peek() {
            if ch.is_ascii_hexdigit() {
                digits.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        // Check for suffix (H, B, C) — these are NOT hex digits, so check peek
        match self.peek() {
            Some('H') | Some('h') => {
                self.advance();
                let val = i64::from_str_radix(&digits, 16).map_err(|_| {
                    CompileError::lexer(loc.clone(), "invalid hex literal")
                })?;
                return Ok(Token::new(TokenKind::IntLit(val), loc));
            }
            _ => {}
        }

        // B and C ARE hex digits, so they may have been consumed into digits.
        // Check if last char is B or C and remaining chars are octal.
        if digits.len() > 1 {
            let last = digits.chars().last().unwrap();
            if (last == 'B' || last == 'b') {
                let prefix = &digits[..digits.len() - 1];
                if prefix.chars().all(|c| c >= '0' && c <= '7') {
                    let val = i64::from_str_radix(prefix, 8).map_err(|_| {
                        CompileError::lexer(loc.clone(), "invalid octal literal")
                    })?;
                    return Ok(Token::new(TokenKind::IntLit(val), loc));
                }
            }
            if (last == 'C' || last == 'c') {
                let prefix = &digits[..digits.len() - 1];
                if prefix.chars().all(|c| c >= '0' && c <= '7') {
                    let val = u32::from_str_radix(prefix, 8).map_err(|_| {
                        CompileError::lexer(loc.clone(), "invalid octal char literal")
                    })?;
                    let ch = char::from_u32(val).ok_or_else(|| {
                        CompileError::lexer(loc.clone(), "invalid character value")
                    })?;
                    return Ok(Token::new(TokenKind::CharLit(ch), loc));
                }
            }
        }

        // Also check standalone B/C suffix from peek (single digit octal)
        if digits.len() == 1 && digits.chars().all(|c| c >= '0' && c <= '7') {
            match self.peek() {
                Some('B') | Some('b') => {
                    self.advance();
                    let val = i64::from_str_radix(&digits, 8).map_err(|_| {
                        CompileError::lexer(loc.clone(), "invalid octal literal")
                    })?;
                    return Ok(Token::new(TokenKind::IntLit(val), loc));
                }
                Some('C') | Some('c') => {
                    self.advance();
                    let val = u32::from_str_radix(&digits, 8).map_err(|_| {
                        CompileError::lexer(loc.clone(), "invalid octal char literal")
                    })?;
                    let ch = char::from_u32(val).ok_or_else(|| {
                        CompileError::lexer(loc.clone(), "invalid character value")
                    })?;
                    return Ok(Token::new(TokenKind::CharLit(ch), loc));
                }
                _ => {}
            }
        }

        // Check for real number (decimal point followed by digit, not '..')
        if self.peek() == Some('.') && self.peek_ahead(1) != Some('.') {
            if let Some(next) = self.peek_ahead(1) {
                if next.is_ascii_digit() {
                    digits.push('.');
                    self.advance(); // consume '.'
                    while let Some(ch) = self.peek() {
                        if ch.is_ascii_digit() {
                            digits.push(ch);
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    // Exponent
                    if let Some('E') | Some('e') = self.peek() {
                        digits.push('E');
                        self.advance();
                        if let Some('+') | Some('-') = self.peek() {
                            digits.push(self.advance().unwrap());
                        }
                        while let Some(ch) = self.peek() {
                            if ch.is_ascii_digit() {
                                digits.push(ch);
                                self.advance();
                            } else {
                                break;
                            }
                        }
                    }
                    let val: f64 = digits.parse().map_err(|_| {
                        CompileError::lexer(loc.clone(), "invalid real literal")
                    })?;
                    return Ok(Token::new(TokenKind::RealLit(val), loc));
                }
            }
        }

        // Plain decimal integer
        if !digits.chars().all(|c| c.is_ascii_digit()) {
            return Err(CompileError::lexer(
                loc,
                format!("invalid number '{}' (hex digits require H suffix)", digits),
            ));
        }
        let val: i64 = digits.parse().map_err(|_| {
            CompileError::lexer(loc.clone(), "integer literal too large")
        })?;
        Ok(Token::new(TokenKind::IntLit(val), loc))
    }

    fn read_ident_or_keyword(&mut self) -> Token {
        let loc = self.loc();
        let mut name = String::new();
        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                name.push(ch);
                self.advance();
            } else {
                break;
            }
        }
        let kind = TokenKind::keyword_from_str(&name).unwrap_or(TokenKind::Ident(name));
        Token::new(kind, loc)
    }

    pub fn next_token(&mut self) -> CompileResult<Token> {
        loop {
            self.skip_whitespace();
            let loc = self.loc();

            match self.peek() {
                None => return Ok(Token::new(TokenKind::Eof, loc)),

                Some('(') => {
                    if self.peek_ahead(1) == Some('*') {
                        if let Some(tok) = self.skip_comment()? {
                            return Ok(tok);
                        }
                        continue;
                    }
                    self.advance();
                    return Ok(Token::new(TokenKind::LParen, loc));
                }

                Some('"') => return self.read_string('"'),
                Some('\'') => return self.read_string('\''),

                Some(ch) if ch.is_ascii_digit() => return self.read_number(),
                Some(ch) if ch.is_ascii_alphabetic() => return Ok(self.read_ident_or_keyword()),

                Some('+') => { self.advance(); return Ok(Token::new(TokenKind::Plus, loc)); }
                Some('-') => { self.advance(); return Ok(Token::new(TokenKind::Minus, loc)); }
                Some('*') => { self.advance(); return Ok(Token::new(TokenKind::Star, loc)); }
                Some('/') => { self.advance(); return Ok(Token::new(TokenKind::Slash, loc)); }
                Some('=') => { self.advance(); return Ok(Token::new(TokenKind::Eq, loc)); }
                Some('#') => { self.advance(); return Ok(Token::new(TokenKind::Hash, loc)); }
                Some(',') => { self.advance(); return Ok(Token::new(TokenKind::Comma, loc)); }
                Some(';') => { self.advance(); return Ok(Token::new(TokenKind::Semi, loc)); }
                Some(')') => { self.advance(); return Ok(Token::new(TokenKind::RParen, loc)); }
                Some('[') => { self.advance(); return Ok(Token::new(TokenKind::LBrack, loc)); }
                Some(']') => { self.advance(); return Ok(Token::new(TokenKind::RBrack, loc)); }
                Some('{') => { self.advance(); return Ok(Token::new(TokenKind::LBrace, loc)); }
                Some('}') => { self.advance(); return Ok(Token::new(TokenKind::RBrace, loc)); }
                Some('^') => { self.advance(); return Ok(Token::new(TokenKind::Caret, loc)); }
                Some('|') => { self.advance(); return Ok(Token::new(TokenKind::Pipe, loc)); }
                Some('&') => { self.advance(); return Ok(Token::new(TokenKind::Ampersand, loc)); }
                Some('~') => { self.advance(); return Ok(Token::new(TokenKind::Tilde, loc)); }

                Some(':') => {
                    self.advance();
                    if self.peek() == Some('=') {
                        self.advance();
                        return Ok(Token::new(TokenKind::Assign, loc));
                    }
                    return Ok(Token::new(TokenKind::Colon, loc));
                }

                Some('.') => {
                    self.advance();
                    if self.peek() == Some('.') {
                        self.advance();
                        return Ok(Token::new(TokenKind::DotDot, loc));
                    }
                    return Ok(Token::new(TokenKind::Dot, loc));
                }

                Some('<') => {
                    self.advance();
                    match self.peek() {
                        Some('=') => { self.advance(); return Ok(Token::new(TokenKind::Le, loc)); }
                        Some('>') => { self.advance(); return Ok(Token::new(TokenKind::NotEq, loc)); }
                        _ => return Ok(Token::new(TokenKind::Lt, loc)),
                    }
                }

                Some('>') => {
                    self.advance();
                    if self.peek() == Some('=') {
                        self.advance();
                        return Ok(Token::new(TokenKind::Ge, loc));
                    }
                    return Ok(Token::new(TokenKind::Gt, loc));
                }

                Some(ch) => {
                    self.advance();
                    return Err(CompileError::lexer(loc, format!("unexpected character '{}'", ch)));
                }
            }
        }
    }

    pub fn tokenize(&mut self) -> CompileResult<Vec<Token>> {
        let mut tokens = Vec::new();
        loop {
            let tok = self.next_token()?;
            let is_eof = tok.kind == TokenKind::Eof;
            tokens.push(tok);
            if is_eof {
                break;
            }
        }
        Ok(tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lex(input: &str) -> Vec<TokenKind> {
        let mut lexer = Lexer::new(input, "test");
        let tokens = lexer.tokenize().unwrap();
        tokens.into_iter().map(|t| t.kind).collect()
    }

    #[test]
    fn test_keywords() {
        let tokens = lex("MODULE BEGIN END");
        assert_eq!(tokens, vec![
            TokenKind::Module, TokenKind::Begin, TokenKind::End, TokenKind::Eof,
        ]);
    }

    #[test]
    fn test_operators() {
        let tokens = lex(":= <= >= <> ..");
        assert_eq!(tokens, vec![
            TokenKind::Assign, TokenKind::Le, TokenKind::Ge,
            TokenKind::NotEq, TokenKind::DotDot, TokenKind::Eof,
        ]);
    }

    #[test]
    fn test_integers() {
        let tokens = lex("42 0FFH 17B 101C");
        assert_eq!(tokens, vec![
            TokenKind::IntLit(42),
            TokenKind::IntLit(0xFF),
            TokenKind::IntLit(0o17),
            TokenKind::CharLit(char::from_u32(0o101).unwrap()),
            TokenKind::Eof,
        ]);
    }

    #[test]
    fn test_real() {
        let tokens = lex("3.14 1.0E10");
        assert!(matches!(tokens[0], TokenKind::RealLit(_)));
        assert!(matches!(tokens[1], TokenKind::RealLit(_)));
    }

    #[test]
    fn test_strings() {
        let tokens = lex(r#""hello" 'x'"#);
        assert_eq!(tokens, vec![
            TokenKind::StringLit("hello".to_string()),
            TokenKind::StringLit("x".to_string()),
            TokenKind::Eof,
        ]);
    }

    #[test]
    fn test_nested_comments() {
        let tokens = lex("(* outer (* inner *) still comment *) MODULE");
        assert_eq!(tokens, vec![TokenKind::Module, TokenKind::Eof]);
    }

    #[test]
    fn test_identifiers() {
        let tokens = lex("foo Bar baz123");
        assert_eq!(tokens, vec![
            TokenKind::Ident("foo".to_string()),
            TokenKind::Ident("Bar".to_string()),
            TokenKind::Ident("baz123".to_string()),
            TokenKind::Eof,
        ]);
    }

    fn lex_with_features(input: &str, features: &[&str]) -> Vec<TokenKind> {
        let mut lexer = Lexer::new(input, "test");
        let feat_strings: Vec<String> = features.iter().map(|s| s.to_string()).collect();
        lexer.set_features(&feat_strings);
        let tokens = lexer.tokenize().unwrap();
        tokens.into_iter().map(|t| t.kind).collect()
    }

    #[test]
    fn test_feature_if_enabled() {
        let tokens = lex_with_features(
            "MODULE (*$IF threading*) FROM Thread IMPORT Fork; (*$END*) END",
            &["threading"],
        );
        assert_eq!(tokens, vec![
            TokenKind::Module,
            TokenKind::From,
            TokenKind::Ident("Thread".to_string()),
            TokenKind::Import,
            TokenKind::Ident("Fork".to_string()),
            TokenKind::Semi,
            TokenKind::End,
            TokenKind::Eof,
        ]);
    }

    #[test]
    fn test_feature_if_disabled() {
        let tokens = lex_with_features(
            "MODULE (*$IF threading*) FROM Thread IMPORT Fork; (*$END*) END",
            &[],
        );
        assert_eq!(tokens, vec![
            TokenKind::Module,
            TokenKind::End,
            TokenKind::Eof,
        ]);
    }

    #[test]
    fn test_feature_if_else_enabled() {
        let tokens = lex_with_features(
            "(*$IF gc*) VAR x: INTEGER; (*$ELSE*) VAR y: INTEGER; (*$END*)",
            &["gc"],
        );
        assert_eq!(tokens, vec![
            TokenKind::Var,
            TokenKind::Ident("x".to_string()),
            TokenKind::Colon,
            TokenKind::Ident("INTEGER".to_string()),
            TokenKind::Semi,
            TokenKind::Eof,
        ]);
    }

    #[test]
    fn test_feature_if_else_disabled() {
        let tokens = lex_with_features(
            "(*$IF gc*) VAR x: INTEGER; (*$ELSE*) VAR y: INTEGER; (*$END*)",
            &[],
        );
        assert_eq!(tokens, vec![
            TokenKind::Var,
            TokenKind::Ident("y".to_string()),
            TokenKind::Colon,
            TokenKind::Ident("INTEGER".to_string()),
            TokenKind::Semi,
            TokenKind::Eof,
        ]);
    }

    #[test]
    fn test_feature_nested_if() {
        let tokens = lex_with_features(
            "A (*$IF a*) B (*$IF b*) C (*$END*) D (*$END*) E",
            &["a"],
        );
        // a is enabled, b is not: A B D E
        assert_eq!(tokens, vec![
            TokenKind::Ident("A".to_string()),
            TokenKind::Ident("B".to_string()),
            TokenKind::Ident("D".to_string()),
            TokenKind::Ident("E".to_string()),
            TokenKind::Eof,
        ]);
    }
}
