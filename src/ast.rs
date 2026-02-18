use crate::errors::SourceLoc;

pub type Ident = String;

#[derive(Debug, Clone)]
pub struct QualIdent {
    pub module: Option<Ident>,
    pub name: Ident,
    pub loc: SourceLoc,
}

// ── Compilation unit ────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum CompilationUnit {
    ProgramModule(ProgramModule),
    DefinitionModule(DefinitionModule),
    ImplementationModule(ImplementationModule),
}

#[derive(Debug, Clone)]
pub struct ProgramModule {
    pub name: Ident,
    pub priority: Option<Box<Expr>>,
    pub imports: Vec<Import>,
    pub block: Block,
    pub is_safe: bool,
    pub is_unsafe: bool,
    pub loc: SourceLoc,
}

#[derive(Debug, Clone)]
pub struct DefinitionModule {
    pub name: Ident,
    pub imports: Vec<Import>,
    pub export: Option<Export>,
    pub definitions: Vec<Definition>,
    pub is_safe: bool,
    pub is_unsafe: bool,
    pub foreign_lang: Option<String>,
    pub loc: SourceLoc,
}

#[derive(Debug, Clone)]
pub struct ImplementationModule {
    pub name: Ident,
    pub priority: Option<Box<Expr>>,
    pub imports: Vec<Import>,
    pub block: Block,
    pub is_safe: bool,
    pub is_unsafe: bool,
    pub loc: SourceLoc,
}

// ── Imports / Exports ───────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Import {
    pub from_module: Option<Ident>,
    pub names: Vec<Ident>,
    pub loc: SourceLoc,
}

#[derive(Debug, Clone)]
pub struct Export {
    pub qualified: bool,
    pub names: Vec<Ident>,
    pub loc: SourceLoc,
}

// ── Definitions (in DEFINITION MODULE) ──────────────────────────────

#[derive(Debug, Clone)]
pub enum Definition {
    Const(ConstDecl),
    Type(TypeDecl),
    Var(VarDecl),
    Procedure(ProcHeading),
    /// Modula-2+ exception declaration in definition modules
    Exception(ExceptionDecl),
}

// ── Declarations ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Block {
    pub decls: Vec<Declaration>,
    pub body: Option<Vec<Statement>>,
    pub finally: Option<Vec<Statement>>,
    pub except: Option<Vec<Statement>>,
    pub loc: SourceLoc,
}

#[derive(Debug, Clone)]
pub enum Declaration {
    Const(ConstDecl),
    Type(TypeDecl),
    Var(VarDecl),
    Procedure(ProcDecl),
    Module(ProgramModule),
    /// Modula-2+ exception declaration: EXCEPTION Foo;
    Exception(ExceptionDecl),
}

#[derive(Debug, Clone)]
pub struct ExceptionDecl {
    pub name: Ident,
    pub loc: SourceLoc,
}

#[derive(Debug, Clone)]
pub struct ConstDecl {
    pub name: Ident,
    pub expr: Expr,
    pub loc: SourceLoc,
}

#[derive(Debug, Clone)]
pub struct TypeDecl {
    pub name: Ident,
    pub typ: Option<TypeNode>, // None = opaque type in DEFINITION MODULE
    pub loc: SourceLoc,
}

#[derive(Debug, Clone)]
pub struct VarDecl {
    pub names: Vec<Ident>,
    pub typ: TypeNode,
    pub loc: SourceLoc,
}

#[derive(Debug, Clone)]
pub struct ProcDecl {
    pub heading: ProcHeading,
    pub block: Block,
    pub loc: SourceLoc,
}

#[derive(Debug, Clone)]
pub struct ProcHeading {
    pub name: Ident,
    pub params: Vec<FormalParam>,
    pub return_type: Option<Box<TypeNode>>,
    /// Modula-2+ RAISES clause: list of exception names this proc may raise
    pub raises: Option<Vec<QualIdent>>,
    /// (*$EXPORTC "name"*) — emit procedure with C linkage under this name
    pub export_c_name: Option<String>,
    pub loc: SourceLoc,
}

#[derive(Debug, Clone)]
pub struct FormalParam {
    pub is_var: bool,
    pub names: Vec<Ident>,
    pub typ: TypeNode,
    pub loc: SourceLoc,
}

// ── Type nodes ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum TypeNode {
    Named(QualIdent),
    Array {
        index_types: Vec<TypeNode>,
        elem_type: Box<TypeNode>,
        loc: SourceLoc,
    },
    OpenArray {
        elem_type: Box<TypeNode>,
        loc: SourceLoc,
    },
    Record {
        fields: Vec<FieldList>,
        loc: SourceLoc,
    },
    Pointer {
        base: Box<TypeNode>,
        loc: SourceLoc,
    },
    Set {
        base: Box<TypeNode>,
        loc: SourceLoc,
    },
    Enumeration {
        variants: Vec<Ident>,
        loc: SourceLoc,
    },
    Subrange {
        low: Box<Expr>,
        high: Box<Expr>,
        loc: SourceLoc,
    },
    ProcedureType {
        params: Vec<FormalParam>,
        return_type: Option<Box<TypeNode>>,
        loc: SourceLoc,
    },
    /// Modula-2+ REF type (traced/GC reference)
    Ref {
        target: Box<TypeNode>,
        branded: Option<String>,
        loc: SourceLoc,
    },
    /// Modula-2+ REFANY built-in type
    RefAny {
        loc: SourceLoc,
    },
    /// Modula-2+ OBJECT type
    Object {
        parent: Option<QualIdent>,
        fields: Vec<Field>,
        methods: Vec<MethodDecl>,
        overrides: Vec<OverrideDecl>,
        loc: SourceLoc,
    },
}

#[derive(Debug, Clone)]
pub struct MethodDecl {
    pub name: Ident,
    pub params: Vec<FormalParam>,
    pub return_type: Option<Box<TypeNode>>,
    pub loc: SourceLoc,
}

#[derive(Debug, Clone)]
pub struct OverrideDecl {
    pub name: Ident,
    pub proc_name: QualIdent,
    pub loc: SourceLoc,
}

#[derive(Debug, Clone)]
pub struct FieldList {
    pub fixed: Vec<Field>,
    pub variant: Option<VariantPart>,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub names: Vec<Ident>,
    pub typ: TypeNode,
    pub loc: SourceLoc,
}

#[derive(Debug, Clone)]
pub struct VariantPart {
    pub tag_name: Option<Ident>,
    pub tag_type: QualIdent,
    pub variants: Vec<Variant>,
    pub loc: SourceLoc,
}

#[derive(Debug, Clone)]
pub struct Variant {
    pub labels: Vec<CaseLabel>,
    pub fields: Vec<FieldList>,
    pub loc: SourceLoc,
}

#[derive(Debug, Clone)]
pub enum CaseLabel {
    Single(Expr),
    Range(Expr, Expr),
}

// ── Statements ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Statement {
    pub kind: StatementKind,
    pub loc: SourceLoc,
}

#[derive(Debug, Clone)]
pub enum StatementKind {
    Empty,
    Assign {
        desig: Designator,
        expr: Expr,
    },
    ProcCall {
        desig: Designator,
        args: Vec<Expr>,
    },
    If {
        cond: Expr,
        then_body: Vec<Statement>,
        elsifs: Vec<(Expr, Vec<Statement>)>,
        else_body: Option<Vec<Statement>>,
    },
    Case {
        expr: Expr,
        branches: Vec<CaseBranch>,
        else_body: Option<Vec<Statement>>,
    },
    While {
        cond: Expr,
        body: Vec<Statement>,
    },
    Repeat {
        body: Vec<Statement>,
        cond: Expr,
    },
    For {
        var: Ident,
        start: Expr,
        end: Expr,
        step: Option<Expr>,
        body: Vec<Statement>,
    },
    Loop {
        body: Vec<Statement>,
    },
    With {
        desig: Designator,
        body: Vec<Statement>,
    },
    Return {
        expr: Option<Expr>,
    },
    Exit,
    /// ISO: RAISE expression — raise an exception
    Raise {
        expr: Option<Expr>,
    },
    /// ISO: RETRY — re-execute the body from the EXCEPT handler
    Retry,
    /// Modula-2+ TRY/EXCEPT/FINALLY
    Try {
        body: Vec<Statement>,
        excepts: Vec<ExceptClause>,
        finally_body: Option<Vec<Statement>>,
    },
    /// Modula-2+ LOCK mutex DO stmts END
    Lock {
        mutex: Expr,
        body: Vec<Statement>,
    },
    /// Modula-2+ TYPECASE
    TypeCase {
        expr: Expr,
        branches: Vec<TypeCaseBranch>,
        else_body: Option<Vec<Statement>>,
    },
}

#[derive(Debug, Clone)]
pub struct ExceptClause {
    /// The exception name to catch (None = catch all)
    pub exception: Option<QualIdent>,
    /// Optional variable binding for the exception value
    pub var: Option<Ident>,
    pub body: Vec<Statement>,
    pub loc: SourceLoc,
}

#[derive(Debug, Clone)]
pub struct TypeCaseBranch {
    pub types: Vec<QualIdent>,
    pub var: Option<Ident>,
    pub body: Vec<Statement>,
    pub loc: SourceLoc,
}

#[derive(Debug, Clone)]
pub struct CaseBranch {
    pub labels: Vec<CaseLabel>,
    pub body: Vec<Statement>,
    pub loc: SourceLoc,
}

// ── Expressions ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub loc: SourceLoc,
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    IntLit(i64),
    RealLit(f64),
    StringLit(String),
    CharLit(char),
    BoolLit(bool),
    NilLit,
    Designator(Designator),
    FuncCall {
        desig: Designator,
        args: Vec<Expr>,
    },
    UnaryOp {
        op: UnaryOp,
        operand: Box<Expr>,
    },
    BinaryOp {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    SetConstructor {
        base_type: Option<QualIdent>,
        elements: Vec<SetElement>,
    },
    Not(Box<Expr>),
}

#[derive(Debug, Clone)]
pub enum SetElement {
    Single(Expr),
    Range(Expr, Expr),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Pos,
    Neg,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    RealDiv,
    IntDiv,
    Mod,
    And,
    Or,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    In,
}

// ── Designators ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Designator {
    pub ident: QualIdent,
    pub selectors: Vec<Selector>,
    pub loc: SourceLoc,
}

#[derive(Debug, Clone)]
pub enum Selector {
    Field(Ident, SourceLoc),
    Index(Vec<Expr>, SourceLoc),
    Deref(SourceLoc),
}
