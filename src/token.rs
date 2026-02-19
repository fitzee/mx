use crate::errors::SourceLoc;

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub loc: SourceLoc,
}

impl Token {
    pub fn new(kind: TokenKind, loc: SourceLoc) -> Self {
        Self { kind, loc }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Literals
    IntLit(i64),
    RealLit(f64),
    StringLit(String),
    CharLit(char),

    // Identifier
    Ident(String),

    // Keywords
    And,
    Array,
    Begin,
    By,
    Case,
    Const,
    Definition,
    Div,
    Do,
    Else,
    Elsif,
    End,
    Except,
    Exit,
    Export,
    Finally,
    For,
    From,
    If,
    Implementation,
    Import,
    In,
    Loop,
    Mod,
    Module,
    Not,
    Of,
    Or,
    Pointer,
    Procedure,
    Qualified,
    Raise,
    Record,
    Repeat,
    Retry,
    Return,
    Set,
    Then,
    To,
    Type,
    Until,
    Var,
    While,
    With,

    // Modula-2+ keywords
    Branded,
    Exception,
    Lock,
    Methods,
    Object,
    Override,
    Ref,
    Refany,
    Reveal,
    Safe,
    Try,
    Typecase,
    Unsafe,

    // Operators and punctuation
    Plus,       // +
    Minus,      // -
    Star,       // *
    Slash,      // /
    Assign,     // :=
    Eq,         // =
    Hash,       // #
    NotEq,      // <>
    Lt,         // <
    Gt,         // >
    Le,         // <=
    Ge,         // >=
    DotDot,     // ..
    Dot,        // .
    Comma,      // ,
    Semi,       // ;
    Colon,      // :
    LParen,     // (
    RParen,     // )
    LBrack,     // [
    RBrack,     // ]
    LBrace,     // {
    RBrace,     // }
    Caret,      // ^
    Pipe,       // |
    Ampersand,  // &
    Tilde,      // ~

    // Pragmas
    Pragma(String, Option<String>),   // (*$DIRECTIVE "arg"*)

    // Documentation comment: (** ... *) or (*! ... *)
    DocComment(String),

    // Special
    Eof,
}

impl TokenKind {
    pub fn keyword_from_str(s: &str) -> Option<TokenKind> {
        match s {
            "AND" => Some(TokenKind::And),
            "ARRAY" => Some(TokenKind::Array),
            "BEGIN" => Some(TokenKind::Begin),
            "BY" => Some(TokenKind::By),
            "CASE" => Some(TokenKind::Case),
            "CONST" => Some(TokenKind::Const),
            "DEFINITION" => Some(TokenKind::Definition),
            "DIV" => Some(TokenKind::Div),
            "DO" => Some(TokenKind::Do),
            "ELSE" => Some(TokenKind::Else),
            "ELSIF" => Some(TokenKind::Elsif),
            "END" => Some(TokenKind::End),
            "EXCEPT" => Some(TokenKind::Except),
            "EXIT" => Some(TokenKind::Exit),
            "EXPORT" => Some(TokenKind::Export),
            "FINALLY" => Some(TokenKind::Finally),
            "FOR" => Some(TokenKind::For),
            "FROM" => Some(TokenKind::From),
            "IF" => Some(TokenKind::If),
            "IMPLEMENTATION" => Some(TokenKind::Implementation),
            "IMPORT" => Some(TokenKind::Import),
            "IN" => Some(TokenKind::In),
            "LOOP" => Some(TokenKind::Loop),
            "MOD" => Some(TokenKind::Mod),
            "MODULE" => Some(TokenKind::Module),
            "NOT" => Some(TokenKind::Not),
            "OF" => Some(TokenKind::Of),
            "OR" => Some(TokenKind::Or),
            "POINTER" => Some(TokenKind::Pointer),
            "PROCEDURE" => Some(TokenKind::Procedure),
            "QUALIFIED" => Some(TokenKind::Qualified),
            "RAISE" => Some(TokenKind::Raise),
            "RECORD" => Some(TokenKind::Record),
            "REPEAT" => Some(TokenKind::Repeat),
            "RETRY" => Some(TokenKind::Retry),
            "RETURN" => Some(TokenKind::Return),
            "SET" => Some(TokenKind::Set),
            "THEN" => Some(TokenKind::Then),
            "TO" => Some(TokenKind::To),
            "TYPE" => Some(TokenKind::Type),
            "UNTIL" => Some(TokenKind::Until),
            "VAR" => Some(TokenKind::Var),
            "WHILE" => Some(TokenKind::While),
            "WITH" => Some(TokenKind::With),
            // Modula-2+ keywords
            "BRANDED" => Some(TokenKind::Branded),
            "EXCEPTION" => Some(TokenKind::Exception),
            "LOCK" => Some(TokenKind::Lock),
            "METHODS" => Some(TokenKind::Methods),
            "OBJECT" => Some(TokenKind::Object),
            "OVERRIDE" => Some(TokenKind::Override),
            "REF" => Some(TokenKind::Ref),
            "REFANY" => Some(TokenKind::Refany),
            "REVEAL" => Some(TokenKind::Reveal),
            "SAFE" => Some(TokenKind::Safe),
            "TRY" => Some(TokenKind::Try),
            "TYPECASE" => Some(TokenKind::Typecase),
            "UNSAFE" => Some(TokenKind::Unsafe),
            _ => None,
        }
    }

    pub fn describe(&self) -> &'static str {
        match self {
            TokenKind::IntLit(_) => "integer literal",
            TokenKind::RealLit(_) => "real literal",
            TokenKind::StringLit(_) => "string literal",
            TokenKind::CharLit(_) => "character literal",
            TokenKind::Ident(_) => "identifier",
            TokenKind::And => "'AND'",
            TokenKind::Array => "'ARRAY'",
            TokenKind::Begin => "'BEGIN'",
            TokenKind::By => "'BY'",
            TokenKind::Case => "'CASE'",
            TokenKind::Const => "'CONST'",
            TokenKind::Definition => "'DEFINITION'",
            TokenKind::Div => "'DIV'",
            TokenKind::Do => "'DO'",
            TokenKind::Else => "'ELSE'",
            TokenKind::Elsif => "'ELSIF'",
            TokenKind::End => "'END'",
            TokenKind::Except => "'EXCEPT'",
            TokenKind::Exit => "'EXIT'",
            TokenKind::Export => "'EXPORT'",
            TokenKind::Finally => "'FINALLY'",
            TokenKind::For => "'FOR'",
            TokenKind::From => "'FROM'",
            TokenKind::If => "'IF'",
            TokenKind::Implementation => "'IMPLEMENTATION'",
            TokenKind::Import => "'IMPORT'",
            TokenKind::In => "'IN'",
            TokenKind::Loop => "'LOOP'",
            TokenKind::Mod => "'MOD'",
            TokenKind::Module => "'MODULE'",
            TokenKind::Not => "'NOT'",
            TokenKind::Of => "'OF'",
            TokenKind::Or => "'OR'",
            TokenKind::Pointer => "'POINTER'",
            TokenKind::Procedure => "'PROCEDURE'",
            TokenKind::Qualified => "'QUALIFIED'",
            TokenKind::Raise => "'RAISE'",
            TokenKind::Record => "'RECORD'",
            TokenKind::Repeat => "'REPEAT'",
            TokenKind::Retry => "'RETRY'",
            TokenKind::Return => "'RETURN'",
            TokenKind::Set => "'SET'",
            TokenKind::Then => "'THEN'",
            TokenKind::To => "'TO'",
            TokenKind::Type => "'TYPE'",
            TokenKind::Until => "'UNTIL'",
            TokenKind::Var => "'VAR'",
            TokenKind::While => "'WHILE'",
            TokenKind::With => "'WITH'",
            TokenKind::Branded => "'BRANDED'",
            TokenKind::Exception => "'EXCEPTION'",
            TokenKind::Lock => "'LOCK'",
            TokenKind::Methods => "'METHODS'",
            TokenKind::Object => "'OBJECT'",
            TokenKind::Override => "'OVERRIDE'",
            TokenKind::Ref => "'REF'",
            TokenKind::Refany => "'REFANY'",
            TokenKind::Reveal => "'REVEAL'",
            TokenKind::Safe => "'SAFE'",
            TokenKind::Try => "'TRY'",
            TokenKind::Typecase => "'TYPECASE'",
            TokenKind::Unsafe => "'UNSAFE'",
            TokenKind::Plus => "'+'",
            TokenKind::Minus => "'-'",
            TokenKind::Star => "'*'",
            TokenKind::Slash => "'/'",
            TokenKind::Assign => "':='",
            TokenKind::Eq => "'='",
            TokenKind::Hash => "'#'",
            TokenKind::NotEq => "'<>'",
            TokenKind::Lt => "'<'",
            TokenKind::Gt => "'>'",
            TokenKind::Le => "'<='",
            TokenKind::Ge => "'>='",
            TokenKind::DotDot => "'..'",
            TokenKind::Dot => "'.'",
            TokenKind::Comma => "','",
            TokenKind::Semi => "';'",
            TokenKind::Colon => "':'",
            TokenKind::LParen => "'('",
            TokenKind::RParen => "')'",
            TokenKind::LBrack => "'['",
            TokenKind::RBrack => "']'",
            TokenKind::LBrace => "'{'",
            TokenKind::RBrace => "'}'",
            TokenKind::Caret => "'^'",
            TokenKind::Pipe => "'|'",
            TokenKind::Ampersand => "'&'",
            TokenKind::Tilde => "'~'",
            TokenKind::Pragma(_, _) => "pragma",
            TokenKind::DocComment(_) => "doc comment",
            TokenKind::Eof => "end of file",
        }
    }
}
