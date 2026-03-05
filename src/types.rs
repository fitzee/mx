use std::fmt;

pub type TypeId = usize;

#[derive(Debug, Clone)]
pub struct TypeRegistry {
    types: Vec<Type>,
}

impl TypeRegistry {
    pub fn new() -> Self {
        let mut reg = Self { types: Vec::new() };
        // Register built-in types with well-known IDs
        reg.register(Type::Integer);     // 0
        reg.register(Type::Cardinal);    // 1
        reg.register(Type::Real);        // 2
        reg.register(Type::LongReal);    // 3
        reg.register(Type::Boolean);     // 4
        reg.register(Type::Char);        // 5
        reg.register(Type::Bitset);      // 6
        reg.register(Type::Void);        // 7
        reg.register(Type::Nil);         // 8
        reg.register(Type::StringLit(0));// 9 - placeholder for string type
        reg.register(Type::Word);        // 10
        reg.register(Type::Byte);        // 11
        reg.register(Type::Address);     // 12
        reg.register(Type::LongInt);     // 13
        reg.register(Type::LongCard);    // 14
        reg.register(Type::Complex);     // 15
        reg.register(Type::LongComplex); // 16
        reg.register(Type::RefAny);      // 17
        reg
    }

    pub fn register(&mut self, typ: Type) -> TypeId {
        let id = self.types.len();
        self.types.push(typ);
        id
    }

    pub fn get(&self, id: TypeId) -> &Type {
        &self.types[id]
    }

    pub fn get_mut(&mut self, id: TypeId) -> &mut Type {
        &mut self.types[id]
    }
}

// Well-known type IDs
pub const TY_INTEGER: TypeId = 0;
pub const TY_CARDINAL: TypeId = 1;
pub const TY_REAL: TypeId = 2;
pub const TY_LONGREAL: TypeId = 3;
pub const TY_BOOLEAN: TypeId = 4;
pub const TY_CHAR: TypeId = 5;
pub const TY_BITSET: TypeId = 6;
pub const TY_VOID: TypeId = 7;
pub const TY_NIL: TypeId = 8;
pub const TY_STRING: TypeId = 9;
pub const TY_WORD: TypeId = 10;
pub const TY_BYTE: TypeId = 11;
pub const TY_ADDRESS: TypeId = 12;
pub const TY_LONGINT: TypeId = 13;
pub const TY_LONGCARD: TypeId = 14;
pub const TY_COMPLEX: TypeId = 15;
pub const TY_LONGCOMPLEX: TypeId = 16;
pub const TY_REFANY: TypeId = 17;

#[derive(Debug, Clone)]
pub enum Type {
    Integer,
    Cardinal,
    Real,
    LongReal,
    Boolean,
    Char,
    Bitset,
    Void,
    Nil,
    Word,
    Byte,
    Address,
    LongInt,
    LongCard,
    Complex,
    LongComplex,

    StringLit(usize), // length

    Array {
        index_type: TypeId,
        elem_type: TypeId,
        low: i64,
        high: i64,
    },
    OpenArray {
        elem_type: TypeId,
    },
    Record {
        fields: Vec<RecordField>,
        variants: Option<VariantInfo>,
    },
    Pointer {
        base: TypeId,
    },
    Set {
        base: TypeId,
    },
    Enumeration {
        name: String,
        variants: Vec<String>,
    },
    Subrange {
        base: TypeId,
        low: i64,
        high: i64,
    },
    ProcedureType {
        params: Vec<ParamType>,
        return_type: Option<TypeId>,
    },
    Opaque {
        name: String,
        module: String,
    },
    Alias {
        name: String,
        target: TypeId,
    },
    /// Modula-2+ traced reference (GC-managed pointer)
    Ref {
        target: TypeId,
        branded: Option<String>,
    },
    /// Modula-2+ REFANY (any traced reference)
    RefAny,
    /// Modula-2+ OBJECT type
    Object {
        name: String,
        parent: Option<TypeId>,
        fields: Vec<RecordField>,
        methods: Vec<ObjectMethod>,
    },
    /// Modula-2+ exception type
    Exception {
        name: String,
    },
}

#[derive(Debug, Clone)]
pub struct ObjectMethod {
    pub name: String,
    pub params: Vec<ParamType>,
    pub return_type: Option<TypeId>,
}

#[derive(Debug, Clone)]
pub struct RecordField {
    pub name: String,
    pub typ: TypeId,
    pub offset: usize,
}

#[derive(Debug, Clone)]
pub struct VariantInfo {
    pub tag_name: Option<String>,
    pub tag_type: TypeId,
    pub variants: Vec<VariantCase>,
}

#[derive(Debug, Clone)]
pub struct VariantCase {
    pub labels: Vec<i64>,
    pub fields: Vec<RecordField>,
}

#[derive(Debug, Clone)]
pub struct ParamType {
    pub is_var: bool,
    pub typ: TypeId,
}

impl Type {
    pub fn is_ordinal(&self) -> bool {
        matches!(
            self,
            Type::Integer
                | Type::Cardinal
                | Type::LongInt
                | Type::LongCard
                | Type::Boolean
                | Type::Char
                | Type::Enumeration { .. }
                | Type::Subrange { .. }
        )
    }

    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            Type::Integer | Type::Cardinal | Type::LongInt | Type::LongCard | Type::Real | Type::LongReal
                | Type::Complex | Type::LongComplex
        )
    }

    pub fn is_complex(&self) -> bool {
        matches!(self, Type::Complex | Type::LongComplex)
    }

    pub fn is_integer_type(&self) -> bool {
        matches!(self, Type::Integer | Type::Cardinal | Type::LongInt | Type::LongCard)
    }

    pub fn is_real_type(&self) -> bool {
        matches!(self, Type::Real | Type::LongReal)
    }

    pub fn is_pointer(&self) -> bool {
        matches!(self, Type::Pointer { .. } | Type::Address | Type::Nil | Type::Ref { .. } | Type::RefAny)
    }

    pub fn is_ref(&self) -> bool {
        matches!(self, Type::Ref { .. } | Type::RefAny)
    }

    pub fn is_object(&self) -> bool {
        matches!(self, Type::Object { .. })
    }

    pub fn is_set(&self) -> bool {
        matches!(self, Type::Set { .. } | Type::Bitset)
    }

    pub fn is_string_like(&self) -> bool {
        matches!(self, Type::StringLit(_))
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Integer => write!(f, "INTEGER"),
            Type::Cardinal => write!(f, "CARDINAL"),
            Type::Real => write!(f, "REAL"),
            Type::LongReal => write!(f, "LONGREAL"),
            Type::Boolean => write!(f, "BOOLEAN"),
            Type::Char => write!(f, "CHAR"),
            Type::Bitset => write!(f, "BITSET"),
            Type::Void => write!(f, "VOID"),
            Type::Nil => write!(f, "NIL"),
            Type::Word => write!(f, "WORD"),
            Type::Byte => write!(f, "BYTE"),
            Type::Address => write!(f, "ADDRESS"),
            Type::LongInt => write!(f, "LONGINT"),
            Type::LongCard => write!(f, "LONGCARD"),
            Type::Complex => write!(f, "COMPLEX"),
            Type::LongComplex => write!(f, "LONGCOMPLEX"),
            Type::StringLit(n) => write!(f, "STRING({})", n),
            Type::Array { .. } => write!(f, "ARRAY"),
            Type::OpenArray { .. } => write!(f, "ARRAY OF ..."),
            Type::Record { .. } => write!(f, "RECORD"),
            Type::Pointer { .. } => write!(f, "POINTER"),
            Type::Set { .. } => write!(f, "SET"),
            Type::Enumeration { name, .. } => write!(f, "{}", name),
            Type::Subrange { low, high, .. } => write!(f, "[{}..{}]", low, high),
            Type::ProcedureType { .. } => write!(f, "PROCEDURE"),
            Type::Opaque { name, .. } => write!(f, "{} (opaque)", name),
            Type::Alias { name, .. } => write!(f, "{}", name),
            Type::Ref { .. } => write!(f, "REF"),
            Type::RefAny => write!(f, "REFANY"),
            Type::Object { name, .. } => write!(f, "{} (OBJECT)", name),
            Type::Exception { name } => write!(f, "EXCEPTION {}", name),
        }
    }
}

/// Check if `src` type is assignment-compatible with `dst` type
pub fn assignment_compatible(reg: &TypeRegistry, dst: TypeId, src: TypeId) -> bool {
    if dst == src {
        return true;
    }
    let dt = reg.get(dst);
    let st = reg.get(src);

    // INTEGER and CARDINAL are interchangeable in PIM4
    if dt.is_integer_type() && st.is_integer_type() {
        return true;
    }
    // REAL types
    if dt.is_real_type() && st.is_numeric() {
        return true;
    }
    // COMPLEX types
    if dt.is_complex() && st.is_numeric() {
        return true;
    }
    if dst == TY_LONGCOMPLEX && src == TY_COMPLEX {
        return true;
    }
    // NIL can be assigned to any pointer or procedure variable
    if (dt.is_pointer() || matches!(dt, Type::ProcedureType { .. })) && matches!(st, Type::Nil) {
        return true;
    }
    // Pointer-to-pointer assignment: any pointer can be assigned to another pointer
    // of the same or compatible base type (simplified: allow any pointer assignment)
    if dt.is_pointer() && st.is_pointer() {
        return true;
    }
    // REFANY accepts any REF type
    if matches!(dt, Type::RefAny) && st.is_ref() {
        return true;
    }
    // REF types: same target type or REFANY source
    if matches!(dt, Type::Ref { .. }) && matches!(st, Type::RefAny) {
        return true; // unsafe narrowing, checked at runtime via TYPECASE
    }
    // String literal to ARRAY OF CHAR
    if let Type::Array { elem_type, .. } = dt {
        if *elem_type == TY_CHAR && st.is_string_like() {
            return true;
        }
    }
    if let Type::OpenArray { elem_type } = dt {
        if *elem_type == TY_CHAR && st.is_string_like() {
            return true;
        }
    }
    // CHAR and single-char string
    if dst == TY_CHAR {
        if let Type::StringLit(1) = st {
            return true;
        }
    }
    // Enumerations are compatible with integers and other enums
    if matches!(dt, Type::Enumeration { .. }) && (st.is_integer_type() || matches!(st, Type::Enumeration { .. })) {
        return true;
    }
    if dt.is_integer_type() && matches!(st, Type::Enumeration { .. }) {
        return true;
    }
    // Ordinal types are interassignable (subranges, enums, integers)
    if dt.is_ordinal() && st.is_ordinal() {
        return true;
    }
    // Sets - BITSET and SET of same base
    if dt.is_set() && st.is_set() {
        return true; // simplified check
    }
    // Record types - allow record-to-record assignment (same named type or structural)
    if matches!(dt, Type::Record { .. }) && matches!(st, Type::Record { .. }) {
        return true; // simplified: allow any record assignment
    }
    // Array types - allow array-to-array and open array-to-array assignment
    if matches!(dt, Type::Array { .. }) && matches!(st, Type::Array { .. } | Type::OpenArray { .. }) {
        return true; // simplified: allow any array assignment
    }
    if matches!(dt, Type::OpenArray { .. }) && matches!(st, Type::Array { .. } | Type::OpenArray { .. }) {
        return true;
    }
    // Alias resolution
    if let Type::Alias { target, .. } = dt {
        return assignment_compatible(reg, *target, src);
    }
    if let Type::Alias { target, .. } = st {
        return assignment_compatible(reg, dst, *target);
    }
    // Subrange to base type
    if let Type::Subrange { base, .. } = dt {
        return assignment_compatible(reg, *base, src);
    }
    if let Type::Subrange { base, .. } = st {
        return assignment_compatible(reg, dst, *base);
    }
    false
}

/// Check if two types are expression-compatible (can appear in same binary op)
pub fn expression_compatible(reg: &TypeRegistry, t1: TypeId, t2: TypeId) -> bool {
    if t1 == t2 {
        return true;
    }
    let ty1 = reg.get(t1);
    let ty2 = reg.get(t2);

    if ty1.is_numeric() && ty2.is_numeric() {
        return true;
    }
    if ty1.is_set() && ty2.is_set() {
        return true;
    }
    // Alias resolution
    if let Type::Alias { target, .. } = ty1 {
        return expression_compatible(reg, *target, t2);
    }
    if let Type::Alias { target, .. } = ty2 {
        return expression_compatible(reg, t1, *target);
    }
    if let Type::Subrange { base, .. } = ty1 {
        return expression_compatible(reg, *base, t2);
    }
    if let Type::Subrange { base, .. } = ty2 {
        return expression_compatible(reg, t1, *base);
    }
    false
}
