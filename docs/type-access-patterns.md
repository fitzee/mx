# Type and Access Patterns

Reference for all designator resolution and type access patterns the compiler must handle. Derived from the HIR builder, C backend, and LLVM backend codegen. Use this as a checklist when modifying designator resolution or adding new type support.

There is no single official PIM4 document that enumerates these patterns exhaustively. Wirth's *Programming in Modula-2* (4th ed.) defines the grammar and type rules, but the interaction of ADDRESS, POINTER TO ARRAY, open arrays, string constants, WITH scopes, and module-qualified access creates a combinatorial space that only emerges in real programs.

## Designator Base Resolution

A designator resolves to one of four base kinds:

| Base | Example | Description |
|------|---------|-------------|
| Local | `x` in a procedure | Procedure parameter or local variable |
| Global | `Module.x`, module-level `x` | Module-level variable or imported symbol |
| Constant | `MaxSize`, `"ABCDEF"` | Compile-time constant (integer, string, enum, etc.) |
| FuncRef | `WriteString` | Procedure used as a value (procedure variable) |

### Resolution order in `resolve_base`

1. Module-qualified (`Module.Name`) -- `resolve_module_qualified_base`
2. Constants/enums (checked first to avoid shadowing by codegen vars)
3. Procedure-local variables (direct scope lookup, `module.is_none()`)
4. Scope chain lookup (sema symtab, walks parent scopes)
5. `var_types` fallback (backend-registered variables not in sema)

## Selector Projections

After resolving the base, selectors are applied left-to-right:

### Field (`.name`)

| Base Type | Result |
|-----------|--------|
| Record | Field type, resolved by name -> index |
| Record with variants | Variant field type, two-level index |
| Pointer (fallback) | Auto-deref to base, then field lookup |

### Index (`[expr]`)

| Base Type | Result |
|-----------|--------|
| Array | Element type |
| OpenArray | Element type |
| StringLit | CHAR (string constant indexing: `"ABC"[i]`) |
| Char | CHAR (ADDRESS byte access: `addr^[i]`) |

### Deref (`^`)

| Base Type | Result |
|-----------|--------|
| Pointer { base } | Resolved base type |
| Address | CHAR (byte-level access) |
| Opaque/unresolved | ADDRESS (generic fallback) |

## C Backend Projection Optimizations

The C backend collapses projection pairs:

| Pattern | C Output | When |
|---------|----------|------|
| Deref + Field | `ptr->field` | Always |
| Deref + VariantField | `ptr->variant.vN.field` | Always |
| Deref + Index (ADDRESS byte) | `((char*)ptr)[idx]` | `proj.ty == TY_CHAR` and no further projections |
| Deref + Index (typed array) | `(*ptr)[idx]` | Pointer-to-array types |
| Constant + Index | `"string"[idx]` | String constant with index projection |

## Open Array Patterns

Open array parameters generate a `(ptr, high)` pair at call sites:

| Argument Type | HIGH Value |
|---------------|------------|
| Fixed array | Compile-time constant (high bound) |
| Open array param | Forward the `_high` companion |
| String literal | String length |
| String constant | String length |

### is_open_array_type check

Must match ALL of:
- `Type::OpenArray { .. }`
- `Type::StringLit(_)` (stdlib uses TY_STRING for `ARRAY OF CHAR` params)

## Special Type Handling

### ADDRESS (`void *`)

- `ADDRESS^` dereferences to `CHAR` (byte access)
- `ADDRESS^[i]` is byte indexing (Deref → Char, then Index on Char)
- C emit: cast to `(char*)` for pointer arithmetic
- LLVM: GEP with i8 element type

### POINTER TO ARRAY

- C declaration: `T (*name)[size]` (not `T *name`)
- Deref gives the array type, then Index gives element type
- C emit: `(*ptr)[idx]` (correct for array pointer types)

### String Constants

- Can be indexed: `"0123456789ABCDEF"[nibble]` -> CHAR
- HIR: `PlaceBase::Constant(String)` with `Index` projection
- Must NOT be unwrapped to literal in `lower_expr` when projections exist
- C emit: `"string"[idx]`
- LLVM: intern string as global array, GEP + load

### WITH Scopes

- Bare identifiers checked against WITH field names (innermost first)
- Build Place: WITH record var base + optional Deref + Field projection
- Pointer-to-record WITH: `needs_deref = true`

### VAR Parameters

- `PlaceBase::Local` with `is_var_param = true`
- C: `(*name)` wrapper on access
- LLVM: load pointer from alloca (double indirection)
- ALLOCATE/DEALLOCATE: first VAR arg needs `(void **)` cast in C for clang 16+

## Builtin Special Cases

| Builtin | Special Handling |
|---------|-----------------|
| TSIZE/SIZE | First arg is type name -- must be mangled (`mangle_type_name`) |
| ALLOCATE/DEALLOCATE | First VAR arg cast to `(void **)` in C |
| HIGH | Open array: use `_high` companion; fixed array: compile-time constant |
| CAP, ORD, CHR, Write | Args coerced to char (single-char string -> char literal) |
| INCL, EXCL | Second arg coerced to char |
| NEW/DISPOSE | M2+ typed versions use RTTI type descriptors |

## Alias Resolution

Every type match is preceded by `resolve_alias()` which follows `Type::Alias { target }` chains (max depth 50). All pattern matching operates on canonical (non-alias) types.

## Scope Requirements for Embedded Modules

Sema must run full `analyze_implementation_module` (not the lightweight `register_impl_types`) for all embedded `.mod` files. The HIR builder requires:

- Module-level variables, constants, types
- Procedure symbols with parameter info (ParamInfo with types, VAR flags)
- Procedure scopes with parameters registered as Variable symbols
- Local variables and constants within procedures
- Import resolution (FROM Module IMPORT ...)

Without full scope info, `scope_lookup` returns None and designator resolution fails.
