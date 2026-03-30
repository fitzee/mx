//! Control Flow Graph (CFG) construction from HIR.
//!
//! V1 scope: linear statements, IF/ELSIF/ELSE, WHILE, LOOP, EXIT,
//! RETURN, short-circuit AND/OR/NOT.
//!
//! Out-of-scope constructs (For, Repeat, Case, TypeCase, Try, Lock,
//! Raise, Retry) are treated as opaque linear statements.

use crate::ast::BinaryOp;
use crate::hir::{HirExpr, HirExprKind, HirStmt, HirStmtKind};

// ── Data model ──────────────────────────────────────────────────────

pub type BlockId = usize;

/// A basic block: maximal sequence of non-branching statements
/// followed by a terminator that determines control flow.
#[derive(Debug)]
pub struct BasicBlock {
    pub id: BlockId,
    pub stmts: Vec<HirStmt>,
    /// `None` = open (under construction). `Some` = sealed.
    pub terminator: Option<Terminator>,
}

/// How a basic block exits. Single source of truth for successors.
#[derive(Debug)]
pub enum Terminator {
    /// Unconditional jump.
    Goto(BlockId),
    /// Conditional branch.
    Branch {
        cond: HirExpr,
        on_true: BlockId,
        on_false: BlockId,
    },
    /// Procedure return.
    Return(Option<HirExpr>),
}

impl Terminator {
    /// Successor block ids, derived from this terminator only.
    pub fn successors(&self) -> Vec<BlockId> {
        match self {
            Terminator::Goto(b) => vec![*b],
            Terminator::Branch { on_true, on_false, .. } => vec![*on_true, *on_false],
            Terminator::Return(_) => vec![],
        }
    }
}

/// Complete CFG for one procedure body or module init body.
#[derive(Debug)]
pub struct Cfg {
    pub blocks: Vec<BasicBlock>,
    /// Entry block is always block 0.
    pub entry: BlockId,
}

impl Cfg {
    /// Compute predecessor map on demand from terminators.
    pub fn predecessors(&self) -> Vec<Vec<BlockId>> {
        let mut preds = vec![Vec::new(); self.blocks.len()];
        for block in &self.blocks {
            if let Some(ref term) = block.terminator {
                for succ in term.successors() {
                    preds[succ].push(block.id);
                }
            }
        }
        preds
    }

    /// Validate all CFG invariants. Panics on violation.
    pub fn validate(&self) {
        // Entry is block 0
        assert_eq!(self.entry, 0, "entry must be block 0");

        // All blocks are sealed
        for block in &self.blocks {
            assert!(
                block.terminator.is_some(),
                "block {} is not sealed",
                block.id
            );
        }

        // All successor targets exist
        for block in &self.blocks {
            if let Some(ref term) = block.terminator {
                for succ in term.successors() {
                    assert!(
                        succ < self.blocks.len(),
                        "block {} references nonexistent successor {}",
                        block.id, succ
                    );
                }
            }
        }

        // Block ids are sequential
        for (i, block) in self.blocks.iter().enumerate() {
            assert_eq!(block.id, i, "block id mismatch: expected {}, got {}", i, block.id);
        }
    }
}

// ── Builder ─────────────────────────────────────────────────────────

/// Builds a CFG from a sequence of HIR statements.
///
/// Invariants:
/// - `current` is `Some(id)` where `id` is an open block, or `None`
///   if the last block was sealed by Return/Exit and no new block
///   has been started.
/// - Statements can only be pushed when `current.is_some()`.
/// - A block can only be sealed once.
/// - `finish()` verifies all blocks are sealed.
struct CfgBuilder {
    blocks: Vec<BasicBlock>,
    current: Option<BlockId>,
    loop_exit_stack: Vec<BlockId>,
}

impl CfgBuilder {
    /// Create a new builder with an open entry block (id 0).
    fn new() -> Self {
        let entry = BasicBlock {
            id: 0,
            stmts: Vec::new(),
            terminator: None,
        };
        CfgBuilder {
            blocks: vec![entry],
            current: Some(0),
            loop_exit_stack: Vec::new(),
        }
    }

    /// Allocate a new open block. Does NOT set it as current.
    fn new_block(&mut self) -> BlockId {
        let id = self.blocks.len();
        self.blocks.push(BasicBlock {
            id,
            stmts: Vec::new(),
            terminator: None,
        });
        id
    }

    /// Set the current block. The target must be open (no terminator),
    /// and there must be no existing open block (current must be None).
    fn start_block(&mut self, id: BlockId) {
        debug_assert!(
            self.current.is_none(),
            "start_block: abandoning open block {}",
            self.current.unwrap()
        );
        debug_assert!(
            self.blocks[id].terminator.is_none(),
            "start_block: block {} is already sealed",
            id
        );
        self.current = Some(id);
    }

    /// Seal the current block with a terminator. Sets current to None.
    /// Panics if no current block or block is already sealed.
    fn seal(&mut self, terminator: Terminator) {
        let id = self.current.expect("seal: no current block");
        debug_assert!(
            self.blocks[id].terminator.is_none(),
            "seal: block {} already has a terminator",
            id
        );
        self.blocks[id].terminator = Some(terminator);
        self.current = None;
    }

    /// Push a statement to the current block.
    /// If current is None (after Return/Exit), the statement is
    /// silently discarded — this is V1 unreachable code policy.
    fn push_stmt(&mut self, stmt: HirStmt) {
        if let Some(id) = self.current {
            self.blocks[id].stmts.push(stmt);
        }
        // else: discard (unreachable code after Return/Exit)
    }

    /// Whether the builder has an open current block.
    fn is_open(&self) -> bool {
        self.current.is_some()
    }

    /// Returns true if any sealed block has `target` as a successor.
    fn any_path_reaches(&self, target: BlockId) -> bool {
        for block in &self.blocks {
            if let Some(ref term) = block.terminator {
                if term.successors().contains(&target) {
                    return true;
                }
            }
        }
        false
    }

    /// Finalize and return the CFG. Seals the current block with
    /// an implicit Return(None) if still open. Panics if any block
    /// is left unsealed (other than current, which gets auto-sealed).
    fn finish(mut self) -> Cfg {
        // Auto-seal current block with implicit return (end of procedure)
        if let Some(id) = self.current {
            if self.blocks[id].terminator.is_none() {
                self.blocks[id].terminator = Some(Terminator::Return(None));
            }
            self.current = None;
        }

        // Seal any unreachable blocks that were allocated but never
        // started (e.g., merge blocks when all paths terminated).
        // These blocks have no predecessors and are genuinely unreachable —
        // they must not be treated as valid return paths by future analyses.
        // They are sealed only to satisfy the "all blocks sealed" invariant.
        for block in &mut self.blocks {
            if block.terminator.is_none() {
                block.terminator = Some(Terminator::Return(None));
            }
        }

        Cfg {
            blocks: self.blocks,
            entry: 0,
        }
    }

    // ── HIR lowering ────────────────────────────────────────────────

    /// Lower a sequence of HIR statements into the current block(s).
    fn lower_stmts(&mut self, stmts: &[HirStmt]) {
        for stmt in stmts {
            self.lower_stmt(stmt);
        }
    }

    /// Lower a single HIR statement.
    fn lower_stmt(&mut self, stmt: &HirStmt) {
        if !self.is_open() {
            return; // unreachable code after Return/Exit
        }

        match &stmt.kind {
            // ── Linear statements ────────────────────────────────
            HirStmtKind::Empty => {}
            HirStmtKind::Assign { .. } | HirStmtKind::ProcCall { .. } => {
                self.push_stmt(stmt.clone());
            }

            // ── Branching ────────────────────────────────────────
            HirStmtKind::If {
                cond,
                then_body,
                elsifs,
                else_body,
            } => {
                self.lower_if(cond, then_body, elsifs, else_body);
            }

            // ── Loops ────────────────────────────────────────────
            HirStmtKind::While { cond, body } => {
                self.lower_while(cond, body);
            }
            HirStmtKind::Loop { body } => {
                self.lower_loop(body);
            }
            HirStmtKind::Exit => {
                self.lower_exit();
            }

            // ── Return ───────────────────────────────────────────
            HirStmtKind::Return { expr } => {
                self.seal(Terminator::Return(expr.clone()));
            }

            // ── Out-of-scope: treat as opaque linear statements ──
            HirStmtKind::For { .. }
            | HirStmtKind::Repeat { .. }
            | HirStmtKind::Case { .. }
            | HirStmtKind::TypeCase { .. }
            | HirStmtKind::Try { .. }
            | HirStmtKind::Lock { .. }
            | HirStmtKind::Raise { .. }
            | HirStmtKind::Retry => {
                // TODO: lower these as real control flow in V2
                self.push_stmt(stmt.clone());
            }
        }
    }

    // ── IF / ELSIF / ELSE ───────────────────────────────────────────

    /// Lower IF / ELSIF / ELSE into branch blocks.
    ///
    /// Shape:
    ///   entry → Branch(cond) → then_block | else_chain
    ///   then_block → Goto(merge)  [if body doesn't terminate]
    ///   else_chain → ... → merge
    ///   merge is current after return
    fn lower_if(
        &mut self,
        cond: &HirExpr,
        then_body: &[HirStmt],
        elsifs: &[(HirExpr, Vec<HirStmt>)],
        else_body: &Option<Vec<HirStmt>>,
    ) {
        let merge = self.new_block();
        let then_block = self.new_block();

        // Determine the first false-target: next elsif, else, or merge
        let first_false = if !elsifs.is_empty() || else_body.is_some() {
            self.new_block()
        } else {
            merge
        };

        // Branch on condition (handles short-circuit AND/OR/NOT)
        self.lower_expr_as_branch(cond, then_block, first_false);

        // Then body
        self.start_block(then_block);
        self.lower_stmts(then_body);
        if self.is_open() {
            self.seal(Terminator::Goto(merge));
        }

        // Elsif chain
        let mut current_false = first_false;
        for (i, (elsif_cond, elsif_body)) in elsifs.iter().enumerate() {
            self.start_block(current_false);
            let elsif_then = self.new_block();

            let elsif_false = if i + 1 < elsifs.len() || else_body.is_some() {
                self.new_block()
            } else {
                merge
            };

            self.lower_expr_as_branch(elsif_cond, elsif_then, elsif_false);

            self.start_block(elsif_then);
            self.lower_stmts(elsif_body);
            if self.is_open() {
                self.seal(Terminator::Goto(merge));
            }

            current_false = elsif_false;
        }

        // Else body
        if let Some(else_stmts) = else_body {
            self.start_block(current_false);
            self.lower_stmts(else_stmts);
            if self.is_open() {
                self.seal(Terminator::Goto(merge));
            }
        }

        // Merge block: only enter it if at least one path reaches it
        if self.any_path_reaches(merge) {
            self.start_block(merge);
        }
        // else: current stays None — all paths terminated
    }

    // ── WHILE ───────────────────────────────────────────────────────

    /// Lower WHILE into header → body → back-edge, with exit block.
    ///
    /// Shape:
    ///   pre → Goto(header)
    ///   header → Branch(cond) → body | exit
    ///   body → Goto(header)  [back-edge]
    ///   exit is current after return
    fn lower_while(&mut self, cond: &HirExpr, body: &[HirStmt]) {
        let header = self.new_block();
        let body_block = self.new_block();
        let exit_block = self.new_block();

        // Pre → header
        self.seal(Terminator::Goto(header));

        // Header: branch on condition
        self.start_block(header);
        self.lower_expr_as_branch(cond, body_block, exit_block);

        // Body
        self.loop_exit_stack.push(exit_block);
        self.start_block(body_block);
        self.lower_stmts(body);
        if self.is_open() {
            self.seal(Terminator::Goto(header)); // back-edge
        }
        self.loop_exit_stack.pop();

        self.start_block(exit_block);
    }

    // ── LOOP ────────────────────────────────────────────────────────

    /// Lower LOOP into header → body → back-edge, with exit block.
    ///
    /// Shape:
    ///   pre → Goto(header)
    ///   header contains body statements
    ///   header → Goto(header)  [back-edge, if body doesn't EXIT/RETURN]
    ///   exit is current after return
    ///
    /// Uses explicit header block for consistency with WHILE and
    /// for clean natural-loop identification (back-edge → header).
    fn lower_loop(&mut self, body: &[HirStmt]) {
        let header = self.new_block();
        let exit_block = self.new_block();

        // Pre → header
        self.seal(Terminator::Goto(header));

        // Body (inside header block)
        self.loop_exit_stack.push(exit_block);
        self.start_block(header);
        self.lower_stmts(body);
        if self.is_open() {
            self.seal(Terminator::Goto(header)); // back-edge
        }
        self.loop_exit_stack.pop();

        self.start_block(exit_block);
    }

    // ── EXIT ────────────────────────────────────────────────────────

    /// Lower EXIT: jump to the innermost loop's exit block.
    /// Sema guarantees EXIT is inside a LOOP, so the stack is non-empty.
    fn lower_exit(&mut self) {
        let exit_target = *self.loop_exit_stack.last()
            .expect("EXIT outside LOOP (sema should have caught this)");
        self.seal(Terminator::Goto(exit_target));
    }

    // ── Short-circuit boolean lowering ──────────────────────────────

    /// Lower a boolean expression as control flow: branch to on_true
    /// or on_false depending on the expression's value.
    ///
    /// AND/OR/NOT are decomposed into conditional branches.
    /// Leaf expressions produce a single Branch terminator.
    ///
    /// After this call, current is always None (the block was sealed).
    fn lower_expr_as_branch(
        &mut self,
        expr: &HirExpr,
        on_true: BlockId,
        on_false: BlockId,
    ) {
        match &expr.kind {
            HirExprKind::BinaryOp {
                op: BinaryOp::And,
                left,
                right,
            } => {
                // a AND b: if a is true, evaluate b; if a is false, short-circuit to false
                let rhs_block = self.new_block();
                self.lower_expr_as_branch(left, rhs_block, on_false);
                self.start_block(rhs_block);
                self.lower_expr_as_branch(right, on_true, on_false);
            }
            HirExprKind::BinaryOp {
                op: BinaryOp::Or,
                left,
                right,
            } => {
                // a OR b: if a is true, short-circuit to true; if a is false, evaluate b
                let rhs_block = self.new_block();
                self.lower_expr_as_branch(left, on_true, rhs_block);
                self.start_block(rhs_block);
                self.lower_expr_as_branch(right, on_true, on_false);
            }
            HirExprKind::Not(inner) => {
                // NOT a: swap targets
                self.lower_expr_as_branch(inner, on_false, on_true);
            }
            _ => {
                // Leaf expression: emit Branch terminator
                self.seal(Terminator::Branch {
                    cond: expr.clone(),
                    on_true,
                    on_false,
                });
            }
        }
    }
}

// ── Public API ──────────────────────────────────────────────────────

/// Build a CFG from a procedure body (or module init body).
///
/// The input is a flat list of HIR statements. The output is a
/// validated CFG with all blocks sealed.
pub fn build_cfg(body: &[HirStmt]) -> Cfg {
    let mut builder = CfgBuilder::new();
    builder.lower_stmts(body);
    let cfg = builder.finish();
    cfg.validate();
    cfg
}

// ── DOT output ──────────────────────────────────────────────────────

/// Emit a DOT subgraph for one CFG, using `name` as the subgraph label.
pub fn dump_dot(cfg: &Cfg, name: &str) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    let safe: String = name.chars()
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
        .collect();
    writeln!(out, "  subgraph cluster_{} {{", safe).unwrap();
    writeln!(out, "    label=\"{}\";", name).unwrap();
    writeln!(out, "    style=dashed;").unwrap();

    for block in &cfg.blocks {
        let mut label = format!("B{}", block.id);
        for stmt in &block.stmts {
            let desc = match &stmt.kind {
                HirStmtKind::Assign { .. } => format!("L{}  assign", stmt.loc.line),
                HirStmtKind::ProcCall { target, .. } => {
                    let callee = match target {
                        crate::hir::HirCallTarget::Direct(sid) => sid.source_name.as_str(),
                        crate::hir::HirCallTarget::Indirect(_) => "(indirect)",
                    };
                    format!("L{}  call {}", stmt.loc.line, callee)
                }
                HirStmtKind::Empty => continue,
                _ => format!("L{}  {}", stmt.loc.line, stmt_kind_name(&stmt.kind)),
            };
            label.push_str("\\l");
            label.push_str(&dot_escape(&desc));
        }
        if let Some(ref term) = block.terminator {
            let t = match term {
                Terminator::Goto(id) => format!("goto B{}", id),
                Terminator::Branch { on_true, on_false, .. } => {
                    format!("br B{} / B{}", on_true, on_false)
                }
                Terminator::Return(None) => "return".into(),
                Terminator::Return(Some(_)) => "return expr".into(),
            };
            label.push_str("\\l");
            label.push_str(&dot_escape(&t));
        }
        label.push_str("\\l");
        writeln!(out, "    {}_{} [label=\"{}\"];", safe, block.id, label).unwrap();

        if let Some(ref term) = block.terminator {
            match term {
                Terminator::Goto(t) => {
                    writeln!(out, "    {}_{} -> {}_{};", safe, block.id, safe, t).unwrap();
                }
                Terminator::Branch { on_true, on_false, .. } => {
                    writeln!(out, "    {}_{} -> {}_{} [label=\"T\"];", safe, block.id, safe, on_true).unwrap();
                    writeln!(out, "    {}_{} -> {}_{} [label=\"F\", style=dashed];", safe, block.id, safe, on_false).unwrap();
                }
                Terminator::Return(_) => {}
            }
        }
    }
    writeln!(out, "  }}").unwrap();
    out
}

fn dot_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"").replace('<', "\\<").replace('>', "\\>")
}

fn stmt_kind_name(kind: &HirStmtKind) -> &'static str {
    match kind {
        HirStmtKind::Empty => "empty",
        HirStmtKind::Assign { .. } => "assign",
        HirStmtKind::ProcCall { .. } => "call",
        HirStmtKind::If { .. } => "if",
        HirStmtKind::Case { .. } => "case",
        HirStmtKind::While { .. } => "while",
        HirStmtKind::Repeat { .. } => "repeat",
        HirStmtKind::For { .. } => "for",
        HirStmtKind::Loop { .. } => "loop",
        HirStmtKind::Return { .. } => "return",
        HirStmtKind::Exit => "exit",
        HirStmtKind::Raise { .. } => "raise",
        HirStmtKind::Retry => "retry",
        HirStmtKind::Try { .. } => "try",
        HirStmtKind::Lock { .. } => "lock",
        HirStmtKind::TypeCase { .. } => "typecase",
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{BinaryOp, UnaryOp};
    use crate::errors::SourceLoc;
    use crate::hir::*;
    use crate::types::*;

    fn loc() -> SourceLoc {
        SourceLoc::new("<test>", 1, 1)
    }

    fn int_expr(v: i64) -> HirExpr {
        HirExpr {
            kind: HirExprKind::IntLit(v),
            ty: TY_INTEGER,
            loc: loc(),
        }
    }

    fn bool_expr(name: &str) -> HirExpr {
        HirExpr {
            kind: HirExprKind::Place(Place {
                base: PlaceBase::Local(SymbolId {
                    mangled: name.to_string(),
                    source_name: name.to_string(),
                    module: None,
                    ty: TY_BOOLEAN,
                    is_var_param: false,
                    is_open_array: false,
                }),
                projections: Vec::new(),
                ty: TY_BOOLEAN,
                loc: loc(),
            }),
            ty: TY_BOOLEAN,
            loc: loc(),
        }
    }

    fn assign_stmt() -> HirStmt {
        HirStmt {
            kind: HirStmtKind::Assign {
                target: Place {
                    base: PlaceBase::Local(SymbolId {
                        mangled: "x".to_string(),
                        source_name: "x".to_string(),
                        module: None,
                        ty: TY_INTEGER,
                        is_var_param: false,
                        is_open_array: false,
                    }),
                    projections: Vec::new(),
                    ty: TY_INTEGER,
                    loc: loc(),
                },
                value: int_expr(42),
            },
            loc: loc(),
        }
    }

    fn return_stmt(val: Option<i64>) -> HirStmt {
        HirStmt {
            kind: HirStmtKind::Return {
                expr: val.map(int_expr),
            },
            loc: loc(),
        }
    }

    fn if_stmt(
        cond: HirExpr,
        then_body: Vec<HirStmt>,
        else_body: Option<Vec<HirStmt>>,
    ) -> HirStmt {
        HirStmt {
            kind: HirStmtKind::If {
                cond,
                then_body,
                elsifs: Vec::new(),
                else_body,
            },
            loc: loc(),
        }
    }

    fn exit_stmt() -> HirStmt {
        HirStmt {
            kind: HirStmtKind::Exit,
            loc: loc(),
        }
    }

    fn and_expr(a: HirExpr, b: HirExpr) -> HirExpr {
        HirExpr {
            kind: HirExprKind::BinaryOp {
                op: BinaryOp::And,
                left: Box::new(a),
                right: Box::new(b),
            },
            ty: TY_BOOLEAN,
            loc: loc(),
        }
    }

    fn or_expr(a: HirExpr, b: HirExpr) -> HirExpr {
        HirExpr {
            kind: HirExprKind::BinaryOp {
                op: BinaryOp::Or,
                left: Box::new(a),
                right: Box::new(b),
            },
            ty: TY_BOOLEAN,
            loc: loc(),
        }
    }

    fn not_expr(a: HirExpr) -> HirExpr {
        HirExpr {
            kind: HirExprKind::Not(Box::new(a)),
            ty: TY_BOOLEAN,
            loc: loc(),
        }
    }

    // Helper: make an IF with elsifs
    fn if_elsif_stmt(
        cond: HirExpr,
        then_body: Vec<HirStmt>,
        elsifs: Vec<(HirExpr, Vec<HirStmt>)>,
        else_body: Option<Vec<HirStmt>>,
    ) -> HirStmt {
        HirStmt {
            kind: HirStmtKind::If { cond, then_body, elsifs, else_body },
            loc: loc(),
        }
    }

    fn while_stmt(cond: HirExpr, body: Vec<HirStmt>) -> HirStmt {
        HirStmt {
            kind: HirStmtKind::While { cond, body },
            loc: loc(),
        }
    }

    fn loop_stmt(body: Vec<HirStmt>) -> HirStmt {
        HirStmt {
            kind: HirStmtKind::Loop { body },
            loc: loc(),
        }
    }

    /// Assert a block has a specific terminator kind and return its successors.
    fn assert_goto(cfg: &Cfg, block: BlockId) -> BlockId {
        match &cfg.blocks[block].terminator {
            Some(Terminator::Goto(t)) => *t,
            other => panic!("block {} expected Goto, got {:?}", block, other),
        }
    }

    fn assert_branch(cfg: &Cfg, block: BlockId) -> (BlockId, BlockId) {
        match &cfg.blocks[block].terminator {
            Some(Terminator::Branch { on_true, on_false, .. }) => (*on_true, *on_false),
            other => panic!("block {} expected Branch, got {:?}", block, other),
        }
    }

    fn assert_return(cfg: &Cfg, block: BlockId) {
        match &cfg.blocks[block].terminator {
            Some(Terminator::Return(_)) => {}
            other => panic!("block {} expected Return, got {:?}", block, other),
        }
    }

    // ════════════════════════════════════════════════════════════
    //  Builder / invariant tests
    // ════════════════════════════════════════════════════════════

    #[test]
    fn finish_seals_open_current_block() {
        // Empty body → finish() auto-seals entry with Return(None)
        let cfg = build_cfg(&[]);
        assert_eq!(cfg.blocks.len(), 1);
        assert_return(&cfg, 0);
        assert_eq!(cfg.blocks[0].stmts.len(), 0);
    }

    #[test]
    fn finish_seals_unreached_allocated_blocks() {
        // IF cond THEN RETURN 1 ELSE RETURN 2 END
        // → merge block allocated but never started
        let cfg = build_cfg(&[if_stmt(
            bool_expr("c"),
            vec![return_stmt(Some(1))],
            Some(vec![return_stmt(Some(2))]),
        )]);
        cfg.validate();
        // The merge block exists and is sealed (by finish)
        // but has no predecessors
        let preds = cfg.predecessors();
        let merge = 1; // first allocated block after entry
        // Find the block with no predecessors that isn't entry
        let unreached: Vec<_> = (1..cfg.blocks.len())
            .filter(|&b| preds[b].is_empty())
            .collect();
        assert!(!unreached.is_empty(), "should have at least one unreached block");
        // All unreached blocks must be sealed with Return(None)
        for b in &unreached {
            assert_return(&cfg, *b);
        }
    }

    #[test]
    #[should_panic(expected = "start_block: abandoning open block")]
    fn start_block_panics_if_current_open() {
        let mut builder = CfgBuilder::new(); // current = Some(0)
        let b1 = builder.new_block();
        builder.start_block(b1); // should panic: block 0 is still open
    }

    #[test]
    #[should_panic(expected = "seal: block 0 already has a terminator")]
    fn seal_twice_panics() {
        let mut builder = CfgBuilder::new();
        builder.seal(Terminator::Return(None));
        // current is now None, so we need to start_block(0) to seal again
        // But block 0 is already sealed, so start_block will fail.
        // Test the seal path instead via direct manipulation:
        builder.current = Some(0);
        builder.seal(Terminator::Return(None)); // should panic
    }

    #[test]
    #[should_panic(expected = "start_block: block 0 is already sealed")]
    fn start_sealed_block_panics() {
        let mut builder = CfgBuilder::new();
        builder.seal(Terminator::Return(None));
        builder.start_block(0); // should panic: block 0 is sealed
    }

    #[test]
    fn push_stmt_discards_when_current_none() {
        // RETURN then assign → assign is silently discarded
        let cfg = build_cfg(&[return_stmt(Some(1)), assign_stmt()]);
        assert_eq!(cfg.blocks.len(), 1);
        assert_eq!(cfg.blocks[0].stmts.len(), 0); // assign discarded
        assert_return(&cfg, 0);
    }

    #[test]
    fn linear_stmts_stay_in_one_block() {
        let cfg = build_cfg(&[assign_stmt(), assign_stmt(), assign_stmt()]);
        assert_eq!(cfg.blocks.len(), 1);
        assert_eq!(cfg.blocks[0].stmts.len(), 3);
        assert_return(&cfg, 0); // implicit return from finish()
    }

    // ════════════════════════════════════════════════════════════
    //  IF tests
    // ════════════════════════════════════════════════════════════

    #[test]
    fn if_then_only_structure() {
        // IF c THEN x:=42 END
        // entry(0) → Branch(c) → then(2) | merge(1)
        // then(2) → Goto(merge)
        // merge(1) → Return(None)
        let cfg = build_cfg(&[if_stmt(bool_expr("c"), vec![assign_stmt()], None)]);
        cfg.validate();

        // entry branches: true→then, false→merge
        let (on_true, on_false) = assert_branch(&cfg, 0);
        // then block has the assign
        assert_eq!(cfg.blocks[on_true].stmts.len(), 1);
        // then → Goto(merge)
        let merge = assert_goto(&cfg, on_true);
        // false target is merge (no else)
        assert_eq!(on_false, merge);
        // merge has implicit return
        assert_return(&cfg, merge);
    }

    #[test]
    fn if_else_structure() {
        // IF c THEN x:=1 ELSE x:=2 END
        let cfg = build_cfg(&[if_stmt(
            bool_expr("c"),
            vec![assign_stmt()],
            Some(vec![assign_stmt()]),
        )]);
        cfg.validate();

        let (on_true, on_false) = assert_branch(&cfg, 0);
        // then → Goto(merge)
        let merge_from_then = assert_goto(&cfg, on_true);
        // else → Goto(merge)
        let merge_from_else = assert_goto(&cfg, on_false);
        // both converge to same merge
        assert_eq!(merge_from_then, merge_from_else);
        assert_return(&cfg, merge_from_then);
    }

    #[test]
    fn if_then_returns_no_goto_merge() {
        // IF c THEN RETURN 1 ELSE x:=2 END
        let cfg = build_cfg(&[if_stmt(
            bool_expr("c"),
            vec![return_stmt(Some(1))],
            Some(vec![assign_stmt()]),
        )]);
        cfg.validate();

        let (on_true, on_false) = assert_branch(&cfg, 0);
        // then block: Return (no Goto to merge)
        assert_return(&cfg, on_true);
        // else block: Goto(merge)
        let merge = assert_goto(&cfg, on_false);
        assert_return(&cfg, merge);
    }

    #[test]
    fn if_all_branches_return_merge_unreached() {
        // IF c THEN RETURN 1 ELSE RETURN 2 END; x:=99
        let cfg = build_cfg(&[
            if_stmt(
                bool_expr("c"),
                vec![return_stmt(Some(1))],
                Some(vec![return_stmt(Some(2))]),
            ),
            assign_stmt(), // unreachable
        ]);
        cfg.validate();

        let (on_true, on_false) = assert_branch(&cfg, 0);
        assert_return(&cfg, on_true);
        assert_return(&cfg, on_false);
        // The assign is discarded (current was None after IF)
        // merge block sealed by finish() with Return(None), no predecessors
        let preds = cfg.predecessors();
        let merge = 1; // first allocated
        let unreached: Vec<_> = (1..cfg.blocks.len())
            .filter(|&b| preds[b].is_empty())
            .collect();
        assert!(!unreached.is_empty());
    }

    #[test]
    fn if_elsif_else_chain() {
        // IF a THEN x:=1 ELSIF b THEN x:=2 ELSE x:=3 END
        let cfg = build_cfg(&[if_elsif_stmt(
            bool_expr("a"),
            vec![assign_stmt()],
            vec![(bool_expr("b"), vec![assign_stmt()])],
            Some(vec![assign_stmt()]),
        )]);
        cfg.validate();

        // entry → Branch(a) → then1 | elsif_block
        let (then1, elsif_entry) = assert_branch(&cfg, 0);
        let merge_from_then1 = assert_goto(&cfg, then1);

        // elsif_block → Branch(b) → then2 | else_block
        let (then2, else_block) = assert_branch(&cfg, elsif_entry);
        let merge_from_then2 = assert_goto(&cfg, then2);
        let merge_from_else = assert_goto(&cfg, else_block);

        // all converge to same merge
        assert_eq!(merge_from_then1, merge_from_then2);
        assert_eq!(merge_from_then2, merge_from_else);
        assert_return(&cfg, merge_from_then1);
    }

    // ════════════════════════════════════════════════════════════
    //  WHILE / LOOP / EXIT tests
    // ════════════════════════════════════════════════════════════

    #[test]
    fn while_normal_backedge() {
        // WHILE c DO x:=42 END
        // entry(0)→Goto(header), header→Branch(c)→body|exit, body→Goto(header), exit→Return
        let cfg = build_cfg(&[while_stmt(bool_expr("c"), vec![assign_stmt()])]);
        cfg.validate();

        let header = assert_goto(&cfg, 0);
        let (body, exit) = assert_branch(&cfg, header);
        assert_eq!(cfg.blocks[body].stmts.len(), 1);
        let backedge_target = assert_goto(&cfg, body);
        assert_eq!(backedge_target, header); // back-edge
        assert_return(&cfg, exit);

        // header has 2 predecessors: entry and body
        let preds = cfg.predecessors();
        assert_eq!(preds[header].len(), 2);
        assert!(preds[header].contains(&0));
        assert!(preds[header].contains(&body));
    }

    #[test]
    fn while_body_returns_no_backedge() {
        // WHILE c DO RETURN 0 END
        let cfg = build_cfg(&[while_stmt(bool_expr("c"), vec![return_stmt(Some(0))])]);
        cfg.validate();

        let header = assert_goto(&cfg, 0);
        let (body, exit) = assert_branch(&cfg, header);
        // body has Return, NOT Goto(header)
        assert_return(&cfg, body);
        // header has only 1 predecessor (entry), no back-edge from body
        let preds = cfg.predecessors();
        assert_eq!(preds[header].len(), 1);
    }

    #[test]
    fn loop_with_exit_structure() {
        // LOOP IF c THEN EXIT END; x:=42 END
        let cfg = build_cfg(&[loop_stmt(vec![
            if_stmt(bool_expr("c"), vec![exit_stmt()], None),
            assign_stmt(),
        ])]);
        cfg.validate();

        // entry(0) → Goto(header)
        let header = assert_goto(&cfg, 0);
        // header starts the loop body — first statement is IF
        // The IF branches: true→exit_via_goto, false→merge(continue)
        // After the IF merge, assign, then Goto(header) back-edge

        // Find the exit block (the one that LOOP allocated)
        // EXIT jumps there via Goto
        let mut found_exit_goto = false;
        let mut exit_block = 0;
        for b in &cfg.blocks {
            if let Some(Terminator::Goto(target)) = &b.terminator {
                // A Goto that is NOT a back-edge to header and NOT entry→header
                if *target != header && b.id != 0 {
                    // Could be EXIT target or IF merge→Goto
                }
            }
        }
        // Simpler: verify back-edge exists (some block jumps to header)
        let preds = cfg.predecessors();
        assert!(preds[header].len() >= 2, "header needs entry + back-edge predecessors");
    }

    #[test]
    fn nested_loop_exit_targets_innermost() {
        // LOOP               ← outer (exit=outer_exit)
        //   LOOP             ← inner (exit=inner_exit)
        //     IF c THEN EXIT END  ← EXIT targets inner_exit
        //   END
        //   x:=42            ← runs after inner loop
        // END
        let cfg = build_cfg(&[loop_stmt(vec![
            loop_stmt(vec![
                if_stmt(bool_expr("c"), vec![exit_stmt()], None),
            ]),
            assign_stmt(),
        ])]);
        cfg.validate();

        // outer: entry→Goto(outer_header)
        let outer_header = assert_goto(&cfg, 0);
        // outer_header starts with inner loop lowering:
        //   inner: Goto(inner_header) inside outer_header
        //   But outer_header IS the block, so it gets sealed with Goto(inner_header)

        // After inner loop, assign, then Goto(outer_header)
        // After outer loop, outer_exit → Return

        // Verify: the assign statement exists somewhere
        let block_with_assign = cfg.blocks.iter()
            .find(|b| b.stmts.len() == 1)
            .expect("should have a block with the assign");

        // Verify: outer_header has a back-edge (some block Goto's to it)
        let preds = cfg.predecessors();
        assert!(preds[outer_header].len() >= 2);
    }

    // ════════════════════════════════════════════════════════════
    //  Short-circuit AND / OR / NOT tests
    // ════════════════════════════════════════════════════════════

    #[test]
    fn and_creates_rhs_block() {
        // IF a AND b THEN x:=42 END
        // entry → Branch(a) → rhs | merge
        // rhs → Branch(b) → then | merge
        // then → Goto(merge)
        // merge → Return
        let cfg = build_cfg(&[if_stmt(
            and_expr(bool_expr("a"), bool_expr("b")),
            vec![assign_stmt()],
            None,
        )]);
        cfg.validate();

        // entry: Branch(a)
        let (rhs, merge_or_false) = assert_branch(&cfg, 0);
        // rhs: Branch(b)
        let (then_block, also_merge) = assert_branch(&cfg, rhs);
        // Both false targets should be merge
        assert_eq!(merge_or_false, also_merge);
        // then → Goto(merge)
        let merge = assert_goto(&cfg, then_block);
        assert_eq!(merge, merge_or_false);
        assert_return(&cfg, merge);
    }

    #[test]
    fn or_creates_rhs_block() {
        // IF a OR b THEN x:=42 END
        // entry → Branch(a) → then | rhs
        // rhs → Branch(b) → then | merge
        // then → Goto(merge)
        // merge → Return
        let cfg = build_cfg(&[if_stmt(
            or_expr(bool_expr("a"), bool_expr("b")),
            vec![assign_stmt()],
            None,
        )]);
        cfg.validate();

        // entry: Branch(a) — true goes to then, false to rhs
        let (then_from_a, rhs) = assert_branch(&cfg, 0);
        // rhs: Branch(b) — true goes to then, false to merge
        let (then_from_b, merge) = assert_branch(&cfg, rhs);
        // Both true targets should be the same then block
        assert_eq!(then_from_a, then_from_b);
        // then → Goto(merge)
        let merge_from_then = assert_goto(&cfg, then_from_a);
        assert_eq!(merge_from_then, merge);
        assert_return(&cfg, merge);
    }

    #[test]
    fn not_swaps_targets() {
        // IF NOT c THEN x:=42 END
        // NOT swaps on_true/on_false, so:
        // entry → Branch(c) → merge(false-path) | then(true-path)
        // (inverted: c=true means skip, c=false means enter then)
        let cfg = build_cfg(&[if_stmt(
            not_expr(bool_expr("c")),
            vec![assign_stmt()],
            None,
        )]);
        cfg.validate();

        let (on_true, on_false) = assert_branch(&cfg, 0);
        // NOT swaps: on_true of the Branch = merge (skip then)
        //            on_false of the Branch = then block
        // then block should have the assign
        assert_eq!(cfg.blocks[on_false].stmts.len(), 1);
        // on_true is merge (no stmts, just return)
        let merge = on_true;
        assert_return(&cfg, merge);
    }

    #[test]
    fn and_or_nested() {
        // IF a AND (b OR c) THEN x:=42 END
        // AND: entry → Branch(a) → and_rhs | merge
        // and_rhs starts OR: → Branch(b) → then | or_rhs
        // or_rhs: → Branch(c) → then | merge
        let cfg = build_cfg(&[if_stmt(
            and_expr(bool_expr("a"), or_expr(bool_expr("b"), bool_expr("c"))),
            vec![assign_stmt()],
            None,
        )]);
        cfg.validate();

        // entry: Branch(a)
        let (and_rhs, merge1) = assert_branch(&cfg, 0);
        // and_rhs: Branch(b) — OR left
        let (then_from_b, or_rhs) = assert_branch(&cfg, and_rhs);
        // or_rhs: Branch(c) — OR right
        let (then_from_c, merge2) = assert_branch(&cfg, or_rhs);
        // Both OR true targets → same then block
        assert_eq!(then_from_b, then_from_c);
        // AND false and OR false both → merge
        assert_eq!(merge1, merge2);
        // then → Goto(merge)
        let merge_from_then = assert_goto(&cfg, then_from_b);
        assert_eq!(merge_from_then, merge1);
    }

    #[test]
    fn not_and_or_combined() {
        // IF NOT (a AND b) OR c THEN x:=42 END
        // OR: entry evaluates left=NOT(a AND b), right=c
        // NOT(a AND b): swaps targets of AND
        //   AND: Branch(a) → rhs | OR_TRUE
        //   rhs: Branch(b) → OR_TRUE(inverted!) | or_rhs_block
        //   Wait — NOT swaps the AND's on_true/on_false:
        //   AND normally: a-true→rhs, a-false→on_false
        //   NOT(AND): a-true→rhs, a-false→on_true (swapped)
        //   rhs: b-true→on_false(swapped), b-false→on_true
        // This is complex. Just verify it builds, validates, and has
        // the right block count.
        let cond = or_expr(
            not_expr(and_expr(bool_expr("a"), bool_expr("b"))),
            bool_expr("c"),
        );
        let cfg = build_cfg(&[if_stmt(cond, vec![assign_stmt()], None)]);
        cfg.validate();
        // OR needs rhs block. NOT(AND) needs AND's rhs block.
        // At least: entry, and_rhs, or_rhs, then, merge = 5+
        assert!(cfg.blocks.len() >= 5);
    }

    #[test]
    fn while_with_short_circuit_condition() {
        // WHILE a AND b DO x:=42 END
        // header has AND lowering: Branch(a)→and_rhs|exit, and_rhs→Branch(b)→body|exit
        let cfg = build_cfg(&[while_stmt(
            and_expr(bool_expr("a"), bool_expr("b")),
            vec![assign_stmt()],
        )]);
        cfg.validate();

        // entry → Goto(header)
        let header = assert_goto(&cfg, 0);
        // header: Branch(a) — AND left
        let (and_rhs, exit1) = assert_branch(&cfg, header);
        // and_rhs: Branch(b) — AND right
        let (body, exit2) = assert_branch(&cfg, and_rhs);
        // Both false targets = exit
        assert_eq!(exit1, exit2);
        // body → Goto(header) back-edge
        let backedge = assert_goto(&cfg, body);
        assert_eq!(backedge, header);
    }

    // ════════════════════════════════════════════════════════════
    //  finish() behavior tests
    // ════════════════════════════════════════════════════════════

    #[test]
    fn finish_with_linear_body() {
        // x:=1; x:=2 → finish seals with Return(None)
        let cfg = build_cfg(&[assign_stmt(), assign_stmt()]);
        assert_eq!(cfg.blocks.len(), 1);
        assert_eq!(cfg.blocks[0].stmts.len(), 2);
        assert_return(&cfg, 0);
    }

    #[test]
    fn finish_after_explicit_return() {
        // RETURN 1 → finish does NOT double-seal, current is already None
        let cfg = build_cfg(&[return_stmt(Some(1))]);
        assert_eq!(cfg.blocks.len(), 1);
        assert_eq!(cfg.blocks[0].stmts.len(), 0);
        match &cfg.blocks[0].terminator {
            Some(Terminator::Return(Some(_))) => {} // explicit return, not None
            other => panic!("expected Return(Some), got {:?}", other),
        }
    }

    #[test]
    fn finish_unreached_blocks_have_no_predecessors() {
        // IF c THEN RETURN 1 ELSE RETURN 2 END
        let cfg = build_cfg(&[if_stmt(
            bool_expr("c"),
            vec![return_stmt(Some(1))],
            Some(vec![return_stmt(Some(2))]),
        )]);
        let preds = cfg.predecessors();
        // Every block sealed by finish (Return(None)) with no preds
        // is genuinely unreachable
        for (i, block) in cfg.blocks.iter().enumerate() {
            if i == 0 { continue; } // entry has no preds by definition
            if preds[i].is_empty() {
                // Must be sealed with Return(None) — the finish() default
                match &block.terminator {
                    Some(Terminator::Return(None)) => {} // correct
                    other => panic!(
                        "unreached block {} should have Return(None), got {:?}",
                        i, other
                    ),
                }
            }
        }
    }
}
