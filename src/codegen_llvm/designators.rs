use super::*;
use crate::hir_build::HirBuilder;
use crate::types::{Type, TypeId, TY_ADDRESS, TY_CHAR};

impl LLVMCodeGen {
    pub(crate) fn get_array_high(&mut self, name: &str) -> String {
        // Check for _high companion alloca first (open array parameters).
        // This is the most reliable path — no sema scope dependency.
        let high_name = format!("{}_high", name);
        if let Some((alloca, _)) = self.lookup_local(&high_name) {
            let alloca = alloca.clone();
            let tmp = self.next_tmp();
            self.emitln(&format!("  {} = load i32, ptr {}", tmp, alloca));
            return tmp;
        }
        // Look up the symbol's type from sema
        if let Some(sym) = self.sema.symtab.lookup_any(name) {
            let resolved = self.tl_resolve(sym.typ);
            if let Some(size) = self.tl_array_size(resolved) {
                return format!("{}", size - 1);
            }
        }
        // Fallback: parse LLVM type string
        let ty_opt = self.lookup_local(name).map(|(_, t)| t.clone())
            .or_else(|| self.globals.get(name).map(|(_, t)| t.clone()));
        if let Some(ty) = ty_opt {
            if ty.starts_with('[') {
                if let Some(n_str) = ty.strip_prefix('[').and_then(|s| s.split(' ').next()) {
                    if let Ok(n) = n_str.parse::<usize>() {
                        return format!("{}", n - 1);
                    }
                }
            }
        }
        // Check string constant lengths (CONST s = "..." passed to open array)
        if let Some(&len) = self.string_const_lengths.get(name) {
            return format!("{}", len.saturating_sub(1));
        }
        "0".to_string()
    }

    // ── HIR builder construction ────────────────────────────────────


    pub(crate) fn make_hir_builder(&self) -> HirBuilder<'_> {
        // Build local names set from the backend's alloca stack
        let local_names: HashSet<String> = if self.in_function {
            self.locals.last()
                .map(|scope| scope.keys().cloned().collect())
                .unwrap_or_default()
        } else {
            HashSet::new()
        };

        let ctx = crate::hir_build::CodegenContext {
            import_alias_map: &self.import_alias_map,
            imported_modules: &self.imported_modules,
            var_types: &self.var_types,
            local_names,
        };
        let mut hb = HirBuilder::with_context(
            &self.sema.types,
            &self.sema.symtab,
            &self.module_name,
            &self.sema.foreign_modules,
            ctx,
        );
        // Mirror WITH stack
        for (record_var, _type_name, _fields, _deref, tid) in &self.with_stack {
            if let Some(tid) = tid {
                hb.push_with(record_var, *tid);
            }
        }
        if self.in_function {
            if let Some(proc_name) = self.parent_proc_stack.last() {
                let bare_name = proc_name.strip_prefix(&format!("{}_", self.module_name))
                    .unwrap_or(proc_name);
                hb.enter_procedure_named(bare_name);
            }
            // No else: module init bodies are in_function but NOT procedures.
            // Their variables are module-level globals, not locals.
        }
        hb
    }

    /// Resolve a designator via HIR and validate against the old path.
    /// Returns the HIR Place if resolution succeeded.
    /// In debug builds, logs any disagreements with the legacy path.
    #[allow(dead_code)]

    pub(crate) fn emit_place_addr(&mut self, place: &crate::hir::Place) -> Val {
        use crate::hir::*;

        // Resolve the base address
        let (mut current_addr, mut current_ty, mut current_type_id) = match &place.base {
            PlaceBase::Local(sid) => {
                let (addr, ty) = if let Some((a, t)) = self.lookup_local(&sid.source_name) {
                    (a.clone(), t.clone())
                } else {
                    (sid.mangled.clone(), self.tl_type_str(sid.ty))
                };
                // VAR param: load the pointer (double-indirection)
                if sid.is_var_param {
                    let ptr = self.next_tmp();
                    self.emitln(&format!("  {} = load ptr, ptr {}", ptr, addr));
                    (ptr, ty, Some(sid.ty))
                } else if sid.is_open_array {
                    // Open array params: alloca stores the array base pointer
                    let ptr = self.next_tmp();
                    self.emitln(&format!("  {} = load ptr, ptr {}", ptr, addr));
                    (ptr, "ptr".to_string(), Some(sid.ty))
                } else {
                    (addr, ty, Some(sid.ty))
                }
            }
            PlaceBase::Global(sid) => {
                if let Some((addr, ty)) = self.globals.get(&sid.mangled) {
                    (addr.clone(), ty.clone(), Some(sid.ty))
                } else if let Some((addr, ty)) = self.globals.get(&sid.source_name) {
                    (addr.clone(), ty.clone(), Some(sid.ty))
                } else {
                    (format!("@{}", sid.mangled), self.tl_type_str(sid.ty), Some(sid.ty))
                }
            }
            PlaceBase::Constant(cv) => {
                match cv {
                    ConstVal::String(s) if !place.projections.is_empty() => {
                        // String constant with projections (e.g., "ABCDEF"[i]):
                        // intern the string and fall through to the projection loop.
                        let (name, len) = self.intern_string(s);
                        let arr_ty = format!("[{} x i8]", len);
                        (name, arr_ty, Some(place.ty))
                    }
                    _ => {
                        let val = match cv {
                            ConstVal::Integer(v) => format!("{}", v),
                            ConstVal::Real(v) => format!("0x{:016X}", v.to_bits()),
                            ConstVal::Boolean(v) => if *v { "1".into() } else { "0".into() },
                            ConstVal::Char(v) => format!("{}", *v as u32),
                            ConstVal::String(s) => {
                                let (name, _len) = self.intern_string(s);
                                return Val::with_tid(name, "ptr".to_string(), place.ty);
                            }
                            ConstVal::Set(v) => format!("{}", v),
                            ConstVal::Nil => "null".into(),
                            ConstVal::EnumVariant(v) => format!("{}", v),
                        };
                        let ty = self.tl_type_str(place.ty);
                        return Val::with_tid(val, ty, place.ty);
                    }
                }
            }
            PlaceBase::FuncRef(sid) => {
                return Val::with_tid(
                    format!("@{}", sid.mangled),
                    "ptr".to_string(),
                    sid.ty,
                );
            }
        };

        // Apply projections
        for proj in &place.projections {
            match &proj.kind {
                ProjectionKind::Field { index, record_ty, .. } => {
                    // Get the LLVM struct type for GEP
                    let record_llvm_ty = self.tl_record_type_str(*record_ty)
                        .unwrap_or_else(|| current_ty.clone());
                    let gep = self.next_tmp();
                    self.emitln(&format!(
                        "  {} = getelementptr inbounds {}, ptr {}, i32 0, i32 {}",
                        gep, record_llvm_ty, current_addr, index
                    ));
                    current_addr = gep;
                    current_ty = self.tl_type_str(proj.ty);
                    current_type_id = Some(proj.ty);
                }
                ProjectionKind::VariantField { field_index, record_ty, .. } => {
                    // Variant fields: use the field_index within the flattened struct
                    let record_llvm_ty = self.tl_record_type_str(*record_ty)
                        .unwrap_or_else(|| current_ty.clone());
                    let gep = self.next_tmp();
                    self.emitln(&format!(
                        "  {} = getelementptr inbounds {}, ptr {}, i32 0, i32 {}",
                        gep, record_llvm_ty, current_addr, field_index
                    ));
                    current_addr = gep;
                    current_ty = self.tl_type_str(proj.ty);
                    current_type_id = Some(proj.ty);
                }
                ProjectionKind::Index(idx_expr) => {
                    let idx = self.gen_hir_expr(idx_expr);
                    let idx_i64 = self.coerce_val(&idx, "i64");
                    let gep = self.next_tmp();
                    if current_ty.starts_with('[') {
                        let elem_ty = self.extract_array_elem_type(&current_ty);
                        self.emitln(&format!(
                            "  {} = getelementptr inbounds {}, ptr {}, i64 0, i64 {}",
                            gep, current_ty, current_addr, idx_i64.name
                        ));
                        current_addr = gep;
                        current_ty = elem_ty;
                    } else if current_ty == "ptr" {
                        let elem_ty = self.tl_type_str(proj.ty);
                        self.emitln(&format!(
                            "  {} = getelementptr inbounds {}, ptr {}, i64 {}",
                            gep, elem_ty, current_addr, idx_i64.name
                        ));
                        current_addr = gep;
                        current_ty = elem_ty;
                    } else {
                        self.emitln(&format!(
                            "  {} = getelementptr inbounds {}, ptr {}, i64 {}",
                            gep, current_ty, current_addr, idx_i64.name
                        ));
                        current_addr = gep;
                    }
                    current_type_id = Some(proj.ty);
                }
                ProjectionKind::Deref => {
                    let loaded = self.next_tmp();
                    self.emitln(&format!("  {} = load ptr, ptr {}", loaded, current_addr));
                    current_addr = loaded;
                    current_ty = self.tl_type_str(proj.ty);
                    current_type_id = Some(proj.ty);
                }
            }
        }

        Val { name: current_addr, ty: current_ty, type_id: current_type_id }
    }
}
