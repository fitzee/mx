# PIM4 Conformance Audit — mx Compiler & Grammar Reference

**Date:** 2026-03-14
**Compiler version:** 1.0.6
**Scope:** Parser, sema, type system, grammar doc, extension gating
**Status:** ANALYSIS ONLY — no changes made

---

## 1. Executive Summary

The mx compiler is **broadly correct** for practical PIM4 usage: 383/483 gm2 PIM4 tests pass, and the compiler handles the major language features (modules, types, records, pointers, sets, procedures, control flow) well. However, this audit identifies several categories of concern:

**Top risks:**

1. **No extension gating in the parser.** Every M2+ construct (TRY, LOCK, TYPECASE, REF, OBJECT, EXCEPTION, RAISE, RETRY, SAFE/UNSAFE, RAISES, import AS aliases) is accepted in PIM4 mode. The `m2plus` flag is never checked during parsing. M2+ keywords are always reserved, preventing their use as PIM4 identifiers.
2. **Operator precedence diverges from PIM4.** The grammar doc correctly shows PIM4 precedence (OR at additive level, AND at multiplicative level), but the parser gives OR and AND their own lower-priority levels (C-style). This changes the parse of expressions mixing boolean and arithmetic operators.
3. **Overly permissive type system.** `assignment_compatible` allows any-record-to-any-record, any-array-to-any-array, and any-pointer-to-any-pointer. PIM4 requires name equivalence for structured types.
4. **ISO/M2+ clauses in Block.** `EXCEPT` and `FINALLY` are accepted in every `Block` parse, unconditionally, which is ISO Modula-2 — not PIM4.
5. **Grammar doc inaccuracies.** Several productions don't match the parser, and PIM4 vs M2+ constructs are mixed in core productions.

None of these are show-stoppers for practical use, but they create correctness risks for strict PIM4 conformance and could confuse users relying on the grammar doc as authoritative.

---

## 2. Findings Table

| ID | Area | Current Behavior | Expected PIM4 Behavior | Severity | Action | Kind |
|----|------|-----------------|----------------------|----------|--------|------|
| F01 | Parser/gating | M2+ constructs (TRY, LOCK, TYPECASE, REF, OBJECT, EXCEPTION, RAISE, SAFE/UNSAFE, RAISES, AS) parsed unconditionally | M2+ constructs rejected unless `--m2plus` | **Critical** | Gate parsing on `m2plus` flag | Code fix |
| F02 | Parser/gating | M2+ keywords (BRANDED, EXCEPTION, LOCK, METHODS, OBJECT, OVERRIDE, REF, REFANY, REVEAL, SAFE, TRY, TYPECASE, UNSAFE) always reserved | In PIM4 mode, these should be legal identifiers | **Critical** | Conditional keyword recognition in lexer | Code fix |
| F03 | Parser | `EXCEPT`/`FINALLY` accepted in every Block (ISO style) | PIM4 has no EXCEPT/FINALLY in Block | **High** | Gate behind m2plus or iso flag | Code fix |
| F04 | Parser/grammar | OR and AND have dedicated precedence levels (C-style) | PIM4: OR is at additive level (+, -), AND is at multiplicative level (*, /, DIV, MOD) | **High** | Policy decision: keep C-style or match PIM4 | Policy decision |
| F05 | Grammar doc | `Statement` production includes TryStatement, LockStatement, TypecaseStatement | These should only appear in M2+ section | **High** | Move to M2+ section of grammar doc | Doc fix |
| F06 | Grammar doc | `AddOp = "+" \| "-" \| "OR"` and `MulOp = ... \| "AND"` (PIM4 style) | Parser uses separate precedence levels for OR/AND | **High** | Grammar doc must match parser, or parser must match doc | Doc fix or code fix |
| F07 | Sema | `assignment_compatible` allows any record→record, array→array, pointer→pointer | PIM4 requires name equivalence for structured types | **High** | Tighten type compatibility checks | Code fix |
| F08 | Sema | No validation that opaque types must be POINTER in implementation | PIM4: opaque types in .def must be revealed as POINTER TO in .mod | **Medium** | Add sema check | Code fix |
| F09 | Grammar doc | `TypeDecl = ident "=" Type \| ident .` combines def and impl behavior | Opaque declarations (`ident ;` alone) are only valid in .def modules | **Medium** | Separate in grammar doc; note already correct in parser | Doc fix |
| F10 | Parser | `parse_formal_type` allows recursive `ARRAY OF ARRAY OF QualIdent` | PIM4 `FormalType = ["ARRAY" "OF"] QualIdent` — only one level of ARRAY OF | **Medium** | Restrict to single ARRAY OF in PIM4 mode | Code fix |
| F11 | Parser | Import `AS` alias always accepted | Import aliases are an mx extension, not PIM4 | **Medium** | Gate behind m2plus or extension flag | Code fix |
| F12 | Grammar doc | `ForeignDefModule` not reachable from `CompilationUnit` | If documented, should be shown as M2+ variant of CompilationUnit | **Medium** | Fix grammar doc | Doc fix |
| F13 | Grammar doc | `CharConst` referenced in Factor but undefined | Should be `CharLit` or defined as terminal | **Low** | Fix grammar doc | Doc fix |
| F14 | Grammar doc | `ConstExpr` referenced but never defined | Should note: same as Expr but must be compile-time evaluable | **Low** | Add note to grammar doc | Doc fix |
| F15 | Grammar doc | `number` and `string` in Factor not defined as terminals | Should define terminal symbols | **Low** | Fix grammar doc | Doc fix |
| F16 | Grammar doc | `&` (AND synonym) and `~` (NOT synonym) undocumented | PIM4 defines these as synonyms | **Low** | Document in grammar | Doc fix |
| F17 | Grammar doc | `<>` accepted as not-equal but undocumented | PIM4 uses `#` primarily; `<>` is standard too | **Low** | Document in grammar | Doc fix |
| F18 | Sema | No check that FOR variable is not assigned inside loop body | PIM4 forbids assignment to FOR control variable | **Medium** | Add sema check | Code fix |
| F19 | Sema | RETURN with/without expression not validated against procedure kind | PIM4: function procedures require RETURN expr; proper procedures forbid it | **Medium** | Add sema check | Code fix |
| F20 | Sema | No validation that CASE labels are constants and non-overlapping | PIM4 requires constant, non-overlapping case labels | **Low** | Add sema check | Code fix |
| F21 | Parser | Module priority `MODULE name [expr] ;` parsed but undocumented | PIM4 allows module priority for interrupt handling | **Low** | Document or remove | Doc fix |
| F22 | Parser | `RAISE` and `RETRY` parsed as statements unconditionally | These are M2+/ISO, not PIM4 | **High** | Gate behind extension flag | Code fix |
| F23 | Grammar doc | `RaiseStatement` in M2+ section but parsed without gating | Inconsistency between doc and implementation | **Medium** | Match doc (gate in parser) | Code fix |
| F24 | Grammar doc | Missing `Definition` production for def modules | Grammar shows `{ Definition }` but doesn't define it separately from `Declaration` | **Medium** | Add `Definition` production to grammar | Doc fix |
| F25 | Sema | Set constructors always typed as BITSET regardless of base_type | Should use declared base type for typed set constructors | **Low** | Fix set constructor type inference | Code fix |
| F26 | Grammar doc | Bare set constructor `{1, 2, 3}` undocumented | Parser accepts it (line 1606-1608), produces BITSET | **Low** | Document as mx extension or match PIM4 | Doc fix |
| F27 | Parser | `SubrangeType` accepts `QualIdent "[" ConstExpr ".." ConstExpr "]"` form | PIM4: `[low..high]` only; base-typed subrange is not standard | **Low** | Document as extension or gate | Policy decision |

---

## 3. Syntax/Grammar Review

### 3.1 Compilation Units

**Grammar doc:**
```
CompilationUnit = ProgramModule | DefinitionModule | ImplementationModule .
```

**Issues:**
- `ForeignDefModule` is documented separately but not reachable from `CompilationUnit`. **(F12)**
- `SAFE`/`UNSAFE` prefix handling is not shown in the grammar. The parser accepts `SAFE MODULE ...` etc. unconditionally. **(F01)**
- ProgramModule priority expression `[expr]` is parsed (line 219) but not in grammar doc. **(F21)**

**Parser behavior:** Correct structure. Module name matching is enforced.

### 3.2 Imports/Exports

**Grammar doc:**
```
Import = "IMPORT" IdentList ";" | "FROM" ident "IMPORT" IdentList ";" .
Export = "EXPORT" [ "QUALIFIED" ] IdentList ";" .
```

**Issues:**
- Parser accepts `FROM M IMPORT x AS y` (import aliases) unconditionally. Not PIM4. **(F11)**
- `IdentList` in grammar doesn't show the alias option. **(F11)**
- EXPORT is correctly parsed for local modules and definition modules. EXPORT in definition modules is redundant in PIM4 (everything is exported). The parser and sema handle this correctly.
- Grammar doc's `IdentList` for Import should note that plain `IMPORT` takes module names (qualified import), while `FROM...IMPORT` takes individual names.

### 3.3 Declarations

**Grammar doc:**
```
Declaration = "CONST" ... | "TYPE" ... | "VAR" ... | ProcedureDecl ";" .
ProcedureDecl = ProcedureHeading ";" Block ident .
```

**Issues:**
- The grammar doesn't show a separate `Definition` production for definition modules. The parser correctly uses `parse_definition_module` which calls `parse_proc_heading` (heading only, no block), while `parse_declarations` calls `parse_proc_decl` (heading + block). **(F24)**
- `TypeDecl = ident "=" Type | ident .` conflates def-module and impl-module behavior. The bare `ident` (opaque type) is only valid in definition modules. Parser correctly separates this (`parse_type_decl` vs `parse_type_decl_def`). **(F09)**
- `EXCEPTION` declarations are parsed unconditionally in both `parse_declarations` (line 549-557) and `parse_definition_module` (line 345-352). Should be gated. **(F01)**
- Local module declarations (`Declaration::Module`) are parsed correctly via `parse_local_module`.
- Grammar doc doesn't show local (nested) module declarations.

### 3.4 Types

**Grammar doc productions vs parser behavior:**

| Production | Grammar Doc | Parser | Match? |
|-----------|------------|--------|--------|
| ArrayType | `ARRAY SimpleType {, SimpleType} OF Type` | `parse_array_type`: ARRAY Type {, Type} OF Type | **Mismatch** — parser uses full Type for index, grammar says SimpleType |
| RecordType | Correct | Correct | OK |
| SetType | `SET OF SimpleType` | `parse_set_type`: SET OF Type (full type) | **Mismatch** — parser is more permissive |
| PointerType | Correct | Correct | OK |
| ProcedureType | Correct | Correct (with lookahead for named vs unnamed params) | OK |
| EnumType | Correct | Correct | OK |
| SubrangeType | Shows `QualIdent "[" ...]` form | Parser accepts this form (line 771-783) | Match, but this form is not PIM4 **(F27)** |
| RefType | Marked M2+ | Parsed unconditionally | **Extension leak (F01)** |
| ObjectType | Marked M2+ | Parsed unconditionally | **Extension leak (F01)** |

**Note on ArrayType index:** PIM4 says `ARRAY SimpleType {, SimpleType} OF Type` where `SimpleType = QualIdent | SubrangeType | EnumType`. The parser accepts any `Type` for array indices, which is more permissive (e.g., allows `ARRAY RECORD ... END OF INTEGER`). Not a practical risk but technically non-conformant.

### 3.5 Statements

**Grammar doc:**
```
Statement = [ Assignment | ProcedureCall | IfStatement | CaseStatement
            | WhileStatement | RepeatStatement | ForStatement
            | LoopStatement | WithStatement | "EXIT" | "RETURN" [ Expr ]
            | TryStatement | LockStatement | TypecaseStatement ] .
```

**Issues:**
- `TryStatement`, `LockStatement`, `TypecaseStatement` are in the base `Statement` production. They should only be in the M2+ section. **(F05)**
- `RAISE` and `RETRY` are parsed unconditionally (lines 1185-1198) but not shown in the base Statement production. They appear only in the M2+ grammar section. **(F22, F23)**
- Grammar doc correctly shows `ForStatement = "FOR" ident ":=" Expr "TO" Expr ["BY" ConstExpr] ...` but PIM4 requires BY to be a constant expression. Parser accepts any expression for BY step. Sema doesn't validate constness. **(F20)**

### 3.6 Expressions

**Grammar doc:**
```
SimpleExpr = [ "+" | "-" ] Term { AddOp Term } .
AddOp      = "+" | "-" | "OR" .
Term       = Factor { MulOp Factor } .
MulOp      = "*" | "/" | "DIV" | "MOD" | "AND" .
```

**Parser implementation:**
```
parse_or_expr → parse_and_expr → parse_rel_expr → parse_add_expr → parse_mul_expr → parse_unary_expr → parse_factor
```

The parser gives OR and AND their own dedicated, lower-priority levels. In PIM4, OR has the **same** precedence as `+`/`-`, and AND has the **same** precedence as `*`/`/`/`DIV`/`MOD`. **(F04, F06)**

**Practical impact:** Expression `a + b OR c` parses as `(a + b) OR c` in both schemes (OR is lower or equal to +). But `a OR b + c` parses as `a OR (b + c)` in mx (+ binds tighter) vs `(a OR b) + c` in strict PIM4 (same level, left-to-right). This matters if someone writes mixed arithmetic+boolean (unusual but legal in PIM4 with BITSET operations).

**Other expression issues:**
- `&` (Ampersand) parsed as AND synonym — correct for PIM4 but undocumented. **(F16)**
- `~` (Tilde) parsed as NOT synonym — correct for PIM4 but undocumented. **(F16)**
- `<>` parsed as not-equal (NotEq) — standard but undocumented in grammar. **(F17)**
- `Factor` in grammar doc mentions `CharConst` which is not a defined production. **(F13)**
- `number` and `string` in grammar doc are not defined as terminals. **(F15)**

### 3.7 Dead/Unreachable Productions

| Production | Status |
|-----------|--------|
| `ForeignDefModule` | Documented but not reachable from CompilationUnit |
| `RaiseStatement` | Documented in M2+ section, parsed unconditionally, not in base Statement production |
| `SafetyAnnotation` | Documented in M2+ section, parsed unconditionally, noted as "not enforced" |
| `ExceptionDecl` | Documented in M2+ section only, but parsed unconditionally in both definition and implementation modules |
| `Ellipsis` token | Defined in token.rs but never referenced in parser |

---

## 4. Semantic Review

### 4.1 Type Compatibility (F07)

`assignment_compatible()` (types.rs:281-409) is the most significant semantic weakness:

- **Record types:** Any record is assignable to any record (line 367-369). PIM4 requires name equivalence — only the same named type is compatible with itself.
- **Array types:** Any array to any array (lines 371-376). PIM4 requires same index type and element type (name equivalence).
- **Pointer types:** Any pointer to any pointer (lines 315-317). PIM4 is more restrictive — only same-named pointer types are compatible, plus NIL.
- **Set types:** Any set to any set (lines 363-365). Should check base type compatibility.
- **Enum-integer compatibility:** Enums freely interassignable with integers (lines 352-357). PIM4 is stricter.
- **Ordinal interassignment:** All ordinal types freely interassignable (lines 359-361). Too permissive.

These are marked "simplified" in comments, clearly intentional for initial implementation. For PIM4 conformance, name equivalence must be enforced for structured types.

### 4.2 Opaque Types (F08)

PIM4 requires that opaque types declared in a `.def` must be revealed as `POINTER TO T` in the corresponding `.mod`. The sema (`analyze_implementation_module`, line 380) imports types from the def module but does not validate that opaque types are properly revealed as pointers.

### 4.3 FOR Loop Variable (F18)

PIM4 forbids assignment to the FOR control variable inside the loop body. The sema doesn't check this — it only validates the variable exists and is ordinal (lines 1074-1098).

### 4.4 RETURN Semantics (F19)

PIM4 distinguishes function procedures (with return type) from proper procedures (no return type):
- Function procedures must have a RETURN expr on every exit path
- Proper procedures must not have RETURN expr

The sema checks RETURN type compatibility (line 1118-1127) but doesn't enforce the function/proper distinction — a proper procedure can have `RETURN expr` without error.

### 4.5 Constant Expression Validation (F20)

Multiple places require constant expressions (CASE labels, FOR BY step, subrange bounds, CONST declarations). The sema has `eval_const_expr` but doesn't verify that expressions used in constant contexts are actually compile-time evaluable. A function call in a CASE label would parse and only fail at codegen.

### 4.6 Set Constructor Typing (F25)

`analyze_expr` returns `TY_BITSET` for all set constructors regardless of the `base_type` field (line 1323). A set constructor with an explicit type like `CharSet{0C..37C}` is typed as BITSET instead of the declared set type.

### 4.7 Module Kind Validation

The sema correctly:
- Separates definition module analysis from implementation module analysis
- Prevents procedure bodies in definition modules (by using `Definition::Procedure(ProcHeading)` instead of `ProcDecl`)
- Imports types/constants from .def into .mod (lines 387-399)

But does not check:
- That implementation module procedure signatures match their .def declarations
- That all procedures declared in .def are implemented in .mod

### 4.8 WITH Statement

The sema checks that WITH operates on a record type (line 1110-1112), but the codegen handles pointer-to-record auto-dereferencing. The sema should resolve through pointers to validate the underlying record type.

---

## 5. Extension Boundary Review

### 5.1 What is Strict PIM4

PIM4 constructs (all should work without `--m2plus`):
- MODULE, DEFINITION MODULE, IMPLEMENTATION MODULE
- IMPORT, FROM...IMPORT, EXPORT QUALIFIED (in local/def modules)
- CONST, TYPE, VAR, PROCEDURE declarations
- All standard types: INTEGER, CARDINAL, REAL, LONGREAL, BOOLEAN, CHAR, BITSET, WORD, BYTE, ADDRESS, LONGINT, LONGCARD
- Array, Record (with variant part), Set, Pointer, Procedure types, Enumeration, Subrange
- Opaque types in .def modules
- IF, CASE, WHILE, REPEAT, FOR, LOOP, WITH, EXIT, RETURN
- Standard operators and precedence
- Nested (local) modules
- Module priority expression
- `&` for AND, `~` for NOT, `#` and `<>` for not-equal

### 5.2 What is M2+ / Extension

These must be gated behind `--m2plus`:

| Construct | Parser location | Currently gated? |
|-----------|----------------|-----------------|
| TRY/EXCEPT/FINALLY (M2+ statement) | `parse_try()`, line 1717 | **No** |
| LOCK statement | `parse_lock()`, line 1810 | **No** |
| TYPECASE statement | `parse_typecase()`, line 1824 | **No** |
| RAISE statement | `parse_statement()`, line 1189 | **No** |
| RETRY statement | `parse_statement()`, line 1185 | **No** |
| EXCEPTION declaration | `parse_declarations()`, line 549; `parse_definition_module()`, line 345 | **No** |
| EXCEPT/FINALLY in Block | `parse_block()`, lines 457-468 | **No** |
| REF type | `parse_ref_type()`, line 955 | **No** |
| BRANDED REF type | `parse_branded_ref_type()`, line 963 | **No** |
| REFANY type | Multiple locations | **No** |
| OBJECT type | `parse_object_type()`, line 978 | **No** |
| SAFE/UNSAFE prefix | `parse_compilation_unit()`, line 159 | **No** |
| RAISES clause | `parse_proc_heading()`, line 656 | **No** |
| Import AS alias | `parse_import()`, line 421 | **No** |
| DEFINITION MODULE FOR "lang" | `parse_definition_module()`, line 276 | **No** |

### 5.3 Keyword Reservation Leakage (F02)

All M2+ keywords are **always** reserved by the lexer (`keyword_from_str`, token.rs:128-191). This means in PIM4 mode, a user cannot use `object`, `exception`, `try`, `lock`, `ref`, `safe`, `unsafe`, `typecase`, `branded`, `methods`, `override`, `reveal`, `refany` as identifier names. This is wrong — in PIM4 mode these should be valid identifiers.

Additionally, `EXCEPT`, `FINALLY`, `RAISE`, `RETRY`, `AS` are reserved as keywords even though they are not PIM4 keywords.

---

## 6. Documentation Review

### 6.1 Current State

`docs/lang/grammar.md` is titled "Modula-2 PIM4 Grammar Reference" but functions as an "mx accepted syntax reference" — it documents what the parser accepts, not strict PIM4. M2+ constructs appear both in the base productions (Statement) and in a separate M2+ section.

### 6.2 Recommended Structure

The grammar doc should be restructured into three clear sections:

1. **Strict PIM4 Core** — Productions matching PIM4, 4th Edition. OR at additive level, AND at multiplicative level, no extension constructs.
2. **mx Accepted Differences** — Where mx intentionally diverges from strict PIM4 for practical reasons (e.g., C-style operator precedence if kept, bare set constructors, `<>` as not-equal synonym).
3. **Modula-2+ Extensions** — All M2+ constructs, clearly separated.

### 6.3 Specific Sections Needing Rewrite

| Section | Issue |
|---------|-------|
| Compilation Units | Add SAFE/UNSAFE prefix (M2+ only), show module priority |
| Imports | Document AS alias as extension |
| Declarations | Add `Definition` production for .def modules; separate opaque type rule |
| Types | Note which forms are extensions (QualIdent[lo..hi] subrange, bare set constructors) |
| Statements | Remove TryStatement/LockStatement/TypecaseStatement from base production |
| Expressions | Fix precedence: either match parser (document C-style levels) or match PIM4 |
| Factor | Define terminals (number, string, CharConst) |
| M2+ section | Move RAISE, RETRY, EXCEPT/FINALLY-in-Block here |
| New section | Add "mx extensions" for non-M2+ extensions (import aliases, bare sets, etc.) |

---

## 7. Proposed Implementation Plan

### Phase 1: Validation Tests

**Goal:** Establish a test baseline before making changes.

**Work:**
- Add parser tests for each M2+ construct to verify they are rejected in PIM4 mode (negative tests)
- Add parser tests for each PIM4 construct to verify they still work (positive tests)
- Add sema tests for type compatibility edge cases
- Add tests for M2+ keywords as PIM4 identifiers

**Files:** `src/parser.rs` (test module), new test files in `tests/adversarial/`

**Risk:** Low. Tests only.

**Test strategy:** Run full `cargo test` + adversarial suite before and after.

### Phase 2: Extension Gating in Parser

**Goal:** M2+ constructs rejected in PIM4 mode.

**Work:**
- Add `m2plus: bool` field to Parser struct
- Plumb `m2plus` from `CompileOptions` through to Parser constructor
- Gate each M2+ parse path on `self.m2plus`:
  - `parse_statement()`: TRY, LOCK, TYPECASE, RAISE, RETRY
  - `parse_declarations()` / `parse_definition_module()`: EXCEPTION
  - `parse_block()`: EXCEPT, FINALLY clauses
  - `parse_type()`: REF, BRANDED, REFANY, OBJECT
  - `parse_proc_heading()`: RAISES clause
  - `parse_import()`: AS alias
  - `parse_compilation_unit()`: SAFE/UNSAFE prefix
  - `parse_definition_module()`: FOR "lang" foreign module
- Gate M2+ keywords in lexer: add a `m2plus` flag that controls whether M2+ keywords are recognized or treated as identifiers

**Files:** `src/parser.rs`, `src/lexer.rs`, `src/token.rs`, `src/driver.rs`

**Risk:** Medium. Must ensure existing M2+ library code still compiles with `--m2plus`. Must ensure PIM4 code using M2+ keyword names as identifiers now works.

**Test strategy:** All existing tests must pass. New negative tests from Phase 1 must now pass. Run adversarial suite in both modes.

### Phase 3: Semantic Enforcement

**Goal:** Tighten semantic checks toward PIM4 correctness.

**Work (ordered by impact):**
1. **FOR variable assignment check (F18)** — Track FOR variables in sema, error on assignment in loop body
2. **RETURN validation (F19)** — Error if function procedure has bare RETURN or proper procedure has RETURN expr
3. **Set constructor typing (F25)** — Use base_type from SetConstructor in analyze_expr
4. **Constant expression validation (F20)** — Warn/error when non-constant expressions appear in CASE labels, FOR BY step

**Deferred (higher risk, lower priority):**
- Type name equivalence (F07) — Major change, risk of breaking existing code. Should be behind a `--strict` flag initially.
- Opaque type revelation check (F08) — Requires cross-module analysis infrastructure
- Procedure signature matching between .def and .mod — Same

**Files:** `src/sema.rs`, `src/types.rs`

**Risk:** Medium for items 1-4. High for deferred items (breaking changes to existing code).

**Test strategy:** Add targeted sema test cases. Run adversarial suite. Monitor gm2 pass rate.

### Phase 4: Operator Precedence Decision (F04, F06)

**Goal:** Align grammar doc and parser on operator precedence.

**Options:**
- **Option A: Keep C-style precedence** (current parser behavior). Update grammar doc to show separate OR/AND levels. Document as intentional mx divergence from PIM4. This is what nearly all practical M2 compilers do.
- **Option B: Match PIM4** exactly. Move OR into additive parsing and AND into multiplicative parsing. Risk: changes semantics of existing code.

**Recommendation:** Option A. Document as intentional divergence. This matches programmer expectations and every other modern M2 compiler.

**Files:** `docs/lang/grammar.md` (doc fix if Option A), or `src/parser.rs` (code fix if Option B)

**Risk:** Low (Option A), Medium (Option B — could change existing code behavior).

### Phase 5: Documentation Rewrite

**Goal:** Grammar doc accurately reflects mx behavior, clearly separates PIM4 core from extensions.

**Work:**
- Restructure into three sections: PIM4 Core, mx Divergences, M2+ Extensions
- Fix all specific issues from Section 6.3
- Add `Definition` production for def modules
- Fix/define terminal symbols (number, string, CharConst → integer/real/string/char)
- Document `ConstExpr` usage
- Document `&`, `~`, `<>` synonyms
- Document module priority syntax
- Document bare set constructors
- Remove/relocate misplaced M2+ constructs from core productions
- Add "Accepted Differences" subsection for base-typed subranges, import aliases, etc.

**Files:** `docs/lang/grammar.md`, possibly `docs/ai/LANGUAGE_RULES.md`, `docs/ai/CLAUDE.md`

**Risk:** Low. Documentation only.

**Test strategy:** N/A (doc changes).

---

## 8. Test Plan

### Positive Tests (should parse/compile)

| Test | Mode | What it validates |
|------|------|-------------------|
| PIM4 module with M2+ keyword identifiers (`VAR object: INTEGER`) | PIM4 | F02: M2+ keywords available as identifiers |
| Basic PIM4 program module | PIM4 | Baseline |
| Definition module with opaque type | PIM4 | Opaque type handling |
| Local module with EXPORT QUALIFIED | PIM4 | Export scoping |
| Variant record with nested variant | PIM4 | Variant parsing |
| Set constructor with typed base | PIM4 | Set typing |
| FOR loop with BY constant | PIM4 | FOR semantics |
| All PIM4 statement types | PIM4 | Statement coverage |
| TRY/EXCEPT/FINALLY program | M2+ | Extension acceptance |
| REF/OBJECT/LOCK program | M2+ | Extension acceptance |
| Import with AS alias | M2+ | Extension acceptance |

### Negative Tests (should reject)

| Test | Mode | What it validates |
|------|------|-------------------|
| TRY statement in PIM4 mode | PIM4 | F01: TRY rejected |
| LOCK statement in PIM4 mode | PIM4 | F01: LOCK rejected |
| TYPECASE in PIM4 mode | PIM4 | F01: TYPECASE rejected |
| RAISE in PIM4 mode | PIM4 | F22: RAISE rejected |
| EXCEPTION declaration in PIM4 mode | PIM4 | F01: EXCEPTION rejected |
| REF type in PIM4 mode | PIM4 | F01: REF rejected |
| OBJECT type in PIM4 mode | PIM4 | F01: OBJECT rejected |
| SAFE MODULE in PIM4 mode | PIM4 | F01: SAFE rejected |
| Import AS alias in PIM4 mode | PIM4 | F11: AS rejected |
| RAISES clause in PIM4 mode | PIM4 | F01: RAISES rejected |
| EXCEPT in Block in PIM4 mode | PIM4 | F03: EXCEPT rejected |
| FINALLY in Block in PIM4 mode | PIM4 | F03: FINALLY rejected |
| FOR variable assigned in loop body | Both | F18: Sema error |
| Bare RETURN in function procedure | Both | F19: Sema error |
| RETURN expr in proper procedure | Both | F19: Sema error |

---

## 9. Open Questions / Policy Decisions

### Q1: Operator Precedence (F04)

**Question:** Should mx match PIM4 operator precedence exactly (OR at additive level, AND at multiplicative), or keep C-style (OR/AND at their own lower levels)?

**Arguments for C-style (status quo):**
- Matches programmer expectations from C/Java/Python
- Every practical M2 compiler does this (gm2, XDS, ADW, p1)
- Changing would alter semantics of existing code
- Mixed boolean+arithmetic expressions are rare

**Arguments for PIM4 strict:**
- Grammar doc already documents PIM4 precedence
- Wirth specified it deliberately

**Recommendation:** Keep C-style. Document as intentional divergence.

### Q2: Formal Type Recursion (F10)

**Question:** Should `ARRAY OF ARRAY OF T` be allowed in formal parameters?

PIM4 says `FormalType = ["ARRAY" "OF"] QualIdent`, allowing only one level. However, multi-dimensional open arrays are useful and accepted by other compilers.

**Recommendation:** Allow but document as extension.

### Q3: Base-Typed Subrange (F27)

**Question:** Should `INTEGER[0..255]` (base-typed subrange) be accepted?

PIM4 only defines `[low..high]`. The base-typed form is common in other dialects.

**Recommendation:** Accept in all modes. Document as mx extension.

### Q4: Type Strictness (F07)

**Question:** Should type name equivalence be enforced?

Enforcing it would break a lot of existing code (e.g., any record-to-record assignment where the types aren't the same named type).

**Recommendation:** Defer to a `--strict` flag. Keep permissive behavior as default. Eventually: warn in default mode, error in strict mode.

### Q5: Bare Set Constructors (F26)

**Question:** Should `{1, 2, 3}` without a type prefix be accepted?

PIM4 requires a type name: `BITSET{1, 2, 3}`. Bare constructors are accepted by some compilers.

**Recommendation:** Continue accepting. Document as mx extension.

---

## 10. Recommendation

### Priority Order

1. **Phase 2 (Extension gating)** — Highest value. This is the most visible conformance issue and affects what code is accepted. Ship this first.
2. **Phase 5 (Documentation rewrite)** — Can be done in parallel with Phase 2. Aligns the grammar doc with reality.
3. **Phase 1 (Tests)** — Write tests concurrently with Phase 2 to validate the gating.
4. **Phase 4 (Operator precedence decision)** — Quick decision + doc update if keeping C-style.
5. **Phase 3 (Semantic enforcement)** — Items 1-4 (FOR variable, RETURN, set typing, const validation) are low-risk and high-value. Defer F07/F08 to a future `--strict` mode.

### What to Fix First

1. Lexer keyword gating (F02) — prevents M2+ keywords from shadowing PIM4 identifiers
2. Parser M2+ gating (F01, F03, F22) — reject M2+ syntax in PIM4 mode
3. Grammar doc restructure (F05, F06, F09, F12-F17, F24, F26)
4. Sema: FOR variable check, RETURN validation, set constructor typing (F18, F19, F25)

### What to Defer

- Type name equivalence (F07) — behind `--strict` flag
- Opaque type revelation check (F08) — needs cross-module infrastructure
- Procedure signature matching .def↔.mod — same
- Operator precedence change (F04) — keep C-style, document divergence

---

**STOP HERE — awaiting approval before implementation.**
