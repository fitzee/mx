/// TypeLowering: the single source of truth for M2 TypeId → LLVM type mapping.
///
/// Built once from the sema TypeRegistry before codegen starts.
/// All field lookups, pointer target resolution, and array element
/// resolution go through TypeId, never through LLVM type string matching.

use std::collections::HashMap;
use crate::types::*;
use super::llvm_types::LLVMType;

/// Layout information for a record (struct) type
#[derive(Clone, Debug)]
pub(crate) struct RecordLayout {
    pub(crate) fields: Vec<FieldInfo>,
    pub(crate) llvm_type: LLVMType,
}

/// Information about a single record field
#[derive(Clone, Debug)]
pub(crate) struct FieldInfo {
    pub(crate) name: String,
    pub(crate) m2_type: TypeId,
    pub(crate) llvm_type: LLVMType,
    pub(crate) index: usize,
}

/// Central type lowering table
pub(crate) struct TypeLowering {
    /// M2 TypeId → lowered LLVM type
    types: HashMap<TypeId, LLVMType>,

    /// M2 TypeId → record field layout (only for Record types)
    record_layouts: HashMap<TypeId, RecordLayout>,

    /// M2 TypeId → pointer target TypeId (for Pointer and Alias-to-Pointer)
    pointer_targets: HashMap<TypeId, TypeId>,

    /// M2 TypeId → array element TypeId
    array_elements: HashMap<TypeId, TypeId>,

    /// M2 TypeId → array size (high + 1)
    array_sizes: HashMap<TypeId, usize>,
}

impl TypeLowering {
    /// Build the type lowering table from a sema TypeRegistry.
    /// This is called once before codegen starts.
    pub(crate) fn build(reg: &TypeRegistry) -> Self {
        let mut tl = Self {
            types: HashMap::new(),
            record_layouts: HashMap::new(),
            pointer_targets: HashMap::new(),
            array_elements: HashMap::new(),
            array_sizes: HashMap::new(),
        };

        // First pass: lower all types
        // We need to handle forward references, so iterate until stable
        let count = reg.len();
        for id in 0..count {
            tl.lower_type(reg, id);
        }

        // Second pass: resolve aliases (Alias types point through to target)
        for id in 0..count {
            if let Type::Alias { target, .. } = reg.get(id) {
                // Propagate pointer target through alias
                if let Some(target_target) = tl.pointer_targets.get(target).copied() {
                    tl.pointer_targets.insert(id, target_target);
                }
                // Propagate array element through alias
                if let Some(elem) = tl.array_elements.get(target).copied() {
                    tl.array_elements.insert(id, elem);
                }
                if let Some(size) = tl.array_sizes.get(target).copied() {
                    tl.array_sizes.insert(id, size);
                }
            }
        }

        tl
    }

    fn lower_type(&mut self, reg: &TypeRegistry, id: TypeId) {
        if self.types.contains_key(&id) {
            return;
        }

        let ty = reg.get(id).clone();
        let llvm_ty = match &ty {
            Type::Integer => LLVMType::I32,
            Type::Cardinal => LLVMType::I32,
            Type::LongInt => LLVMType::I64,
            Type::LongCard => LLVMType::I64,
            Type::Real => LLVMType::Float,
            Type::LongReal => LLVMType::Double,
            Type::Boolean => LLVMType::I32,
            Type::Char => LLVMType::I8,
            Type::Bitset => LLVMType::I32,
            Type::Word => LLVMType::I32,
            Type::Byte => LLVMType::I8,
            Type::Address => LLVMType::Ptr,
            Type::Nil => LLVMType::Ptr,
            // TY_VOID (id 7) is the real void type for function returns.
            // Any other id with Type::Void is an unresolved imported type —
            // default to I32 (most common: enum/subrange).
            Type::Void => if id == TY_VOID { LLVMType::Void } else { LLVMType::I32 },
            Type::StringLit(_) => LLVMType::Ptr,
            Type::Complex => LLVMType::Struct(vec![LLVMType::Float, LLVMType::Float]),
            Type::LongComplex => LLVMType::Struct(vec![LLVMType::Double, LLVMType::Double]),

            Type::Pointer { base } => {
                self.pointer_targets.insert(id, *base);
                LLVMType::Ptr
            }

            Type::Array { elem_type, high, .. } => {
                // Ensure element type is lowered
                self.lower_type(reg, *elem_type);
                let elem_llvm = self.types.get(elem_type).cloned()
                    .unwrap_or(LLVMType::I32);
                let size = (*high + 1) as usize;
                self.array_elements.insert(id, *elem_type);
                self.array_sizes.insert(id, size);
                LLVMType::Array(size, Box::new(elem_llvm))
            }

            Type::OpenArray { elem_type } => {
                self.array_elements.insert(id, *elem_type);
                LLVMType::Ptr
            }

            Type::Record { fields, variants } => {
                let (field_infos, field_types) = self.lower_record_fields(reg, fields, variants);
                let llvm_ty = if field_types.is_empty() {
                    LLVMType::Struct(vec![LLVMType::I8])
                } else {
                    LLVMType::Struct(field_types)
                };
                self.record_layouts.insert(id, RecordLayout {
                    fields: field_infos,
                    llvm_type: llvm_ty.clone(),
                });
                llvm_ty
            }

            Type::Set { .. } => LLVMType::I32,
            Type::Enumeration { .. } => LLVMType::I32,
            Type::Subrange { .. } => LLVMType::I32,
            Type::ProcedureType { .. } => LLVMType::Ptr,
            Type::Opaque { .. } => LLVMType::Ptr,

            Type::Alias { target, .. } => {
                self.lower_type(reg, *target);
                self.types.get(target).cloned().unwrap_or(LLVMType::Ptr)
            }

            Type::Ref { .. } | Type::RefAny => LLVMType::Ptr,
            Type::Object { .. } => LLVMType::Ptr,
            Type::Exception { .. } => LLVMType::I32,
        };

        self.types.insert(id, llvm_ty);
    }

    fn lower_record_fields(&mut self, reg: &TypeRegistry,
                            fields: &[RecordField], variants: &Option<VariantInfo>)
        -> (Vec<FieldInfo>, Vec<LLVMType>)
    {
        let mut infos = Vec::new();
        let mut llvm_fields = Vec::new();
        let mut idx = 0;

        for f in fields {
            // Skip the synthetic "variant" pseudo-field added by sema
            // for C backend's s.variant.v0.field syntax
            if f.name == "variant" {
                if let Type::Record { fields: vf, variants: None } = reg.get(f.typ) {
                    if vf.is_empty() { continue; }
                }
            }
            self.lower_type(reg, f.typ);
            let ft = self.types.get(&f.typ).cloned().unwrap_or(LLVMType::I32);
            infos.push(FieldInfo {
                name: f.name.clone(),
                m2_type: f.typ,
                llvm_type: ft.clone(),
                index: idx,
            });
            llvm_fields.push(ft);
            idx += 1;
        }

        // Handle variant part (tagged union)
        if let Some(ref vi) = variants {
            // Note: the tag field is already included in Record.fields by the sema
            // (see sema.rs line 680-686), so we DON'T add it again here.
            // Just check if it was already counted.
            let tag_already_in_fields = vi.tag_name.as_ref()
                .map(|tn| infos.iter().any(|fi| fi.name == *tn))
                .unwrap_or(true);
            if !tag_already_in_fields {
                if let Some(ref tag_name) = vi.tag_name {
                    let tag_ty = LLVMType::I32;
                    infos.push(FieldInfo {
                        name: tag_name.clone(),
                        m2_type: vi.tag_type,
                        llvm_type: tag_ty.clone(),
                        index: idx,
                    });
                    llvm_fields.push(tag_ty);
                    idx += 1;
                }
            }

            // Union fields — all variants start at the same index
            let union_start = idx;
            let mut max_variant_fields = 0usize;

            for vc in &vi.variants {
                let mut variant_offset = 0usize;
                for vf in &vc.fields {
                    self.lower_type(reg, vf.typ);
                    let ft = self.types.get(&vf.typ).cloned().unwrap_or(LLVMType::I32);
                    // Only add if not already present (union overlap)
                    if !infos.iter().any(|fi| fi.name == vf.name) {
                        infos.push(FieldInfo {
                            name: vf.name.clone(),
                            m2_type: vf.typ,
                            llvm_type: ft.clone(),
                            index: union_start + variant_offset,
                        });
                    }
                    variant_offset += 1;
                }
                if variant_offset > max_variant_fields {
                    max_variant_fields = variant_offset;
                }
            }

            // Add LLVM fields for the largest variant
            let mut max_types = Vec::new();
            for vc in &vi.variants {
                let mut vt = Vec::new();
                for vf in &vc.fields {
                    let ft = self.types.get(&vf.typ).cloned().unwrap_or(LLVMType::I32);
                    vt.push(ft);
                }
                if vt.len() > max_types.len() { max_types = vt; }
            }
            llvm_fields.extend(max_types);
        }

        (infos, llvm_fields)
    }

    // ── Public query API ────────────────────────────────────────────

    /// Get the LLVM type for a semantic TypeId
    pub(crate) fn get_type(&self, id: TypeId) -> Option<&LLVMType> {
        self.types.get(&id)
    }

    /// Get the LLVM type string for a semantic TypeId (for compatibility)
    pub(crate) fn get_type_str(&self, id: TypeId) -> String {
        self.types.get(&id).map(|t| t.to_ir()).unwrap_or_else(|| "i32".into())
    }

    /// Get record field layout for a TypeId
    pub(crate) fn get_record_layout(&self, id: TypeId) -> Option<&RecordLayout> {
        self.record_layouts.get(&id)
    }

    /// Reverse-lookup: find a record TypeId whose LLVM IR type string matches.
    /// Used to recover type tracking when the TypeId is lost but the IR
    /// type string is still known (e.g. after cross-module deref/index).
    pub(crate) fn find_record_by_ir(&self, ir: &str) -> Option<TypeId> {
        for (&id, layout) in &self.record_layouts {
            if layout.llvm_type.to_ir() == ir {
                return Some(id);
            }
        }
        None
    }

    /// Look up a field by name in a record type
    pub(crate) fn lookup_field(&self, record_type: TypeId, field_name: &str) -> Option<&FieldInfo> {
        self.record_layouts.get(&record_type)
            .and_then(|layout| layout.fields.iter().find(|f| f.name == field_name))
    }

    /// Get the pointer target TypeId for a pointer type
    pub(crate) fn pointer_target(&self, ptr_type: TypeId) -> Option<TypeId> {
        self.pointer_targets.get(&ptr_type).copied()
    }

    /// Get the array element TypeId
    pub(crate) fn array_element_type(&self, array_type: TypeId) -> Option<TypeId> {
        self.array_elements.get(&array_type).copied()
    }

    /// Get the array size (number of elements)
    pub(crate) fn array_size(&self, array_type: TypeId) -> Option<usize> {
        self.array_sizes.get(&array_type).copied()
    }

    /// Resolve through aliases to get the "real" type
    pub(crate) fn resolve_alias(&self, reg: &TypeRegistry, id: TypeId) -> TypeId {
        match reg.get(id) {
            Type::Alias { target, .. } => self.resolve_alias(reg, *target),
            _ => id,
        }
    }

    /// Get the number of types registered (for iteration)
    pub(crate) fn type_count(&self) -> usize {
        self.types.len()
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_types() {
        let reg = TypeRegistry::new();
        let tl = TypeLowering::build(&reg);

        assert_eq!(tl.get_type(TY_INTEGER), Some(&LLVMType::I32));
        assert_eq!(tl.get_type(TY_CHAR), Some(&LLVMType::I8));
        assert_eq!(tl.get_type(TY_REAL), Some(&LLVMType::Float));
        assert_eq!(tl.get_type(TY_LONGREAL), Some(&LLVMType::Double));
        assert_eq!(tl.get_type(TY_ADDRESS), Some(&LLVMType::Ptr));
        assert_eq!(tl.get_type(TY_BOOLEAN), Some(&LLVMType::I32));
        assert_eq!(tl.get_type(TY_VOID), Some(&LLVMType::Void));
    }

    #[test]
    fn test_builtin_type_strings() {
        let reg = TypeRegistry::new();
        let tl = TypeLowering::build(&reg);

        assert_eq!(tl.get_type_str(TY_INTEGER), "i32");
        assert_eq!(tl.get_type_str(TY_CHAR), "i8");
        assert_eq!(tl.get_type_str(TY_ADDRESS), "ptr");
    }

    #[test]
    fn test_complex_type() {
        let reg = TypeRegistry::new();
        let tl = TypeLowering::build(&reg);

        assert_eq!(tl.get_type(TY_COMPLEX),
            Some(&LLVMType::Struct(vec![LLVMType::Float, LLVMType::Float])));
    }

    #[test]
    fn test_array_type() {
        let mut reg = TypeRegistry::new();
        let arr_id = reg.register(Type::Array {
            index_type: TY_INTEGER,
            elem_type: TY_INTEGER,
            low: 0,
            high: 9,
        });
        let tl = TypeLowering::build(&reg);

        assert_eq!(tl.get_type(arr_id),
            Some(&LLVMType::Array(10, Box::new(LLVMType::I32))));
        assert_eq!(tl.array_element_type(arr_id), Some(TY_INTEGER));
        assert_eq!(tl.array_size(arr_id), Some(10));
    }

    #[test]
    fn test_pointer_target() {
        let mut reg = TypeRegistry::new();
        let rec_id = reg.register(Type::Record {
            fields: vec![
                RecordField { name: "x".into(), typ: TY_INTEGER, offset: 0 },
                RecordField { name: "y".into(), typ: TY_INTEGER, offset: 1 },
            ],
            variants: None,
        });
        let ptr_id = reg.register(Type::Pointer { base: rec_id });
        let tl = TypeLowering::build(&reg);

        assert_eq!(tl.get_type(ptr_id), Some(&LLVMType::Ptr));
        assert_eq!(tl.pointer_target(ptr_id), Some(rec_id));
    }

    #[test]
    fn test_record_layout() {
        let mut reg = TypeRegistry::new();
        let rec_id = reg.register(Type::Record {
            fields: vec![
                RecordField { name: "x".into(), typ: TY_INTEGER, offset: 0 },
                RecordField { name: "y".into(), typ: TY_CHAR, offset: 1 },
            ],
            variants: None,
        });
        let tl = TypeLowering::build(&reg);

        let layout = tl.get_record_layout(rec_id).unwrap();
        assert_eq!(layout.fields.len(), 2);
        assert_eq!(layout.fields[0].name, "x");
        assert_eq!(layout.fields[0].llvm_type, LLVMType::I32);
        assert_eq!(layout.fields[0].index, 0);
        assert_eq!(layout.fields[1].name, "y");
        assert_eq!(layout.fields[1].llvm_type, LLVMType::I8);
        assert_eq!(layout.fields[1].index, 1);

        let field = tl.lookup_field(rec_id, "y").unwrap();
        assert_eq!(field.index, 1);
        assert_eq!(field.m2_type, TY_CHAR);
    }

    #[test]
    fn test_alias_resolution() {
        let mut reg = TypeRegistry::new();
        let rec_id = reg.register(Type::Record {
            fields: vec![
                RecordField { name: "val".into(), typ: TY_INTEGER, offset: 0 },
            ],
            variants: None,
        });
        let ptr_id = reg.register(Type::Pointer { base: rec_id });
        let alias_id = reg.register(Type::Alias { name: "NodePtr".into(), target: ptr_id });
        let tl = TypeLowering::build(&reg);

        // Alias should resolve to same LLVM type
        assert_eq!(tl.get_type(alias_id), Some(&LLVMType::Ptr));
        // Alias should propagate pointer target
        assert_eq!(tl.pointer_target(alias_id), Some(rec_id));
    }
}
