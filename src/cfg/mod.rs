//! Control Flow Graph (CFG) construction from HIR.
//!
//! V2: Complete control-flow representation with verifier, predecessor
//! tracking, all control constructs, exception regions, cleanup pass.
//!
//! Supported: linear statements, IF/ELSIF/ELSE, WHILE, LOOP, EXIT,
//! RETURN, short-circuit AND/OR/NOT, REPEAT/UNTIL, FOR, CASE,
//! TYPECASE, TRY/EXCEPT/FINALLY, RAISE, LOCK.

use crate::ast::BinaryOp;
use crate::hir::{
    ForDirection, HirCallTarget, HirCaseBranch, HirCaseLabel, HirExceptClause,
    HirExpr, HirExprKind, HirStmt, HirStmtKind, HirTypeCaseBranch,
    Place, PlaceBase, SymbolId,
};
use crate::types::TypeId;
use std::collections::HashSet;

// ── Data model ──────────────────────────────────────────────────────

pub type BlockId = usize;

/// A basic block: maximal sequence of non-branching statements
/// followed by a terminator that determines control flow.
#[derive(Debug, Clone)]
pub struct BasicBlock {
    pub id: BlockId,
    pub stmts: Vec<HirStmt>,
    /// `None` = open (under construction). `Some` = sealed.
    pub terminator: Option<Terminator>,
    /// Exception handler region. If this block throws (via Raise or
    /// a potentially-throwing ProcCall), control transfers to this block
    /// at runtime. None = no active handler (unhandled → abort).
    pub handler: Option<BlockId>,
}

/// How a basic block exits. Single source of truth for successors.
#[derive(Debug, Clone)]
pub enum Terminator {
    /// Unconditional jump.
    Goto(BlockId),
    /// Conditional branch.
    Branch {
        cond: HirExpr,
        on_true: BlockId,
        on_false: BlockId,
    },
    /// Multi-way dispatch (CASE, TYPECASE).
    Switch {
        expr: HirExpr,
        arms: Vec<SwitchArm>,
        default: BlockId,
    },
    /// Procedure return.
    Return(Option<HirExpr>),
    /// Exception raise. No CFG successors — handler dispatch is
    /// structural (via block.handler), not a CFG edge.
    Raise(Option<HirExpr>),
}

/// One arm of a Switch terminator.
#[derive(Debug, Clone)]
pub struct SwitchArm {
    pub labels: Vec<CaseLabel>,
    pub target: BlockId,
}

/// Label for a Switch arm.
#[derive(Debug, Clone)]
pub enum CaseLabel {
    Single(HirExpr),
    Range(HirExpr, HirExpr),
    /// TYPECASE type match with optional binding variable name.
    Type(SymbolId, Option<String>),
}

impl Terminator {
    /// Successor block ids, derived from this terminator only.
    pub fn successors(&self) -> Vec<BlockId> {
        match self {
            Terminator::Goto(b) => vec![*b],
            Terminator::Branch { on_true, on_false, .. } => vec![*on_true, *on_false],
            Terminator::Switch { arms, default, .. } => {
                let mut s: Vec<BlockId> = arms.iter().map(|a| a.target).collect();
                s.push(*default);
                s
            }
            Terminator::Return(_) | Terminator::Raise(_) => vec![],
        }
    }

    /// Rewrite all target block IDs using a mapping.
    fn remap_targets(&mut self, map: &[usize]) {
        match self {
            Terminator::Goto(t) => *t = map[*t],
            Terminator::Branch { on_true, on_false, .. } => {
                *on_true = map[*on_true];
                *on_false = map[*on_false];
            }
            Terminator::Switch { arms, default, .. } => {
                for arm in arms { arm.target = map[arm.target]; }
                *default = map[*default];
            }
            Terminator::Return(_) | Terminator::Raise(_) => {}
        }
    }

    /// Replace all occurrences of `old` target with `new`.
    fn replace_target(&mut self, old: BlockId, new: BlockId) {
        match self {
            Terminator::Goto(t) => { if *t == old { *t = new; } }
            Terminator::Branch { on_true, on_false, .. } => {
                if *on_true == old { *on_true = new; }
                if *on_false == old { *on_false = new; }
            }
            Terminator::Switch { arms, default, .. } => {
                for arm in arms {
                    if arm.target == old { arm.target = new; }
                }
                if *default == old { *default = new; }
            }
            Terminator::Return(_) | Terminator::Raise(_) => {}
        }
    }
}

/// Complete CFG for one procedure body or module init body.
#[derive(Debug, Clone)]
pub struct Cfg {
    pub blocks: Vec<BasicBlock>,
    /// Entry block is always block 0.
    pub entry: BlockId,
    /// Materialized predecessor lists. Valid only when preds_valid is true.
    pub preds: Vec<Vec<BlockId>>,
    /// Whether preds is current. Set true by compute_preds, false by mutations.
    pub preds_valid: bool,
}

impl Cfg {
    /// Compute predecessor lists from terminators.
    pub fn compute_preds(&mut self) {
        let n = self.blocks.len();
        self.preds = vec![Vec::new(); n];
        for block in &self.blocks {
            if let Some(ref term) = block.terminator {
                for succ in term.successors() {
                    if !self.preds[succ].contains(&block.id) {
                        self.preds[succ].push(block.id);
                    }
                }
            }
        }
        self.preds_valid = true;
    }

    // ── Verifier ────────────────────────────────────────────────────

    /// Verify all CFG invariants. Panics on violation.
    /// Run after construction and after every transformation pass.
    pub fn verify(&self) {
        let n = self.blocks.len();

        // S1: entry is block 0
        assert_eq!(self.entry, 0, "S1: entry must be block 0");

        // S2: non-empty
        assert!(n >= 1, "S2: CFG must have at least one block");

        // S3: sequential IDs
        for (i, block) in self.blocks.iter().enumerate() {
            assert_eq!(block.id, i, "S3: block id mismatch: expected {}, got {}", i, block.id);
        }

        // S4: all sealed
        for block in &self.blocks {
            assert!(block.terminator.is_some(), "S4: block {} is not sealed", block.id);
        }

        // T1: all successor targets exist
        for block in &self.blocks {
            for succ in block.terminator.as_ref().unwrap().successors() {
                assert!(succ < n, "T1: block {} references nonexistent successor {}", block.id, succ);
            }
        }

        // T3: Switch has at least one arm
        // T5: every SwitchArm has at least one label
        for block in &self.blocks {
            if let Some(Terminator::Switch { arms, .. }) = &block.terminator {
                assert!(!arms.is_empty(), "T3: block {} Switch has no arms", block.id);
                for (i, arm) in arms.iter().enumerate() {
                    assert!(!arm.labels.is_empty(),
                        "T5: block {} Switch arm {} has no labels", block.id, i);
                }
            }
        }

        // H1: handler targets exist
        // H2: handler is not self
        for block in &self.blocks {
            if let Some(h) = block.handler {
                assert!(h < n, "H1: block {} handler references nonexistent block {}", block.id, h);
                assert_ne!(h, block.id, "H2: block {} handler is self", block.id);
            }
        }

        // H3: handler chains are acyclic
        for block in &self.blocks {
            if block.handler.is_some() {
                let mut visited = HashSet::new();
                let mut cur = block.handler;
                while let Some(h) = cur {
                    assert!(visited.insert(h),
                        "H3: block {} has cyclic handler chain at {}", block.id, h);
                    cur = self.blocks[h].handler;
                }
            }
        }

        // P1-P4: predecessor consistency (only when materialized)
        if self.preds_valid {
            assert_eq!(self.preds.len(), n, "preds length mismatch");
            // P3: entry has no predecessors
            assert!(self.preds[self.entry].is_empty(),
                "P3: entry block has predecessors");
            // P1: every successor S of B has B in preds[S]
            for block in &self.blocks {
                for succ in block.terminator.as_ref().unwrap().successors() {
                    assert!(self.preds[succ].contains(&block.id),
                        "P1: block {} is successor of {} but not in preds", succ, block.id);
                }
            }
            // P2: every pred P of B has B in successors of P
            for (b, pred_list) in self.preds.iter().enumerate() {
                for &p in pred_list {
                    assert!(self.blocks[p].terminator.as_ref().unwrap()
                        .successors().contains(&b),
                        "P2: block {} claims pred {} but {} doesn't target {}", b, p, p, b);
                }
            }
        }
    }

    // ── Cleanup ─────────────────────────────────────────────────────

    /// Remove unreachable blocks and collapse trivial gotos.
    pub fn cleanup(&mut self) {
        self.remove_unreachable();
        self.compute_preds();
        self.collapse_trivial_gotos();
        self.compute_preds();
    }

    /// Remove blocks not reachable from entry. Handler targets of
    /// reachable blocks are also treated as reachable roots.
    fn remove_unreachable(&mut self) {
        let n = self.blocks.len();
        let mut reachable = vec![false; n];
        let mut queue = vec![self.entry];

        while let Some(b) = queue.pop() {
            if reachable[b] { continue; }
            reachable[b] = true;
            for succ in self.blocks[b].terminator.as_ref().unwrap().successors() {
                queue.push(succ);
            }
            if let Some(h) = self.blocks[b].handler {
                queue.push(h);
            }
        }

        // Build old→new ID mapping
        let mut new_id = vec![0usize; n];
        let mut next = 0;
        for i in 0..n {
            if reachable[i] {
                new_id[i] = next;
                next += 1;
            }
        }

        // Rewrite all references
        for block in &mut self.blocks {
            if let Some(ref mut term) = block.terminator {
                term.remap_targets(&new_id);
            }
            if let Some(h) = block.handler {
                block.handler = Some(new_id[h]);
            }
        }

        // Remove dead blocks, renumber
        self.blocks.retain(|b| reachable[b.id]);
        for (i, block) in self.blocks.iter_mut().enumerate() {
            block.id = i;
        }
        self.entry = 0;
        self.preds_valid = false;
    }

    /// Collapse blocks with no statements and Goto terminator,
    /// unless they are entry, handler targets, or merge points.
    fn collapse_trivial_gotos(&mut self) {
        debug_assert!(self.preds_valid, "preds must be current before collapse");

        let handler_targets: HashSet<BlockId> = self.blocks.iter()
            .filter_map(|b| b.handler)
            .collect();

        loop {
            let mut changed = false;
            for i in 0..self.blocks.len() {
                if i == self.entry { continue; }
                if handler_targets.contains(&i) { continue; }
                if self.preds[i].len() > 1 { continue; }        // merge point
                if !self.blocks[i].stmts.is_empty() { continue; }
                if self.blocks[i].handler.is_some() { continue; } // exception context
                let target = match &self.blocks[i].terminator {
                    Some(Terminator::Goto(t)) if *t != i => *t,
                    _ => continue,
                };
                // Rewrite all references to i → target
                for block in &mut self.blocks {
                    if let Some(ref mut term) = block.terminator {
                        term.replace_target(i, target);
                    }
                    if block.handler == Some(i) {
                        block.handler = Some(target);
                    }
                }
                changed = true;
            }
            if !changed { break; }
        }
        self.preds_valid = false;
    }
}

// ── Builder ─────────────────────────────────────────────────────────

/// Builds a CFG from a sequence of HIR statements.

mod build;

pub use build::build_cfg;
use build::CfgBuilder;

// ── DOT output ──────────────────────────────────────────────────────

/// Emit a DOT subgraph for one CFG.
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
        if let Some(h) = block.handler {
            label.push_str(&format!(" [h=B{}]", h));
        }
        for stmt in &block.stmts {
            let desc = match &stmt.kind {
                HirStmtKind::Assign { .. } => format!("L{}  assign", stmt.loc.line),
                HirStmtKind::ProcCall { target, .. } => {
                    let callee = match target {
                        HirCallTarget::Direct(sid) => sid.source_name.as_str(),
                        HirCallTarget::Indirect(_) => "(indirect)",
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
                Terminator::Branch { on_true, on_false, .. } =>
                    format!("br B{} / B{}", on_true, on_false),
                Terminator::Switch { arms, default, .. } => {
                    let arm_str: Vec<String> = arms.iter()
                        .map(|a| format!("B{}", a.target)).collect();
                    format!("switch [{}] else B{}", arm_str.join(","), default)
                }
                Terminator::Return(None) => "return".into(),
                Terminator::Return(Some(_)) => "return expr".into(),
                Terminator::Raise(_) => "raise".into(),
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
                Terminator::Switch { arms, default, .. } => {
                    for (i, arm) in arms.iter().enumerate() {
                        writeln!(out, "    {}_{} -> {}_{} [label=\"arm{}\"];", safe, block.id, safe, arm.target, i).unwrap();
                    }
                    writeln!(out, "    {}_{} -> {}_{} [label=\"else\", style=dashed];", safe, block.id, safe, default).unwrap();
                }
                Terminator::Return(_) | Terminator::Raise(_) => {}
            }
        }
        // Handler edge (dotted)
        if let Some(h) = block.handler {
            writeln!(out, "    {}_{} -> {}_{} [label=\"exc\", style=dotted, color=red];",
                safe, block.id, safe, h).unwrap();
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
    use crate::ast::BinaryOp;
    use crate::errors::SourceLoc;
    use crate::hir::*;
    use crate::types::*;

    fn loc() -> SourceLoc { SourceLoc::new("<test>", 1, 1) }

    fn int_expr(v: i64) -> HirExpr {
        HirExpr { kind: HirExprKind::IntLit(v), ty: TY_INTEGER, loc: loc() }
    }

    fn bool_expr(name: &str) -> HirExpr {
        HirExpr {
            kind: HirExprKind::Place(Place {
                base: PlaceBase::Local(SymbolId {
                    mangled: name.to_string(), source_name: name.to_string(),
                    module: None, ty: TY_BOOLEAN, is_var_param: false, is_open_array: false,
                }),
                projections: Vec::new(), ty: TY_BOOLEAN, loc: loc(),
            }),
            ty: TY_BOOLEAN, loc: loc(),
        }
    }

    fn assign_stmt() -> HirStmt {
        HirStmt {
            kind: HirStmtKind::Assign {
                target: Place {
                    base: PlaceBase::Local(SymbolId {
                        mangled: "x".into(), source_name: "x".into(),
                        module: None, ty: TY_INTEGER, is_var_param: false, is_open_array: false,
                    }),
                    projections: Vec::new(), ty: TY_INTEGER, loc: loc(),
                },
                value: int_expr(42),
            },
            loc: loc(),
        }
    }

    fn return_stmt(val: Option<i64>) -> HirStmt {
        HirStmt { kind: HirStmtKind::Return { expr: val.map(int_expr) }, loc: loc() }
    }

    fn if_stmt(cond: HirExpr, then_body: Vec<HirStmt>, else_body: Option<Vec<HirStmt>>) -> HirStmt {
        HirStmt {
            kind: HirStmtKind::If { cond, then_body, elsifs: Vec::new(), else_body },
            loc: loc(),
        }
    }

    fn exit_stmt() -> HirStmt { HirStmt { kind: HirStmtKind::Exit, loc: loc() } }

    fn and_expr(a: HirExpr, b: HirExpr) -> HirExpr {
        HirExpr {
            kind: HirExprKind::BinaryOp { op: BinaryOp::And, left: Box::new(a), right: Box::new(b) },
            ty: TY_BOOLEAN, loc: loc(),
        }
    }

    fn or_expr(a: HirExpr, b: HirExpr) -> HirExpr {
        HirExpr {
            kind: HirExprKind::BinaryOp { op: BinaryOp::Or, left: Box::new(a), right: Box::new(b) },
            ty: TY_BOOLEAN, loc: loc(),
        }
    }

    fn not_expr(a: HirExpr) -> HirExpr {
        HirExpr { kind: HirExprKind::Not(Box::new(a)), ty: TY_BOOLEAN, loc: loc() }
    }

    fn while_stmt(cond: HirExpr, body: Vec<HirStmt>) -> HirStmt {
        HirStmt { kind: HirStmtKind::While { cond, body }, loc: loc() }
    }

    fn loop_stmt(body: Vec<HirStmt>) -> HirStmt {
        HirStmt { kind: HirStmtKind::Loop { body }, loc: loc() }
    }

    fn repeat_stmt(body: Vec<HirStmt>, cond: HirExpr) -> HirStmt {
        HirStmt { kind: HirStmtKind::Repeat { body, cond }, loc: loc() }
    }

    fn for_stmt(body: Vec<HirStmt>) -> HirStmt {
        HirStmt {
            kind: HirStmtKind::For {
                var: "i".into(), var_ty: TY_INTEGER,
                start: int_expr(0), end: int_expr(10),
                step: None, direction: ForDirection::Up, body,
            },
            loc: loc(),
        }
    }

    fn case_stmt(branches: Vec<(Vec<i64>, Vec<HirStmt>)>, else_body: Option<Vec<HirStmt>>) -> HirStmt {
        let hir_branches: Vec<HirCaseBranch> = branches.into_iter().map(|(labels, body)| {
            HirCaseBranch {
                labels: labels.into_iter().map(|v| HirCaseLabel::Single(int_expr(v))).collect(),
                body,
            }
        }).collect();
        HirStmt {
            kind: HirStmtKind::Case { expr: int_expr(0), branches: hir_branches, else_body },
            loc: loc(),
        }
    }

    fn raise_stmt() -> HirStmt {
        HirStmt { kind: HirStmtKind::Raise { expr: None }, loc: loc() }
    }

    fn try_stmt(body: Vec<HirStmt>, finally_body: Option<Vec<HirStmt>>) -> HirStmt {
        HirStmt {
            kind: HirStmtKind::Try { body, excepts: Vec::new(), finally_body },
            loc: loc(),
        }
    }

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

    fn assert_switch(cfg: &Cfg, block: BlockId) -> (Vec<BlockId>, BlockId) {
        match &cfg.blocks[block].terminator {
            Some(Terminator::Switch { arms, default, .. }) =>
                (arms.iter().map(|a| a.target).collect(), *default),
            other => panic!("block {} expected Switch, got {:?}", block, other),
        }
    }

    fn assert_raise(cfg: &Cfg, block: BlockId) {
        match &cfg.blocks[block].terminator {
            Some(Terminator::Raise(_)) => {}
            other => panic!("block {} expected Raise, got {:?}", block, other),
        }
    }

    // ── Structural tests ────────────────────────────────────────────

    #[test]
    fn empty_body() {
        let cfg = build_cfg(&[]);
        assert_eq!(cfg.blocks.len(), 1);
        assert_return(&cfg, 0);
        assert!(cfg.preds_valid);
    }

    #[test]
    fn linear_sequence() {
        let cfg = build_cfg(&[assign_stmt(), assign_stmt()]);
        assert_eq!(cfg.blocks.len(), 1);
        assert_eq!(cfg.blocks[0].stmts.len(), 2);
    }

    #[test]
    fn return_discards_rest() {
        let cfg = build_cfg(&[return_stmt(Some(1)), assign_stmt()]);
        assert_eq!(cfg.blocks[0].stmts.len(), 0);
        assert_return(&cfg, 0);
    }

    #[test]
    fn preds_computed() {
        let cfg = build_cfg(&[if_stmt(bool_expr("c"), vec![assign_stmt()], None)]);
        assert!(cfg.preds_valid);
        assert!(cfg.preds[0].is_empty()); // entry has no preds
    }

    // ── IF tests ────────────────────────────────────────────────────

    #[test]
    fn if_then_else() {
        let cfg = build_cfg(&[if_stmt(
            bool_expr("c"), vec![assign_stmt()], Some(vec![assign_stmt()]),
        )]);
        cfg.verify();
        let (on_true, on_false) = assert_branch(&cfg, 0);
        let m1 = assert_goto(&cfg, on_true);
        let m2 = assert_goto(&cfg, on_false);
        assert_eq!(m1, m2);
    }

    #[test]
    fn if_all_return() {
        let cfg = build_cfg(&[if_stmt(
            bool_expr("c"), vec![return_stmt(Some(1))], Some(vec![return_stmt(Some(2))]),
        )]);
        cfg.verify();
    }

    // ── WHILE test ──────────────────────────────────────────────────

    #[test]
    fn while_backedge() {
        let cfg = build_cfg(&[while_stmt(bool_expr("c"), vec![assign_stmt()])]);
        cfg.verify();
        let header = assert_goto(&cfg, 0);
        let (body, exit) = assert_branch(&cfg, header);
        let back = assert_goto(&cfg, body);
        assert_eq!(back, header);
        assert!(cfg.preds[header].len() >= 2);
    }

    // ── LOOP + EXIT ─────────────────────────────────────────────────

    #[test]
    fn loop_exit() {
        let cfg = build_cfg(&[loop_stmt(vec![
            if_stmt(bool_expr("c"), vec![exit_stmt()], None),
            assign_stmt(),
        ])]);
        cfg.verify();
    }

    // ── REPEAT ──────────────────────────────────────────────────────

    #[test]
    fn repeat_until() {
        let cfg = build_cfg(&[repeat_stmt(vec![assign_stmt()], bool_expr("done"))]);
        cfg.verify();
        // entry → Goto(body), body has assign, then Branch(done, exit, body)
        let body = assert_goto(&cfg, 0);
        assert_eq!(cfg.blocks[body].stmts.len(), 1);
        let (exit, back) = assert_branch(&cfg, body);
        assert_eq!(back, body); // back-edge
        assert_return(&cfg, exit);
    }

    // ── FOR ─────────────────────────────────────────────────────────

    #[test]
    fn for_loop() {
        let cfg = build_cfg(&[for_stmt(vec![assign_stmt()])]);
        cfg.verify();
        // entry has init assign, then Goto(cond)
        assert_eq!(cfg.blocks[0].stmts.len(), 1); // i := 0
        let cond = assert_goto(&cfg, 0);
        let (body, exit) = assert_branch(&cfg, cond);
        let latch = assert_goto(&cfg, body);
        assert_eq!(cfg.blocks[latch].stmts.len(), 1); // i := i + 1
        let back = assert_goto(&cfg, latch);
        assert_eq!(back, cond);
        assert_return(&cfg, exit);
    }

    // ── CASE ────────────────────────────────────────────────────────

    #[test]
    fn case_switch() {
        let cfg = build_cfg(&[case_stmt(
            vec![(vec![1], vec![assign_stmt()]), (vec![2], vec![assign_stmt()])],
            Some(vec![assign_stmt()]),
        )]);
        cfg.verify();
        let (arms, default) = assert_switch(&cfg, 0);
        assert_eq!(arms.len(), 2);
        // All arms and default converge to merge
        let m1 = assert_goto(&cfg, arms[0]);
        let m2 = assert_goto(&cfg, arms[1]);
        let m3 = assert_goto(&cfg, default);
        assert_eq!(m1, m2);
        assert_eq!(m2, m3);
    }

    // ── RAISE ───────────────────────────────────────────────────────

    #[test]
    fn raise_terminates() {
        let cfg = build_cfg(&[raise_stmt(), assign_stmt()]);
        cfg.verify();
        assert_raise(&cfg, 0);
        assert_eq!(cfg.blocks[0].stmts.len(), 0); // assign discarded
    }

    // ── TRY / FINALLY ───────────────────────────────────────────────

    #[test]
    fn try_finally() {
        let cfg = build_cfg(&[try_stmt(
            vec![assign_stmt()],
            Some(vec![assign_stmt()]),
        )]);
        cfg.verify();
        // TRY body blocks should have handler set
        let has_handler = cfg.blocks.iter().any(|b| b.handler.is_some());
        assert!(has_handler, "TRY body blocks should have handler");
    }

    // ── AND / OR ────────────────────────────────────────────────────

    #[test]
    fn and_short_circuit() {
        let cfg = build_cfg(&[if_stmt(
            and_expr(bool_expr("a"), bool_expr("b")), vec![assign_stmt()], None,
        )]);
        cfg.verify();
        let (rhs, merge1) = assert_branch(&cfg, 0);
        let (then_block, merge2) = assert_branch(&cfg, rhs);
        assert_eq!(merge1, merge2);
    }

    #[test]
    fn or_short_circuit() {
        let cfg = build_cfg(&[if_stmt(
            or_expr(bool_expr("a"), bool_expr("b")), vec![assign_stmt()], None,
        )]);
        cfg.verify();
    }

    // ── Cleanup ─────────────────────────────────────────────────────

    #[test]
    fn cleanup_removes_unreachable() {
        let mut cfg = build_cfg(&[if_stmt(
            bool_expr("c"), vec![return_stmt(Some(1))], Some(vec![return_stmt(Some(2))]),
        )]);
        let before = cfg.blocks.len();
        cfg.cleanup();
        cfg.verify();
        assert!(cfg.blocks.len() <= before);
    }

    // ── Verifier ────────────────────────────────────────────────────

    #[test]
    #[should_panic(expected = "start_block: abandoning open block")]
    fn start_block_panics_if_current_open() {
        let mut builder = CfgBuilder::new();
        let b1 = builder.new_block();
        builder.start_block(b1);
    }

    #[test]
    #[should_panic(expected = "seal: block 0 already has a terminator")]
    fn seal_twice_panics() {
        let mut builder = CfgBuilder::new();
        builder.seal(Terminator::Return(None));
        builder.current = Some(0);
        builder.seal(Terminator::Return(None));
    }
}
