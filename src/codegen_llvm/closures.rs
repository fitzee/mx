use super::*;

impl LLVMCodeGen {
    /// Collect free variables: names used in a block that are in the parent scope.
    pub(crate) fn collect_free_vars(&self, block: &Block, parent_locals: &HashSet<String>) -> HashSet<String> {
        let mut free_vars = HashSet::new();
        let mut local_names = HashSet::new();
        for decl in &block.decls {
            if let Declaration::Var(v) = decl {
                for n in &v.names { local_names.insert(n.clone()); }
            }
        }
        if let Some(stmts) = &block.body {
            self.collect_refs_in_stmts(stmts, parent_locals, &local_names, &mut free_vars);
        }
        // Also check nested procedures recursively
        for decl in &block.decls {
            if let Declaration::Procedure(p) = decl {
                // Add this proc's params to its local names
                let mut nested_locals = local_names.clone();
                for fp in &p.heading.params {
                    for n in &fp.names { nested_locals.insert(n.clone()); }
                }
                let nested_free = self.collect_free_vars(&p.block, parent_locals);
                for name in nested_free {
                    if !local_names.contains(&name) {
                        free_vars.insert(name);
                    }
                }
            }
        }
        free_vars
    }

    pub(crate) fn collect_refs_in_stmts(&self, stmts: &[Statement], parents: &HashSet<String>,
                              locals: &HashSet<String>, free: &mut HashSet<String>) {
        for stmt in stmts {
            match &stmt.kind {
                StatementKind::Assign { desig, expr } => {
                    self.check_desig_ref(&desig.ident.name, parents, locals, free);
                    self.collect_refs_in_expr(expr, parents, locals, free);
                }
                StatementKind::ProcCall { desig, args } => {
                    self.check_desig_ref(&desig.ident.name, parents, locals, free);
                    for a in args { self.collect_refs_in_expr(a, parents, locals, free); }
                }
                StatementKind::If { cond, then_body, elsifs, else_body } => {
                    self.collect_refs_in_expr(cond, parents, locals, free);
                    self.collect_refs_in_stmts(then_body, parents, locals, free);
                    for (c, b) in elsifs {
                        self.collect_refs_in_expr(c, parents, locals, free);
                        self.collect_refs_in_stmts(b, parents, locals, free);
                    }
                    if let Some(eb) = else_body { self.collect_refs_in_stmts(eb, parents, locals, free); }
                }
                StatementKind::While { cond, body } => {
                    self.collect_refs_in_expr(cond, parents, locals, free);
                    self.collect_refs_in_stmts(body, parents, locals, free);
                }
                StatementKind::Repeat { body, cond } => {
                    self.collect_refs_in_stmts(body, parents, locals, free);
                    self.collect_refs_in_expr(cond, parents, locals, free);
                }
                StatementKind::For { var, start, end, body, .. } => {
                    self.check_desig_ref(var, parents, locals, free);
                    self.collect_refs_in_expr(start, parents, locals, free);
                    self.collect_refs_in_expr(end, parents, locals, free);
                    self.collect_refs_in_stmts(body, parents, locals, free);
                }
                StatementKind::Return { expr } => {
                    if let Some(e) = expr { self.collect_refs_in_expr(e, parents, locals, free); }
                }
                _ => {}
            }
        }
    }

    pub(crate) fn collect_refs_in_expr(&self, expr: &Expr, parents: &HashSet<String>,
                             locals: &HashSet<String>, free: &mut HashSet<String>) {
        match &expr.kind {
            ExprKind::Designator(d) => {
                self.check_desig_ref(&d.ident.name, parents, locals, free);
            }
            ExprKind::FuncCall { desig, args } => {
                self.check_desig_ref(&desig.ident.name, parents, locals, free);
                for a in args { self.collect_refs_in_expr(a, parents, locals, free); }
            }
            ExprKind::BinaryOp { left, right, .. } => {
                self.collect_refs_in_expr(left, parents, locals, free);
                self.collect_refs_in_expr(right, parents, locals, free);
            }
            ExprKind::UnaryOp { operand, .. } => {
                self.collect_refs_in_expr(operand, parents, locals, free);
            }
            ExprKind::Not(e) => self.collect_refs_in_expr(e, parents, locals, free),
            _ => {}
        }
    }

    pub(crate) fn check_desig_ref(&self, name: &str, parents: &HashSet<String>,
                        locals: &HashSet<String>, free: &mut HashSet<String>) {
        if parents.contains(name) && !locals.contains(name)
            && !builtins::is_builtin_proc(name)
            && !self.import_map.contains_key(name)
            && !self.imported_modules.contains(name) {
            free.insert(name.to_string());
        }
    }
}
