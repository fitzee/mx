//! CFG-driven code emission for the LLVM backend.
//!
//! Emits procedure and init bodies by iterating CFG basic blocks in order.
//! All control flow comes from CFG terminators (Goto, Branch, Switch,
//! Return, Raise). No structured control flow is reconstructed.

use std::collections::HashMap;
use crate::cfg::{BlockId, CaseLabel, Cfg, Terminator};
use crate::hir::*;

impl super::LLVMCodeGen {
    /// Emit a procedure/init body from its pre-built CFG.
    /// `is_void`: true for void-returning functions, false for non-void.
    pub(crate) fn emit_cfg_body(&mut self, cfg: &Cfg, is_void: bool) {
        // Collect handler regions for SJLJ frame management
        let handler_frames = self.collect_handler_regions_llvm(cfg);

        // Allocate SJLJ frames at function entry (before any labels)
        if !handler_frames.is_empty() {
            self.declare_exc_runtime();
        }
        for (_handler_id, frame_name) in &handler_frames {
            self.emitln(&format!("  {} = alloca [256 x i8]", frame_name));
        }

        // Allocate TYPECASE binding variables and register in locals
        {
            let mut declared = std::collections::HashSet::new();
            for block in &cfg.blocks {
                if let Some(crate::cfg::Terminator::Switch { arms, .. }) = &block.terminator {
                    for arm in arms {
                        for label in &arm.labels {
                            if let CaseLabel::Type(_, Some(ref var_name)) = label {
                                if declared.insert(var_name.clone()) {
                                    let alloca = self.next_tmp();
                                    self.emitln(&format!("  {} = alloca ptr", alloca));
                                    self.locals.last_mut().unwrap().insert(
                                        var_name.clone(), (alloca, "ptr".to_string()));
                                }
                            }
                        }
                    }
                }
            }
        }

        // Track current handler region
        let mut current_handler: Option<BlockId> = None;
        // Track active catch frame for Raise(None) reraise
        let mut active_catch_frame: Option<String> = None;
        // Track which handler frames have been initialized
        let mut initialized_frames: std::collections::HashSet<BlockId> = std::collections::HashSet::new();

        for block in &cfg.blocks {
            // Emit label (block 0 = entry, shares bb.entry)
            if block.id > 0 {
                self.emitln(&format!("B{}:", block.id));
                self.current_block = format!("B{}", block.id);
            }

            // Track active catch frame — if this block IS a handler target
            if let Some(frame) = handler_frames.get(&block.id) {
                active_catch_frame = Some(frame.clone());
            }

            // Handler region transition
            if block.handler != current_handler {
                // Pop old handler frame ONLY when leaving all handler regions.
                // Skip pop if this block IS a handler target (setjmp code already popped).
                if let Some(old_h) = current_handler {
                    if block.handler.is_none() && !handler_frames.contains_key(&block.id) {
                        if let Some(frame_name) = handler_frames.get(&old_h) {
                            self.emitln(&format!("  call void @m2_exc_pop(ptr {})", frame_name));
                        }
                    }
                }
                // Set up new handler frame (only if not already initialized)
                if let Some(new_h) = block.handler {
                    if !initialized_frames.contains(&new_h) {
                        if let Some(frame_name) = handler_frames.get(&new_h) {
                            self.emitln(&format!("  call void @m2_exc_push(ptr {})", frame_name));
                            let sjret = self.next_tmp();
                            self.emitln(&format!("  {} = call i32 @setjmp(ptr {})", sjret, frame_name));
                            let caught = self.next_tmp();
                            self.emitln(&format!("  {} = icmp ne i32 {}, 0", caught, sjret));
                            let exc_label = self.next_label("sjlj.exc");
                            let cont_label = self.next_label("sjlj.cont");
                            self.emitln(&format!("  br i1 {}, label %{}, label %{}", caught, exc_label, cont_label));
                            // Exception path: pop frame then goto handler
                            self.emitln(&format!("{}:", exc_label));
                            self.emitln(&format!("  call void @m2_exc_pop(ptr {})", frame_name));
                            self.emitln(&format!("  br label %B{}", new_h));
                            // Normal path continues
                            self.emitln(&format!("{}:", cont_label));
                            self.current_block = cont_label;
                            initialized_frames.insert(new_h);
                        }
                    }
                }
                current_handler = block.handler;
            }

            // Emit block statements (only Assign/ProcCall/Empty in CFG blocks)
            for stmt in &block.stmts {
                // Update debug location per statement
                if stmt.loc.line > 0 {
                    if let Some(ref mut di) = self.di {
                        di.set_location(stmt.loc.line, 0, &stmt.loc.file);
                    }
                }
                self.gen_hir_statement(stmt);
            }

            // For Return inside a handler region, pop frame before exiting.
            // Raise does NOT pop — the handler frame must stay active.
            if let Some(ref term) = block.terminator {
                if current_handler.is_some() {
                    if matches!(term, Terminator::Return(_)) {
                        if let Some(h) = current_handler {
                            if let Some(frame_name) = handler_frames.get(&h) {
                                self.emitln(&format!("  call void @m2_exc_pop(ptr {})", frame_name));
                            }
                        }
                    }
                }
            }

            // Emit terminator
            if let Some(ref term) = block.terminator {
                self.emit_llvm_terminator(term, cfg, is_void, &handler_frames, block.id, &active_catch_frame);
            }
        }
    }

    /// Emit an LLVM terminator instruction.
    fn emit_llvm_terminator(&mut self, term: &Terminator, _cfg: &Cfg, is_void: bool,
                            handler_frames: &HashMap<BlockId, String>, current_block_id: BlockId,
                            active_catch_frame: &Option<String>) {
        match term {
            Terminator::Goto(target) => {
                self.emitln(&format!("  br label %B{}", target));
            }

            Terminator::Branch { cond, on_true, on_false } => {
                // Check for exception dispatch condition
                let cond_i1 = if let HirExprKind::Place(ref place) = cond.kind {
                    if let crate::hir::PlaceBase::Local(ref sid) = place.base {
                        if matches!(self.sema.types.get(sid.ty), crate::types::Type::Exception { .. })
                            || self.sema.symtab.lookup_any(&sid.source_name)
                                .map(|s| matches!(s.kind, crate::symtab::SymbolKind::Constant(_)))
                                .unwrap_or(false)
                                && {
                                    // Check if it's an exception constant
                                    let exc_id = self.sema.symtab.lookup_any(&sid.source_name)
                                        .and_then(|s| if let crate::symtab::SymbolKind::Constant(
                                            crate::symtab::ConstValue::Integer(v)) = &s.kind { Some(*v) } else { None });
                                    exc_id.is_some()
                                }
                        {
                            // Exception dispatch: compare frame.exception_id against exception ID
                            let exc_id = self.sema.symtab.lookup_any(&sid.source_name)
                                .and_then(|s| if let crate::symtab::SymbolKind::Constant(
                                    crate::symtab::ConstValue::Integer(v)) = &s.kind { Some(*v as i32) } else { None })
                                .unwrap_or(0);
                            let frame = handler_frames.get(&current_block_id)
                                .or_else(|| active_catch_frame.as_ref())
                                .cloned()
                                .unwrap_or_else(|| "%_ef".to_string());
                            // Get exception ID from frame
                            if !self.declared_fns.contains("m2_exc_get_id") {
                                self.declare_exc_runtime();
                            }
                            let got_id = self.next_tmp();
                            self.emitln(&format!("  {} = call i32 @m2_exc_get_id(ptr {})", got_id, frame));
                            let cmp = self.next_tmp();
                            self.emitln(&format!("  {} = icmp eq i32 {}, {}", cmp, got_id, exc_id));
                            cmp
                        } else {
                            let val = self.gen_hir_expr(cond);
                            if val.ty != "i1" {
                                let tmp = self.next_tmp();
                                self.emitln(&format!("  {} = icmp ne {} {}, 0", tmp, val.ty, val.name));
                                tmp
                            } else { val.name }
                        }
                    } else {
                        let val = self.gen_hir_expr(cond);
                        if val.ty != "i1" {
                            let tmp = self.next_tmp();
                            self.emitln(&format!("  {} = icmp ne {} {}, 0", tmp, val.ty, val.name));
                            tmp
                        } else { val.name }
                    }
                } else {
                    let val = self.gen_hir_expr(cond);
                    if val.ty != "i1" {
                        let tmp = self.next_tmp();
                        self.emitln(&format!("  {} = icmp ne {} {}, 0", tmp, val.ty, val.name));
                        tmp
                    } else { val.name }
                };
                self.emitln(&format!("  br i1 {}, label %B{}, label %B{}", cond_i1, on_true, on_false));
            }

            Terminator::Switch { expr, arms, default } => {
                let switch_val = self.gen_hir_expr(expr);
                // Emit as if-chain (matching C backend pattern)
                for (i, arm) in arms.iter().enumerate() {
                    let match_label = self.next_label("sw.match");
                    let next_label = self.next_label("sw.next");

                    // Build OR of all labels in this arm
                    let mut result: Option<String> = None;
                    for label in &arm.labels {
                        let cmp = match label {
                            CaseLabel::Single(e) => {
                                let v = self.gen_hir_expr(e);
                                let tmp = self.next_tmp();
                                self.emitln(&format!("  {} = icmp eq {} {}, {}", tmp, switch_val.ty, switch_val.name, v.name));
                                tmp
                            }
                            CaseLabel::Range(lo, hi) => {
                                let lo_v = self.gen_hir_expr(lo);
                                let hi_v = self.gen_hir_expr(hi);
                                let ge = self.next_tmp();
                                let le = self.next_tmp();
                                let both = self.next_tmp();
                                self.emitln(&format!("  {} = icmp sge {} {}, {}", ge, switch_val.ty, switch_val.name, lo_v.name));
                                self.emitln(&format!("  {} = icmp sle {} {}, {}", le, switch_val.ty, switch_val.name, hi_v.name));
                                self.emitln(&format!("  {} = and i1 {}, {}", both, ge, le));
                                both
                            }
                            CaseLabel::Type(sym, _) => {
                                // TYPECASE: call M2_ISA
                                let td_name = if let Some(ref module) = sym.module {
                                    format!("@M2_TD_{}_{}", module, sym.source_name)
                                } else {
                                    format!("@M2_TD_{}_{}", self.module_name, sym.source_name)
                                };
                                if !self.declared_fns.contains("M2_ISA") {
                                    self.emit_preambleln("declare i32 @M2_ISA(ptr, ptr)");
                                    self.declared_fns.insert("M2_ISA".to_string());
                                }
                                let isa = self.next_tmp();
                                self.emitln(&format!("  {} = call i32 @M2_ISA(ptr {}, ptr {})", isa, switch_val.name, td_name));
                                let isa_bool = self.next_tmp();
                                self.emitln(&format!("  {} = icmp ne i32 {}, 0", isa_bool, isa));
                                isa_bool
                            }
                        };
                        result = Some(match result {
                            None => cmp,
                            Some(prev) => {
                                let or_tmp = self.next_tmp();
                                self.emitln(&format!("  {} = or i1 {}, {}", or_tmp, prev, cmp));
                                or_tmp
                            }
                        });
                    }

                    if let Some(final_cond) = result {
                        // Last arm: false goes to default. Others: false goes to next test.
                        let false_target = if i + 1 < arms.len() {
                            format!("{}", next_label)
                        } else {
                            format!("B{}", default)
                        };
                        self.emitln(&format!("  br i1 {}, label %B{}, label %{}", final_cond, arm.target, false_target));
                        if i + 1 < arms.len() {
                            self.emitln(&format!("{}:", next_label));
                            self.current_block = next_label;
                        }
                    } else {
                        // Empty arm — goto target unconditionally
                        self.emitln(&format!("  br label %B{}", arm.target));
                    }
                }
                if arms.is_empty() {
                    // No arms — goto default
                    self.emitln(&format!("  br label %B{}", default));
                }
            }

            Terminator::Return(expr) => {
                // Pop stack frame before return
                if let Some(ref frame) = self.stack_frame_alloca.clone() {
                    self.emitln(&format!("  call void @m2_stack_pop(ptr {})", frame));
                }
                if let Some(e) = expr {
                    let val = self.gen_hir_expr(e);
                    let ret_ty = self.current_return_type.clone().unwrap_or_else(|| "void".to_string());
                    let final_val = if ret_ty.starts_with('{') && val.ty == "ptr" {
                        let tmp = self.next_tmp();
                        self.emitln(&format!("  {} = load {}, ptr {}", tmp, ret_ty, val.name));
                        super::Val::new(tmp, ret_ty.clone())
                    } else {
                        val
                    };
                    let coerced = self.coerce_val(&final_val, &ret_ty);
                    self.emitln(&format!("  ret {} {}", ret_ty, coerced.name));
                } else if is_void {
                    self.emitln("  ret void");
                } else {
                    let ret_ty = self.current_return_type.clone().unwrap_or_else(|| "i32".to_string());
                    let zero = self.llvm_zero_initializer(&ret_ty);
                    self.emitln(&format!("  ret {} {}", ret_ty, zero));
                }
                // Dead label after return (LLVM requires basic blocks to end with terminator)
                let dead = self.next_label("ret.dead");
                self.emitln(&format!("{}:", dead));
                self.emitln("  unreachable");
                self.current_block = dead;
            }

            Terminator::Raise(expr) => {
                self.declare_exc_runtime();
                if let Some(e) = expr {
                    let val = self.gen_hir_expr(e);
                    let name_str = if let HirExprKind::IntLit(v) = &e.kind {
                        if let crate::types::Type::Exception { name } = self.sema.types.get(*v as crate::types::TypeId) {
                            let name_owned = name.clone();
                            let (sname, _) = self.intern_string(&name_owned);
                            sname
                        } else { "null".to_string() }
                    } else { "null".to_string() };
                    self.emitln(&format!("  call void @m2_raise(i32 {}, ptr {}, ptr null)", val.name, name_str));
                } else {
                    // Reraise — use the caught exception's data from the handler frame
                    if let Some(ref frame) = active_catch_frame {
                        self.emitln(&format!("  call void @m2_exc_reraise(ptr {})", frame));
                    } else {
                        self.emitln("  call void @m2_raise(i32 1, ptr null, ptr null)");
                    }
                }
                self.emitln("  unreachable");
                let dead = self.next_label("raise.dead");
                self.emitln(&format!("{}:", dead));
                self.emitln("  unreachable");
                self.current_block = dead;
            }
        }
    }

    /// Scan CFG for distinct handler regions, allocate LLVM temp names.
    fn collect_handler_regions_llvm(&mut self, cfg: &Cfg) -> HashMap<BlockId, String> {
        let mut frames = HashMap::new();
        for block in &cfg.blocks {
            if let Some(handler_id) = block.handler {
                if !frames.contains_key(&handler_id) {
                    let name = self.next_tmp();
                    frames.insert(handler_id, name);
                }
            }
        }
        frames
    }
}
