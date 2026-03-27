use super::*;

impl CodeGen {
    /// Lower an AST statement to HIR, then emit C.
    pub(crate) fn gen_statement_hir(&mut self, stmt: &Statement) {
        let mut hb = self.make_hir_builder();
        let hir_stmt = hb.lower_stmt(stmt);
        self.emit_hir_stmt(&hir_stmt);
    }

    /// Lower and emit a list of AST statements via HIR.
    pub(crate) fn gen_statements_hir(&mut self, stmts: &[Statement]) {
        for stmt in stmts {
            self.gen_statement_hir(stmt);
        }
    }

    /// Lower a single AST statement through HIR and emit C.
    /// This is the entry point called from all codegen paths.
    pub(crate) fn gen_statement(&mut self, stmt: &Statement) {
        self.gen_statement_hir(stmt);
    }
}
