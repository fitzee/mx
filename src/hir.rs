//! High-level Intermediate Representation (HIR)
//!
//! Phase 1: Place expressions — flattened, resolved designator chains
//! with TypeIds at every step. No name resolution or type inference
//! needed in backends.
//!
//! Phase 2: Closure analysis — unified free variable computation.
//! `CapturedVar` replaces both backends' independent capture detection.

use crate::types::TypeId;
use crate::errors::SourceLoc;

// ── Place expressions ───────────────────────────────────────────────

/// A fully resolved place expression (lvalue). Replaces the AST
/// `Designator` + selector chain with explicit projections carrying
/// resolved field indices and TypeIds.
#[derive(Debug, Clone)]
pub struct Place {
    pub base: PlaceBase,
    pub projections: Vec<Projection>,
    /// The TypeId of the final resolved type (after all projections).
    pub ty: TypeId,
    pub loc: SourceLoc,
}

/// The root of a place expression.
#[derive(Debug, Clone)]
pub enum PlaceBase {
    /// A local variable in the current procedure.
    Local(SymbolId),
    /// A module-level (global) variable.
    Global(SymbolId),
    /// A constant value (inlined, no address).
    Constant(ConstVal),
    /// A procedure / function reference.
    FuncRef(SymbolId),
}

/// A resolved symbol identifier — carries both the mangled name
/// (ready for emission) and the original source name (for debug info).
#[derive(Debug, Clone)]
pub struct SymbolId {
    /// Mangled name ready for codegen: "Module_Proc", "Module_var", etc.
    pub mangled: String,
    /// Original source name for debug info / diagnostics.
    pub source_name: String,
    /// Owning module (None for locals in current module).
    pub module: Option<String>,
    /// Semantic TypeId of the symbol.
    pub ty: TypeId,
    /// True if this is a VAR parameter (needs extra indirection).
    pub is_var_param: bool,
    /// True if this is an open array parameter.
    pub is_open_array: bool,
}

/// A single projection step in a place expression.
#[derive(Debug, Clone)]
pub struct Projection {
    pub kind: ProjectionKind,
    /// TypeId of the result after this projection.
    pub ty: TypeId,
}

#[derive(Debug, Clone)]
pub enum ProjectionKind {
    /// Record field access: resolved index + field name (for debug).
    Field {
        index: usize,
        name: String,
        /// TypeId of the record being projected through.
        record_ty: TypeId,
    },
    /// Array index (HIR expression).
    Index(Box<HirExpr>),
    /// Pointer dereference.
    Deref,
    /// Variant record field: variant_index selects the variant arm,
    /// field_index selects the field within that variant.
    VariantField {
        variant_index: usize,
        field_index: usize,
        name: String,
        record_ty: TypeId,
    },
}

/// A constant value resolved during HIR construction.
#[derive(Debug, Clone)]
pub enum ConstVal {
    Integer(i64),
    Real(f64),
    Boolean(bool),
    Char(char),
    String(String),
    Set(u64),
    Nil,
    EnumVariant(i64),
}

// ── Expressions (Phase 3) ───────────────────────────────────────────

/// A typed HIR expression. Every expression carries its resolved TypeId.
#[derive(Debug, Clone)]
pub struct HirExpr {
    pub kind: HirExprKind,
    /// Resolved type of this expression.
    pub ty: TypeId,
    pub loc: SourceLoc,
}

#[derive(Debug, Clone)]
pub enum HirExprKind {
    /// Integer literal.
    IntLit(i64),
    /// Real literal.
    RealLit(f64),
    /// String literal — carries the actual content so backends can
    /// intern into their own string pools.
    StringLit(String),
    /// Character literal.
    CharLit(char),
    /// Boolean literal.
    BoolLit(bool),
    /// NIL pointer.
    NilLit,

    /// Place expression (variable, field, array element, deref, etc.).
    Place(Place),

    /// Direct call to a known procedure (resolved at HIR time).
    DirectCall {
        target: SymbolId,
        args: Vec<HirExpr>,
    },
    /// Indirect call through a procedure variable.
    IndirectCall {
        callee: Box<HirExpr>,
        args: Vec<HirExpr>,
    },

    /// Unary operator.
    UnaryOp {
        op: crate::ast::UnaryOp,
        operand: Box<HirExpr>,
    },
    /// Binary operator.
    BinaryOp {
        op: crate::ast::BinaryOp,
        left: Box<HirExpr>,
        right: Box<HirExpr>,
    },
    /// SET constructor.
    SetConstructor {
        elements: Vec<HirSetElement>,
    },
    /// Logical NOT.
    Not(Box<HirExpr>),
    /// Pointer dereference as expression (Modula-2+ expr^).
    Deref(Box<HirExpr>),
    /// Address-of a place (for VAR parameter passing).
    /// The backend should emit the address, not load the value.
    AddrOf(Place),
    /// Type transfer (cast): T(expr) where T is a type name.
    /// The target TypeId is in the parent HirExpr.ty.
    TypeTransfer(Box<HirExpr>),
}

/// Set element in a SET constructor.
#[derive(Debug, Clone)]
pub enum HirSetElement {
    Single(HirExpr),
    Range(HirExpr, HirExpr),
}

/// Index into a module's string pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StringId(pub usize);

// ── FOR direction (Phase 3) ─────────────────────────────────────────

/// Resolved FOR loop direction — computed once during HIR lowering,
/// not re-derived by each backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForDirection {
    Up,
    Down,
}

// ── Statements (Phase 4) ────────────────────────────────────────────

/// An HIR statement. WITH is eliminated (desugared to Place projections).
/// FOR carries an explicit direction. All designators are resolved Places.
#[derive(Debug, Clone)]
pub struct HirStmt {
    pub kind: HirStmtKind,
    pub loc: SourceLoc,
}

#[derive(Debug, Clone)]
pub enum HirStmtKind {
    /// No-op.
    Empty,

    /// Assignment: place := expr.
    Assign {
        target: Place,
        value: HirExpr,
    },

    /// Procedure call (no return value used).
    ProcCall {
        target: HirCallTarget,
        args: Vec<HirExpr>,
    },

    /// IF / ELSIF / ELSE.
    If {
        cond: HirExpr,
        then_body: Vec<HirStmt>,
        elsifs: Vec<(HirExpr, Vec<HirStmt>)>,
        else_body: Option<Vec<HirStmt>>,
    },

    /// CASE expr OF branches [ELSE] END.
    Case {
        expr: HirExpr,
        branches: Vec<HirCaseBranch>,
        else_body: Option<Vec<HirStmt>>,
    },

    /// WHILE cond DO body END.
    While {
        cond: HirExpr,
        body: Vec<HirStmt>,
    },

    /// REPEAT body UNTIL cond.
    Repeat {
        body: Vec<HirStmt>,
        cond: HirExpr,
    },

    /// FOR var := start TO/DOWNTO end [BY step] DO body END.
    /// Direction is pre-computed; backends just read it.
    For {
        var: String,
        var_ty: TypeId,
        start: HirExpr,
        end: HirExpr,
        step: Option<HirExpr>,
        direction: ForDirection,
        body: Vec<HirStmt>,
    },

    /// LOOP body END (infinite loop, exited with EXIT).
    Loop {
        body: Vec<HirStmt>,
    },

    // Note: no WITH variant — eliminated during HIR lowering.

    /// RETURN [expr].
    Return {
        expr: Option<HirExpr>,
    },

    /// EXIT (break from LOOP).
    Exit,

    /// ISO RAISE [expr].
    Raise {
        expr: Option<HirExpr>,
    },

    /// ISO RETRY.
    Retry,

    /// M2+ TRY / EXCEPT / FINALLY.
    Try {
        body: Vec<HirStmt>,
        excepts: Vec<HirExceptClause>,
        finally_body: Option<Vec<HirStmt>>,
    },

    /// M2+ LOCK mutex DO body END.
    Lock {
        mutex: HirExpr,
        body: Vec<HirStmt>,
    },

    /// M2+ TYPECASE.
    TypeCase {
        expr: HirExpr,
        branches: Vec<HirTypeCaseBranch>,
        else_body: Option<Vec<HirStmt>>,
    },
}

/// Call target for a procedure call statement.
#[derive(Debug, Clone)]
pub enum HirCallTarget {
    /// Direct call to a known procedure.
    Direct(SymbolId),
    /// Indirect call through a procedure variable / expression.
    Indirect(HirExpr),
}

/// CASE branch with resolved label values.
#[derive(Debug, Clone)]
pub struct HirCaseBranch {
    pub labels: Vec<HirCaseLabel>,
    pub body: Vec<HirStmt>,
}

/// CASE label — resolved to concrete integer values where possible.
#[derive(Debug, Clone)]
pub enum HirCaseLabel {
    Single(HirExpr),
    Range(HirExpr, HirExpr),
}

/// TRY/EXCEPT clause.
#[derive(Debug, Clone)]
pub struct HirExceptClause {
    pub exception: Option<SymbolId>,
    pub var: Option<String>,
    pub body: Vec<HirStmt>,
}

/// TYPECASE branch.
#[derive(Debug, Clone)]
pub struct HirTypeCaseBranch {
    pub types: Vec<SymbolId>,
    pub var: Option<String>,
    pub body: Vec<HirStmt>,
}

// ── Module structure (Phase 5) ───────────────────────────────────────

/// Top-level HIR module — the complete lowered representation of a
/// compilation unit. Both backends consume `&HirModule` and never
/// need to touch the AST directly.
#[derive(Debug, Clone)]
pub struct HirModule {
    pub name: String,
    pub source_file: String,
    /// Interned string pool. Backends emit these as global constants.
    pub string_pool: Vec<String>,
    /// Module-level constants.
    pub constants: Vec<HirConst>,
    /// Module-level type declarations (name → TypeId).
    pub types: Vec<HirTypeDecl>,
    /// Module-level variables.
    pub globals: Vec<HirVar>,
    /// Procedures (including nested, already lifted).
    pub procedures: Vec<HirProc>,
    /// Module initialization body (the main block's statements).
    pub init_body: Option<Vec<HirStmt>>,
    /// Embedded module init bodies (module_name → lowered statements).
    pub embedded_init_bodies: Vec<(String, Vec<HirStmt>)>,
    /// External declarations from imported modules.
    pub externals: Vec<HirExternal>,
}

/// A module-level constant.
#[derive(Debug, Clone)]
pub struct HirConst {
    pub name: SymbolId,
    pub value: ConstVal,
    pub ty: TypeId,
}

/// A module-level type declaration.
#[derive(Debug, Clone)]
pub struct HirTypeDecl {
    pub name: String,
    pub ty: TypeId,
    pub exported: bool,
}

/// A module-level variable.
#[derive(Debug, Clone)]
pub struct HirVar {
    pub name: SymbolId,
    pub ty: TypeId,
    pub exported: bool,
}

/// A procedure in the HIR.
#[derive(Debug, Clone)]
pub struct HirProc {
    pub name: SymbolId,
    pub params: Vec<HirParam>,
    pub return_type: Option<TypeId>,
    pub captures: Vec<CapturedVar>,
    pub locals: Vec<HirVar>,
    pub body: Option<Vec<HirStmt>>,
    pub nested_procs: Vec<HirProc>,
    pub is_exported: bool,
}

/// A procedure parameter.
#[derive(Debug, Clone)]
pub struct HirParam {
    pub name: String,
    pub ty: TypeId,
    pub is_var: bool,
    pub is_open_array: bool,
}

/// An external symbol imported from another module.
#[derive(Debug, Clone)]
pub struct HirExternal {
    pub name: SymbolId,
    pub kind: HirExternalKind,
}

#[derive(Debug, Clone)]
pub enum HirExternalKind {
    Variable(TypeId),
    Procedure {
        params: Vec<HirParam>,
        return_type: Option<TypeId>,
    },
    Type(TypeId),
    Constant(ConstVal, TypeId),
}

// ── Closure analysis (Phase 2) ──────────────────────────────────────

/// A variable captured by a nested procedure from an enclosing scope.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CapturedVar {
    /// Variable name as it appears in source.
    pub name: String,
    /// TypeId of the captured variable.
    pub ty: TypeId,
    /// True if this is a `_high` companion auto-captured for an open array.
    pub is_high_companion: bool,
}
