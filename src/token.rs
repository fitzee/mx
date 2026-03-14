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
    As,
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
    pub fn keyword_from_str(s: &str, m2plus: bool) -> Option<TokenKind> {
        // PIM4 keywords — always recognized
        match s {
            "AND" => return Some(TokenKind::And),
            "ARRAY" => return Some(TokenKind::Array),
            "BEGIN" => return Some(TokenKind::Begin),
            "BY" => return Some(TokenKind::By),
            "CASE" => return Some(TokenKind::Case),
            "CONST" => return Some(TokenKind::Const),
            "DEFINITION" => return Some(TokenKind::Definition),
            "DIV" => return Some(TokenKind::Div),
            "DO" => return Some(TokenKind::Do),
            "ELSE" => return Some(TokenKind::Else),
            "ELSIF" => return Some(TokenKind::Elsif),
            "END" => return Some(TokenKind::End),
            "EXIT" => return Some(TokenKind::Exit),
            "EXPORT" => return Some(TokenKind::Export),
            "FOR" => return Some(TokenKind::For),
            "FROM" => return Some(TokenKind::From),
            "IF" => return Some(TokenKind::If),
            "IMPLEMENTATION" => return Some(TokenKind::Implementation),
            "IMPORT" => return Some(TokenKind::Import),
            "IN" => return Some(TokenKind::In),
            "LOOP" => return Some(TokenKind::Loop),
            "MOD" => return Some(TokenKind::Mod),
            "MODULE" => return Some(TokenKind::Module),
            "NOT" => return Some(TokenKind::Not),
            "OF" => return Some(TokenKind::Of),
            "OR" => return Some(TokenKind::Or),
            "POINTER" => return Some(TokenKind::Pointer),
            "PROCEDURE" => return Some(TokenKind::Procedure),
            "QUALIFIED" => return Some(TokenKind::Qualified),
            "RECORD" => return Some(TokenKind::Record),
            "REPEAT" => return Some(TokenKind::Repeat),
            "RETURN" => return Some(TokenKind::Return),
            "SET" => return Some(TokenKind::Set),
            "THEN" => return Some(TokenKind::Then),
            "TO" => return Some(TokenKind::To),
            "TYPE" => return Some(TokenKind::Type),
            "UNTIL" => return Some(TokenKind::Until),
            "VAR" => return Some(TokenKind::Var),
            "WHILE" => return Some(TokenKind::While),
            "WITH" => return Some(TokenKind::With),
            _ => {}
        }
        // M2+ / extension keywords — only recognized in m2plus mode
        if m2plus {
            match s {
                "AS" => return Some(TokenKind::As),
                "BRANDED" => return Some(TokenKind::Branded),
                "EXCEPT" => return Some(TokenKind::Except),
                "EXCEPTION" => return Some(TokenKind::Exception),
                "FINALLY" => return Some(TokenKind::Finally),
                "LOCK" => return Some(TokenKind::Lock),
                "METHODS" => return Some(TokenKind::Methods),
                "OBJECT" => return Some(TokenKind::Object),
                "OVERRIDE" => return Some(TokenKind::Override),
                "RAISE" => return Some(TokenKind::Raise),
                "REF" => return Some(TokenKind::Ref),
                "REFANY" => return Some(TokenKind::Refany),
                "RETRY" => return Some(TokenKind::Retry),
                "REVEAL" => return Some(TokenKind::Reveal),
                "SAFE" => return Some(TokenKind::Safe),
                "TRY" => return Some(TokenKind::Try),
                "TYPECASE" => return Some(TokenKind::Typecase),
                "UNSAFE" => return Some(TokenKind::Unsafe),
                _ => {}
            }
        }
        None
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
            TokenKind::As => "'AS'",
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
