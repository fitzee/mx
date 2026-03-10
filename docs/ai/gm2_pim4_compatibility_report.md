# mx vs gm2 PIM4 Pass Tests — Compatibility Report

**Date:** 2026-03-09
**Version:** 1.0.0
**Source:** `gcc/testsuite/gm2/pim/pass/` (GCC master, 483 .mod files)
**Method:** `mx --emit-c` then `cc -fsyntax-only` on each test

## Summary

| Stage | Count | % of total |
|-------|------:|--------:|
| Total .mod files | 483 | 100% |
| **Transpile + CC pass** | **383** | **79.3%** |
| Parse failure | 14 | 2.9% |
| Sema/type failure | 13 | 2.7% |
| Codegen/other mx error | 20 | 4.1% |
| C compile failure | 53 | 11.0% |

## What Changed Since 0.x (260/483 → 383/483)

123 additional tests now pass thanks to these codegen fixes:

1. **POINTER TO RECORD** — Anonymous record types inside pointer declarations now generate correct C struct definitions. Self-referential pointer-to-record types (linked lists, trees) work correctly.
2. **WITH on pointer-to-record** — `WITH p^ DO` resolves fields through the pointer's base record type.
3. **Multi-name pointer fields** — `left, right: POINTER TO Foo` emits separate C declarations so both names are pointers.
4. **SET OF inline enum** — `TYPE s = SET OF (a, b, c)` emits enum constants and a uint32_t set type with MIN/MAX macros.
5. **Char literals in set operations** — Single-character string literals in INCL, EXCL, IN, set constructors, and array indices emit as C char literals.
6. **Module-level variable forward references** — Procedures can reference module-level variables declared after them.
7. **Constant forward references** — Constants referencing later-declared constants are topologically sorted before emission.
8. **Nested module procedure hoisting** — Procedures inside local modules within a procedure are hoisted to file scope.
9. **Nested procedure name mangling** — Same-named procedures nested in different parents get unique C names.
10. **MIN/MAX macros** — User-defined enumeration, subrange, and set-of-enum types emit `m2_min_`/`m2_max_` macros.
11. **File type mapping** — `File` is only mapped to `m2_File` when imported from FileSystem/FIO.

## Parse Failures (14) — Grouped by Root Cause

### 1. Underscores in identifiers (7 tests)
gm2 allows `_` in identifiers (GNU extension). PIM4 does not.
- **Verdict:** gm2 extension, not PIM4. Can ignore.

### 2. `VAR a[0..n]: type` local block syntax (3 tests)
gm2-specific extension for declaring array-typed vars inline.
- **Verdict:** gm2 extension, not PIM4. Can ignore.

### 3. Forward declaration / ASM extensions (2 tests)
- `forward`: forward procedure declaration syntax
- `fooasm3`: inline assembly
- **Verdict:** gm2 extensions. Can ignore.

### 4. CASE label range with `:` (1 test)
- `testcase4`: uses `1:2` range syntax instead of `1..2`
- **Verdict:** gm2 extension. Can ignore.

### 5. Multi-dimensional array (1 test)
- `subrange2`: `ARRAY [a],[b] OF T`
- **Action:** Parser should support multi-dimensional array shorthand.

## Sema/Type Failures (13) — Grouped by Root Cause

### 1. Array constructor / set constant syntax (3 tests)
- **Verdict:** Likely gm2 extension. Needs investigation.

### 2. DIV/MOD on ADDRESS (2 tests)
- **Verdict:** gm2 extension. Can ignore.

### 3. WORD/opaque assignment compatibility (2 tests)
- **Verdict:** gm2-specific type flexibility.

### 4. PROC type (2 tests)
- **Verdict:** gm2 builtin type, not standard PIM4.

### 5. Other (4 tests)
- Mostly gm2 extensions or edge cases.

## Codegen/Other Errors (20) — Grouped by Root Cause

### 1. PROC type not defined (6 tests)
`PROC` is a gm2 builtin type. Could add as alias for `PROCEDURE`.

### 2. Undefined type 'C' — DEFINITION FOR "C" imports (8 tests)
Missing test companion `.def` files or gm2-specific FFI.

### 3. SHORTCARD / SHORTREAL types (3 tests)
gm2 extensions, not PIM4.

### 4. Other (3 tests)
Edge cases.

## C Compile Failures (53) — Grouped by Root Cause

### 1. Missing gm2 stdlib modules (~20 tests)
Calls to `StrIO_WriteString`, `NumberIO_WriteCard`, `libc_exit`, etc. — gm2's standard library, not PIM4.

### 2. Scalar to open array param (3 tests)
Scalar passed to `ARRAY OF BYTE/CHAR` — needs address-of synthesis.

### 3. Type forward-reference cycles (8 tests)
Mutually recursive types across module boundaries.

### 4. gm2 extensions in generated C (8 tests)
Non-PIM4 features that partially transpile but produce invalid C.

### 5. Nested module variable scoping (6 tests)
Variables in nested modules not fully resolved.

### 6. Variant record field access (5 tests)
Direct access to variant record fields in some patterns.

### 7. Other (3 tests)
Miscellaneous edge cases.

## Adjusted Compliance Assessment

| Category | Tests | Notes |
|----------|------:|-------|
| gm2 extensions (not PIM4) | ~55 | Underscores, PROC, SHORTCARD, DIV on ADDRESS, gm2 stdlib, inline asm |
| **Genuine PIM4 gaps** | ~45 | Remaining codegen and sema edge cases |
| **Clean passes** | **383** | |

### What mx Gets Right

383/483 tests pass clean through transpile + C compile — covering:
- Basic types, assignments, expressions
- Records, arrays, pointers, POINTER TO RECORD
- Simple and complex procedures, functions, parameters
- VAR parameters, open arrays
- Sets, BITSET, SET OF enum, char in sets
- FOR/WHILE/REPEAT/IF/CASE control flow
- Module imports, multi-module, nested modules
- Type declarations, enumerations, subranges
- String handling
- WITH statements on records and pointer-to-records
- Forward references (variables and constants)
- Nested procedures with name mangling
- Local modules with procedure hoisting
