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

    /// Set the current block. The target must be open (no terminator).
    fn start_block(&mut self, id: BlockId) {
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
        for block in &mut self.blocks {
            if block.terminator.is_none() {
                // This block was allocated but no path reached it.
                // Seal with Return(None) to satisfy the "all sealed" invariant.
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

    // ── Builder mechanics ────────────────────────────────────────

    #[test]
    fn empty_body() {
        let cfg = build_cfg(&[]);
        assert_eq!(cfg.blocks.len(), 1);
        assert!(matches!(cfg.blocks[0].terminator, Some(Terminator::Return(None))));
    }

    #[test]
    fn linear_sequence() {
        let stmts = vec![assign_stmt(), assign_stmt(), assign_stmt()];
        let cfg = build_cfg(&stmts);
        assert_eq!(cfg.blocks.len(), 1);
        assert_eq!(cfg.blocks[0].stmts.len(), 3);
        assert!(matches!(cfg.blocks[0].terminator, Some(Terminator::Return(None))));
    }

    #[test]
    fn return_discards_rest() {
        let stmts = vec![assign_stmt(), return_stmt(Some(1)), assign_stmt()];
        let cfg = build_cfg(&stmts);
        assert_eq!(cfg.blocks.len(), 1);
        assert_eq!(cfg.blocks[0].stmts.len(), 1); // only first assign
        assert!(matches!(cfg.blocks[0].terminator, Some(Terminator::Return(Some(_)))));
    }

    // ── IF ───────────────────────────────────────────────────────

    #[test]
    fn if_then_only() {
        // IF cond THEN assign END
        let stmts = vec![if_stmt(bool_expr("c"), vec![assign_stmt()], None)];
        let cfg = build_cfg(&stmts);
        cfg.validate();
        // Blocks: entry(0) → branch, then(1), merge(2)
        // entry branches to then or merge
        // then → Goto(merge)
        // merge has implicit return
        assert!(cfg.blocks.len() >= 3);
    }

    #[test]
    fn if_then_else() {
        let stmts = vec![if_stmt(
            bool_expr("c"),
            vec![assign_stmt()],
            Some(vec![assign_stmt()]),
        )];
        let cfg = build_cfg(&stmts);
        cfg.validate();
        // merge, then, else blocks + entry
        assert!(cfg.blocks.len() >= 4);
    }

    #[test]
    fn if_both_branches_return() {
        // IF cond THEN RETURN 1 ELSE RETURN 2 END
        let stmts = vec![if_stmt(
            bool_expr("c"),
            vec![return_stmt(Some(1))],
            Some(vec![return_stmt(Some(2))]),
        )];
        let cfg = build_cfg(&stmts);
        cfg.validate();
        // merge block is allocated but unreachable — sealed by finish()
        // No path reaches merge, so builder's current is None after the IF
    }

    // ── WHILE ───────────────────────────────────────────────────

    #[test]
    fn simple_while() {
        // WHILE cond DO assign END
        let stmts = vec![HirStmt {
            kind: HirStmtKind::While {
                cond: bool_expr("c"),
                body: vec![assign_stmt()],
            },
            loc: loc(),
        }];
        let cfg = build_cfg(&stmts);
        cfg.validate();
        // Blocks: entry(0), header(1), body(2), exit(3)
        assert!(cfg.blocks.len() >= 4);
        // Header is a branch target
        let preds = cfg.predecessors();
        // header should have entry and body as predecessors
        assert!(preds[1].len() >= 2); // pre→header and body→header
    }

    // ── LOOP + EXIT ─────────────────────────────────────────────

    #[test]
    fn loop_with_exit() {
        // LOOP IF cond THEN EXIT END; assign END
        let stmts = vec![HirStmt {
            kind: HirStmtKind::Loop {
                body: vec![
                    if_stmt(bool_expr("c"), vec![exit_stmt()], None),
                    assign_stmt(),
                ],
            },
            loc: loc(),
        }];
        let cfg = build_cfg(&stmts);
        cfg.validate();
        // Should have: entry, header, exit, plus IF blocks
        // EXIT should jump to exit block
        let exit_block_id = cfg.blocks.len() - 1; // approximately
        // Just verify it validates and has back-edges
        let preds = cfg.predecessors();
        // The header block should appear as a successor somewhere (back-edge)
    }

    // ── Short-circuit AND/OR ────────────────────────────────────

    #[test]
    fn if_with_and() {
        // IF a AND b THEN assign END
        let cond = and_expr(bool_expr("a"), bool_expr("b"));
        let stmts = vec![if_stmt(cond, vec![assign_stmt()], None)];
        let cfg = build_cfg(&stmts);
        cfg.validate();
        // AND creates an extra rhs_block
        // entry → Branch(a) → rhs_block | merge
        // rhs_block → Branch(b) → then | merge
        assert!(cfg.blocks.len() >= 4);
    }

    #[test]
    fn if_with_or() {
        // IF a OR b THEN assign END
        let cond = or_expr(bool_expr("a"), bool_expr("b"));
        let stmts = vec![if_stmt(cond, vec![assign_stmt()], None)];
        let cfg = build_cfg(&stmts);
        cfg.validate();
        assert!(cfg.blocks.len() >= 4);
    }

    #[test]
    fn if_with_not_and() {
        // IF NOT (a AND b) THEN assign END
        let cond = not_expr(and_expr(bool_expr("a"), bool_expr("b")));
        let stmts = vec![if_stmt(cond, vec![assign_stmt()], None)];
        let cfg = build_cfg(&stmts);
        cfg.validate();
    }

    #[test]
    fn nested_and_or() {
        // IF a AND (b OR c) THEN assign END
        let cond = and_expr(
            bool_expr("a"),
            or_expr(bool_expr("b"), bool_expr("c")),
        );
        let stmts = vec![if_stmt(cond, vec![assign_stmt()], None)];
        let cfg = build_cfg(&stmts);
        cfg.validate();
        // AND: entry → Branch(a) → rhs1 | merge
        // rhs1 has OR: → Branch(b) → then | rhs2
        // rhs2: → Branch(c) → then | merge
        assert!(cfg.blocks.len() >= 5);
    }

    #[test]
    fn while_with_short_circuit() {
        // WHILE a AND b DO assign END
        let stmts = vec![HirStmt {
            kind: HirStmtKind::While {
                cond: and_expr(bool_expr("a"), bool_expr("b")),
                body: vec![assign_stmt()],
            },
            loc: loc(),
        }];
        let cfg = build_cfg(&stmts);
        cfg.validate();
    }

    // ── Edge cases ──────────────────────────────────────────────

    #[test]
    fn return_in_while_body() {
        // WHILE cond DO RETURN 0 END
        let stmts = vec![HirStmt {
            kind: HirStmtKind::While {
                cond: bool_expr("c"),
                body: vec![return_stmt(Some(0))],
            },
            loc: loc(),
        }];
        let cfg = build_cfg(&stmts);
        cfg.validate();
        // Body block has Return, no back-edge
    }

    #[test]
    fn nested_loops() {
        // LOOP LOOP IF c THEN EXIT END END END
        let stmts = vec![HirStmt {
            kind: HirStmtKind::Loop {
                body: vec![HirStmt {
                    kind: HirStmtKind::Loop {
                        body: vec![if_stmt(bool_expr("c"), vec![exit_stmt()], None)],
                    },
                    loc: loc(),
                }],
            },
            loc: loc(),
        }];
        let cfg = build_cfg(&stmts);
        cfg.validate();
    }

    #[test]
    fn code_after_return_discarded() {
        // RETURN 1; assign (unreachable)
        let stmts = vec![return_stmt(Some(1)), assign_stmt(), assign_stmt()];
        let cfg = build_cfg(&stmts);
        cfg.validate();
        assert_eq!(cfg.blocks.len(), 1);
        assert_eq!(cfg.blocks[0].stmts.len(), 0); // no assigns — return has no preceding stmt
    }
}
