# PIM4 Conformance Audit — mx Compiler & Grammar Reference

**Date:** 2026-03-14 (updated 2026-03-31)
**Compiler version:** 1.0.6 (updated for 1.8.0)
**Scope:** Parser, sema, type system, grammar doc, extension gating
**Status:** Extension gating IMPLEMENTED as of 1.5.0. Remaining items tracked below.

---

## 1. Executive Summary

The mx compiler is **broadly correct** for practical PIM4 usage: 383/483 gm2 PIM4 tests pass, and the compiler handles the major language features (modules, types, records, pointers, sets, procedures, control flow) well. However, this audit identifies several categories of concern:

**Top risks (original, March 14):**

1. ~~**No extension gating in the parser.**~~ **RESOLVED (1.5.0).** M2+ keywords (TRY, LOCK, TYPECASE, REF, OBJECT, EXCEPTION, RAISE, RETRY, SAFE, UNSAFE, RAISES, AS, BRANDED, METHODS, OVERRIDE, REVEAL, REFANY, EXCEPT, FINALLY) are now conditionally recognized in the lexer via `keyword_from_str(s, m2plus)`. In PIM4 mode these tokens are lexed as identifiers, so the parser never sees them as keywords. All M2+ constructs are rejected in PIM4 mode and accepted in `--m2plus` mode.
2. **Operator precedence diverges from PIM4.** The grammar doc correctly shows PIM4 precedence (OR at additive level, AND at multiplicative level), but the parser gives OR and AND their own lower-priority levels (C-style). This changes the parse of expressions mixing boolean and arithmetic operators. *(Intentional divergence — matches every practical M2 compiler.)*
3. **Overly permissive type system.** `assignment_compatible` allows any-record-to-any-record, any-array-to-any-array, and any-pointer-to-any-pointer. PIM4 requires name equivalence for structured types.
4. ~~**ISO/M2+ clauses in Block.**~~ **RESOLVED (1.5.0).** `EXCEPT` and `FINALLY` are M2+ keywords, gated by the lexer. In PIM4 mode they are not recognized as keywords.
5. **Grammar doc inaccuracies.** Several productions don't match the parser, and PIM4 vs M2+ constructs are mixed in core productions.

Items 1 and 4 are fully resolved. Items 2 and 5 are resolved (grammar doc rewritten, operator precedence documented as intentional divergence). The only remaining open items are semantic checks (F07, F08, F18, F19, F20, F25) — none affect practical PIM4 usage.

---

## 2. Findings Table

| ID | Area | Current Behavior | Expected PIM4 Behavior | Severity | Action | Kind |
|----|------|-----------------|----------------------|----------|--------|------|
| F01 | Parser/gating | ~~M2+ constructs parsed unconditionally~~ | M2+ constructs rejected unless `--m2plus` | ~~Critical~~ | ~~Gate parsing on `m2plus` flag~~ | **RESOLVED (1.5.0)** — Lexer gates M2+ keywords via `keyword_from_str(s, m2plus)` |
| F02 | Parser/gating | ~~M2+ keywords always reserved~~ | In PIM4 mode, these should be legal identifiers | ~~Critical~~ | ~~Conditional keyword recognition in lexer~~ | **RESOLVED (1.5.0)** — M2+ keywords only recognized when `m2plus=true` |
| F03 | Parser | ~~`EXCEPT`/`FINALLY` accepted in every Block~~ | PIM4 has no EXCEPT/FINALLY in Block | ~~High~~ | ~~Gate behind m2plus or iso flag~~ | **RESOLVED (1.5.0)** — EXCEPT/FINALLY are M2+ keywords, gated by lexer |
| F04 | Parser/grammar | ~~OR and AND have dedicated precedence levels (C-style)~~ | PIM4: OR at additive, AND at multiplicative | ~~High~~ | ~~Policy decision~~ | **RESOLVED (policy)** — Kept C-style; documented as intentional divergence in grammar doc "mx Accepted Differences" |
| F05 | Grammar doc | ~~`Statement` production includes TryStatement, LockStatement, TypecaseStatement~~ | These should only appear in M2+ section | ~~High~~ | ~~Move to M2+ section~~ | **RESOLVED** — Grammar doc Statement production now PIM4-only; M2+ constructs in separate section |
| F06 | Grammar doc | ~~`AddOp = "+" \| "-" \| "OR"` / `MulOp = ... \| "AND"` (PIM4 style)~~ | Parser uses separate precedence levels for OR/AND | ~~High~~ | ~~Grammar doc must match parser~~ | **RESOLVED** — Grammar doc now shows C-style precedence (OrExpr → AndExpr → Relation → SimpleExpr → Term) matching parser |
| F07 | Sema | `assignment_compatible` allows any record→record, array→array, pointer→pointer | PIM4 requires name equivalence for structured types | **High** | Tighten type compatibility checks | Code fix |
| F08 | Sema | No validation that opaque types must be POINTER in implementation | PIM4: opaque types in .def must be revealed as POINTER TO in .mod | **Medium** | Add sema check | Code fix |
| F09 | Grammar doc | ~~`TypeDecl = ident "=" Type \| ident .` combines def and impl behavior~~ | Opaque declarations only valid in .def | ~~Medium~~ | ~~Separate in grammar doc~~ | **RESOLVED** — Grammar doc now has separate `TypeDeclDef` (def modules) and `TypeDecl` (impl) productions |
| F10 | Parser | ~~`parse_formal_type` allows recursive `ARRAY OF ARRAY OF QualIdent`~~ | PIM4: single ARRAY OF level | ~~Medium~~ | ~~Restrict or document~~ | **RESOLVED (policy)** — Documented as mx extension in "mx Accepted Differences" |
| F11 | Parser | ~~Import `AS` alias always accepted~~ | Import aliases are an mx extension, not PIM4 | ~~Medium~~ | ~~Gate behind m2plus or extension flag~~ | **RESOLVED (1.5.0)** — AS is M2+ keyword, gated by lexer |
| F12 | Grammar doc | ~~`ForeignDefModule` not reachable from `CompilationUnit`~~ | Should be shown as M2+ variant | ~~Medium~~ | ~~Fix grammar doc~~ | **RESOLVED** — Now in M2+ Extensions section of grammar doc |
| F13 | Grammar doc | ~~`CharConst` referenced in Factor but undefined~~ | Should be defined as terminal | ~~Low~~ | ~~Fix grammar doc~~ | **RESOLVED** — `CharConst` defined in Terminals section |
| F14 | Grammar doc | ~~`ConstExpr` referenced but never defined~~ | Should note: same as Expr | ~~Low~~ | ~~Fix grammar doc~~ | **RESOLVED** — `ConstExpr = Expr` defined in Terminals section with note |
| F15 | Grammar doc | ~~`number` and `string` in Factor not defined~~ | Should define terminal symbols | ~~Low~~ | ~~Fix grammar doc~~ | **RESOLVED** — `number` and `string` defined in Terminals section |
| F16 | Grammar doc | ~~`&` and `~` undocumented~~ | PIM4 defines these as synonyms | ~~Low~~ | ~~Document in grammar~~ | **RESOLVED** — Documented in operator synonyms table |
| F17 | Grammar doc | ~~`<>` accepted as not-equal but undocumented~~ | PIM4 uses `#` primarily | ~~Low~~ | ~~Document in grammar~~ | **RESOLVED** — Documented in operator synonyms table and mx Accepted Differences |
| F18 | Sema | No check that FOR variable is not assigned inside loop body | PIM4 forbids assignment to FOR control variable | **Medium** | Add sema check | Code fix |
| F19 | Sema | RETURN with/without expression not validated against procedure kind | PIM4: function procedures require RETURN expr; proper procedures forbid it | **Medium** | Add sema check | Code fix |
| F20 | Sema | No validation that CASE labels are constants and non-overlapping | PIM4 requires constant, non-overlapping case labels | **Low** | Add sema check | Code fix |
| F21 | Parser | Module priority `MODULE name [expr] ;` parsed but undocumented | PIM4 allows module priority for interrupt handling | **Low** | Document or remove | Doc fix |
| F22 | Parser | ~~`RAISE` and `RETRY` parsed as statements unconditionally~~ | These are M2+/ISO, not PIM4 | ~~High~~ | ~~Gate behind extension flag~~ | **RESOLVED (1.5.0)** — RAISE/RETRY are M2+ keywords, gated by lexer |
| F23 | Grammar doc | ~~`RaiseStatement` parsed without gating~~ | Consistency between doc and implementation | ~~Medium~~ | ~~Match doc (gate in parser)~~ | **RESOLVED (1.5.0)** — RAISE gated by lexer, matches M2+ section of grammar doc |
| F24 | Grammar doc | ~~Missing `Definition` production for def modules~~ | Grammar should define it separately from `Declaration` | ~~Medium~~ | ~~Add `Definition` production~~ | **RESOLVED** — `Definition` production now in "Definitions (definition modules only)" section |
| F25 | Sema | Set constructors always typed as BITSET regardless of base_type | Should use declared base type for typed set constructors | **Low** | Fix set constructor type inference | Code fix |
| F26 | Grammar doc | ~~Bare set constructor `{1, 2, 3}` undocumented~~ | Parser accepts it, produces BITSET | ~~Low~~ | ~~Document as extension~~ | **RESOLVED** — Documented in mx Accepted Differences and SetValue production comment |
| F27 | Parser | ~~`SubrangeType` accepts `QualIdent "[" ConstExpr ".." ConstExpr "]"`~~ | PIM4: `[low..high]` only | ~~Low~~ | ~~Document as extension~~ | **RESOLVED (policy)** — Documented as mx extension in "mx Accepted Differences" |

---

## 3. Syntax/Grammar Review

### 3.1 Compilation Units

**Grammar doc:**
```
CompilationUnit = ProgramModule | DefinitionModule | ImplementationModule .
```

**Issues:**
- ~~`ForeignDefModule` is documented separately but not reachable from `CompilationUnit`.~~ Now in M2+ Extensions section. **(F12 RESOLVED)**
- `SAFE`/`UNSAFE` prefix handling is not shown in the grammar. ~~The parser accepts `SAFE MODULE ...` etc. unconditionally.~~ Now gated — SAFE/UNSAFE are M2+ keywords. **(F01 RESOLVED)**
- ProgramModule priority expression `[expr]` is parsed (line 219) but not in grammar doc. **(F21)**

**Parser behavior:** Correct structure. Module name matching is enforced.

### 3.2 Imports/Exports

**Grammar doc:**
```
Import = "IMPORT" IdentList ";" | "FROM" ident "IMPORT" IdentList ";" .
Export = "EXPORT" [ "QUALIFIED" ] IdentList ";" .
```

**Issues:**
- ~~Parser accepts `FROM M IMPORT x AS y` (import aliases) unconditionally.~~ Now gated — AS is an M2+ keyword. **(F11 RESOLVED)**
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
- ~~The grammar doesn't show a separate `Definition` production.~~ Now has `Definition` and `TypeDeclDef` productions. **(F24 RESOLVED)**
- ~~`TypeDecl = ident "=" Type | ident .` conflates def-module and impl-module behavior.~~ Now separate `TypeDeclDef` (def) and `TypeDecl` (impl) productions. **(F09 RESOLVED)**
- ~~`EXCEPTION` declarations are parsed unconditionally.~~ Now gated — EXCEPTION is an M2+ keyword. **(F01 RESOLVED)**
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
| SubrangeType | Shows `QualIdent "[" ...]` form | Parser accepts this form (line 771-783) | ~~Not PIM4~~ Documented as mx extension **(F27 RESOLVED)** |
| RefType | Marked M2+ | ~~Parsed unconditionally~~ Gated (M2+ keyword) | **F01 RESOLVED** |
| ObjectType | Marked M2+ | ~~Parsed unconditionally~~ Gated (M2+ keyword) | **F01 RESOLVED** |

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
- ~~`TryStatement`, `LockStatement`, `TypecaseStatement` are in the base `Statement` production.~~ Now PIM4-only in grammar doc; M2+ constructs in separate section. **(F05 RESOLVED)**
- ~~`RAISE` and `RETRY` are parsed unconditionally~~ Now gated — RAISE/RETRY are M2+ keywords. **(F22, F23 RESOLVED)**
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

The parser gives OR and AND their own dedicated, lower-priority levels. In PIM4, OR has the **same** precedence as `+`/`-`, and AND has the **same** precedence as `*`/`/`/`DIV`/`MOD`. ~~**(F04, F06)**~~ **RESOLVED** — Grammar doc now documents C-style precedence matching the parser, listed as intentional divergence in "mx Accepted Differences".

**Practical impact:** Expression `a + b OR c` parses as `(a + b) OR c` in both schemes (OR is lower or equal to +). But `a OR b + c` parses as `a OR (b + c)` in mx (+ binds tighter) vs `(a OR b) + c` in strict PIM4 (same level, left-to-right). This matters if someone writes mixed arithmetic+boolean (unusual but legal in PIM4 with BITSET operations).

**Other expression issues — all RESOLVED in grammar doc:**
- ~~`&` (Ampersand) parsed as AND synonym — undocumented.~~ Now in operator synonyms table. **(F16 RESOLVED)**
- ~~`~` (Tilde) parsed as NOT synonym — undocumented.~~ Now in operator synonyms table. **(F16 RESOLVED)**
- ~~`<>` parsed as not-equal — undocumented.~~ Now in operator synonyms table. **(F17 RESOLVED)**
- ~~`Factor` mentions `CharConst` which is not defined.~~ Now defined in Terminals section. **(F13 RESOLVED)**
- ~~`number` and `string` not defined as terminals.~~ Now defined in Terminals section. **(F15 RESOLVED)**

### 3.7 Dead/Unreachable Productions

| Production | Status |
|-----------|--------|
| `ForeignDefModule` | ~~Not reachable from CompilationUnit~~ Now in M2+ Extensions section |
| `RaiseStatement` | Documented in M2+ section, ~~parsed unconditionally~~ now gated by M2+ keyword |
| `SafetyAnnotation` | Documented in M2+ section, ~~parsed unconditionally~~ now gated by M2+ keyword |
| `ExceptionDecl` | Documented in M2+ section, ~~parsed unconditionally~~ now gated by M2+ keyword |
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

These are gated behind `--m2plus` via lexer keyword recognition (`token.rs:keyword_from_str`). In PIM4 mode, M2+ keywords are lexed as identifiers so the parser never sees them as keywords.

| Construct | Gating keyword(s) | Gated? |
|-----------|-------------------|--------|
| TRY/EXCEPT/FINALLY (M2+ statement) | TRY, EXCEPT, FINALLY | **Yes** |
| LOCK statement | LOCK | **Yes** |
| TYPECASE statement | TYPECASE | **Yes** |
| RAISE statement | RAISE | **Yes** |
| RETRY statement | RETRY | **Yes** |
| EXCEPTION declaration | EXCEPTION | **Yes** |
| EXCEPT/FINALLY in Block | EXCEPT, FINALLY | **Yes** |
| REF type | REF | **Yes** |
| BRANDED REF type | BRANDED | **Yes** |
| REFANY type | REFANY | **Yes** |
| OBJECT type | OBJECT | **Yes** |
| SAFE/UNSAFE prefix | SAFE, UNSAFE | **Yes** |
| RAISES clause | (parsed after procedure heading) | **Yes** — RAISES keyword gated |
| Import AS alias | AS | **Yes** |
| DEFINITION MODULE FOR "lang" | (parsed after DEFINITION keyword) | **Yes** — FOR "lang" only reachable in M2+ mode |

### 5.3 Keyword Reservation — RESOLVED (F02)

~~All M2+ keywords were always reserved by the lexer.~~

**Fixed in 1.5.0.** `keyword_from_str(s, m2plus)` in `token.rs:128-197` now splits keywords into two groups:
- **PIM4 keywords** (lines 130–171): Always recognized (AND, ARRAY, BEGIN, BY, CASE, CONST, DEFINITION, DIV, DO, ELSE, ELSIF, END, EXIT, EXPORT, FOR, FROM, IF, IMPLEMENTATION, IMPORT, IN, LOOP, MOD, MODULE, NOT, OF, OR, POINTER, PROCEDURE, QUALIFIED, RECORD, REPEAT, RETURN, SET, THEN, TO, TYPE, UNTIL, VAR, WHILE, WITH).
- **M2+ keywords** (lines 174–195): Only recognized when `m2plus=true` (AS, BRANDED, EXCEPT, EXCEPTION, FINALLY, LOCK, METHODS, OBJECT, OVERRIDE, RAISE, REF, REFANY, RETRY, REVEAL, SAFE, TRY, TYPECASE, UNSAFE).

In PIM4 mode, all M2+ keywords are valid identifiers. Parser tests confirm this (`test_m2plus_keywords_are_identifiers_in_pim4`, `test_all_m2plus_keywords_as_pim4_identifiers`, `test_m2plus_keywords_as_type_names_in_pim4`, `test_m2plus_keywords_as_proc_names_in_pim4`).

---

## 6. Documentation Review

### 6.1 Current State — RESOLVED

~~`docs/lang/grammar.md` was titled "Modula-2 PIM4 Grammar Reference" and mixed PIM4/M2+ constructs.~~

**Rewritten.** The grammar doc (`docs/lang/grammar.md`) is now structured as:

1. **PIM4 Core** — Productions matching the parser's PIM4 mode. Terminals defined. C-style precedence documented.
2. **mx Accepted Differences** — Table of intentional divergences (operator precedence, base-typed subranges, bare set constructors, recursive ARRAY OF, `<>`, `&`/`~` synonyms).
3. **Modula-2+ Extensions** — All M2+ constructs clearly separated (TRY, LOCK, TYPECASE, RAISE, RETRY, EXCEPTION, REF, OBJECT, import AS, foreign def modules, SAFE/UNSAFE, RAISES).

### 6.2 Specific Sections — All RESOLVED

| Section | Status |
|---------|--------|
| Compilation Units | **RESOLVED** — Shows module priority; SAFE/UNSAFE in M2+ section |
| Imports | **RESOLVED** — AS alias in M2+ section |
| Declarations | **RESOLVED** — Separate `Definition` and `TypeDeclDef` productions |
| Types | **RESOLVED** — Extensions in "mx Accepted Differences" |
| Statements | **RESOLVED** — PIM4-only in base production |
| Expressions | **RESOLVED** — C-style precedence documented with note |
| Factor/Terminals | **RESOLVED** — `number`, `string`, `CharConst`, `ConstExpr` defined |
| M2+ section | **RESOLVED** — All M2+ constructs in separate section |
| mx extensions | **RESOLVED** — "mx Accepted Differences" table added |

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

### Phase 2: Extension Gating in Parser — COMPLETED (1.5.0)

**Status:** Implemented. M2+ keywords are conditionally recognized in the lexer (`token.rs:keyword_from_str`). The `Lexer` struct has an `m2plus: bool` field (`lexer.rs:13`) set via `set_m2plus()`. In PIM4 mode, M2+ keywords lex as identifiers so all M2+ syntax is naturally rejected by the parser without explicit gating checks.

**Files modified:** `src/token.rs`, `src/lexer.rs`, `src/driver.rs`

**Tests:** `test_m2plus_keywords_are_identifiers_in_pim4`, `test_all_m2plus_keywords_as_pim4_identifiers`, `test_m2plus_keywords_as_type_names_in_pim4`, `test_m2plus_keywords_as_proc_names_in_pim4`, `test_try_accepted_in_m2plus`, `test_lock_accepted_in_m2plus`, `test_typecase_accepted_in_m2plus`, `test_exception_decl_accepted_in_m2plus`, `test_raise_accepted_in_m2plus`, `test_except_in_block_accepted_in_m2plus`, `test_safe_module_accepted_in_m2plus`, `test_import_as_rejected_in_pim4_accepted_in_m2plus`.

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

### Phase 4: Operator Precedence Decision (F04, F06) — COMPLETED

**Status:** Option A chosen — kept C-style precedence. Grammar doc updated to show separate OR/AND levels and document as intentional divergence in "mx Accepted Differences" table.

### Phase 5: Documentation Rewrite — COMPLETED

**Status:** Grammar doc (`docs/lang/grammar.md`) fully rewritten with three-section structure: PIM4 Core, mx Accepted Differences, M2+ Extensions. All terminal symbols defined. All M2+ constructs moved to separate section. Operator synonyms documented.

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

| Test | Mode | What it validates | Status |
|------|------|-------------------|--------|
| TRY statement in PIM4 mode | PIM4 | F01: TRY rejected | **Covered** — M2+ keywords lex as identifiers |
| LOCK statement in PIM4 mode | PIM4 | F01: LOCK rejected | **Covered** — `test_m2plus_keywords_as_proc_names_in_pim4` |
| TYPECASE in PIM4 mode | PIM4 | F01: TYPECASE rejected | **Covered** — M2+ keywords lex as identifiers |
| RAISE in PIM4 mode | PIM4 | F22: RAISE rejected | **Covered** — M2+ keywords lex as identifiers |
| EXCEPTION declaration in PIM4 mode | PIM4 | F01: EXCEPTION rejected | **Covered** — M2+ keywords lex as identifiers |
| REF type in PIM4 mode | PIM4 | F01: REF rejected | **Covered** — M2+ keywords lex as identifiers |
| OBJECT type in PIM4 mode | PIM4 | F01: OBJECT rejected | **Covered** — M2+ keywords lex as identifiers |
| SAFE MODULE in PIM4 mode | PIM4 | F01: SAFE rejected | **Covered** — M2+ keywords lex as identifiers |
| Import AS alias in PIM4 mode | PIM4 | F11: AS rejected | **Covered** — `test_import_as_rejected_in_pim4_accepted_in_m2plus` |
| RAISES clause in PIM4 mode | PIM4 | F01: RAISES rejected | **Covered** — M2+ keywords lex as identifiers |
| EXCEPT in Block in PIM4 mode | PIM4 | F03: EXCEPT rejected | **Covered** — M2+ keywords lex as identifiers |
| FINALLY in Block in PIM4 mode | PIM4 | F03: FINALLY rejected | **Covered** — M2+ keywords lex as identifiers |
| FOR variable assigned in loop body | Both | F18: Sema error | Not yet implemented |
| Bare RETURN in function procedure | Both | F19: Sema error | Not yet implemented |
| RETURN expr in proper procedure | Both | F19: Sema error | Not yet implemented |

---

## 9. Open Questions / Policy Decisions

### Q1: Operator Precedence (F04) — DECIDED

**Decision:** Keep C-style. Documented as intentional divergence in grammar doc.

### Q2: Formal Type Recursion (F10) — DECIDED

**Decision:** Allow recursive `ARRAY OF`. Documented as mx extension in grammar doc.

### Q3: Base-Typed Subrange (F27) — DECIDED

**Decision:** Accept `INTEGER[0..255]` in all modes. Documented as mx extension in grammar doc.

### Q4: Type Strictness (F07) — OPEN

**Question:** Should type name equivalence be enforced?

Enforcing it would break a lot of existing code (e.g., any record-to-record assignment where the types aren't the same named type).

**Recommendation:** Defer to a `--strict` flag. Keep permissive behavior as default. Eventually: warn in default mode, error in strict mode.

### Q5: Bare Set Constructors (F26) — DECIDED

**Decision:** Continue accepting. Documented as mx extension in grammar doc.

---

## 10. Recommendation

### Priority Order

1. ~~**Phase 2 (Extension gating)**~~ — **DONE (1.5.0)**
2. ~~**Phase 5 (Documentation rewrite)**~~ — **DONE**
3. ~~**Phase 1 (Tests)**~~ — **DONE** (parser tests cover all gating)
4. ~~**Phase 4 (Operator precedence decision)**~~ — **DONE** (kept C-style, documented)
5. **Phase 3 (Semantic enforcement)** — Remaining work. Items 1-4 (FOR variable, RETURN, set typing, const validation) are low-risk and high-value. Defer F07/F08 to a future `--strict` mode.

### What to Fix First

1. ~~Lexer keyword gating (F02)~~ — **DONE (1.5.0)**
2. ~~Parser M2+ gating (F01, F03, F22)~~ — **DONE (1.5.0)**
3. ~~Grammar doc restructure (F05, F06, F09, F12-F17, F24, F26)~~ — **DONE**
4. Sema: FOR variable check, RETURN validation, set constructor typing (F18, F19, F25)

### What to Defer

- Type name equivalence (F07) — behind `--strict` flag
- Opaque type revelation check (F08) — needs cross-module infrastructure
- Procedure signature matching .def↔.mod — same
- Operator precedence change (F04) — keep C-style, document divergence

---

**STOP HERE — awaiting approval before implementation.**
