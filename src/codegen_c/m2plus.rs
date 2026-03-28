use super::*;

impl CodeGen {
    // ── Modula-2+ Exception Declaration ─────────────────────────────

    pub(crate) fn next_exception_id(&mut self) -> usize {
        self.exception_counter += 1;
        self.exception_counter
    }

    /// Allocate a new unique type ID and register a type descriptor to be emitted.
    /// Returns the C symbol name for the descriptor (e.g. "M2_TD_ModName_TypeName").
    pub(crate) fn register_type_desc(&mut self, type_name: &str, display_name: &str, parent_c_sym: Option<String>) -> String {
        self.type_id_counter += 1;
        let id = self.type_id_counter;
        let depth = if let Some(ref parent) = parent_c_sym {
            // Find parent depth from already-registered descriptors
            self.type_descs.iter()
                .find(|(sym, _, _, _)| sym == parent)
                .map(|(_, _, _, d)| d + 1)
                .unwrap_or(1)
        } else {
            0
        };
        let c_sym = format!("M2_TD_{}", type_name);
        self.type_descs.push((c_sym.clone(), display_name.to_string(), parent_c_sym, depth));
        // Store the ID for later use
        let _ = id;
        c_sym
    }

    /// Emit all registered type descriptors as C globals.
    /// Must be called after all type declarations have been processed.
    /// Parents are always registered before children (due to topo-sorted embedded modules).
    pub(crate) fn emit_type_descs(&mut self) {
        if self.type_descs.is_empty() {
            return;
        }
        let descs = std::mem::take(&mut self.type_descs);
        let mut id = 0usize;
        for (c_sym, display, parent, depth) in &descs {
            id += 1;
            let parent_expr = if let Some(p) = parent {
                format!("&{}", p)
            } else {
                "NULL".to_string()
            };
            self.emitln(&format!(
                "M2_TypeDesc {} = {{ {}, \"{}\", {}, {} }};",
                c_sym, id, display, parent_expr, depth
            ));
        }
        self.newline();
    }

    // M2+ statement codegen (TRY/EXCEPT, LOCK, TYPECASE) is in hir_emit.rs

    /// Scan sema + HIR to determine which M2+ runtime features are needed.
    /// Replaces AST walking with queries on import_map, type registry, and HIR.
    pub(crate) fn scan_m2plus_features(&mut self) {
        // Check imports for threading modules
        for module in self.import_map.values() {
            match module.as_str() {
                "Thread" | "Mutex" | "Condition"
                | "THREAD" | "MUTEX" | "CONDITION" => self.uses_threads = true,
                _ => {}
            }
        }
        // Check sema type registry for Ref/Object types → GC needed
        for tid in 0..self.sema.types.len() {
            match self.sema.types.get(tid) {
                crate::types::Type::Ref { .. } | crate::types::Type::Object { .. } => {
                    self.uses_gc = true;
                    break;
                }
                _ => {}
            }
        }
        // Check HIR for Lock statements → threads needed
        if let Some(ref hir) = self.prebuilt_hir {
            fn scan_stmts_for_lock(stmts: &[crate::hir::HirStmt]) -> bool {
                stmts.iter().any(|s| matches!(s.kind, crate::hir::HirStmtKind::Lock { .. }))
            }
            for proc in &hir.procedures {
                if let Some(ref body) = proc.body {
                    if scan_stmts_for_lock(body) {
                        self.uses_threads = true;
                        break;
                    }
                }
            }
            if let Some(ref init) = hir.init_body {
                if scan_stmts_for_lock(init) {
                    self.uses_threads = true;
                }
            }
        }
    }

}
