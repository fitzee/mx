//! Target abstraction layer.
//!
//! `TargetInfo` formalizes the target platform semantics that the compiler
//! and both backends depend on.  It is constructed once at driver level and
//! threaded through to backends, builtins, and runtime emission.
//!
//! Supported targets:
//!   x86_64-linux, aarch64-linux, x86_64-darwin, aarch64-darwin

use std::fmt;

use crate::errors::{CompileError, CompileResult};
use crate::types::{TypeId, TypeRegistry, Type,
    TY_INTEGER, TY_CARDINAL, TY_REAL, TY_LONGREAL, TY_BOOLEAN, TY_CHAR,
    TY_BITSET, TY_WORD, TY_BYTE, TY_ADDRESS, TY_LONGINT, TY_LONGCARD,
    TY_COMPLEX, TY_LONGCOMPLEX, TY_VOID, TY_NIL, TY_REFANY, TY_PROC,
};

// ── Enumerations ───────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arch {
    X86_64,
    Aarch64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Linux,
    Darwin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endianness {
    Little,
    Big,
}

/// C-level ABI convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CAbi {
    /// System V AMD64 / AAPCS64 (Linux, FreeBSD, etc.)
    SysV,
    /// Apple ARM64 / x86-64 (macOS, iOS)
    Darwin,
}

// ── IntLayout ──────────────────────────────────────────────────────

/// Sizes (in bytes) of Modula-2 integer and cardinal types as lowered
/// to C / LLVM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntLayout {
    /// INTEGER  → int32_t  (4 bytes on all supported targets)
    pub integer_bytes: u32,
    /// CARDINAL → uint32_t
    pub cardinal_bytes: u32,
    /// LONGINT  → int64_t
    pub longint_bytes: u32,
    /// LONGCARD → uint64_t
    pub longcard_bytes: u32,
    /// REAL     → float
    pub real_bytes: u32,
    /// LONGREAL → double
    pub longreal_bytes: u32,
    /// BITSET   → uint32_t
    pub bitset_bytes: u32,
}

impl IntLayout {
    /// LP64 layout used on all supported 64-bit targets.
    pub fn lp64() -> Self {
        Self {
            integer_bytes: 4,
            cardinal_bytes: 4,
            longint_bytes: 8,
            longcard_bytes: 8,
            real_bytes: 4,
            longreal_bytes: 8,
            bitset_bytes: 4,
        }
    }
}

// ── AlignmentInfo ──────────────────────────────────────────────────

/// Alignment rules for the target, in bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlignmentInfo {
    /// Alignment of a pointer (void *)
    pub pointer_align: u32,
    /// Alignment of i8/char
    pub char_align: u32,
    /// Alignment of i16/short
    pub short_align: u32,
    /// Alignment of i32/int
    pub int_align: u32,
    /// Alignment of i64/long long
    pub long_align: u32,
    /// Alignment of float
    pub float_align: u32,
    /// Alignment of double
    pub double_align: u32,
    /// Maximum alignment the ABI requires for any scalar
    pub max_align: u32,
    /// Minimum struct alignment (usually 1, but some ABIs require more)
    pub struct_min_align: u32,
}

impl AlignmentInfo {
    /// Standard LP64 alignment (x86-64 + aarch64, both Linux and Darwin).
    pub fn lp64() -> Self {
        Self {
            pointer_align: 8,
            char_align: 1,
            short_align: 2,
            int_align: 4,
            long_align: 8,
            float_align: 4,
            double_align: 8,
            max_align: 16,
            struct_min_align: 1,
        }
    }
}

// ── TargetInfo ─────────────────────────────────────────────────────

/// Complete description of a compilation target.
///
/// Created once at driver level.  Passed by reference to backends and
/// runtime-related code.  Must NOT depend on AST or HIR.
#[derive(Debug, Clone)]
pub struct TargetInfo {
    /// Canonical target triple, e.g. "x86_64-unknown-linux-gnu"
    pub triple: String,
    /// Parsed architecture
    pub arch: Arch,
    /// Parsed OS
    pub os: Os,
    /// Pointer width in bits (64 for all supported targets)
    pub pointer_bits: u32,
    /// Byte order
    pub endian: Endianness,
    /// C ABI family
    pub c_abi: CAbi,
    /// Integer/float type sizes
    pub int_layout: IntLayout,
    /// Alignment rules
    pub alignments: AlignmentInfo,
    /// Whether the target supports setjmp/longjmp (true for all POSIX targets)
    pub supports_setjmp: bool,
}

impl TargetInfo {
    // ── Constructors ───────────────────────────────────────────────

    /// Detect target from the host platform.
    pub fn from_host() -> Self {
        let arch = std::env::consts::ARCH;
        let os = std::env::consts::OS;
        let triple = host_triple_string(arch, os);
        Self::build(parse_arch(arch), parse_os(os), &triple)
    }

    /// Construct from a user-supplied target triple string.
    ///
    /// Accepted forms:
    ///   - `x86_64-linux`, `aarch64-darwin`  (short)
    ///   - `x86_64-unknown-linux-gnu`, `aarch64-apple-darwin`  (full)
    ///   - `arm64-apple-macosx14.0.0`  (LLVM canonical)
    pub fn from_triple(triple: &str) -> CompileResult<Self> {
        let lower = triple.to_lowercase();
        let parts: Vec<&str> = lower.split('-').collect();

        let arch = match parts.first().copied() {
            Some("x86_64") => Arch::X86_64,
            Some("aarch64" | "arm64") => Arch::Aarch64,
            Some(other) => {
                return Err(CompileError::driver(
                    format!("unsupported target architecture: '{}'\nsupported: x86_64, aarch64", other),
                ));
            }
            None => {
                return Err(CompileError::driver("empty target triple".to_string()));
            }
        };

        // Detect OS from any position after arch
        let tail = parts[1..].join("-");
        let os = if tail.contains("linux") {
            Os::Linux
        } else if tail.contains("darwin") || tail.contains("macos") || tail.contains("apple") {
            Os::Darwin
        } else {
            return Err(CompileError::driver(
                format!("unsupported target OS in '{}'\nsupported: linux, darwin/macos", triple),
            ));
        };

        // Normalize to canonical triple
        let canonical = canonical_triple(arch, os);
        Ok(Self::build(arch, os, &canonical))
    }

    /// Internal constructor — all fields derived from arch + os.
    fn build(arch: Arch, os: Os, triple: &str) -> Self {
        let c_abi = match os {
            Os::Darwin => CAbi::Darwin,
            Os::Linux => CAbi::SysV,
        };

        Self {
            triple: triple.to_string(),
            arch,
            os,
            pointer_bits: 64,  // all supported targets are 64-bit
            endian: Endianness::Little,  // all supported targets are LE
            c_abi,
            int_layout: IntLayout::lp64(),
            alignments: AlignmentInfo::lp64(),
            supports_setjmp: true,  // all POSIX targets
        }
    }

    // ── Queries ────────────────────────────────────────────────────

    /// Pointer size in bytes.
    pub fn pointer_bytes(&self) -> u32 {
        self.pointer_bits / 8
    }

    /// Whether this is a Darwin/macOS target.
    pub fn is_darwin(&self) -> bool {
        self.os == Os::Darwin
    }

    /// Whether this is a Linux target.
    pub fn is_linux(&self) -> bool {
        self.os == Os::Linux
    }

    /// Default C compiler flags for this target.
    /// Applied to every cc/clang invocation (compile and link).
    pub fn default_cflags(&self) -> Vec<&'static str> {
        match self.os {
            Os::Linux => vec!["-D_GNU_SOURCE"],
            Os::Darwin => vec![],
        }
    }

    /// Default linker flags for this target.
    pub fn default_ldflags(&self) -> Vec<&'static str> {
        match self.os {
            Os::Linux => vec!["-Wl,--gc-sections", "-lpthread"],
            Os::Darwin => vec!["-Wl,-dead_strip"],
        }
    }

    /// Whether this is an x86_64 target.
    pub fn is_x86_64(&self) -> bool {
        self.arch == Arch::X86_64
    }

    /// Whether this is an aarch64 target.
    pub fn is_aarch64(&self) -> bool {
        self.arch == Arch::Aarch64
    }

    /// LLVM target triple string (for target triple = "..." in .ll output).
    pub fn llvm_triple(&self) -> String {
        match (self.arch, self.os) {
            (Arch::Aarch64, Os::Darwin) => "arm64-apple-macosx14.0.0".to_string(),
            (Arch::X86_64,  Os::Darwin) => "x86_64-apple-macosx14.0.0".to_string(),
            (Arch::X86_64,  Os::Linux)  => "x86_64-unknown-linux-gnu".to_string(),
            (Arch::Aarch64, Os::Linux)  => "aarch64-unknown-linux-gnu".to_string(),
        }
    }

    /// LLVM data layout string.
    pub fn llvm_datalayout(&self) -> String {
        match (self.arch, self.os) {
            (Arch::Aarch64, Os::Darwin) =>
                "e-m:o-i64:64-i128:128-n32:64-S128".to_string(),
            (Arch::X86_64, Os::Darwin) =>
                "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-f80:128-n8:16:32:64-S128".to_string(),
            (Arch::X86_64, Os::Linux) =>
                "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-f80:128-n8:16:32:64-S128".to_string(),
            (Arch::Aarch64, Os::Linux) =>
                "e-m:e-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128".to_string(),
        }
    }

    /// Linker flags appropriate for this target.
    pub fn linker_gc_flags(&self) -> Vec<&'static str> {
        match self.os {
            Os::Darwin => vec!["-Wl,-dead_strip"],
            Os::Linux => vec!["-Wl,--gc-sections"],
        }
    }

    // ── Type size queries ──────────────────────────────────────────

    /// Return the size in bytes of a primitive TypeId on this target.
    /// Returns None for composite types (records, arrays) — those need
    /// layout computation.
    pub fn primitive_size(&self, tid: TypeId) -> Option<u32> {
        match tid {
            TY_INTEGER  => Some(self.int_layout.integer_bytes),
            TY_CARDINAL => Some(self.int_layout.cardinal_bytes),
            TY_REAL     => Some(self.int_layout.real_bytes),
            TY_LONGREAL => Some(self.int_layout.longreal_bytes),
            TY_BOOLEAN  => Some(4),  // int in C
            TY_CHAR     => Some(1),
            TY_BITSET   => Some(self.int_layout.bitset_bytes),
            TY_WORD     => Some(4),  // uint32_t
            TY_BYTE     => Some(1),
            TY_ADDRESS  => Some(self.pointer_bytes()),
            TY_LONGINT  => Some(self.int_layout.longint_bytes),
            TY_LONGCARD => Some(self.int_layout.longcard_bytes),
            TY_COMPLEX  => Some(self.int_layout.real_bytes * 2),
            TY_LONGCOMPLEX => Some(self.int_layout.longreal_bytes * 2),
            TY_VOID | TY_NIL => Some(0),
            TY_REFANY | TY_PROC => Some(self.pointer_bytes()),
            _ => None,
        }
    }

    /// Return the alignment in bytes of a primitive TypeId.
    pub fn primitive_align(&self, tid: TypeId) -> Option<u32> {
        match tid {
            TY_CHAR | TY_BYTE => Some(self.alignments.char_align),
            TY_INTEGER | TY_CARDINAL | TY_BITSET | TY_WORD | TY_BOOLEAN =>
                Some(self.alignments.int_align),
            TY_LONGINT | TY_LONGCARD =>
                Some(self.alignments.long_align),
            TY_REAL => Some(self.alignments.float_align),
            TY_LONGREAL => Some(self.alignments.double_align),
            TY_ADDRESS | TY_REFANY | TY_PROC =>
                Some(self.alignments.pointer_align),
            TY_COMPLEX => Some(self.alignments.float_align),
            TY_LONGCOMPLEX => Some(self.alignments.double_align),
            TY_VOID | TY_NIL => Some(1),
            _ => None,
        }
    }

    /// Resolve the size of any type, walking through aliases and
    /// computing record/array layouts.
    pub fn type_size(&self, tid: TypeId, types: &TypeRegistry) -> u32 {
        let resolved = resolve_type(tid, types);
        if let Some(prim) = self.primitive_size(resolved) {
            return prim;
        }
        match types.get(resolved) {
            Type::Enumeration { .. } => self.int_layout.integer_bytes,
            Type::Subrange { .. } => self.int_layout.integer_bytes,
            Type::Set { .. } => self.int_layout.bitset_bytes,
            Type::Pointer { .. } => self.pointer_bytes(),
            Type::Ref { .. } => self.pointer_bytes(),
            Type::ProcedureType { .. } => self.pointer_bytes(),
            Type::Opaque { .. } => self.pointer_bytes(),
            Type::Array { elem_type, low, high, .. } => {
                let count = (high - low + 1).max(0) as u32;
                let elem_size = self.type_size(*elem_type, types);
                let elem_align = self.type_align(*elem_type, types);
                let stride = align_up(elem_size, elem_align);
                count * stride
            }
            Type::OpenArray { .. } => {
                // Open arrays are passed as (ptr, high) — size is not statically known
                self.pointer_bytes() + self.int_layout.integer_bytes
            }
            Type::Record { fields, variants } => {
                self.compute_record_size(fields, variants.as_ref(), types)
            }
            Type::Object { fields, .. } => {
                // Object has an implicit vtable pointer + fields
                let vtable_size = self.pointer_bytes();
                let vtable_align = self.alignments.pointer_align;
                let mut offset = vtable_size;
                let mut max_align = vtable_align;
                for f in fields {
                    let fa = self.type_align(f.typ, types);
                    offset = align_up(offset, fa);
                    offset += self.type_size(f.typ, types);
                    max_align = max_align.max(fa);
                }
                align_up(offset, max_align)
            }
            Type::StringLit(len) => (*len as u32) + 1,  // +1 for NUL
            Type::Exception { .. } | Type::Error => 0,
            _ => 0,
        }
    }

    /// Resolve the alignment of any type.
    pub fn type_align(&self, tid: TypeId, types: &TypeRegistry) -> u32 {
        let resolved = resolve_type(tid, types);
        if let Some(prim) = self.primitive_align(resolved) {
            return prim;
        }
        match types.get(resolved) {
            Type::Enumeration { .. } => self.alignments.int_align,
            Type::Subrange { .. } => self.alignments.int_align,
            Type::Set { .. } => self.alignments.int_align,
            Type::Pointer { .. } | Type::Ref { .. } | Type::Opaque { .. } =>
                self.alignments.pointer_align,
            Type::ProcedureType { .. } => self.alignments.pointer_align,
            Type::Array { elem_type, .. } => self.type_align(*elem_type, types),
            Type::OpenArray { elem_type } => self.type_align(*elem_type, types),
            Type::Record { fields, .. } => {
                let mut max_align = self.alignments.struct_min_align;
                for f in fields {
                    max_align = max_align.max(self.type_align(f.typ, types));
                }
                max_align
            }
            Type::Object { fields, .. } => {
                let mut max_align = self.alignments.pointer_align;
                for f in fields {
                    max_align = max_align.max(self.type_align(f.typ, types));
                }
                max_align
            }
            Type::StringLit(_) => self.alignments.char_align,
            _ => 1,
        }
    }

    /// Compute the total size of a record (with padding and tail alignment).
    fn compute_record_size(
        &self,
        fields: &[crate::types::RecordField],
        variants: Option<&crate::types::VariantInfo>,
        types: &TypeRegistry,
    ) -> u32 {
        let mut offset: u32 = 0;
        let mut max_align: u32 = self.alignments.struct_min_align;

        // Fixed fields
        for f in fields {
            let fa = self.type_align(f.typ, types);
            let fs = self.type_size(f.typ, types);
            offset = align_up(offset, fa);
            offset += fs;
            max_align = max_align.max(fa);
        }

        // Variant part: treated as union (max of all variant sizes) after tag
        if let Some(vi) = variants {
            // Tag field
            let tag_align = self.type_align(vi.tag_type, types);
            let tag_size = self.type_size(vi.tag_type, types);
            offset = align_up(offset, tag_align);
            offset += tag_size;
            max_align = max_align.max(tag_align);

            // Union of variant cases
            let mut union_size: u32 = 0;
            let mut union_align: u32 = 1;
            for case in &vi.variants {
                let mut case_size: u32 = 0;
                let mut case_align: u32 = 1;
                for f in &case.fields {
                    let fa = self.type_align(f.typ, types);
                    let fs = self.type_size(f.typ, types);
                    case_size = align_up(case_size, fa);
                    case_size += fs;
                    case_align = case_align.max(fa);
                }
                union_size = union_size.max(align_up(case_size, case_align));
                union_align = union_align.max(case_align);
            }
            offset = align_up(offset, union_align);
            offset += union_size;
            max_align = max_align.max(union_align);
        }

        // Tail padding to struct alignment
        align_up(offset, max_align)
    }
}

// ── RecordLayout ───────────────────────────────────────────────────

/// Computed layout for a record type.
#[derive(Debug, Clone)]
pub struct RecordLayout {
    /// (field_name, offset_bytes, size_bytes, align_bytes)
    pub fields: Vec<FieldLayout>,
    /// Total size including tail padding
    pub total_size: u32,
    /// Overall alignment
    pub alignment: u32,
}

#[derive(Debug, Clone)]
pub struct FieldLayout {
    pub name: String,
    pub offset: u32,
    pub size: u32,
    pub align: u32,
}

/// Compute the complete record layout for a given record TypeId.
pub fn compute_record_layout(
    tid: TypeId,
    types: &TypeRegistry,
    target: &TargetInfo,
) -> Option<RecordLayout> {
    let resolved = resolve_type(tid, types);
    match types.get(resolved) {
        Type::Record { fields, variants } => {
            let mut layout_fields = Vec::new();
            let mut offset: u32 = 0;
            let mut max_align: u32 = target.alignments.struct_min_align;

            for f in fields {
                let fa = target.type_align(f.typ, types);
                let fs = target.type_size(f.typ, types);
                offset = align_up(offset, fa);
                layout_fields.push(FieldLayout {
                    name: f.name.clone(),
                    offset,
                    size: fs,
                    align: fa,
                });
                offset += fs;
                max_align = max_align.max(fa);
            }

            // Variant union (simplified — reported as single "variant" pseudo-field)
            if let Some(vi) = variants {
                let tag_align = target.type_align(vi.tag_type, types);
                let tag_size = target.type_size(vi.tag_type, types);
                offset = align_up(offset, tag_align);
                if let Some(ref tag_name) = vi.tag_name {
                    layout_fields.push(FieldLayout {
                        name: tag_name.clone(),
                        offset,
                        size: tag_size,
                        align: tag_align,
                    });
                }
                offset += tag_size;
                max_align = max_align.max(tag_align);

                // Compute union size
                let mut union_size: u32 = 0;
                let mut union_align: u32 = 1;
                for case in &vi.variants {
                    let mut cs: u32 = 0;
                    let mut ca: u32 = 1;
                    for f in &case.fields {
                        let fa = target.type_align(f.typ, types);
                        let fs = target.type_size(f.typ, types);
                        cs = align_up(cs, fa);
                        cs += fs;
                        ca = ca.max(fa);
                    }
                    union_size = union_size.max(align_up(cs, ca));
                    union_align = union_align.max(ca);
                }
                offset = align_up(offset, union_align);
                offset += union_size;
                max_align = max_align.max(union_align);
            }

            let total_size = align_up(offset, max_align);
            Some(RecordLayout {
                fields: layout_fields,
                total_size,
                alignment: max_align,
            })
        }
        _ => None,
    }
}

// ── Display ────────────────────────────────────────────────────────

impl fmt::Display for TargetInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({}bit, {:?}, {:?})", self.triple, self.pointer_bits, self.endian, self.c_abi)
    }
}

impl fmt::Display for Arch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Arch::X86_64 => write!(f, "x86_64"),
            Arch::Aarch64 => write!(f, "aarch64"),
        }
    }
}

impl fmt::Display for Os {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Os::Linux => write!(f, "linux"),
            Os::Darwin => write!(f, "darwin"),
        }
    }
}

// ── Helpers ────────────────────────────────────────────────────────

/// Align `offset` up to the next multiple of `align`.
fn align_up(offset: u32, align: u32) -> u32 {
    if align == 0 { return offset; }
    (offset + align - 1) & !(align - 1)
}

/// Resolve through type aliases.
fn resolve_type(tid: TypeId, types: &TypeRegistry) -> TypeId {
    let mut cur = tid;
    loop {
        match types.get(cur) {
            Type::Alias { target, .. } => cur = *target,
            _ => return cur,
        }
    }
}

fn parse_arch(s: &str) -> Arch {
    match s {
        "aarch64" | "arm64" => Arch::Aarch64,
        _ => Arch::X86_64,
    }
}

fn parse_os(s: &str) -> Os {
    match s {
        "macos" | "darwin" => Os::Darwin,
        _ => Os::Linux,
    }
}

fn host_triple_string(arch: &str, os: &str) -> String {
    match (arch, os) {
        ("aarch64", "macos") => "aarch64-apple-darwin".to_string(),
        ("x86_64", "macos") => "x86_64-apple-darwin".to_string(),
        ("x86_64", "linux") => "x86_64-unknown-linux-gnu".to_string(),
        ("aarch64", "linux") => "aarch64-unknown-linux-gnu".to_string(),
        _ => format!("{}-unknown-{}", arch, os),
    }
}

fn canonical_triple(arch: Arch, os: Os) -> String {
    match (arch, os) {
        (Arch::Aarch64, Os::Darwin) => "aarch64-apple-darwin".to_string(),
        (Arch::X86_64,  Os::Darwin) => "x86_64-apple-darwin".to_string(),
        (Arch::X86_64,  Os::Linux)  => "x86_64-unknown-linux-gnu".to_string(),
        (Arch::Aarch64, Os::Linux)  => "aarch64-unknown-linux-gnu".to_string(),
    }
}

/// List of all supported target triples for `--print-targets`.
pub fn supported_targets() -> Vec<&'static str> {
    vec![
        "x86_64-unknown-linux-gnu",
        "aarch64-unknown-linux-gnu",
        "x86_64-apple-darwin",
        "aarch64-apple-darwin",
    ]
}

/// Generate C static assertions that validate the target layout assumptions
/// at C compile time.  Emitted into the runtime header.
pub fn emit_c_layout_guards(target: &TargetInfo) -> String {
    let pb = target.pointer_bytes();
    format!(
        r#"/* Target layout guards — compile-time validation */
_Static_assert(sizeof(void *) == {pb}, "pointer size mismatch: expected {pb} bytes");
_Static_assert(sizeof(int32_t) == {int}, "int32_t size mismatch");
_Static_assert(sizeof(int64_t) == {long}, "int64_t size mismatch");
_Static_assert(sizeof(float) == {real}, "float size mismatch");
_Static_assert(sizeof(double) == {longreal}, "double size mismatch");
_Static_assert(_Alignof(void *) == {pa}, "pointer alignment mismatch");
"#,
        pb = pb,
        int = target.int_layout.integer_bytes,
        long = target.int_layout.longint_bytes,
        real = target.int_layout.real_bytes,
        longreal = target.int_layout.longreal_bytes,
        pa = target.alignments.pointer_align,
    )
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_host() {
        let t = TargetInfo::from_host();
        assert_eq!(t.pointer_bits, 64);
        assert_eq!(t.endian, Endianness::Little);
        assert!(t.supports_setjmp);
        assert!(!t.triple.is_empty());
    }

    #[test]
    fn test_from_triple_short() {
        let t = TargetInfo::from_triple("x86_64-linux").unwrap();
        assert_eq!(t.arch, Arch::X86_64);
        assert_eq!(t.os, Os::Linux);
        assert_eq!(t.c_abi, CAbi::SysV);
        assert_eq!(t.triple, "x86_64-unknown-linux-gnu");
    }

    #[test]
    fn test_from_triple_full() {
        let t = TargetInfo::from_triple("aarch64-apple-darwin").unwrap();
        assert_eq!(t.arch, Arch::Aarch64);
        assert_eq!(t.os, Os::Darwin);
        assert_eq!(t.c_abi, CAbi::Darwin);
    }

    #[test]
    fn test_from_triple_llvm_canonical() {
        let t = TargetInfo::from_triple("arm64-apple-macosx14.0.0").unwrap();
        assert_eq!(t.arch, Arch::Aarch64);
        assert_eq!(t.os, Os::Darwin);
    }

    #[test]
    fn test_from_triple_invalid_arch() {
        assert!(TargetInfo::from_triple("mips-linux").is_err());
    }

    #[test]
    fn test_from_triple_invalid_os() {
        assert!(TargetInfo::from_triple("x86_64-windows").is_err());
    }

    #[test]
    fn test_pointer_bytes() {
        let t = TargetInfo::from_host();
        assert_eq!(t.pointer_bytes(), 8);
    }

    #[test]
    fn test_llvm_triple() {
        let t = TargetInfo::from_triple("aarch64-linux").unwrap();
        assert_eq!(t.llvm_triple(), "aarch64-unknown-linux-gnu");

        let t = TargetInfo::from_triple("aarch64-darwin").unwrap();
        assert_eq!(t.llvm_triple(), "arm64-apple-macosx14.0.0");
    }

    #[test]
    fn test_llvm_datalayout() {
        let t = TargetInfo::from_triple("x86_64-linux").unwrap();
        assert!(t.llvm_datalayout().starts_with("e-m:e"));

        let t = TargetInfo::from_triple("x86_64-darwin").unwrap();
        assert!(t.llvm_datalayout().starts_with("e-m:o"));
    }

    #[test]
    fn test_primitive_sizes() {
        let t = TargetInfo::from_host();
        assert_eq!(t.primitive_size(TY_INTEGER), Some(4));
        assert_eq!(t.primitive_size(TY_LONGINT), Some(8));
        assert_eq!(t.primitive_size(TY_ADDRESS), Some(8));
        assert_eq!(t.primitive_size(TY_CHAR), Some(1));
        assert_eq!(t.primitive_size(TY_REAL), Some(4));
        assert_eq!(t.primitive_size(TY_LONGREAL), Some(8));
    }

    #[test]
    fn test_primitive_alignments() {
        let t = TargetInfo::from_host();
        assert_eq!(t.primitive_align(TY_CHAR), Some(1));
        assert_eq!(t.primitive_align(TY_INTEGER), Some(4));
        assert_eq!(t.primitive_align(TY_ADDRESS), Some(8));
        assert_eq!(t.primitive_align(TY_LONGREAL), Some(8));
    }

    #[test]
    fn test_align_up() {
        assert_eq!(align_up(0, 4), 0);
        assert_eq!(align_up(1, 4), 4);
        assert_eq!(align_up(4, 4), 4);
        assert_eq!(align_up(5, 8), 8);
        assert_eq!(align_up(16, 8), 16);
    }

    #[test]
    fn test_type_size_array() {
        let mut reg = TypeRegistry::new();
        let arr_tid = reg.register(Type::Array {
            index_type: TY_INTEGER,
            elem_type: TY_INTEGER,
            low: 0,
            high: 9,
        });
        let t = TargetInfo::from_host();
        assert_eq!(t.type_size(arr_tid, &reg), 40);  // 10 * 4
    }

    #[test]
    fn test_type_size_record() {
        let mut reg = TypeRegistry::new();
        let rec_tid = reg.register(Type::Record {
            fields: vec![
                crate::types::RecordField {
                    name: "x".to_string(),
                    typ: TY_CHAR,
                    type_name: "CHAR".to_string(),
                    offset: 0,
                },
                crate::types::RecordField {
                    name: "y".to_string(),
                    typ: TY_INTEGER,
                    type_name: "INTEGER".to_string(),
                    offset: 0,
                },
            ],
            variants: None,
        });
        let t = TargetInfo::from_host();
        // CHAR(1) + 3 padding + INTEGER(4) = 8
        assert_eq!(t.type_size(rec_tid, &reg), 8);
    }

    #[test]
    fn test_compute_record_layout() {
        let mut reg = TypeRegistry::new();
        let rec_tid = reg.register(Type::Record {
            fields: vec![
                crate::types::RecordField {
                    name: "a".to_string(),
                    typ: TY_CHAR,
                    type_name: "CHAR".to_string(),
                    offset: 0,
                },
                crate::types::RecordField {
                    name: "b".to_string(),
                    typ: TY_LONGINT,
                    type_name: "LONGINT".to_string(),
                    offset: 0,
                },
                crate::types::RecordField {
                    name: "c".to_string(),
                    typ: TY_CHAR,
                    type_name: "CHAR".to_string(),
                    offset: 0,
                },
            ],
            variants: None,
        });
        let t = TargetInfo::from_host();
        let layout = compute_record_layout(rec_tid, &reg, &t).unwrap();
        assert_eq!(layout.fields[0].offset, 0);   // a: CHAR at 0
        assert_eq!(layout.fields[1].offset, 8);   // b: LONGINT at 8 (aligned)
        assert_eq!(layout.fields[2].offset, 16);  // c: CHAR at 16
        assert_eq!(layout.total_size, 24);         // padded to align 8
        assert_eq!(layout.alignment, 8);
    }

    #[test]
    fn test_linker_gc_flags() {
        let linux = TargetInfo::from_triple("x86_64-linux").unwrap();
        assert_eq!(linux.linker_gc_flags(), vec!["-Wl,--gc-sections"]);

        let darwin = TargetInfo::from_triple("x86_64-darwin").unwrap();
        assert_eq!(darwin.linker_gc_flags(), vec!["-Wl,-dead_strip"]);
    }

    #[test]
    fn test_c_layout_guards() {
        let t = TargetInfo::from_host();
        let guards = emit_c_layout_guards(&t);
        assert!(guards.contains("_Static_assert"));
        assert!(guards.contains("sizeof(void *)"));
    }

    #[test]
    fn test_display() {
        let t = TargetInfo::from_triple("x86_64-linux").unwrap();
        let s = format!("{}", t);
        assert!(s.contains("x86_64"));
        assert!(s.contains("64bit"));
    }

    #[test]
    fn test_all_supported_targets_parse() {
        for triple in supported_targets() {
            let t = TargetInfo::from_triple(triple).unwrap();
            assert_eq!(t.pointer_bits, 64);
            assert!(!t.llvm_triple().is_empty());
            assert!(!t.llvm_datalayout().is_empty());
        }
    }
}
