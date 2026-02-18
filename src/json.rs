/// Minimal JSON parser and serializer for build plan files. No external dependencies.
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum Json {
    Null,
    Bool(bool),
    Number(f64),
    Str(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    pub fn parse(input: &str) -> Result<Json, String> {
        let mut p = JsonParser { chars: input.chars().collect(), pos: 0 };
        p.skip_ws();
        let v = p.parse_value()?;
        Ok(v)
    }

    // ── Builder helpers ──────────────────────────────────────────

    pub fn obj(entries: Vec<(&str, Json)>) -> Json {
        Json::Object(entries.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    }

    pub fn arr(items: Vec<Json>) -> Json {
        Json::Array(items)
    }

    pub fn str_val(s: &str) -> Json {
        Json::Str(s.to_string())
    }

    pub fn int_val(n: i64) -> Json {
        Json::Number(n as f64)
    }

    pub fn bool_val(b: bool) -> Json {
        Json::Bool(b)
    }

    // ── Serialization ────────────────────────────────────────────

    pub fn serialize(&self) -> String {
        let mut out = String::new();
        self.write_to(&mut out);
        out
    }

    fn write_to(&self, out: &mut String) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Json::Number(n) => {
                if *n == (*n as i64) as f64 && n.is_finite() {
                    out.push_str(&(*n as i64).to_string());
                } else {
                    out.push_str(&n.to_string());
                }
            }
            Json::Str(s) => {
                out.push('"');
                for ch in s.chars() {
                    match ch {
                        '"' => out.push_str("\\\""),
                        '\\' => out.push_str("\\\\"),
                        '\n' => out.push_str("\\n"),
                        '\t' => out.push_str("\\t"),
                        '\r' => out.push_str("\\r"),
                        c if (c as u32) < 0x20 => {
                            out.push_str(&format!("\\u{:04x}", c as u32));
                        }
                        c => out.push(c),
                    }
                }
                out.push('"');
            }
            Json::Array(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 { out.push(','); }
                    item.write_to(out);
                }
                out.push(']');
            }
            Json::Object(entries) => {
                out.push('{');
                for (i, (key, val)) in entries.iter().enumerate() {
                    if i > 0 { out.push(','); }
                    Json::Str(key.clone()).write_to(out);
                    out.push(':');
                    val.write_to(out);
                }
                out.push('}');
            }
        }
    }

    // ── Accessors ────────────────────────────────────────────────

    pub fn get(&self, key: &str) -> Option<&Json> {
        if let Json::Object(ref entries) = self {
            for (k, v) in entries {
                if k == key { return Some(v); }
            }
        }
        None
    }

    pub fn as_str(&self) -> Option<&str> {
        if let Json::Str(ref s) = self { Some(s) } else { None }
    }

    pub fn as_bool(&self) -> Option<bool> {
        if let Json::Bool(b) = self { Some(*b) } else { None }
    }

    pub fn as_i64(&self) -> Option<i64> {
        if let Json::Number(n) = self { Some(*n as i64) } else { None }
    }

    pub fn as_array(&self) -> Option<&[Json]> {
        if let Json::Array(ref a) = self { Some(a) } else { None }
    }

    pub fn as_str_or(&self, key: &str, default: &str) -> String {
        self.get(key).and_then(|v| v.as_str()).unwrap_or(default).to_string()
    }

    pub fn as_bool_or(&self, key: &str, default: bool) -> bool {
        self.get(key).and_then(|v| v.as_bool()).unwrap_or(default)
    }

    pub fn as_string_array(&self, key: &str) -> Vec<String> {
        self.get(key)
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default()
    }
}

struct JsonParser {
    chars: Vec<char>,
    pos: usize,
}

impl JsonParser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.chars.get(self.pos).copied();
        if c.is_some() { self.pos += 1; }
        c
    }

    fn skip_ws(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() { self.advance(); } else { break; }
        }
    }

    fn expect(&mut self, ch: char) -> Result<(), String> {
        self.skip_ws();
        match self.advance() {
            Some(c) if c == ch => Ok(()),
            Some(c) => Err(format!("expected '{}', got '{}' at pos {}", ch, c, self.pos)),
            None => Err(format!("expected '{}', got EOF", ch)),
        }
    }

    fn parse_value(&mut self) -> Result<Json, String> {
        self.skip_ws();
        match self.peek() {
            Some('"') => self.parse_string().map(Json::Str),
            Some('{') => self.parse_object(),
            Some('[') => self.parse_array(),
            Some('t') | Some('f') => self.parse_bool(),
            Some('n') => self.parse_null(),
            Some(c) if c == '-' || c.is_ascii_digit() => self.parse_number(),
            Some(c) => Err(format!("unexpected char '{}' at pos {}", c, self.pos)),
            None => Err("unexpected EOF".to_string()),
        }
    }

    fn parse_string(&mut self) -> Result<String, String> {
        self.expect('"')?;
        let mut s = String::new();
        loop {
            match self.advance() {
                Some('"') => return Ok(s),
                Some('\\') => {
                    match self.advance() {
                        Some('n') => s.push('\n'),
                        Some('t') => s.push('\t'),
                        Some('\\') => s.push('\\'),
                        Some('"') => s.push('"'),
                        Some('/') => s.push('/'),
                        Some(c) => { s.push('\\'); s.push(c); }
                        None => return Err("unterminated escape".to_string()),
                    }
                }
                Some(c) => s.push(c),
                None => return Err("unterminated string".to_string()),
            }
        }
    }

    fn parse_number(&mut self) -> Result<Json, String> {
        let start = self.pos;
        if self.peek() == Some('-') { self.advance(); }
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() || c == '.' || c == 'e' || c == 'E' || c == '+' || c == '-' {
                // avoid consuming '-' that starts the number twice
                if (c == '+' || c == '-') && self.pos > start + 1 {
                    let prev = self.chars[self.pos - 1];
                    if prev != 'e' && prev != 'E' { break; }
                }
                self.advance();
            } else {
                break;
            }
        }
        let num_str: String = self.chars[start..self.pos].iter().collect();
        num_str.parse::<f64>().map(Json::Number).map_err(|e| format!("bad number '{}': {}", num_str, e))
    }

    fn parse_bool(&mut self) -> Result<Json, String> {
        if self.chars[self.pos..].starts_with(&['t','r','u','e']) {
            self.pos += 4; Ok(Json::Bool(true))
        } else if self.chars[self.pos..].starts_with(&['f','a','l','s','e']) {
            self.pos += 5; Ok(Json::Bool(false))
        } else {
            Err(format!("expected bool at pos {}", self.pos))
        }
    }

    fn parse_null(&mut self) -> Result<Json, String> {
        if self.chars[self.pos..].starts_with(&['n','u','l','l']) {
            self.pos += 4; Ok(Json::Null)
        } else {
            Err(format!("expected null at pos {}", self.pos))
        }
    }

    fn parse_array(&mut self) -> Result<Json, String> {
        self.expect('[')?;
        let mut arr = Vec::new();
        self.skip_ws();
        if self.peek() == Some(']') { self.advance(); return Ok(Json::Array(arr)); }
        loop {
            arr.push(self.parse_value()?);
            self.skip_ws();
            match self.peek() {
                Some(',') => { self.advance(); }
                Some(']') => { self.advance(); return Ok(Json::Array(arr)); }
                _ => return Err(format!("expected ',' or ']' at pos {}", self.pos)),
            }
        }
    }

    fn parse_object(&mut self) -> Result<Json, String> {
        self.expect('{')?;
        let mut entries = Vec::new();
        self.skip_ws();
        if self.peek() == Some('}') { self.advance(); return Ok(Json::Object(entries)); }
        loop {
            self.skip_ws();
            let key = self.parse_string()?;
            self.skip_ws();
            self.expect(':')?;
            let val = self.parse_value()?;
            entries.push((key, val));
            self.skip_ws();
            match self.peek() {
                Some(',') => { self.advance(); }
                Some('}') => { self.advance(); return Ok(Json::Object(entries)); }
                _ => return Err(format!("expected ',' or '}}' at pos {}", self.pos)),
            }
        }
    }
}
