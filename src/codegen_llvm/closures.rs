use super::*;
use std::collections::HashMap;
use crate::hir;
use crate::hir_build;
use crate::types::TypeId;

impl LLVMCodeGen {
    /// Compute captured variables for a nested procedure using the unified
    /// HIR closure analysis. Returns `Vec<CapturedVar>` with TypeIds.
    ///
    /// `outer_vars` maps variable names in the enclosing scope to their TypeIds.
    pub(crate) fn compute_captures_hir(
        &self,
        proc: &ProcDecl,
        outer_vars: &HashMap<String, TypeId>,
    ) -> Vec<hir::CapturedVar> {
        let imported_modules: HashSet<String> = self.imported_modules.iter().cloned().collect();
        hir_build::compute_captures(proc, outer_vars, &self.import_map, &imported_modules)
    }

    /// Build the outer_vars map for HIR capture analysis from the current
    /// codegen state. Maps local variable names → TypeIds.
    pub(crate) fn build_outer_vars_for_captures(&self) -> HashMap<String, TypeId> {
        let mut outer_vars = HashMap::new();
        // Collect all locals from all scopes
        for scope in &self.locals {
            for name in scope.keys() {
                if let Some(&tid) = self.var_types.get(name) {
                    outer_vars.insert(name.clone(), tid);
                } else {
                    // Fallback: use TY_INTEGER for untyped locals
                    outer_vars.insert(name.clone(), crate::types::TY_INTEGER);
                }
            }
        }
        outer_vars
    }
}
