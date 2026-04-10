# Lint warnings

mx includes a built-in static analysis linter that catches common Modula-2 pitfalls at compile time. Warnings appear in VS Code as yellow squiggles (via the LSP) and on stderr during `mx build`. Warnings never block compilation.

## Warning codes

| Code | Category | Description |
|------|----------|-------------|
| W01 | Unsigned arithmetic | Comparison of unsigned type against 0 (`CARDINAL >= 0` is always true, `CARDINAL < 0` is always false) |
| W02 | Unsigned arithmetic | `WHILE c >= 0 DO DEC(c) END` with unsigned variable — infinite loop since unsigned can never be negative |
| W03 | Short-circuit | Pointer dereference or array index on RHS of `AND`/`OR` where short-circuit evaluation is required for safety (e.g. `(p # NIL) AND (p^.x = 1)`) |
| W04 | Aliasing | Same variable passed to multiple VAR parameters in one call — modifications through one alias may interfere with the other |
| W05 | Subrange | `INC` or `DEC` on a bounded subrange type (`[0..15]`) may overflow/underflow the declared bounds |
| W06 | Portability | `FROM SYSTEM IMPORT` — use of SYSTEM module reduces portability across compilers and platforms |
| W07 | Exhaustiveness | `CASE` on enumeration or subrange without `ELSE` does not cover all possible values |
| W08 | Signedness | Mixed signed (`INTEGER`) and unsigned (`CARDINAL`) operands in arithmetic — implicit conversion may produce unexpected results |
| W09 | Unsigned arithmetic | Subtraction in `FOR` upper bound with unsigned loop variable (`FOR i := 0 TO n - 1`) — underflows to `MAX(CARDINAL)` when `n = 0` |
| W10 | Initialization | Variable may be used before being assigned a value (path-sensitive: considers all branches) |
| W11 | Nil safety | Pointer may be `NIL` when dereferenced — uninitialized, assigned `NIL`, or set to `NIL` after `DISPOSE` |

## Analysis tiers

Warnings are checked at two levels:

**Tier 1 (AST-level)** — W01–W09. Pattern matching on the typed syntax tree. Runs instantly during editing in VS Code.

**Tier 2 (CFG dataflow)** — W10–W11. Forward dataflow analysis over the control flow graph. Builds HIR and CFG, then runs a worklist solver to fixpoint. Also runs in VS Code via the LSP.

## Suppressing warnings

### Line-level suppression

Add `(*!Wxx*)` as a comment on the line to suppress that warning:

```modula2
FROM SYSTEM IMPORT ADDRESS, ADR; (*!W06*)
```

### File-level suppression

Place `(*!Wxx*)` before the `MODULE` keyword to suppress file-wide:

```modula2
(*!W06*)
(*!W08*)
MODULE LowLevel;
FROM SYSTEM IMPORT ADDRESS, ADR, TSIZE;
(* All W06 and W08 warnings suppressed in this file *)
```

### Suppression scope

- Each `(*!Wxx*)` pragma suppresses exactly one warning code
- Multiple codes can be suppressed with multiple pragmas
- Suppression does not affect other warning codes
- File-level suppression applies to the entire file including all procedures

## Examples

### W09: CARDINAL underflow in FOR loop

```modula2
(* WARNING: if count = 0, "count - 1" underflows to MAX(CARDINAL) *)
FOR i := 0 TO count - 1 DO
  Process(items[i])
END;

(* FIX: guard with IF *)
IF count > 0 THEN
  FOR i := 0 TO count - 1 DO
    Process(items[i])
  END
END;
```

### W03: Short-circuit safety

```modula2
(* WARNING: if p = NIL, p^.value crashes — AND does not guarantee
   short-circuit evaluation in standard Modula-2 *)
IF (p # NIL) AND (p^.value > 0) THEN ...

(* FIX: use & (AND THEN) which guarantees short-circuit *)
IF (p # NIL) & (p^.value > 0) THEN ...
```

### W10: Uninitialized variable

```modula2
(* WARNING: x used on ELSE path without assignment *)
PROCEDURE Foo(flag: BOOLEAN): INTEGER;
VAR x: INTEGER;
BEGIN
  IF flag THEN x := 10 END;
  RETURN x    (* W10: x may be uninitialized when flag is FALSE *)
END Foo;
```

### W11: Pointer NIL dereference

```modula2
(* WARNING: p is NIL after DISPOSE *)
NEW(p);
p^.val := 42;
DISPOSE(p);
RETURN p^.val   (* W11: p may be NIL *)
```

## Architecture

The lint system is implemented across three layers:

- **`src/sema.rs`** — Tier 1 checks: AST pattern matching during semantic analysis
- **`src/cfg/dataflow.rs`** — Generic forward dataflow framework (worklist solver over CFG)
- **`src/cfg/lint.rs`** — Tier 2 checks: `DefinitelyAssigned` (W10) and `NilSafety` (W11) analyses
- **`src/analyze.rs`** — Integration: runs both tiers, applies suppression, feeds LSP diagnostics
- **`src/errors.rs`** — `ErrorKind::Warning` with optional code, never blocks compilation
