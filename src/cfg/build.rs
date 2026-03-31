//! CFG construction: CfgBuilder and all control-flow lowering methods.

use crate::ast::BinaryOp;
use crate::hir::{
    ForDirection, HirCallTarget, HirCaseBranch, HirCaseLabel, HirExceptClause,
    HirExpr, HirExprKind, HirStmt, HirStmtKind, HirTypeCaseBranch,
    Place, PlaceBase, SymbolId,
};
use crate::types::TypeId;

use super::{BasicBlock, BlockId, Cfg, CaseLabel, SwitchArm, Terminator};

pub(super) struct CfgBuilder {
    pub(super) blocks: Vec<BasicBlock>,
    pub(super) current: Option<BlockId>,
    pub(super) loop_exit_stack: Vec<BlockId>,
    /// Active exception handler stack. Blocks created while non-empty
    /// get handler = Some(handler_stack.last()).
    pub(super) handler_stack: Vec<BlockId>,
}

impl CfgBuilder {
    pub(super) fn new() -> Self {
        let entry = BasicBlock {
            id: 0,
            stmts: Vec::new(),
            terminator: None,
            handler: None,
        };
        CfgBuilder {
            blocks: vec![entry],
            current: Some(0),
            loop_exit_stack: Vec::new(),
            handler_stack: Vec::new(),
        }
    }

    pub(super) fn new_block(&mut self) -> BlockId {
        let id = self.blocks.len();
        let handler = self.handler_stack.last().copied();
        self.blocks.push(BasicBlock {
            id,
            stmts: Vec::new(),
            terminator: None,
            handler,
        });
        id
    }

    /// Allocate a block that is NOT in the current handler region.
    /// Used for handler/catch blocks themselves.
    pub(super) fn new_block_no_handler(&mut self) -> BlockId {
        let id = self.blocks.len();
        self.blocks.push(BasicBlock {
            id,
            stmts: Vec::new(),
            terminator: None,
            handler: None,
        });
        id
    }

    pub(super) fn start_block(&mut self, id: BlockId) {
        debug_assert!(self.current.is_none(),
            "start_block: abandoning open block {}", self.current.unwrap());
        debug_assert!(self.blocks[id].terminator.is_none(),
            "start_block: block {} is already sealed", id);
        self.current = Some(id);
    }

    pub(super) fn seal(&mut self, terminator: Terminator) {
        let id = self.current.expect("seal: no current block");
        debug_assert!(self.blocks[id].terminator.is_none(),
            "seal: block {} already has a terminator", id);
        self.blocks[id].terminator = Some(terminator);
        self.current = None;
    }

    pub(super) fn push_stmt(&mut self, stmt: HirStmt) {
        if let Some(id) = self.current {
            self.blocks[id].stmts.push(stmt);
        }
    }

    pub(super) fn is_open(&self) -> bool {
        self.current.is_some()
    }

    pub(super) fn any_path_reaches(&self, target: BlockId) -> bool {
        self.blocks.iter().any(|b| {
            b.terminator.as_ref().map_or(false, |t| t.successors().contains(&target))
        })
    }

    pub(super) fn finish(mut self) -> Cfg {
        if let Some(id) = self.current {
            if self.blocks[id].terminator.is_none() {
                self.blocks[id].terminator = Some(Terminator::Return(None));
            }
            self.current = None;
        }
        // Seal unreachable allocated blocks
        for block in &mut self.blocks {
            if block.terminator.is_none() {
                block.terminator = Some(Terminator::Return(None));
            }
        }
        let mut cfg = Cfg {
            blocks: self.blocks,
            entry: 0,
            preds: Vec::new(),
            preds_valid: false,
        };
        cfg.compute_preds();
        cfg
    }

    // ── HIR lowering ────────────────────────────────────────────────

    pub(super) fn lower_stmts(&mut self, stmts: &[HirStmt]) {
        for stmt in stmts {
            self.lower_stmt(stmt);
        }
    }

    pub(super) fn lower_stmt(&mut self, stmt: &HirStmt) {
        if !self.is_open() { return; }

        match &stmt.kind {
            HirStmtKind::Empty => {}
            HirStmtKind::Assign { .. } | HirStmtKind::ProcCall { .. } => {
                self.push_stmt(stmt.clone());
            }

            HirStmtKind::If { cond, then_body, elsifs, else_body } => {
                self.lower_if(cond, then_body, elsifs, else_body);
            }

            HirStmtKind::While { cond, body } => {
                self.lower_while(cond, body);
            }
            HirStmtKind::Loop { body } => {
                self.lower_loop(body);
            }
            HirStmtKind::Exit => {
                self.lower_exit();
            }

            HirStmtKind::Repeat { body, cond } => {
                self.lower_repeat(body, cond);
            }
            HirStmtKind::For { var, var_ty, start, end, step, direction, body } => {
                self.lower_for(var, *var_ty, start, end, step, *direction, body);
            }

            HirStmtKind::Case { expr, branches, else_body } => {
                self.lower_case(expr, branches, else_body);
            }
            HirStmtKind::TypeCase { expr, branches, else_body } => {
                self.lower_typecase(expr, branches, else_body);
            }

            HirStmtKind::Try { body, excepts, finally_body } => {
                self.lower_try(body, excepts, finally_body);
            }
            HirStmtKind::Raise { expr } => {
                self.seal(Terminator::Raise(expr.clone()));
            }
            HirStmtKind::Retry => {
                // RETRY jumps back to the TRY body — modeled as opaque for now
                self.push_stmt(stmt.clone());
            }

            HirStmtKind::Lock { mutex, body } => {
                // Lock: lower as acquire → body → release (linear)
                self.push_stmt(HirStmt {
                    kind: HirStmtKind::ProcCall {
                        target: HirCallTarget::Direct(SymbolId {
                            mangled: "m2_Mutex_Lock".into(),
                            source_name: "Lock".into(),
                            module: None,
                            ty: crate::types::TY_VOID,
                            is_var_param: false,
                            is_open_array: false,
                        }),
                        args: vec![mutex.clone()],
                    },
                    loc: stmt.loc.clone(),
                });
                self.lower_stmts(body);
                if self.is_open() {
                    self.push_stmt(HirStmt {
                        kind: HirStmtKind::ProcCall {
                            target: HirCallTarget::Direct(SymbolId {
                                mangled: "m2_Mutex_Unlock".into(),
                                source_name: "Unlock".into(),
                                module: None,
                                ty: crate::types::TY_VOID,
                                is_var_param: false,
                                is_open_array: false,
                            }),
                            args: vec![mutex.clone()],
                        },
                        loc: stmt.loc.clone(),
                    });
                }
            }

            HirStmtKind::Return { expr } => {
                self.seal(Terminator::Return(expr.clone()));
            }
        }
    }

    // ── IF / ELSIF / ELSE ───────────────────────────────────────────

    pub(super) fn lower_if(
        &mut self,
        cond: &HirExpr,
        then_body: &[HirStmt],
        elsifs: &[(HirExpr, Vec<HirStmt>)],
        else_body: &Option<Vec<HirStmt>>,
    ) {
        let merge = self.new_block();
        let then_block = self.new_block();
        let first_false = if !elsifs.is_empty() || else_body.is_some() {
            self.new_block()
        } else {
            merge
        };

        self.lower_expr_as_branch(cond, then_block, first_false);

        self.start_block(then_block);
        self.lower_stmts(then_body);
        if self.is_open() { self.seal(Terminator::Goto(merge)); }

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
            if self.is_open() { self.seal(Terminator::Goto(merge)); }
            current_false = elsif_false;
        }

        if let Some(else_stmts) = else_body {
            self.start_block(current_false);
            self.lower_stmts(else_stmts);
            if self.is_open() { self.seal(Terminator::Goto(merge)); }
        }

        if self.any_path_reaches(merge) {
            self.start_block(merge);
        }
    }

    // ── WHILE ───────────────────────────────────────────────────────

    pub(super) fn lower_while(&mut self, cond: &HirExpr, body: &[HirStmt]) {
        let header = self.new_block();
        let body_block = self.new_block();
        let exit_block = self.new_block();

        self.seal(Terminator::Goto(header));
        self.start_block(header);
        self.lower_expr_as_branch(cond, body_block, exit_block);

        self.loop_exit_stack.push(exit_block);
        self.start_block(body_block);
        self.lower_stmts(body);
        if self.is_open() { self.seal(Terminator::Goto(header)); }
        self.loop_exit_stack.pop();

        self.start_block(exit_block);
    }

    // ── LOOP ────────────────────────────────────────────────────────

    pub(super) fn lower_loop(&mut self, body: &[HirStmt]) {
        let header = self.new_block();
        let exit_block = self.new_block();

        self.seal(Terminator::Goto(header));

        self.loop_exit_stack.push(exit_block);
        self.start_block(header);
        self.lower_stmts(body);
        if self.is_open() { self.seal(Terminator::Goto(header)); }
        self.loop_exit_stack.pop();

        self.start_block(exit_block);
    }

    // ── EXIT ────────────────────────────────────────────────────────

    pub(super) fn lower_exit(&mut self) {
        let exit_target = *self.loop_exit_stack.last()
            .expect("EXIT outside LOOP (sema should have caught this)");
        self.seal(Terminator::Goto(exit_target));
    }

    // ── REPEAT / UNTIL ──────────────────────────────────────────────

    pub(super) fn lower_repeat(&mut self, body: &[HirStmt], cond: &HirExpr) {
        let body_block = self.new_block();
        let exit_block = self.new_block();

        self.seal(Terminator::Goto(body_block));

        self.loop_exit_stack.push(exit_block);
        self.start_block(body_block);
        self.lower_stmts(body);
        if self.is_open() {
            // true = exit (condition met), false = loop back
            self.lower_expr_as_branch(cond, exit_block, body_block);
        }
        self.loop_exit_stack.pop();

        self.start_block(exit_block);
    }

    // ── FOR ─────────────────────────────────────────────────────────

    pub(super) fn lower_for(
        &mut self,
        var: &str,
        var_ty: TypeId,
        start: &HirExpr,
        end: &HirExpr,
        step: &Option<HirExpr>,
        direction: ForDirection,
        body: &[HirStmt],
    ) {
        // Emit init: var := start
        self.push_assign(var, var_ty, start);

        let cond_block = self.new_block();
        let body_block = self.new_block();
        let latch = self.new_block();
        let exit_block = self.new_block();

        self.seal(Terminator::Goto(cond_block));

        // Condition: var <= end (Up) or var >= end (Down)
        self.start_block(cond_block);
        let cmp_op = match direction {
            ForDirection::Up => BinaryOp::Le,
            ForDirection::Down => BinaryOp::Ge,
        };
        let cond_expr = self.make_binop(cmp_op, var, var_ty, end);
        self.seal(Terminator::Branch {
            cond: cond_expr,
            on_true: body_block,
            on_false: exit_block,
        });

        // Body
        self.loop_exit_stack.push(exit_block);
        self.start_block(body_block);
        self.lower_stmts(body);
        if self.is_open() { self.seal(Terminator::Goto(latch)); }
        self.loop_exit_stack.pop();

        // Latch: var := var + step
        // When step is provided (BY expr), it's the signed increment (e.g., -1 for BY -1).
        // When step is absent, default to +1 (Up) or -1 (Down).
        self.start_block(latch);
        let step_val = step.clone().unwrap_or_else(|| HirExpr {
            kind: HirExprKind::IntLit(match direction {
                ForDirection::Up => 1,
                ForDirection::Down => -1,
            }),
            ty: var_ty,
            loc: crate::errors::SourceLoc::new("<cfg>", 0, 0),
        });
        let new_val = self.make_binop(BinaryOp::Add, var, var_ty, &step_val);
        self.push_assign_expr(var, var_ty, new_val);
        self.seal(Terminator::Goto(cond_block));

        self.start_block(exit_block);
    }

    /// Push an assignment statement: var := expr
    pub(super) fn push_assign(&mut self, var: &str, ty: TypeId, value: &HirExpr) {
        self.push_assign_expr(var, ty, value.clone());
    }

    pub(super) fn push_assign_expr(&mut self, var: &str, ty: TypeId, value: HirExpr) {
        let place = Place {
            base: PlaceBase::Local(SymbolId {
                mangled: var.to_string(),
                source_name: var.to_string(),
                module: None,
                ty,
                is_var_param: false,
                is_open_array: false,
            }),
            projections: Vec::new(),
            ty,
            loc: crate::errors::SourceLoc::new("<cfg>", 0, 0),
        };
        self.push_stmt(HirStmt {
            kind: HirStmtKind::Assign { target: place, value },
            loc: crate::errors::SourceLoc::new("<cfg>", 0, 0),
        });
    }

    /// Create a binary operation: var op rhs
    pub(super) fn make_binop(&self, op: BinaryOp, var: &str, var_ty: TypeId, rhs: &HirExpr) -> HirExpr {
        // Comparison ops return BOOLEAN; arithmetic ops return the operand type.
        let result_ty = match op {
            BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Le
            | BinaryOp::Gt | BinaryOp::Ge => crate::types::TY_BOOLEAN,
            _ => var_ty,
        };
        HirExpr {
            kind: HirExprKind::BinaryOp {
                op,
                left: Box::new(HirExpr {
                    kind: HirExprKind::Place(Place {
                        base: PlaceBase::Local(SymbolId {
                            mangled: var.to_string(),
                            source_name: var.to_string(),
                            module: None,
                            ty: var_ty,
                            is_var_param: false,
                            is_open_array: false,
                        }),
                        projections: Vec::new(),
                        ty: var_ty,
                        loc: crate::errors::SourceLoc::new("<cfg>", 0, 0),
                    }),
                    ty: var_ty,
                    loc: crate::errors::SourceLoc::new("<cfg>", 0, 0),
                }),
                right: Box::new(rhs.clone()),
            },
            ty: result_ty,
            loc: crate::errors::SourceLoc::new("<cfg>", 0, 0),
        }
    }

    // ── CASE ────────────────────────────────────────────────────────

    pub(super) fn lower_case(
        &mut self,
        expr: &HirExpr,
        branches: &[HirCaseBranch],
        else_body: &Option<Vec<HirStmt>>,
    ) {
        let merge = self.new_block();
        let default_block = self.new_block();

        let mut arms = Vec::new();
        let mut arm_blocks = Vec::new();
        for branch in branches {
            let arm_block = self.new_block();
            arm_blocks.push(arm_block);
            let labels: Vec<CaseLabel> = branch.labels.iter().map(|l| match l {
                HirCaseLabel::Single(e) => CaseLabel::Single(e.clone()),
                HirCaseLabel::Range(lo, hi) => CaseLabel::Range(lo.clone(), hi.clone()),
            }).collect();
            arms.push(SwitchArm { labels, target: arm_block });
        }

        self.seal(Terminator::Switch {
            expr: expr.clone(),
            arms,
            default: default_block,
        });

        for (i, branch) in branches.iter().enumerate() {
            self.start_block(arm_blocks[i]);
            self.lower_stmts(&branch.body);
            if self.is_open() { self.seal(Terminator::Goto(merge)); }
        }

        self.start_block(default_block);
        if let Some(else_stmts) = else_body {
            self.lower_stmts(else_stmts);
        }
        if self.is_open() { self.seal(Terminator::Goto(merge)); }

        if self.any_path_reaches(merge) {
            self.start_block(merge);
        }
    }

    // ── TYPECASE ────────────────────────────────────────────────────

    pub(super) fn lower_typecase(
        &mut self,
        expr: &HirExpr,
        branches: &[HirTypeCaseBranch],
        else_body: &Option<Vec<HirStmt>>,
    ) {
        let merge = self.new_block();
        let default_block = self.new_block();

        let mut arms = Vec::new();
        let mut arm_blocks = Vec::new();
        for branch in branches {
            let arm_block = self.new_block();
            arm_blocks.push(arm_block);
            let labels: Vec<CaseLabel> = branch.types.iter()
                .map(|sid| CaseLabel::Type(sid.clone(), branch.var.clone()))
                .collect();
            arms.push(SwitchArm { labels, target: arm_block });
        }

        self.seal(Terminator::Switch {
            expr: expr.clone(),
            arms,
            default: default_block,
        });

        for (i, branch) in branches.iter().enumerate() {
            self.start_block(arm_blocks[i]);
            // TYPECASE binding: assign the matched value to the binding variable
            if let Some(ref var_name) = branch.var {
                let bind_place = Place {
                    base: PlaceBase::Local(SymbolId {
                        mangled: var_name.clone(),
                        source_name: var_name.clone(),
                        module: None,
                        ty: crate::types::TY_ADDRESS,
                        is_var_param: false,
                        is_open_array: false,
                    }),
                    projections: Vec::new(),
                    ty: crate::types::TY_ADDRESS,
                    loc: crate::errors::SourceLoc::new("<cfg>", 0, 0),
                };
                self.push_stmt(HirStmt {
                    kind: HirStmtKind::Assign {
                        target: bind_place,
                        value: expr.clone(),
                    },
                    loc: crate::errors::SourceLoc::new("<cfg>", 0, 0),
                });
            }
            self.lower_stmts(&branch.body);
            if self.is_open() { self.seal(Terminator::Goto(merge)); }
        }

        self.start_block(default_block);
        if let Some(else_stmts) = else_body {
            self.lower_stmts(else_stmts);
        }
        if self.is_open() { self.seal(Terminator::Goto(merge)); }

        if self.any_path_reaches(merge) {
            self.start_block(merge);
        }
    }

    // ── TRY / EXCEPT / FINALLY ──────────────────────────────────────

    pub(super) fn lower_try(
        &mut self,
        body: &[HirStmt],
        excepts: &[HirExceptClause],
        finally_body: &Option<Vec<HirStmt>>,
    ) {
        let merge = self.new_block();

        // The catch dispatch block handles exception matching.
        let catch_block = self.new_block_no_handler();

        // Finally block (normal path)
        let finally_normal = if finally_body.is_some() {
            self.new_block()
        } else {
            merge
        };

        // TRY body: all blocks inside get handler = catch_block
        self.handler_stack.push(catch_block);
        // Start a fresh block so it inherits the handler from the stack
        let try_body = self.new_block();
        if self.is_open() {
            self.seal(Terminator::Goto(try_body));
        }
        self.start_block(try_body);
        self.lower_stmts(body);
        self.handler_stack.pop();

        // Normal exit from TRY body → finally (or merge)
        if self.is_open() {
            self.seal(Terminator::Goto(finally_normal));
        }

        // Catch dispatch
        self.start_block(catch_block);
        if excepts.is_empty() {
            // No handlers — run finally then reraise
            if let Some(ref fb) = finally_body {
                for s in fb { self.push_stmt(s.clone()); }
            }
            self.seal(Terminator::Raise(None)); // reraise
        } else {
            // Build exception dispatch as Switch-like chain of branches
            // For simplicity, use IF-chain since exception IDs are runtime values
            let reraise_block = self.new_block_no_handler();
            let mut handler_blocks = Vec::new();
            for _ in excepts {
                handler_blocks.push(self.new_block_no_handler());
            }

            // Chain: check each exception ID
            for (i, ec) in excepts.iter().enumerate() {
                if let Some(ref exc_sym) = ec.exception {
                    let next = if i + 1 < excepts.len() {
                        // next check
                        let next_check = self.new_block_no_handler();
                        // Branch on exception match
                        // Use the exception symbol as the comparison
                        let cond = HirExpr {
                            kind: HirExprKind::Place(Place {
                                base: PlaceBase::Local(exc_sym.clone()),
                                projections: Vec::new(),
                                ty: exc_sym.ty,
                                loc: crate::errors::SourceLoc::new("<cfg>", 0, 0),
                            }),
                            ty: crate::types::TY_BOOLEAN,
                            loc: crate::errors::SourceLoc::new("<cfg>", 0, 0),
                        };
                        self.seal(Terminator::Branch {
                            cond,
                            on_true: handler_blocks[i],
                            on_false: next_check,
                        });
                        self.start_block(next_check);
                        next_check
                    } else {
                        // Last handler — else goes to reraise
                        let cond = HirExpr {
                            kind: HirExprKind::Place(Place {
                                base: PlaceBase::Local(exc_sym.clone()),
                                projections: Vec::new(),
                                ty: exc_sym.ty,
                                loc: crate::errors::SourceLoc::new("<cfg>", 0, 0),
                            }),
                            ty: crate::types::TY_BOOLEAN,
                            loc: crate::errors::SourceLoc::new("<cfg>", 0, 0),
                        };
                        self.seal(Terminator::Branch {
                            cond,
                            on_true: handler_blocks[i],
                            on_false: reraise_block,
                        });
                        reraise_block
                    };
                    let _ = next;
                } else {
                    // Catch-all handler
                    self.seal(Terminator::Goto(handler_blocks[i]));
                }
            }

            // Handler bodies
            for (i, ec) in excepts.iter().enumerate() {
                self.start_block(handler_blocks[i]);
                self.lower_stmts(&ec.body);
                if self.is_open() {
                    self.seal(Terminator::Goto(finally_normal));
                }
            }

            // Reraise block
            self.start_block(reraise_block);
            if let Some(ref fb) = finally_body {
                for s in fb { self.push_stmt(s.clone()); }
            }
            self.seal(Terminator::Raise(None));
        }

        // Finally (normal path)
        if let Some(ref fb) = finally_body {
            self.start_block(finally_normal);
            for s in fb { self.push_stmt(s.clone()); }
            if self.is_open() {
                self.seal(Terminator::Goto(merge));
            }
        }

        if self.any_path_reaches(merge) {
            self.start_block(merge);
        }
    }

    // ── Short-circuit boolean lowering ──────────────────────────────

    pub(super) fn lower_expr_as_branch(
        &mut self,
        expr: &HirExpr,
        on_true: BlockId,
        on_false: BlockId,
    ) {
        match &expr.kind {
            HirExprKind::BinaryOp { op: BinaryOp::And, left, right } => {
                let rhs_block = self.new_block();
                self.lower_expr_as_branch(left, rhs_block, on_false);
                self.start_block(rhs_block);
                self.lower_expr_as_branch(right, on_true, on_false);
            }
            HirExprKind::BinaryOp { op: BinaryOp::Or, left, right } => {
                let rhs_block = self.new_block();
                self.lower_expr_as_branch(left, on_true, rhs_block);
                self.start_block(rhs_block);
                self.lower_expr_as_branch(right, on_true, on_false);
            }
            HirExprKind::Not(inner) => {
                self.lower_expr_as_branch(inner, on_false, on_true);
            }
            _ => {
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

pub fn build_cfg(body: &[HirStmt]) -> Cfg {
    let mut builder = CfgBuilder::new();
    builder.lower_stmts(body);
    let cfg = builder.finish();
    cfg.verify();
    cfg
}
