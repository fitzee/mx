//! CFG-level lint checks (Tier 2).
//!
//! W10: Uninitialized variable detection using forward "definitely assigned"
//! dataflow analysis over the CFG.

use super::{BlockId, Cfg, Terminator};
use super::dataflow::{ForwardAnalysis, solve_forward};
use crate::errors::{CompileError, SourceLoc};
use crate::hir::{
    HirExpr, HirExprKind, HirStmt, HirStmtKind, HirCallTarget,
    Place, PlaceBase, HirProcDecl, HirLocalDecl, HirParamDecl,
};
use crate::types::{Type, TypeId, TypeRegistry};

// ── Helpers ─────────────────────────────────────────────────────────

/// Resolve type aliases to their underlying type.
fn resolve_alias(types: &TypeRegistry, mut id: TypeId) -> TypeId {
    for _ in 0..50 {
        if let Type::Alias { target, .. } = types.get(id) {
            id = *target;
        } else {
            break;
        }
    }
    id
}

/// Builtins that define (assign to) their first argument.
/// These are lowered without AddrOf wrapping in the HIR.
fn is_defining_builtin(name: &str) -> bool {
    matches!(name, "NEW" | "INC" | "DEC" | "INCL" | "EXCL")
}

// ── Variable index mapping ──────────────────────────────────────────

/// Maps local variable names to bitset indices for dataflow analysis.
struct VarMap {
    /// (name, source_loc) pairs indexed by bit position.
    vars: Vec<(String, SourceLoc)>,
}

impl VarMap {
    /// Build a VarMap from procedure locals (not params — params are
    /// always initialized by the caller).
    /// Excludes aggregate types (arrays, records) which always have
    /// stack-allocated storage — uninitialized contents is a different
    /// concern than uninitialized scalars.
    fn from_proc(proc: &HirProcDecl, types: &TypeRegistry) -> Self {
        let mut vars = Vec::new();
        for local in &proc.locals {
            if let HirLocalDecl::Var { name, type_id } = local {
                let resolved = resolve_alias(types, *type_id);
                let ty = types.get(resolved);
                // Skip aggregates: they occupy memory and are "defined" by declaration
                if matches!(ty, Type::Array { .. } | Type::Record { .. }
                    | Type::OpenArray { .. }) {
                    continue;
                }
                vars.push((name.clone(), proc.loc.clone()));
            }
        }
        VarMap { vars }
    }

    fn len(&self) -> usize {
        self.vars.len()
    }

    /// Find the index for a variable name, or None if not tracked.
    fn index_of(&self, name: &str) -> Option<usize> {
        self.vars.iter().position(|(n, _)| n == name)
    }
}

// ── Bitset state ────────────────────────────────────────────────────

/// A fixed-size bitset representing which variables are definitely assigned.
/// Bit i set → variable i has been assigned on all paths reaching this point.
#[derive(Clone, PartialEq)]
struct AssignedSet {
    bits: Vec<bool>,
}

impl AssignedSet {
    fn all_false(n: usize) -> Self {
        AssignedSet { bits: vec![false; n] }
    }

    fn all_true(n: usize) -> Self {
        AssignedSet { bits: vec![true; n] }
    }

    fn set(&mut self, idx: usize) {
        if idx < self.bits.len() {
            self.bits[idx] = true;
        }
    }

    fn get(&self, idx: usize) -> bool {
        idx < self.bits.len() && self.bits[idx]
    }

    /// Intersection: bit is set only if set in both.
    fn intersect(&self, other: &Self) -> Self {
        AssignedSet {
            bits: self.bits.iter().zip(&other.bits)
                .map(|(a, b)| *a && *b)
                .collect(),
        }
    }
}

// ── Definitely Assigned analysis ────────────────────────────────────

struct DefinitelyAssigned<'a> {
    var_map: &'a VarMap,
    param_names: Vec<String>,
}

impl<'a> ForwardAnalysis for DefinitelyAssigned<'a> {
    type State = AssignedSet;

    fn entry_state(&self) -> AssignedSet {
        // At entry, no locals are assigned (params are not tracked)
        AssignedSet::all_false(self.var_map.len())
    }

    fn bottom(&self) -> AssignedSet {
        // For a "must" (intersection) analysis, bottom = all-true
        // so the first real predecessor narrows it down.
        AssignedSet::all_true(self.var_map.len())
    }

    fn join(&self, a: &AssignedSet, b: &AssignedSet) -> AssignedSet {
        // Must-analysis: variable is definitely assigned only if
        // assigned on ALL incoming paths → intersection.
        a.intersect(b)
    }

    fn transfer(&self, cfg: &Cfg, block_id: BlockId, state: &AssignedSet) -> AssignedSet {
        let mut out = state.clone();
        let block = &cfg.blocks[block_id];

        // Process each statement in the block
        for stmt in &block.stmts {
            self.transfer_stmt(stmt, &mut out);
        }

        // Process terminator — scan for function calls with VAR params
        if let Some(ref term) = block.terminator {
            match term {
                Terminator::Branch { cond, .. } => self.scan_expr_defs(cond, &mut out),
                Terminator::Switch { expr, .. } => self.scan_expr_defs(expr, &mut out),
                Terminator::Return(Some(expr)) | Terminator::Raise(Some(expr)) => {
                    self.scan_expr_defs(expr, &mut out);
                }
                _ => {}
            }
        }

        out
    }
}

impl<'a> DefinitelyAssigned<'a> {
    fn transfer_stmt(&self, stmt: &HirStmt, state: &mut AssignedSet) {
        match &stmt.kind {
            HirStmtKind::Assign { target, value } => {
                // Scan RHS for function calls with VAR params
                self.scan_expr_defs(value, state);
                // The LHS is defined — mark the variable as assigned
                if let Some(idx) = self.place_var_index(target) {
                    state.set(idx);
                }
            }
            HirStmtKind::ProcCall { target, args } => {
                // VAR parameters receive values from the callee
                for arg in args {
                    if let HirExprKind::AddrOf(place) = &arg.kind {
                        if let Some(idx) = self.place_var_index(place) {
                            state.set(idx);
                        }
                    }
                }
                // Builtins that define their first argument (lowered without AddrOf)
                if let HirCallTarget::Direct(sym) = target {
                    if is_defining_builtin(&sym.source_name) {
                        if let Some(arg) = args.first() {
                            if let HirExprKind::Place(place) = &arg.kind {
                                if let Some(idx) = self.place_var_index(place) {
                                    state.set(idx);
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// Recursively scan an expression for function calls with VAR params
    /// (AddrOf arguments) and mark those variables as defined.
    fn scan_expr_defs(&self, expr: &HirExpr, state: &mut AssignedSet) {
        match &expr.kind {
            HirExprKind::DirectCall { target, args } => {
                for arg in args {
                    if let HirExprKind::AddrOf(place) = &arg.kind {
                        if let Some(idx) = self.place_var_index(place) {
                            state.set(idx);
                        }
                    }
                    self.scan_expr_defs(arg, state);
                }
                // Builtins that define their first arg
                if is_defining_builtin(&target.source_name) {
                    if let Some(arg) = args.first() {
                        if let HirExprKind::Place(place) = &arg.kind {
                            if let Some(idx) = self.place_var_index(place) {
                                state.set(idx);
                            }
                        }
                    }
                }
            }
            HirExprKind::IndirectCall { args, .. } => {
                for arg in args {
                    if let HirExprKind::AddrOf(place) = &arg.kind {
                        if let Some(idx) = self.place_var_index(place) {
                            state.set(idx);
                        }
                    }
                    self.scan_expr_defs(arg, state);
                }
            }
            HirExprKind::BinaryOp { left, right, .. } => {
                self.scan_expr_defs(left, state);
                self.scan_expr_defs(right, state);
            }
            HirExprKind::UnaryOp { operand, .. } | HirExprKind::Not(operand)
            | HirExprKind::Deref(operand) | HirExprKind::TypeTransfer(operand) => {
                self.scan_expr_defs(operand, state);
            }
            _ => {}
        }
    }

    /// Get the VarMap index for a Place if it's a local variable.
    /// Includes places with projections (field/index access) — writing
    /// to any part of a variable counts as defining it.
    fn place_var_index(&self, place: &Place) -> Option<usize> {
        match &place.base {
            PlaceBase::Local(sym) => self.var_map.index_of(&sym.source_name),
            _ => None,
        }
    }

    /// Get the VarMap index for an expression if it's a simple local variable.
    fn expr_var_index(&self, expr: &HirExpr) -> Option<usize> {
        if let HirExprKind::Place(place) = &expr.kind {
            self.place_var_index(place)
        } else {
            None
        }
    }
}

// ── Use-before-def warning generation ───────────────────────────────

/// Collect all variable uses in an expression.
/// `defs` receives variables that are defined (via AddrOf/VAR param in calls).
fn collect_uses(expr: &HirExpr, uses: &mut Vec<(String, SourceLoc)>, defs: &mut Vec<String>) {
    match &expr.kind {
        HirExprKind::Place(place) => {
            if let PlaceBase::Local(sym) = &place.base {
                uses.push((sym.source_name.clone(), expr.loc.clone()));
            }
            // Also check index expressions in projections
            for proj in &place.projections {
                if let crate::hir::ProjectionKind::Index(idx_expr) = &proj.kind {
                    collect_uses(idx_expr, uses, defs);
                }
            }
        }
        HirExprKind::BinaryOp { left, right, .. } => {
            collect_uses(left, uses, defs);
            collect_uses(right, uses, defs);
        }
        HirExprKind::UnaryOp { operand, .. } | HirExprKind::Not(operand)
        | HirExprKind::Deref(operand) | HirExprKind::TypeTransfer(operand) => {
            collect_uses(operand, uses, defs);
        }
        HirExprKind::DirectCall { target, args } => {
            let is_def_builtin = is_defining_builtin(&target.source_name);
            for (i, arg) in args.iter().enumerate() {
                // AddrOf in function call args = VAR param = definition
                if let HirExprKind::AddrOf(place) = &arg.kind {
                    if let PlaceBase::Local(sym) = &place.base {
                        defs.push(sym.source_name.clone());
                    }
                    for proj in &place.projections {
                        if let crate::hir::ProjectionKind::Index(idx_expr) = &proj.kind {
                            collect_uses(idx_expr, uses, defs);
                        }
                    }
                } else if is_def_builtin && i == 0 {
                    // First arg of defining builtin (NEW, INC, etc.) is a def, not a use
                    if let HirExprKind::Place(place) = &arg.kind {
                        if let PlaceBase::Local(sym) = &place.base {
                            defs.push(sym.source_name.clone());
                        }
                    }
                } else {
                    collect_uses(arg, uses, defs);
                }
            }
        }
        HirExprKind::IndirectCall { args, .. } => {
            for arg in args {
                if let HirExprKind::AddrOf(place) = &arg.kind {
                    if let PlaceBase::Local(sym) = &place.base {
                        defs.push(sym.source_name.clone());
                    }
                    for proj in &place.projections {
                        if let crate::hir::ProjectionKind::Index(idx_expr) = &proj.kind {
                            collect_uses(idx_expr, uses, defs);
                        }
                    }
                } else {
                    collect_uses(arg, uses, defs);
                }
            }
        }
        HirExprKind::AddrOf(place) => {
            // Standalone AddrOf (outside a call) is a use
            if let PlaceBase::Local(sym) = &place.base {
                uses.push((sym.source_name.clone(), expr.loc.clone()));
            }
        }
        HirExprKind::SetConstructor { elements } => {
            for elem in elements {
                match elem {
                    crate::hir::HirSetElement::Single(e) => collect_uses(e, uses, defs),
                    crate::hir::HirSetElement::Range(lo, hi) => {
                        collect_uses(lo, uses, defs);
                        collect_uses(hi, uses, defs);
                    }
                }
            }
        }
        _ => {} // literals have no uses
    }
}

/// Collect variable uses and definitions from a statement.
fn collect_stmt_uses(stmt: &HirStmt, uses: &mut Vec<(String, SourceLoc)>, defs: &mut Vec<String>) {
    match &stmt.kind {
        HirStmtKind::Assign { target, value } => {
            // Uses in the RHS
            collect_uses(value, uses, defs);
            // Uses in LHS projections (e.g., a[i] := x uses i)
            for proj in &target.projections {
                if let crate::hir::ProjectionKind::Index(idx_expr) = &proj.kind {
                    collect_uses(idx_expr, uses, defs);
                }
            }
        }
        HirStmtKind::ProcCall { target, args } => {
            let is_def_builtin = if let HirCallTarget::Direct(sym) = target {
                is_defining_builtin(&sym.source_name)
            } else { false };
            for (i, arg) in args.iter().enumerate() {
                // AddrOf is a VAR parameter pass — the callee defines
                // the variable, so don't count it as a use.
                if let HirExprKind::AddrOf(place) = &arg.kind {
                    if let PlaceBase::Local(sym) = &place.base {
                        defs.push(sym.source_name.clone());
                    }
                    for proj in &place.projections {
                        if let crate::hir::ProjectionKind::Index(idx_expr) = &proj.kind {
                            collect_uses(idx_expr, uses, defs);
                        }
                    }
                } else if is_def_builtin && i == 0 {
                    // First arg of defining builtin is a def, not a use
                    if let HirExprKind::Place(place) = &arg.kind {
                        if let PlaceBase::Local(sym) = &place.base {
                            defs.push(sym.source_name.clone());
                        }
                    }
                } else {
                    collect_uses(arg, uses, defs);
                }
            }
        }
        _ => {}
    }
}

/// Collect uses from a terminator.
fn collect_terminator_uses(term: &Terminator, uses: &mut Vec<(String, SourceLoc)>, defs: &mut Vec<String>) {
    match term {
        Terminator::Branch { cond, .. } => collect_uses(cond, uses, defs),
        Terminator::Switch { expr, .. } => collect_uses(expr, uses, defs),
        Terminator::Return(Some(expr)) | Terminator::Raise(Some(expr)) => {
            collect_uses(expr, uses, defs);
        }
        _ => {}
    }
}

// ── Public API ──────────────────────────────────────────────────────

/// Run uninitialized variable lint on a procedure's CFG.
/// Returns a list of warnings.
pub fn lint_uninitialized_vars(proc: &HirProcDecl, types: &TypeRegistry) -> Vec<CompileError> {
    let cfg = match &proc.cfg {
        Some(cfg) => cfg,
        None => return Vec::new(),
    };

    let var_map = VarMap::from_proc(proc, types);
    if var_map.len() == 0 {
        return Vec::new();
    }

    // Collect parameter names — these are always initialized
    let param_names: Vec<String> = proc.sig.params.iter()
        .map(|p| p.name.clone())
        .collect();

    let analysis = DefinitelyAssigned {
        var_map: &var_map,
        param_names,
    };

    // Need preds for the dataflow solver
    let mut cfg_copy = cfg.clone();
    if !cfg_copy.preds_valid {
        cfg_copy.compute_preds();
    }

    let result = solve_forward(&analysis, &cfg_copy);

    // Now scan each block for uses of unassigned variables
    let mut warnings: Vec<CompileError> = Vec::new();
    let mut warned: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (bid, block) in cfg_copy.blocks.iter().enumerate() {
        let mut state = result.block_in[bid].clone();

        // Check uses in each statement against current state
        for stmt in &block.stmts {
            let mut uses = Vec::new();
            let mut defs = Vec::new();
            collect_stmt_uses(stmt, &mut uses, &mut defs);

            // Check each use
            for (name, loc) in &uses {
                if let Some(idx) = var_map.index_of(name) {
                    if !state.get(idx) && !warned.contains(name) {
                        warnings.push(CompileError::warning_coded(
                            loc.clone(), "W10",
                            format!("variable '{}' may be used before being assigned", name),
                        ));
                        warned.insert(name.clone());
                    }
                }
            }

            // Apply transfer (assignments in this stmt update state)
            analysis.transfer_stmt(stmt, &mut state);
            // Also apply defs from expression-level VAR params (function calls)
            for def_name in &defs {
                if let Some(idx) = var_map.index_of(def_name) {
                    state.set(idx);
                }
            }
        }

        // Check uses in terminator
        if let Some(ref term) = block.terminator {
            let mut uses = Vec::new();
            let mut defs = Vec::new();
            collect_terminator_uses(term, &mut uses, &mut defs);
            for (name, loc) in &uses {
                if let Some(idx) = var_map.index_of(name) {
                    if !state.get(idx) && !warned.contains(name) {
                        warnings.push(CompileError::warning_coded(
                            loc.clone(), "W10",
                            format!("variable '{}' may be used before being assigned", name),
                        ));
                        warned.insert(name.clone());
                    }
                }
            }
        }
    }

    warnings
}

// ── W11: Pointer NIL safety ─────────────────────────────────────────

/// Nullability state for a pointer variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NullState {
    /// Pointer may be NIL (uninitialized, or assigned NIL).
    MaybeNil,
    /// Pointer is definitely not NIL (assigned non-NIL value, or after NEW).
    NonNil,
}

/// Maps pointer-typed local variables to bitset indices.
struct PtrMap {
    vars: Vec<(String, SourceLoc)>,
}

impl PtrMap {
    fn from_proc(proc: &HirProcDecl, types: &TypeRegistry) -> Self {
        let mut vars = Vec::new();
        for local in &proc.locals {
            if let HirLocalDecl::Var { name, type_id } = local {
                let resolved = resolve_alias(types, *type_id);
                if types.get(resolved).is_pointer() {
                    vars.push((name.clone(), proc.loc.clone()));
                }
            }
        }
        // Also include pointer-typed parameters (they start as NonNil
        // since the caller provided them, but we still track them)
        PtrMap { vars }
    }

    fn len(&self) -> usize { self.vars.len() }

    fn index_of(&self, name: &str) -> Option<usize> {
        self.vars.iter().position(|(n, _)| n == name)
    }
}

/// State: one NullState per tracked pointer variable.
#[derive(Clone, PartialEq)]
struct NullabilityState {
    states: Vec<NullState>,
}

impl NullabilityState {
    fn all_maybe_nil(n: usize) -> Self {
        NullabilityState { states: vec![NullState::MaybeNil; n] }
    }

    fn all_non_nil(n: usize) -> Self {
        NullabilityState { states: vec![NullState::NonNil; n] }
    }

    fn set(&mut self, idx: usize, state: NullState) {
        if idx < self.states.len() { self.states[idx] = state; }
    }

    fn get(&self, idx: usize) -> NullState {
        if idx < self.states.len() { self.states[idx] } else { NullState::MaybeNil }
    }

    /// Join: conservative merge. NonNil only if both are NonNil.
    fn join(&self, other: &Self) -> Self {
        NullabilityState {
            states: self.states.iter().zip(&other.states)
                .map(|(a, b)| if *a == NullState::NonNil && *b == NullState::NonNil {
                    NullState::NonNil
                } else {
                    NullState::MaybeNil
                })
                .collect(),
        }
    }
}

struct NilSafety<'a> {
    ptr_map: &'a PtrMap,
    param_names: Vec<String>,
}

impl<'a> ForwardAnalysis for NilSafety<'a> {
    type State = NullabilityState;

    fn entry_state(&self) -> NullabilityState {
        // At entry, pointer locals are MaybeNil (uninitialized).
        // Pointer parameters are NonNil (caller responsibility).
        let mut state = NullabilityState::all_maybe_nil(self.ptr_map.len());
        for pname in &self.param_names {
            if let Some(idx) = self.ptr_map.index_of(pname) {
                state.set(idx, NullState::NonNil);
            }
        }
        state
    }

    fn bottom(&self) -> NullabilityState {
        // For must-analysis (intersection), bottom = all NonNil
        NullabilityState::all_non_nil(self.ptr_map.len())
    }

    fn join(&self, a: &NullabilityState, b: &NullabilityState) -> NullabilityState {
        a.join(b)
    }

    fn transfer(&self, cfg: &Cfg, block_id: BlockId, state: &NullabilityState) -> NullabilityState {
        let mut out = state.clone();
        let block = &cfg.blocks[block_id];

        for stmt in &block.stmts {
            self.transfer_stmt(stmt, &mut out);
        }

        out
    }
}

impl<'a> NilSafety<'a> {
    fn transfer_stmt(&self, stmt: &HirStmt, state: &mut NullabilityState) {
        match &stmt.kind {
            HirStmtKind::Assign { target, value } => {
                // Check if LHS is a simple pointer local
                if target.projections.is_empty() {
                    if let PlaceBase::Local(sym) = &target.base {
                        if let Some(idx) = self.ptr_map.index_of(&sym.source_name) {
                            // Determine if RHS is NIL or non-NIL
                            if Self::is_nil_expr(value) {
                                state.set(idx, NullState::MaybeNil);
                            } else {
                                state.set(idx, NullState::NonNil);
                            }
                        }
                    }
                }
            }
            HirStmtKind::ProcCall { target: HirCallTarget::Direct(sym), args } => {
                // NEW(p) sets p to NonNil
                if sym.source_name == "NEW" {
                    if let Some(arg) = args.first() {
                        let place_opt = match &arg.kind {
                            HirExprKind::AddrOf(p) => Some(p),
                            HirExprKind::Place(p) => Some(p), // builtins lowered without AddrOf
                            _ => None,
                        };
                        if let Some(place) = place_opt {
                            if place.projections.is_empty() {
                                if let PlaceBase::Local(s) = &place.base {
                                    if let Some(idx) = self.ptr_map.index_of(&s.source_name) {
                                        state.set(idx, NullState::NonNil);
                                    }
                                }
                            }
                        }
                    }
                }
                // DISPOSE(p) sets p to MaybeNil (dangling)
                if sym.source_name == "DISPOSE" {
                    if let Some(arg) = args.first() {
                        let place_opt = match &arg.kind {
                            HirExprKind::AddrOf(p) => Some(p),
                            HirExprKind::Place(p) => Some(p),
                            _ => None,
                        };
                        if let Some(place) = place_opt {
                            if place.projections.is_empty() {
                                if let PlaceBase::Local(s) = &place.base {
                                    if let Some(idx) = self.ptr_map.index_of(&s.source_name) {
                                        state.set(idx, NullState::MaybeNil);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn is_nil_expr(expr: &HirExpr) -> bool {
        matches!(expr.kind, HirExprKind::NilLit)
    }
}

/// Check if a place dereferences a pointer (has a Deref projection).
/// Returns the base variable name if it does.
fn place_deref_base(place: &Place) -> Option<&str> {
    if place.projections.iter().any(|p| matches!(p.kind, crate::hir::ProjectionKind::Deref)) {
        if let PlaceBase::Local(sym) = &place.base {
            return Some(&sym.source_name);
        }
    }
    None
}

/// Collect all pointer dereferences in an expression.
fn collect_derefs(expr: &HirExpr, derefs: &mut Vec<(String, SourceLoc)>) {
    match &expr.kind {
        HirExprKind::Place(place) => {
            if let Some(name) = place_deref_base(place) {
                derefs.push((name.to_string(), expr.loc.clone()));
            }
            for proj in &place.projections {
                if let crate::hir::ProjectionKind::Index(idx) = &proj.kind {
                    collect_derefs(idx, derefs);
                }
            }
        }
        HirExprKind::Deref(inner) => {
            // Expression-level deref (not place projection)
            if let HirExprKind::Place(place) = &inner.kind {
                if let PlaceBase::Local(sym) = &place.base {
                    derefs.push((sym.source_name.clone(), expr.loc.clone()));
                }
            }
            collect_derefs(inner, derefs);
        }
        HirExprKind::BinaryOp { left, right, .. } => {
            collect_derefs(left, derefs);
            collect_derefs(right, derefs);
        }
        HirExprKind::UnaryOp { operand, .. } | HirExprKind::Not(operand)
        | HirExprKind::TypeTransfer(operand) => {
            collect_derefs(operand, derefs);
        }
        HirExprKind::DirectCall { args, .. } | HirExprKind::IndirectCall { args, .. } => {
            for arg in args { collect_derefs(arg, derefs); }
        }
        HirExprKind::AddrOf(place) => {
            if let Some(name) = place_deref_base(place) {
                derefs.push((name.to_string(), expr.loc.clone()));
            }
        }
        _ => {}
    }
}

fn collect_stmt_derefs(stmt: &HirStmt, derefs: &mut Vec<(String, SourceLoc)>) {
    match &stmt.kind {
        HirStmtKind::Assign { target, value } => {
            if let Some(name) = place_deref_base(target) {
                derefs.push((name.to_string(), stmt.loc.clone()));
            }
            collect_derefs(value, derefs);
        }
        HirStmtKind::ProcCall { args, .. } => {
            for arg in args { collect_derefs(arg, derefs); }
        }
        _ => {}
    }
}

fn collect_terminator_derefs(term: &Terminator, derefs: &mut Vec<(String, SourceLoc)>) {
    match term {
        Terminator::Branch { cond, .. } => collect_derefs(cond, derefs),
        Terminator::Switch { expr, .. } => collect_derefs(expr, derefs),
        Terminator::Return(Some(expr)) | Terminator::Raise(Some(expr)) => {
            collect_derefs(expr, derefs);
        }
        _ => {}
    }
}

/// Run NIL safety lint on a procedure's CFG.
pub fn lint_nil_safety(proc: &HirProcDecl, types: &TypeRegistry) -> Vec<CompileError> {
    let cfg = match &proc.cfg {
        Some(cfg) => cfg,
        None => return Vec::new(),
    };

    let ptr_map = PtrMap::from_proc(proc, types);
    if ptr_map.len() == 0 {
        return Vec::new();
    }

    let param_names: Vec<String> = proc.sig.params.iter()
        .map(|p| p.name.clone())
        .collect();

    let analysis = NilSafety {
        ptr_map: &ptr_map,
        param_names,
    };

    let mut cfg_copy = cfg.clone();
    if !cfg_copy.preds_valid {
        cfg_copy.compute_preds();
    }

    let result = solve_forward(&analysis, &cfg_copy);

    let mut warnings: Vec<CompileError> = Vec::new();
    let mut warned: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (bid, block) in cfg_copy.blocks.iter().enumerate() {
        let mut state = result.block_in[bid].clone();

        for stmt in &block.stmts {
            // Check for derefs of maybe-nil pointers
            let mut derefs = Vec::new();
            collect_stmt_derefs(stmt, &mut derefs);
            for (name, loc) in &derefs {
                if let Some(idx) = ptr_map.index_of(name) {
                    if state.get(idx) == NullState::MaybeNil && !warned.contains(name) {
                        warnings.push(CompileError::warning_coded(
                            loc.clone(), "W11",
                            format!("pointer '{}' may be NIL when dereferenced", name),
                        ));
                        warned.insert(name.clone());
                    }
                }
            }
            analysis.transfer_stmt(stmt, &mut state);
        }

        // Check terminator
        if let Some(ref term) = block.terminator {
            let mut derefs = Vec::new();
            collect_terminator_derefs(term, &mut derefs);
            for (name, loc) in &derefs {
                if let Some(idx) = ptr_map.index_of(name) {
                    if state.get(idx) == NullState::MaybeNil && !warned.contains(name) {
                        warnings.push(CompileError::warning_coded(
                            loc.clone(), "W11",
                            format!("pointer '{}' may be NIL when dereferenced", name),
                        ));
                        warned.insert(name.clone());
                    }
                }
            }
        }
    }

    warnings
}

// ── Public API ──────────────────────────────────────────────────────

/// Run all CFG-level lint checks on a procedure.
pub fn lint_procedure(proc: &HirProcDecl, types: &TypeRegistry) -> Vec<CompileError> {
    let mut warnings = Vec::new();
    warnings.extend(lint_uninitialized_vars(proc, types));
    warnings.extend(lint_nil_safety(proc, types));
    warnings
}
