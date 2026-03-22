/// Structured LLVM type representation.
/// Replaces string-based type tracking with a proper enum
/// that enables type-safe IR generation and eliminates
/// ambiguous string matching.

use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum LLVMType {
    I1,
    I8,
    I16,
    I32,
    I64,
    Float,
    Double,
    Ptr,
    Void,
    Array(usize, Box<LLVMType>),
    Struct(Vec<LLVMType>),
    /// Function type: return type + param types
    Func(Box<LLVMType>, Vec<LLVMType>),
}

impl LLVMType {
    /// Emit LLVM IR text representation
    pub(crate) fn to_ir(&self) -> String {
        match self {
            LLVMType::I1 => "i1".into(),
            LLVMType::I8 => "i8".into(),
            LLVMType::I16 => "i16".into(),
            LLVMType::I32 => "i32".into(),
            LLVMType::I64 => "i64".into(),
            LLVMType::Float => "float".into(),
            LLVMType::Double => "double".into(),
            LLVMType::Ptr => "ptr".into(),
            LLVMType::Void => "void".into(),
            LLVMType::Array(n, elem) => {
                let elem_ir = if matches!(**elem, LLVMType::Void) {
                    "i32".to_string()
                } else {
                    elem.to_ir()
                };
                format!("[{} x {}]", n, elem_ir)
            }
            LLVMType::Struct(fields) => {
                let fs: Vec<_> = fields.iter().map(|f| {
                    if matches!(f, LLVMType::Void) { "i32".into() } else { f.to_ir() }
                }).collect();
                format!("{{ {} }}", fs.join(", "))
            }
            LLVMType::Func(ret, params) => {
                let ps: Vec<_> = params.iter().map(|p| p.to_ir()).collect();
                format!("{} ({})", ret.to_ir(), ps.join(", "))
            }
        }
    }

    /// Parse an LLVM type string back into an LLVMType.
    /// This enables incremental migration — old string-based code
    /// can be converted to LLVMType on the fly.
    pub(crate) fn parse(s: &str) -> LLVMType {
        let s = s.trim();
        match s {
            "i1" => LLVMType::I1,
            "i8" => LLVMType::I8,
            "i16" => LLVMType::I16,
            "i32" => LLVMType::I32,
            "i64" => LLVMType::I64,
            "float" => LLVMType::Float,
            "double" => LLVMType::Double,
            "ptr" => LLVMType::Ptr,
            "void" => LLVMType::Void,
            _ if s.starts_with('[') => {
                // [N x T]
                if let Some(rest) = s.strip_prefix('[') {
                    if let Some(x_pos) = rest.find(" x ") {
                        if let Ok(n) = rest[..x_pos].trim().parse::<usize>() {
                            let elem_str = &rest[x_pos + 3..];
                            if let Some(elem_str) = elem_str.strip_suffix(']') {
                                return LLVMType::Array(n, Box::new(LLVMType::parse(elem_str)));
                            }
                        }
                    }
                }
                LLVMType::Ptr // fallback
            }
            _ if s.starts_with('{') => {
                // { T1, T2, ... }
                if let Some(inner) = s.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
                    let inner = inner.trim();
                    if inner.is_empty() {
                        return LLVMType::Struct(vec![LLVMType::I8]);
                    }
                    let fields = split_type_list(inner);
                    let parsed: Vec<_> = fields.iter().map(|f| LLVMType::parse(f)).collect();
                    return LLVMType::Struct(parsed);
                }
                LLVMType::Ptr // fallback
            }
            _ => LLVMType::Ptr, // unknown → treat as opaque pointer
        }
    }

    pub(crate) fn is_float(&self) -> bool {
        matches!(self, LLVMType::Float | LLVMType::Double)
    }

    pub(crate) fn is_integer(&self) -> bool {
        matches!(self, LLVMType::I1 | LLVMType::I8 | LLVMType::I16 | LLVMType::I32 | LLVMType::I64)
    }

    pub(crate) fn is_aggregate(&self) -> bool {
        matches!(self, LLVMType::Struct(_) | LLVMType::Array(..))
    }

    pub(crate) fn is_array(&self) -> bool {
        matches!(self, LLVMType::Array(..))
    }

    /// Get the element type of an array, or None
    pub(crate) fn array_element(&self) -> Option<&LLVMType> {
        if let LLVMType::Array(_, elem) = self { Some(elem) } else { None }
    }

    /// Get the array size, or None
    pub(crate) fn array_size(&self) -> Option<usize> {
        if let LLVMType::Array(n, _) = self { Some(*n) } else { None }
    }

    /// Get struct fields, or None
    pub(crate) fn struct_fields(&self) -> Option<&[LLVMType]> {
        if let LLVMType::Struct(fields) = self { Some(fields) } else { None }
    }

    /// Get the number of bits for integer types
    pub(crate) fn int_bits(&self) -> Option<usize> {
        match self {
            LLVMType::I1 => Some(1),
            LLVMType::I8 => Some(8),
            LLVMType::I16 => Some(16),
            LLVMType::I32 => Some(32),
            LLVMType::I64 => Some(64),
            _ => None,
        }
    }

    /// Return the zero initializer for this type
    pub(crate) fn zero_initializer(&self) -> &'static str {
        match self {
            LLVMType::I1 | LLVMType::I8 | LLVMType::I16 |
            LLVMType::I32 | LLVMType::I64 => "0",
            LLVMType::Float | LLVMType::Double => "0.0",
            LLVMType::Ptr => "null",
            LLVMType::Array(..) | LLVMType::Struct(_) => "zeroinitializer",
            LLVMType::Void | LLVMType::Func(..) => "zeroinitializer",
        }
    }
}

impl fmt::Display for LLVMType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_ir())
    }
}

/// Split a comma-separated type list, respecting nested braces and brackets.
fn split_type_list(s: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut depth = 0;
    for ch in s.chars() {
        match ch {
            '{' | '[' => { depth += 1; current.push(ch); }
            '}' | ']' => { depth -= 1; current.push(ch); }
            ',' if depth == 0 => {
                let trimmed = current.trim().to_string();
                if !trimmed.is_empty() {
                    result.push(trimmed);
                }
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        result.push(trimmed);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_types() {
        assert_eq!(LLVMType::I32.to_ir(), "i32");
        assert_eq!(LLVMType::Ptr.to_ir(), "ptr");
        assert_eq!(LLVMType::Double.to_ir(), "double");
    }

    #[test]
    fn test_array_type() {
        let ty = LLVMType::Array(10, Box::new(LLVMType::I8));
        assert_eq!(ty.to_ir(), "[10 x i8]");
        assert_eq!(ty.array_size(), Some(10));
        assert_eq!(ty.array_element(), Some(&LLVMType::I8));
    }

    #[test]
    fn test_struct_type() {
        let ty = LLVMType::Struct(vec![LLVMType::I32, LLVMType::I32]);
        assert_eq!(ty.to_ir(), "{ i32, i32 }");
        assert!(ty.is_aggregate());
    }

    #[test]
    fn test_nested_type() {
        let inner = LLVMType::Array(128, Box::new(LLVMType::I8));
        let outer = LLVMType::Struct(vec![inner, LLVMType::I32]);
        assert_eq!(outer.to_ir(), "{ [128 x i8], i32 }");
    }

    #[test]
    fn test_parse_roundtrip() {
        let cases = vec![
            "i32", "ptr", "float", "double", "i8", "i64", "void",
            "[10 x i8]", "[4 x i32]",
            "{ i32, i32 }",
            "{ i32, [128 x i8], ptr }",
            "[256 x { i32, [4096 x i8], i32, i32 }]",
        ];
        for s in cases {
            let ty = LLVMType::parse(s);
            assert_eq!(ty.to_ir(), s, "roundtrip failed for '{}'", s);
        }
    }

    #[test]
    fn test_parse_complex_struct() {
        let s = "{ i32, i32, [1024 x i8], i32, { ptr, i32, i32 } }";
        let ty = LLVMType::parse(s);
        assert_eq!(ty.to_ir(), s);
        if let LLVMType::Struct(fields) = &ty {
            assert_eq!(fields.len(), 5);
            assert_eq!(fields[2], LLVMType::Array(1024, Box::new(LLVMType::I8)));
        } else {
            panic!("expected struct");
        }
    }
}
