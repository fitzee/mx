use super::*;
use crate::types::{Type, TypeId, TY_ADDRESS, TY_CHAR};

impl LLVMCodeGen {
    // ── Designator generation ───────────────────────────────────────

    /// Get the address (pointer) of a designator for stores.
    /// Returns Val with the pointer and the element type (what's stored there).
    pub(crate) fn gen_designator_addr(&mut self, d: &Designator) -> Val {
        let name = &d.ident.name;

        // Handle Module.Field pattern (whole-module import: IMPORT Module)
        // Parsed as designator "Module" with Field selectors
        if d.ident.module.is_none()
            && !d.selectors.is_empty()
            && self.imported_modules.contains(name)
        {
            if let Some(Selector::Field(field_name, _)) = d.selectors.first() {
                let qualified = format!("{}_{}", name, field_name);
                // Look up in globals, enum variants, or const values
                if let Some((addr, ty)) = self.globals.get(&qualified) {
                    let remaining_selectors = &d.selectors[1..];
                    if remaining_selectors.is_empty() {
                        return Val::new(addr.clone(), ty.clone());
                    }
                    // TODO: handle remaining selectors
                    return Val::new(addr.clone(), ty.clone());
                }
                if let Some(&v) = self.enum_variants.get(&qualified) {
                    // Enum variants are constants, not addresses
                    return Val::new(format!("{}", v), "i32".to_string());
                }
                if let Some(&v) = self.const_values.get(&qualified) {
                    return Val::new(format!("{}", v), "i32".to_string());
                }
                // Not found — declare as external global
                let global_name = format!("@{}", qualified);
                if !self.globals.contains_key(&qualified) {
                    self.emit_preambleln(&format!("{} = external global i32", global_name));
                    self.globals.insert(qualified.clone(), (global_name.clone(), "i32".to_string()));
                }
                return Val::new(global_name, "i32".to_string());
            }
        }

        // Check WITH stack: if name matches a record field in WITH scope,
        // redirect to record_var.field, then process remaining selectors
        if d.ident.module.is_none() {
            let with_match = self.with_stack.iter().rev()
                .find(|(_, _, field_names, _, _)| field_names.contains(name))
                .map(|(rv, tn, _, deref, tid)| (rv.clone(), tn.clone(), *deref, *tid));
            if let Some((record_var, type_name, has_deref, with_tid)) = with_match {
                let mut record_addr = self.get_var_addr(&record_var);
                if has_deref {
                    let ptr = self.next_tmp();
                    self.emitln(&format!("  {} = load ptr, ptr {}", ptr, record_addr.name));
                    record_addr = Val::new(ptr, "ptr".to_string());
                }
                // Try TypeId-based field lookup first
                let field_info: Option<(String, usize, Option<TypeId>)> = with_tid
                    .and_then(|tid| self.tl_lookup_field(tid, name))
                    .map(|fi| (fi.llvm_type.to_ir(), fi.index, Some(fi.m2_type)));
                // Legacy fallback
                let field_info = field_info.or_else(|| {
                    self.record_fields.get(&type_name)
                        .and_then(|fields| fields.iter().find(|(n, _, _)| n == name))
                        .map(|(_, ft, idx)| (ft.clone(), *idx, None))
                });
                if let Some((ft, idx, field_tid)) = field_info {
                    // Get record LLVM type for GEP
                    let record_ty = with_tid
                        .and_then(|tid| self.tl_record_type_str(tid))
                        .unwrap_or_else(|| {
                            self.type_map.get(&type_name).cloned()
                                .unwrap_or_else(|| record_addr.ty.clone())
                        });
                    let gep = self.next_tmp();
                    self.emitln(&format!("  {} = getelementptr inbounds {}, ptr {}, i32 0, i32 {}",
                        gep, record_ty, record_addr.name, idx));

                    if d.selectors.is_empty() {
                        return Val::new(gep, ft);
                    }

                    // Process remaining selectors on the resolved field
                    let mut current_addr = gep;
                    let mut current_ty = ft.clone();
                    let mut sub_tid = field_tid;

                    for sel in &d.selectors {
                        match sel {
                            Selector::Field(sub_field, _) => {
                                // Try TypeId first
                                let sub_result: Option<(String, String, usize, TypeId)> = sub_tid
                                    .and_then(|tid| self.tl_lookup_field(tid, sub_field))
                                    .map(|fi| {
                                        let rec_ty = self.tl_record_type_str(fi.m2_type)
                                            .unwrap_or_else(|| current_ty.clone());
                                        (rec_ty, fi.llvm_type.to_ir(), fi.index, fi.m2_type)
                                    });
                                if let Some((_rec_ty, sft, sidx, new_tid)) = sub_result {
                                    let sub_gep = self.next_tmp();
                                    self.emitln(&format!("  {} = getelementptr inbounds {}, ptr {}, i32 0, i32 {}",
                                        sub_gep, current_ty, current_addr, sidx));
                                    current_addr = sub_gep;
                                    current_ty = sft;
                                    sub_tid = Some(new_tid);
                                } else {
                                    // Legacy fallback
                                    let field_type_name = self.type_map.iter()
                                        .find(|(_, ty)| **ty == current_ty && current_ty != "ptr")
                                        .map(|(tn, _)| tn.clone())
                                        .unwrap_or_default();
                                    let sub_info = self.record_fields.get(&field_type_name)
                                        .and_then(|fields| fields.iter().find(|(n, _, _)| n == sub_field))
                                        .map(|(_, sft, sidx)| (sft.clone(), *sidx));
                                    if let Some((sft, sidx)) = sub_info {
                                        let sub_gep = self.next_tmp();
                                        self.emitln(&format!("  {} = getelementptr inbounds {}, ptr {}, i32 0, i32 {}",
                                            sub_gep, current_ty, current_addr, sidx));
                                        current_addr = sub_gep;
                                        current_ty = sft;
                                        sub_tid = None;
                                    }
                                }
                            }
                            Selector::Index(indices, _) => {
                                for idx_expr in indices {
                                    let idx = self.gen_expr(idx_expr);
                                    let idx_i64 = self.coerce_val(&idx, "i64");
                                    let sub_gep = self.next_tmp();
                                    if current_ty.starts_with('[') {
                                        let elem_ty = self.extract_array_elem_type(&current_ty);
                                        self.emitln(&format!("  {} = getelementptr inbounds {}, ptr {}, i64 0, i64 {}",
                                            sub_gep, current_ty, current_addr, idx_i64.name));
                                        current_addr = sub_gep;
                                        current_ty = elem_ty;
                                    } else {
                                        self.emitln(&format!("  {} = getelementptr inbounds i32, ptr {}, i64 {}",
                                            sub_gep, current_addr, idx_i64.name));
                                        current_addr = sub_gep;
                                    }
                                }
                            }
                            Selector::Deref(_) => {
                                let loaded = self.next_tmp();
                                self.emitln(&format!("  {} = load ptr, ptr {}", loaded, current_addr));
                                current_addr = loaded;
                                current_ty = "i32".to_string();
                            }
                        }
                    }
                    return Val::new(current_addr, current_ty);
                }
            }
        }

        // Check for qualified name (Module.Name)
        let (base_addr, base_ty) = if let Some(ref module) = d.ident.module {
            let mangled = format!("{}_{}", module, name);
            if let Some((addr, ty)) = self.globals.get(&mangled) {
                (addr.clone(), ty.clone())
            } else {
                // Module-qualified but not found — assume global ptr
                (format!("@{}", mangled), "i32".to_string())
            }
        } else if let Some((addr, ty)) = self.lookup_local(name) {
            let addr = addr.clone();
            let ty = ty.clone();
            // If it's a VAR param, we need to load the pointer first (double-indirection)
            if self.is_var_param(name) {
                let ptr = self.next_tmp();
                self.emitln(&format!("  {} = load ptr, ptr {}", ptr, addr));
                (ptr, ty)
            } else if self.is_open_array_param(name) {
                // Open array params: alloca stores the array base pointer
                // Load it so we can GEP through the actual array
                let ptr = self.next_tmp();
                self.emitln(&format!("  {} = load ptr, ptr {}", ptr, addr));
                (ptr, "ptr".to_string())
            } else if ty.starts_with('[') && self.in_function {
                // Named array params passed as ptr: alloca stores a ptr to the array
                // Need to load the ptr before GEP. Detect by checking if this is
                // a function param with array type (alloca is ptr, local type is array)
                // Only applies to params, not regular local arrays
                let is_array_param = self.sema.symtab.lookup_any(name)
                    .map(|sym| matches!(sym.kind, crate::symtab::SymbolKind::Variable))
                    .unwrap_or(false)
                    && d.selectors.iter().any(|s| matches!(s, Selector::Index(..)));
                // Check if the alloca actually stores a ptr (param) vs the array itself (local)
                // Params have alloca ptr + store ptr %name; locals have alloca [N x T]
                // We can't easily distinguish at this point, so check if name is a known param
                // by looking at proc_params
                let is_named_array_param = self.proc_params.values()
                    .any(|params| params.iter().any(|p| p.name.as_str() == name && p.llvm_type.starts_with('[')));
                if is_named_array_param {
                    let ptr = self.next_tmp();
                    self.emitln(&format!("  {} = load ptr, ptr {}", ptr, addr));
                    // Use the actual array type from param info (not the
                    // alloca type "ptr") so Index selectors use correct GEP.
                    let array_ty = self.proc_params.values()
                        .flat_map(|ps| ps.iter())
                        .find(|p| p.name.as_str() == name && p.llvm_type.starts_with('['))
                        .map(|p| p.llvm_type.clone())
                        .unwrap_or_else(|| ty.clone());
                    (ptr, array_ty)
                } else {
                    (addr, ty)
                }
            } else {
                (addr, ty)
            }
        } else if let Some((addr, ty)) = self.globals.get(name) {
            (addr.clone(), ty.clone())
        } else if let Some(module) = self.import_map.get(name).cloned() {
            let orig = self.import_alias_map.get(name).cloned().unwrap_or_else(|| name.to_string());
            let mangled = format!("{}_{}", module, orig);
            if let Some((addr, ty)) = self.globals.get(&mangled) {
                (addr.clone(), ty.clone())
            } else {
                (format!("@{}", mangled), "i32".to_string())
            }
        } else {
            // Try mangled name
            let mangled = self.mangle(name);
            if let Some((addr, ty)) = self.globals.get(&mangled) {
                (addr.clone(), ty.clone())
            } else {
                (format!("@{}", mangled), "i32".to_string())
            }
        };

        // Apply selectors
        let mut current_addr = base_addr;
        let mut current_ty = base_ty;
        // Track the current M2 type name for field resolution (legacy)
        let mut current_type_name = self.var_type_names.get(name).cloned().unwrap_or_default();
        // Track semantic TypeId for type-safe field resolution (new)
        let mut current_type_id: Option<TypeId> = self.var_types.get(name).copied();

        let trace_enabled = false;
        macro_rules! trace_step {
            ($step:expr) => {
                if trace_enabled {
                    if let Some(tid) = current_type_id {
                        let r = resolve_tid(&self.sema.types, tid);
                        let ty = self.sema.types.get(r);
                        let summary = match ty {
                            Type::Pointer { base } => format!("Pointer(target={}→{:?})", base,
                                self.sema.types.get(resolve_tid(&self.sema.types, *base))),
                            Type::Record { fields, .. } => {
                                let n: Vec<_> = fields.iter().take(6).map(|f| format!("{}:{}", f.name, f.typ)).collect();
                                format!("Record[{}]", n.join(", "))
                            }
                            other => format!("{:?}", other),
                        };
                        eprintln!("  c | {} | tid={} resolved={} | {}", $step, tid, r, summary);
                    } else {
                        eprintln!("  c | {} | tid=NONE", $step);
                    }
                }
            }
        }
        trace_step!("base");

        for sel in &d.selectors {
            match sel {
                Selector::Field(field_name, _) => {
                    // Recover current_type_id if lost but IR type string is known.
                    if current_type_id.is_none() && current_ty.starts_with('{') {
                        if let Some(ref tl) = self.type_lowering {
                            current_type_id = tl.find_record_by_ir(&current_ty);
                        }
                    }

                    // Try LEGACY path first (it handles variant records correctly)
                    // Then fall back to TypeLowering for cases legacy can't handle
                    let mut resolved = false;

                    // LEGACY PATH: string-based field resolution
                    {
                        let lookup_name = if self.record_fields.contains_key(&current_type_name) {
                            current_type_name.clone()
                        } else if let Some(target) = self.pointer_target_types.get(&current_type_name) {
                            target.clone()
                        } else {
                            current_type_name.clone()
                        };
                        let field_info = self.record_fields.get(&lookup_name)
                            .and_then(|fields| fields.iter().find(|(n, _, _)| n == field_name))
                            .map(|(_, ft, idx)| (ft.clone(), *idx));
                        if let Some((ft, idx)) = field_info {
                            let record_ty = self.type_map.get(&lookup_name).cloned().unwrap_or_else(|| current_ty.clone());
                            let gep = self.next_tmp();
                            self.emitln(&format!("  {} = getelementptr inbounds {}, ptr {}, i32 0, i32 {}",
                                gep, record_ty, current_addr, idx));
                            current_addr = gep;
                            current_ty = ft.clone();
                            // Update current_type_id from sema directly.
                            // Try the record type first, then pointer-to-record.
                            if let Some(tid) = current_type_id {
                                let resolved = resolve_tid(&self.sema.types, tid);
                                let rec_tid = if let Some(target) = pointer_target(&self.sema.types, resolved) {
                                    resolve_tid(&self.sema.types, target)
                                } else {
                                    resolved
                                };
                                if trace_enabled && name == "b" && self.module_name == "ExprEval" {
                                    let ty = self.sema.types.get(rec_tid);
                                    let fields_desc = match ty {
                                        Type::Record { fields, .. } => {
                                            let names: Vec<_> = fields.iter()
                                                .map(|f| format!("{}:{}", f.name, f.typ)).collect();
                                            format!("Record[{}]", names.join(", "))
                                        }
                                        other => format!("{:?}", other),
                                    };
                                    let result = record_field_tid(&self.sema.types, rec_tid, field_name);
                                    eprintln!("  FIELD-LOOKUP: {}.{} | tid={} resolved={} rec_tid={} | {} | result={:?}",
                                        name, field_name, tid, resolved, rec_tid, fields_desc, result);
                                }
                                current_type_id = record_field_tid(&self.sema.types, rec_tid, field_name);
                            }
                            // Update legacy current_type_name
                            let mut found_type = false;
                            if ft == "ptr" {
                                for (tn, ty) in &self.type_map {
                                    if *ty == "ptr" && self.pointer_target_types.contains_key(tn) {
                                        if let Some(fields) = self.record_fields.get(&current_type_name) {
                                            if fields.iter().any(|(_, fty, fidx)| fty == "ptr" && *fidx == idx) {
                                                current_type_name = tn.clone();
                                                found_type = true;
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                            if !found_type {
                                for (tn, ty) in &self.type_map {
                                    if *ty == ft && ft != "ptr" {
                                        current_type_name = tn.clone();
                                        found_type = true;
                                        break;
                                    }
                                }
                            }
                            if !found_type { current_type_name = String::new(); }
                            trace_step!(&format!("field.legacy({})", field_name));
                            resolved = true;
                        }
                    }

                    // NEW PATH: TypeLowering fallback (for cases legacy can't resolve)
                    let tl_result: Option<(String, String, usize, TypeId)> = if resolved { None } else { (|| {
                        let tid = current_type_id?;
                        let tl = self.type_lowering.as_ref()?;
                        let resolved_tid = tl.resolve_alias(&self.sema.types, tid);
                        let lookup_tid = if tl.get_record_layout(resolved_tid).is_some() {
                            resolved_tid
                        } else {
                            let target = tl.pointer_target(resolved_tid)?;
                            tl.resolve_alias(&self.sema.types, target)
                        };
                        let field = tl.lookup_field(lookup_tid, field_name)?;
                        let field_ty = {
                            let s = field.llvm_type.to_ir();
                            if s == "void" { "i32".into() } else { s }
                        };
                        Some((
                            tl.get_type_str(lookup_tid),
                            field_ty,
                            field.index,
                            field.m2_type,
                        ))
                    })() };

                    if let Some((record_llvm_ty, field_ty_str, field_idx, field_m2_type)) = tl_result {
                        let gep = self.next_tmp();
                        self.emitln(&format!("  {} = getelementptr inbounds {}, ptr {}, i32 0, i32 {}",
                            gep, record_llvm_ty, current_addr, field_idx));
                        current_addr = gep;
                        current_ty = field_ty_str.clone();
                        current_type_id = Some(field_m2_type);
                        trace_step!(&format!("field.tl({})", field_name));
                        // Update legacy tracking
                        current_type_name = String::new();
                        for (tn, ty_str) in &self.type_map {
                            if *ty_str == field_ty_str && field_ty_str != "ptr" && field_ty_str != "i32" {
                                current_type_name = tn.clone();
                                break;
                            }
                        }
                        resolved = true;
                    }

                }
                Selector::Index(indices, _) => {
                    for idx_expr in indices {
                        let idx = self.gen_expr(idx_expr);
                        let idx_i64 = self.coerce_val(&idx, "i64");
                        let gep = self.next_tmp();
                        // For arrays, GEP with two indices: [0, idx]
                        if current_ty.starts_with('[') {
                            let elem_ty = self.extract_array_elem_type(&current_ty);
                            self.emitln(&format!("  {} = getelementptr inbounds {}, ptr {}, i64 0, i64 {}",
                                gep, current_ty, current_addr, idx_i64.name));
                            current_addr = gep;
                            current_ty = elem_ty;
                        } else if current_ty == "ptr" {
                            let elem_ty = self.lookup_open_array_elem_type(name);
                            self.emitln(&format!("  {} = getelementptr inbounds {}, ptr {}, i64 {}",
                                gep, elem_ty, current_addr, idx_i64.name));
                            current_addr = gep;
                            current_ty = elem_ty;
                        } else {
                            // Pointer indexing with known element type
                            self.emitln(&format!("  {} = getelementptr inbounds {}, ptr {}, i64 {}",
                                gep, current_ty, current_addr, idx_i64.name));
                            current_addr = gep;
                        }
                        // Update TypeId per index dimension
                        if let Some(tid) = current_type_id {
                            current_type_id = array_element(&self.sema.types, tid);
                        }
                    }
                    // LEGACY: string-based fallback
                    if let Some(elem_tn) = self.array_elem_type_names.get(name).cloned() {
                        current_type_name = elem_tn;
                    } else {
                        let ct = current_ty.clone();
                        let mut found = false;
                        for (tn, ty) in &self.type_map {
                            if *ty == ct {
                                current_type_name = tn.clone();
                                found = true;
                                break;
                            }
                        }
                        if !found { current_type_name = String::new(); }
                    }
                }
                Selector::Deref(_) => {
                    // Load pointer, then that becomes the new address
                    let loaded = self.next_tmp();
                    self.emitln(&format!("  {} = load ptr, ptr {}", loaded, current_addr));
                    current_addr = loaded;

                    // m2plus: ADDRESS^[i] byte-level indexing
                    let is_address = current_type_id
                        .or_else(|| self.var_types.get(name).copied())
                        .map(|tid| tid == TY_ADDRESS)
                        .unwrap_or(false);
                    if is_address && self.m2plus {
                        current_ty = "i8".to_string();
                        current_type_name = String::new();
                        current_type_id = Some(TY_CHAR);
                    } else {

                    // NEW PATH: resolve pointer target via TypeLowering
                    // Resolve pointer target from sema (the source of truth).
                    let tid = current_type_id
                        .or_else(|| self.var_types.get(name).copied());
                    if let Some(tid) = tid {
                        let resolved = resolve_tid(&self.sema.types, tid);
                        if let Some(target) = pointer_target(&self.sema.types, resolved) {
                            let target_resolved = resolve_tid(&self.sema.types, target);
                            current_type_id = Some(target_resolved);
                            current_ty = self.tl_type_str(target_resolved);
                            if current_ty == "void" { current_ty = "ptr".into(); }
                        } else if resolved == TY_ADDRESS {
                            // ADDRESS deref → byte access (handled separately for m2plus)
                            current_type_id = Some(TY_ADDRESS);
                            current_ty = "ptr".to_string();
                        } else {
                            // Not a pointer — legacy fallback for unresolved types
                            current_type_id = None;
                            if let Some(target) = self.pointer_target_types.get(&current_type_name) {
                                current_type_name = target.clone();
                                current_ty = self.type_map.get(target).cloned().unwrap_or("i32".into());
                            } else {
                                current_ty = "i32".to_string();
                            }
                        }
                    } else {
                        // No TypeId — legacy fallback
                        if let Some(target) = self.pointer_target_types.get(&current_type_name) {
                            current_type_name = target.clone();
                            current_ty = self.type_map.get(target).cloned().unwrap_or("i32".into());
                        } else if let Some(target_ty) = self.resolve_deref_target_type(name) {
                            current_ty = target_ty;
                        } else {
                            current_ty = "i32".to_string();
                        }
                        current_type_id = None;
                    }
                    } // end else (non-ADDRESS deref)
                    trace_step!("deref");
                }
            }
        }

        Val { name: current_addr, ty: current_ty, type_id: current_type_id }
    }

    /// Load the value of a designator.
    pub(crate) fn gen_designator_load(&mut self, d: &Designator) -> Val {
        let name = &d.ident.name;

        // Handle Module.Field pattern for whole-module imports
        if d.ident.module.is_none()
            && !d.selectors.is_empty()
            && self.imported_modules.contains(name)
        {
            if let Some(Selector::Field(field_name, _)) = d.selectors.first() {
                let qualified = format!("{}_{}", name, field_name);
                // Check enum variants and constants first (no load needed).
                // Try module-qualified name, then bare name (for re-exported enums).
                if let Some(&v) = self.enum_variants.get(&qualified)
                    .or_else(|| self.enum_variants.get(field_name)) {
                    return Val::new(format!("{}", v), "i32".to_string());
                }
                if let Some(&v) = self.const_values.get(&qualified)
                    .or_else(|| self.const_values.get(field_name)) {
                    return Val::new(format!("{}", v), "i32".to_string());
                }
                // Check if it's a function being used as a value
                if self.declared_fns.contains(&qualified) {
                    return Val::new(format!("@{}", qualified), "ptr".to_string());
                }
                // Load from global variable
                if let Some((addr, ty)) = self.globals.get(&qualified).cloned() {
                    let tmp = self.next_tmp();
                    self.emitln(&format!("  {} = load {}, ptr {}", tmp, ty, addr));
                    return Val::new(tmp, ty);
                }
                // Assume it's a global i32
                let tmp = self.next_tmp();
                self.emitln(&format!("  {} = load i32, ptr @{}", tmp, qualified));
                return Val::new(tmp, "i32".to_string());
            }
        }

        // Check WITH stack for field resolution
        if d.selectors.is_empty() && d.ident.module.is_none() {
            let with_match = self.with_stack.iter().rev()
                .find(|(_, _, field_names, _, _)| field_names.contains(name))
                .map(|(rv, tn, _, deref, tid)| (rv.clone(), tn.clone(), *deref, *tid));
            if let Some((record_var, type_name, has_deref, with_tid)) = with_match {
                let mut record_addr = self.get_var_addr(&record_var);
                if has_deref {
                    let ptr = self.next_tmp();
                    self.emitln(&format!("  {} = load ptr, ptr {}", ptr, record_addr.name));
                    record_addr = Val::new(ptr, "ptr".to_string());
                }
                // Try TypeId first, then legacy
                let field_info: Option<(String, usize)> = with_tid
                    .and_then(|tid| self.tl_lookup_field(tid, name))
                    .map(|fi| (fi.llvm_type.to_ir(), fi.index))
                    .or_else(|| {
                        self.record_fields.get(&type_name)
                            .and_then(|fields| fields.iter().find(|(n, _, _)| n == name))
                            .map(|(_, ft, idx)| (ft.clone(), *idx))
                    });
                if let Some((ft, idx)) = field_info {
                    let record_ty = with_tid
                        .and_then(|tid| self.tl_record_type_str(tid))
                        .unwrap_or_else(|| {
                            self.type_map.get(&type_name).cloned()
                                .unwrap_or_else(|| record_addr.ty.clone())
                        });
                    let gep = self.next_tmp();
                    self.emitln(&format!("  {} = getelementptr inbounds {}, ptr {}, i32 0, i32 {}",
                        gep, record_ty, record_addr.name, idx));
                    let tmp = self.next_tmp();
                    self.emitln(&format!("  {} = load {}, ptr {}", tmp, ft, gep));
                    return Val::new(tmp, ft);
                }
            }
        }

        // Check built-in constants and enum variants first
        if d.selectors.is_empty() {
            match name.as_str() {
                "TRUE" => return Val::new("1", "i32".to_string()),
                "FALSE" => return Val::new("0", "i32".to_string()),
                "NIL" => return Val::new("null", "ptr".to_string()),
                _ => {}
            }

            // Check if this is a function name being used as a value (proc variable)
            let resolved = if let Some(module) = self.import_map.get(name) {
                let orig = self.import_alias_map.get(name).cloned()
                    .unwrap_or_else(|| name.to_string());
                if self.foreign_modules.contains(module.as_str()) {
                    orig
                } else {
                    let import_name = format!("{}_{}", module, orig);
                    self.stdlib_name_map.get(&import_name).cloned()
                        .unwrap_or(import_name)
                }
            } else {
                self.mangle(name)
            };
            if self.declared_fns.contains(&resolved) {
                // Check it's not also a variable (variable shadows function)
                let mangled = self.mangle(name);
                if self.lookup_local(name).is_none()
                    && !self.globals.contains_key(name)
                    && !self.globals.contains_key(&mangled) {
                    return Val::new(format!("@{}", resolved), "ptr".to_string());
                }
            }
            if let Some(&v) = self.enum_variants.get(name) {
                return Val::new(format!("{}", v), "i32".to_string());
            }
            if let Some(&v) = self.const_values.get(name) {
                return Val::new(format!("{}", v), "i32".to_string());
            }
            // Module-qualified enum/const
            if let Some(module) = d.ident.module.as_ref() {
                let qualified = format!("{}_{}", module, name);
                if let Some(&v) = self.enum_variants.get(&qualified) {
                    return Val::new(format!("{}", v), "i32".to_string());
                }
                if let Some(&v) = self.const_values.get(&qualified) {
                    return Val::new(format!("{}", v), "i32".to_string());
                }
            }
        }

        let addr = self.gen_designator_addr(d);

        // ── Load/stay boundary (the invariant) ────────────────────────
        //
        // Aggregates (records, arrays) stay as addresses — callers that
        // need the SSA value (return, struct-by-value) must load explicitly.

        // 1. Semantic check: TypeId says aggregate → stay as ptr
        if let Some(tid) = addr.type_id {
            if is_aggregate(&self.sema.types, tid) {
                return Val { name: addr.name, ty: "ptr".into(), type_id: addr.type_id };
            }
        }

        // 2. LLVM type string says aggregate → stay as ptr.
        //    This catches cases where TypeId is missing OR incorrect
        //    (wrong TypeId from scope collision). The alloca type is
        //    derived from the actual declaration and is reliable.
        if (addr.ty.starts_with('{') || addr.ty.starts_with('['))
            && !addr.ty.contains("float") && !addr.ty.contains("double")
        {
            return Val { name: addr.name, ty: "ptr".into(), type_id: addr.type_id };
        }

        // 3. Open array params: already a pointer, don't re-load
        if d.selectors.is_empty() && self.is_open_array_param(&d.ident.name) {
            return Val::new(addr.name, "ptr".to_string());
        }

        // 4. Determine load type from TypeId (preferred) or LLVM type string
        let load_ty = if let Some(tid) = addr.type_id {
            let s = self.tl_type_str(tid);
            if s == "void" { addr.ty.clone() } else { s }
        } else {
            addr.ty.clone()
        };

        let tmp = self.next_tmp();
        self.emitln(&format!("  {} = load {}, ptr {}", tmp, load_ty, addr.name));
        Val { name: tmp, ty: load_ty, type_id: addr.type_id }
    }

    pub(crate) fn get_var_addr(&mut self, name: &str) -> Val {
        // Check WITH stack — if name is a field in an outer WITH, resolve through it
        let with_match = self.with_stack.iter().rev()
            .find(|(_, _, field_names, _, _)| field_names.contains(&name.to_string()))
            .map(|(rv, tn, _, deref, tid)| (rv.clone(), tn.clone(), *deref, *tid));
        if let Some((record_var, type_name, has_deref, with_tid)) = with_match {
            let mut record_addr = self.get_var_addr(&record_var);
            if has_deref {
                let ptr = self.next_tmp();
                self.emitln(&format!("  {} = load ptr, ptr {}", ptr, record_addr.name));
                record_addr = Val::new(ptr, "ptr".to_string());
            }
            // Try TypeId first, then legacy
            let field_info: Option<(String, usize)> = with_tid
                .and_then(|tid| self.tl_lookup_field(tid, name))
                .map(|fi| (fi.llvm_type.to_ir(), fi.index))
                .or_else(|| {
                    self.record_fields.get(&type_name)
                        .and_then(|fields| fields.iter().find(|(n, _, _)| n == name))
                        .map(|(_, ft, idx)| (ft.clone(), *idx))
                });
            if let Some((ft, idx)) = field_info {
                let record_ty = with_tid
                    .and_then(|tid| self.tl_record_type_str(tid))
                    .unwrap_or_else(|| {
                        self.type_map.get(&type_name).cloned()
                            .unwrap_or_else(|| record_addr.ty.clone())
                    });
                let gep = self.next_tmp();
                self.emitln(&format!("  {} = getelementptr inbounds {}, ptr {}, i32 0, i32 {}",
                    gep, record_ty, record_addr.name, idx));
                return Val::new(gep, ft);
            }
        }

        if let Some((addr, ty)) = self.lookup_local(name) {
            let addr = addr.clone();
            let ty = ty.clone();
            if self.is_var_param(name) {
                let ptr = self.next_tmp();
                self.emitln(&format!("  {} = load ptr, ptr {}", ptr, addr));
                Val::new(ptr, ty)
            } else {
                Val::new(addr, ty)
            }
        } else if let Some((addr, ty)) = self.globals.get(name) {
            Val::new(addr.clone(), ty.clone())
        } else {
            let mangled = self.mangle(name);
            if let Some((addr, ty)) = self.globals.get(&mangled) {
                Val::new(addr.clone(), ty.clone())
            } else {
                // Create on the fly — shouldn't happen but be defensive
                Val::new(format!("@{}", mangled), "i32".to_string())
            }
        }
    }

    /// Get the HIGH value for a designator that may include field selectors.
    pub(crate) fn get_designator_array_high(&mut self, d: &Designator) -> String {
        let addr = self.gen_designator_addr(d);
        // If the addr type is an array, extract size from type
        if addr.ty.starts_with('[') {
            if let Some(n_str) = addr.ty.strip_prefix('[').and_then(|s| s.split(' ').next()) {
                if let Ok(n) = n_str.parse::<usize>() {
                    return format!("{}", n - 1);
                }
            }
        }
        // Fallback to variable-level HIGH
        self.get_array_high(&d.ident.name)
    }

    /// Look up the element type for an open array parameter or pointer.
    pub(crate) fn lookup_open_array_elem_type(&self, var_name: &str) -> String {
        // Try TypeId-based resolution first
        if let Some(&tid) = self.var_types.get(var_name) {
            let resolved = self.tl_resolve(tid);
            if let Some(elem_tid) = self.tl_array_element(resolved) {
                let ty = self.tl_type_str(elem_tid);
                if ty != "i32" || elem_tid <= 18 { // 18 = TY_PROC, builtin range
                    return ty;
                }
            }
        }

        // Check proc_params for open array element types
        for params in self.proc_params.values() {
            for p in params {
                if p.name == var_name && p.is_open_array {
                    if let Some(ref elem_ty) = p.open_array_elem_type {
                        if elem_ty != "ptr" {
                            return elem_ty.clone();
                        }
                    }
                    if self.char_array_vars.contains(var_name) {
                        return "i8".to_string();
                    }
                    return "i32".to_string();
                }
            }
        }
        // Legacy fallback for non-open-array pointers
        if let Some(type_name) = self.var_type_names.get(var_name) {
            if let Some((elem_ty, _)) = self.array_types.get(type_name) {
                return elem_ty.clone();
            }
        }
        "i32".to_string()
    }

    /// Resolve the type that a pointer variable points to (for deref).
    pub(crate) fn resolve_deref_target_type(&self, var_name: &str) -> Option<String> {
        // Try TypeId-based resolution first
        if let Some(&tid) = self.var_types.get(var_name) {
            let resolved = self.tl_resolve(tid);
            if let Some(target) = self.tl_pointer_target(resolved) {
                let target_resolved = self.tl_resolve(target);
                return Some(self.tl_type_str(target_resolved));
            }
        }
        // Legacy fallback
        if let Some(type_name) = self.var_type_names.get(var_name) {
            if let Some(target) = self.pointer_target_types.get(type_name) {
                if let Some(llvm_ty) = self.type_map.get(target) {
                    return Some(llvm_ty.clone());
                }
            }
        }
        None
    }

    pub(crate) fn resolve_proc_name(&self, desig: &Designator) -> String {
        // Handle Module.Proc pattern (parsed as designator "Module" with Field selector "Proc")
        if desig.ident.module.is_none()
            && !desig.selectors.is_empty()
            && self.imported_modules.contains(&desig.ident.name)
        {
            if let Some(Selector::Field(proc_name, _)) = desig.selectors.first() {
                // Foreign C modules: use bare function name
                if self.foreign_modules.contains(&desig.ident.name) {
                    return proc_name.clone();
                }
                let full_name = format!("{}_{}", desig.ident.name, proc_name);
                if let Some(runtime_name) = self.stdlib_name_map.get(&full_name) {
                    return runtime_name.clone();
                }
                return full_name;
            }
        }

        let import_name = if let Some(ref module) = desig.ident.module {
            // Foreign C modules: bare name
            if self.foreign_modules.contains(module.as_str()) {
                desig.ident.name.clone()
            } else {
                format!("{}_{}", module, desig.ident.name)
            }
        } else if let Some(module) = self.import_map.get(&desig.ident.name) {
            let orig = self.import_alias_map.get(&desig.ident.name)
                .cloned()
                .unwrap_or_else(|| desig.ident.name.clone());
            // Foreign C modules: use bare function name
            if self.foreign_modules.contains(module.as_str()) {
                return orig;
            }
            format!("{}_{}", module, orig)
        } else {
            // Not an import — it's a module-local procedure
            // Check if it's a nested proc (with parent prefix)
            if !self.parent_proc_stack.is_empty() {
                let nested_name = format!("{}_{}", self.parent_proc_stack.last().unwrap(), desig.ident.name);
                if self.declared_fns.contains(&nested_name) {
                    return nested_name;
                }
            }
            return self.mangle(&desig.ident.name);
        };

        // Check stdlib name mapping (InOut_WriteString → m2_WriteString)
        if let Some(runtime_name) = self.stdlib_name_map.get(&import_name) {
            return runtime_name.clone();
        }

        import_name
    }

    pub(crate) fn get_array_high(&mut self, name: &str) -> String {
        // Check if it's an open array param — load the _high value
        if self.is_open_array_param(name) {
            let high_name = format!("{}_high", name);
            if let Some((alloca, _)) = self.lookup_local(&high_name) {
                let alloca = alloca.clone();
                let tmp = self.next_tmp();
                self.emitln(&format!("  {} = load i32, ptr {}", tmp, alloca));
                return tmp;
            }
        }
        // Try TypeId-based array size lookup
        if let Some(&tid) = self.var_types.get(name) {
            let resolved = self.tl_resolve(tid);
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
        "0".to_string()
    }
}
