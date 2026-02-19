use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLoc {
    pub file: String,
    pub line: usize,
    pub col: usize,
}

impl SourceLoc {
    pub fn new(file: &str, line: usize, col: usize) -> Self {
        Self {
            file: file.to_string(),
            line,
            col,
        }
    }
}

impl Default for SourceLoc {
    fn default() -> Self {
        Self {
            file: String::new(),
            line: 0,
            col: 0,
        }
    }
}

impl fmt::Display for SourceLoc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}", self.file, self.line, self.col)
    }
}

#[derive(Debug, Clone)]
pub struct CompileError {
    pub loc: SourceLoc,
    pub message: String,
    pub kind: ErrorKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Lexer,
    Parser,
    Semantic,
    CodeGen,
    Driver,
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self.kind {
            ErrorKind::Lexer
            | ErrorKind::Parser
            | ErrorKind::Semantic
            | ErrorKind::CodeGen
            | ErrorKind::Driver => "error",
        };
        write!(f, "{}: {}: {}", self.loc, kind, self.message)
    }
}

impl std::error::Error for CompileError {}

impl CompileError {
    pub fn new(loc: SourceLoc, message: String, kind: ErrorKind) -> Self {
        Self { loc, message, kind }
    }

    pub fn lexer(loc: SourceLoc, msg: impl Into<String>) -> Self {
        Self::new(loc, msg.into(), ErrorKind::Lexer)
    }

    pub fn parser(loc: SourceLoc, msg: impl Into<String>) -> Self {
        Self::new(loc, msg.into(), ErrorKind::Parser)
    }

    pub fn semantic(loc: SourceLoc, msg: impl Into<String>) -> Self {
        Self::new(loc, msg.into(), ErrorKind::Semantic)
    }

    pub fn codegen(loc: SourceLoc, msg: impl Into<String>) -> Self {
        Self::new(loc, msg.into(), ErrorKind::CodeGen)
    }

    pub fn driver(msg: impl Into<String>) -> Self {
        Self::new(
            SourceLoc::new("<driver>", 0, 0),
            msg.into(),
            ErrorKind::Driver,
        )
    }
}

impl CompileError {
    pub fn to_json(&self) -> String {
        use crate::json::Json;
        let severity = match self.kind {
            ErrorKind::Lexer | ErrorKind::Parser | ErrorKind::Semantic
            | ErrorKind::CodeGen | ErrorKind::Driver => "error",
        };
        let kind_str = match self.kind {
            ErrorKind::Lexer => "lexer",
            ErrorKind::Parser => "parser",
            ErrorKind::Semantic => "semantic",
            ErrorKind::CodeGen => "codegen",
            ErrorKind::Driver => "driver",
        };
        Json::obj(vec![
            ("file", Json::str_val(&self.loc.file)),
            ("line", Json::int_val(self.loc.line as i64)),
            ("col", Json::int_val(self.loc.col as i64)),
            ("severity", Json::str_val(severity)),
            ("kind", Json::str_val(kind_str)),
            ("message", Json::str_val(&self.message)),
        ])
        .serialize()
    }
}

pub type CompileResult<T> = Result<T, CompileError>;
