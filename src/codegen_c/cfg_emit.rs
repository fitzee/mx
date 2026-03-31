//! CFG-driven code emission for the C backend.
//!
//! Emits procedure and init bodies by iterating CFG basic blocks in order.
//! All control flow comes from CFG terminators (Goto, Branch, Switch,
//! Return, Raise). No structured control flow is reconstructed.

use std::collections::HashMap;
use crate::cfg::{BasicBlock, BlockId, CaseLabel, Cfg, Terminator};
use crate::hir::*;
use crate::types::*;

impl super::CodeGen {
    /// Emit a procedure/init body from its pre-built CFG.
    pub(crate) fn emit_cfg_body(&mut self, cfg: &Cfg) {
        self.emit_cfg_body_inner(cfg, None);
    }

    /// Emit a module init body from its pre-built CFG.
    /// void_return_value: for main() context, Return(None) emits "return 0;" instead of "return;".
    pub(crate) fn emit_cfg_body_with_return(&mut self, cfg: &Cfg, void_return_value: &str) {
        self.emit_cfg_body_inner(cfg, Some(void_return_value));
    }

    fn emit_cfg_body_inner(&mut self, cfg: &Cfg, void_return_value: Option<&str>) {
        // Collect handler regions: handler_block_id → frame variable name
        let handler_frames = self.collect_handler_regions(cfg);

        // Declare SJLJ frame variables at function scope (before any labels)
        for (_handler_id, frame_name) in &handler_frames {
            self.emitln(&format!("m2_ExcFrame {};", frame_name));
        }

        // Declare TYPECASE binding variables at function scope
        {
            let mut declared = std::collections::HashSet::new();
            for block in &cfg.blocks {
                if let Some(Terminator::Switch { arms, .. }) = &block.terminator {
                    for arm in arms {
                        for label in &arm.labels {
                            if let CaseLabel::Type(ref sym, Some(ref var_name)) = label {
                                if declared.insert(var_name.clone()) {
                                    // Use the type's C name for proper typing
                                    let type_name = if let Some(ref m) = sym.module {
                                        format!("{}_{}", m, sym.source_name)
                                    } else {
                                        self.mangle(&sym.source_name)
                                    };
                                    self.emitln(&format!("{} {};", type_name, var_name));
                                }
                            }
                        }
                    }
                }
            }
        }

        // Track which handler region we're currently inside
        let mut current_handler: Option<BlockId> = None;
        // Track the "active catch frame" — set when we enter a handler dispatch block
        let mut active_catch_frame: Option<String> = None;
        // Track which handler frames have been initialized (setjmp called)
        let mut initialized_frames: std::collections::HashSet<BlockId> = std::collections::HashSet::new();

        for block in &cfg.blocks {
            // Emit label (block 0 is entry — falls through from function header)
            if block.id > 0 {
                // C labels must be followed by a statement; use ; as empty stmt
                self.emit(&format!("  L{}:;\n", block.id));
            }

            // Track active catch frame — if this block IS a handler target
            if let Some(frame) = handler_frames.get(&block.id) {
                active_catch_frame = Some(frame.clone());
            }

            // Handler region transition — use raw setjmp (not M2_TRY) to avoid scope issues
            if block.handler != current_handler {
                // Pop old handler frame ONLY when leaving all handler regions (going to None).
                // When transitioning to a different handler, don't pop — the new frame
                // chains on top, and the runtime stack handles nesting.
                // Also skip pop if this block IS a handler target (setjmp code already popped).
                if let Some(old_h) = current_handler {
                    if block.handler.is_none() && !handler_frames.contains_key(&block.id) {
                        if let Some(frame_name) = handler_frames.get(&old_h) {
                            self.emitln(&format!("m2_exc_stack = {}.prev;", frame_name));
                        }
                    }
                }
                // Set up new handler frame with setjmp (only if not already initialized)
                if let Some(new_h) = block.handler {
                    if !initialized_frames.contains(&new_h) {
                        if let Some(frame_name) = handler_frames.get(&new_h) {
                            self.emitln(&format!("{}.prev = m2_exc_stack;", frame_name));
                            self.emitln(&format!("{}.exception_id = 0;", frame_name));
                            self.emitln(&format!("{}.exception_name = NULL;", frame_name));
                            self.emitln(&format!("{}.exception_arg = NULL;", frame_name));
                            self.emitln(&format!("m2_exc_stack = &{};", frame_name));
                            self.emitln(&format!("if (setjmp({}.buf) != 0) {{", frame_name));
                            self.emitln(&format!("  m2_exc_stack = {}.prev;", frame_name));
                            self.emitln(&format!("  goto L{};", new_h));
                            self.emitln("}");
                            initialized_frames.insert(new_h);
                        }
                    }
                }
                current_handler = block.handler;
            }

            // Emit block statements (only Assign/ProcCall/Empty in CFG blocks)
            for stmt in &block.stmts {
                self.emit_hir_stmt(stmt);
            }

            // For Return inside a handler region, pop frame before exiting.
            // Raise does NOT pop — the handler frame must stay active for longjmp to catch it.
            if let Some(ref term) = block.terminator {
                if current_handler.is_some() {
                    if matches!(term, Terminator::Return(_)) {
                        if let Some(h) = current_handler {
                            if let Some(frame_name) = handler_frames.get(&h) {
                                self.emitln(&format!("m2_exc_stack = {}.prev;", frame_name));
                            }
                        }
                    }
                }
            }

            // Emit terminator
            if let Some(ref term) = block.terminator {
                self.emit_c_terminator(term, cfg, void_return_value, &handler_frames, block.id, &active_catch_frame);
            }
        }
    }

    /// Emit a C terminator instruction.
    fn emit_c_terminator(&mut self, term: &Terminator, _cfg: &Cfg, void_return_value: Option<&str>,
                         handler_frames: &HashMap<BlockId, String>, current_block_id: BlockId,
                         active_catch_frame: &Option<String>) {
        match term {
            Terminator::Goto(target) => {
                self.emitln(&format!("goto L{};", target));
            }

            Terminator::Branch { cond, on_true, on_false } => {
                // Check if this is an exception dispatch condition
                let cond_str = self.maybe_exception_cond(cond, handler_frames, current_block_id, _cfg);
                self.emitln(&format!("if ({}) goto L{}; else goto L{};", cond_str, on_true, on_false));
            }

            Terminator::Switch { expr, arms, default } => {
                let switch_val = self.hir_expr_to_string(expr);
                // Emit as if-chain (handles Range and Type labels uniformly)
                for arm in arms {
                    let cond = self.switch_arm_condition(&switch_val, &arm.labels);
                    self.emitln(&format!("if ({}) goto L{};", cond, arm.target));
                }
                self.emitln(&format!("goto L{};", default));
            }

            Terminator::Return(expr) => {
                self.emit_indent();
                if let Some(e) = expr {
                    self.emit("return ");
                    self.emit_hir_expr(e);
                    self.emit(";\n");
                } else if let Some(val) = void_return_value {
                    // Module init context — return 0 from main()
                    self.emit(&format!("return {};\n", val));
                } else {
                    self.emit("return;\n");
                }
            }

            Terminator::Raise(expr) => {
                self.emit_indent();
                if let Some(e) = expr {
                    let exc_c_name = match &e.kind {
                        HirExprKind::IntLit(v) => {
                            if let crate::types::Type::Exception { name } = self.sema.types.get(*v as TypeId) {
                                Some(format!("M2_EXC_{}", self.mangle(&name)))
                            } else { None }
                        }
                        _ => None,
                    };
                    if let Some(c_name) = exc_c_name {
                        self.emit(&format!("m2_raise({}, \"{}\", NULL);\n", c_name,
                            c_name.strip_prefix("M2_EXC_").unwrap_or(&c_name)));
                    } else {
                        let s = self.hir_expr_to_string(e);
                        self.emit(&format!("m2_raise((int)({}), NULL, NULL);\n", s));
                    }
                } else {
                    // Reraise — use the caught exception's data from the handler frame
                    if let Some(ref frame) = active_catch_frame {
                        self.emit(&format!("m2_raise({f}.exception_id, {f}.exception_name, {f}.exception_arg);\n", f = frame));
                    } else {
                        self.emitln("m2_raise(1, NULL, NULL);");
                    }
                }
            }
        }
    }

    /// Build condition string for a Switch arm's labels (OR-combined).
    fn switch_arm_condition(&mut self, switch_val: &str, labels: &[CaseLabel]) -> String {
        let parts: Vec<String> = labels.iter().map(|label| {
            match label {
                CaseLabel::Single(expr) => {
                    let v = self.hir_expr_to_string(expr);
                    format!("({} == {})", switch_val, v)
                }
                CaseLabel::Range(lo, hi) => {
                    let lo_s = self.hir_expr_to_string(lo);
                    let hi_s = self.hir_expr_to_string(hi);
                    format!("({} >= {} && {} <= {})", switch_val, lo_s, switch_val, hi_s)
                }
                CaseLabel::Type(sym, _) => {
                    // TYPECASE: M2_ISA(value, &M2_TD_TypeName)
                    let td_name = if let Some(ref module) = sym.module {
                        format!("M2_TD_{}_{}", module, sym.source_name)
                    } else {
                        format!("M2_TD_{}", sym.source_name)
                    };
                    format!("M2_ISA({}, &{})", switch_val, td_name)
                }
            }
        }).collect();

        if parts.len() == 1 {
            parts[0].clone()
        } else {
            parts.join(" || ")
        }
    }

    /// Check if a Branch condition is an exception dispatch (Place referencing an exception symbol).
    /// If so, emit `frame.exception_id == M2_EXC_name` instead of the bare identifier.
    fn maybe_exception_cond(&mut self, cond: &HirExpr, handler_frames: &HashMap<BlockId, String>,
                            current_block_id: BlockId, cfg: &Cfg) -> String {
        if let HirExprKind::Place(ref place) = cond.kind {
            if let PlaceBase::Local(ref sid) = place.base {
                let name = &sid.source_name;
                // Check if this name is a known exception
                let is_exception = self.exception_names.contains(name)
                    || self.def_exception_names.values().any(|v| v.contains(name));
                if is_exception {
                    let exc_name = format!("M2_EXC_{}", self.mangle(name));
                    // Find the frame for this catch dispatch block
                    let frame = handler_frames.get(&current_block_id)
                        .cloned()
                        .unwrap_or_else(|| {
                            // The current block might not be directly in handler_frames.
                            // Walk predecessor blocks to find the handler frame.
                            handler_frames.values().next()
                                .cloned()
                                .unwrap_or_else(|| "_ef".to_string())
                        });
                    return format!("{}.exception_id == {}", frame, exc_name);
                }
            }
        }
        self.hir_expr_to_string(cond)
    }

    /// Scan the CFG for distinct handler regions and allocate frame variable names.
    fn collect_handler_regions(&self, cfg: &Cfg) -> HashMap<BlockId, String> {
        let mut frames = HashMap::new();
        let mut counter = 0;
        for block in &cfg.blocks {
            if let Some(handler_id) = block.handler {
                if !frames.contains_key(&handler_id) {
                    frames.insert(handler_id, format!("_ef_{}", counter));
                    counter += 1;
                }
            }
        }
        frames
    }
}
