# PIM4 Library Refactoring Guide

Standards for refactoring `libs/m2xxx` libraries to be PIM4 compliant, architecture-agnostic, and cache-friendly on ARM64 (Apple Silicon M4, AWS Graviton).

Reference implementation: `libs/m2fsm` — refactored to these standards first.

---

## Ground Rules

1. **DO NOT change any interfaces.** The `.def` file is the contract. All exported types, constants, procedure signatures, and field names must remain identical. Callers must not need to change anything.
2. **Tests must pass unchanged.** Compile and run existing test suites before and after. Run under ASan+UBSan to catch memory errors introduced by pointer arithmetic changes.
3. **Do not change FsmTrace-style helper modules** unless they also have internal violations. If they only re-export or format, leave them alone.
4. **Fix examples that break.** Examples are not interfaces — if they use incorrect PIM4 patterns (see below), fix them as part of the refactor.

---

## 1. Eliminate Hardcoded Array Overlays

**Problem:** Casting an `ADDRESS` to `POINTER TO ARRAY [0..16383] OF T` is a non-portable hack. It imposes an arbitrary upper bound, wastes symbol table space, and is not idiomatic PIM4.

**Before (wrong):**
```modula-2
TYPE
  TransArr  = ARRAY [0..16383] OF Transition;
  TransPtr  = POINTER TO TransArr;

(* usage *)
tp := f.trans;
t := tp^[idx];
```

**After (correct):**
```modula-2
TYPE
  TransPtr = POINTER TO Transition;

(* usage — pointer arithmetic via type transfers *)
tp := TransPtr(LONGCARD(f.trans)
      + LONGCARD(idx * TSIZE(Transition)));
t := tp^;
```

Apply this to every overlay pattern: action arrays, guard arrays, hook arrays, or any `POINTER TO ARRAY [0..N] OF SomeType` used to index into caller-provided memory.

---

## 2. Use LONGCARD for Pointer Arithmetic

**Problem:** mx maps `CARDINAL` to `uint32_t`. On 64-bit targets (ARM64), `CARDINAL(somePointer)` truncates the address. This is silent corruption.

**Rule:** Always use `LONGCARD` (→ `uint64_t`) when converting between pointers and integers.

**Pattern:**
```modula-2
FROM SYSTEM IMPORT ADDRESS, TSIZE;

(* base is ADDRESS, idx is CARDINAL *)
ptr := TargetPtr(LONGCARD(base) + LONGCARD(idx * TSIZE(ElementType)));
value := ptr^;
```

This generates correct C:
```c
ptr = ((ElementType*)((uint64_t)(base) + (uint64_t)(idx * sizeof(ElementType))));
```

**Never do this:**
```modula-2
(* WRONG — truncates 64-bit address to 32 bits *)
ptr := TargetPtr(CARDINAL(base) + CARDINAL(idx * TSIZE(ElementType)));
```

`LONGCARD` is a PIM4 built-in type — no import needed.

---

## 3. Do Not Use ISO ADDADR

ISO Modula-2 provides `ADDADR(addr, offset)` in SYSTEM. PIM4 does not have it. Use the type transfer pattern from section 2 instead.

---

## 4. Natural Alignment

When a record is used as an element in a flat array accessed via pointer arithmetic with `TSIZE`, verify that the record's total size is a multiple of its largest member's alignment.

**Check (for the mx type mapping):**

| Modula-2 type | C type | Size | Alignment |
|---|---|---|---|
| `CARDINAL` | `uint32_t` | 4 | 4 |
| `INTEGER` | `int32_t` | 4 | 4 |
| `LONGCARD` | `uint64_t` | 8 | 8 |
| `LONGINT` | `int64_t` | 8 | 8 |
| `REAL` | `float` | 4 | 4 |
| `LONGREAL` | `double` | 8 | 8 |
| `CHAR` | `char` | 1 | 1 |
| `BOOLEAN` | `int` | 4 | 4 |
| `ADDRESS` | `void*` | 8 | 8 |
| Procedure types | function pointer | 8 | 8 |

**Example:** `Transition = RECORD next, action, guard: CARDINAL END` → 3 × 4 = 12 bytes. Largest member alignment is 4. 12 is a multiple of 4. No padding needed. Every element in a contiguous array is naturally aligned when incremented by `TSIZE(Transition)`.

If the size is NOT a multiple of the largest alignment, add an explicit padding field:
```modula-2
TYPE
  Entry = RECORD
    id:   CARDINAL;   (* 4 bytes *)
    ptr:  ADDRESS;    (* 8 bytes *)
    _pad: CARDINAL;   (* 4 bytes — makes total 16, multiple of 8 *)
  END;
```

---

## 5. Cache-Line Density

A cache line is 64 bytes on both Apple M4 and AWS Graviton. When designing records that live in flat lookup tables:

- Prefer smaller types where the value range permits. `CARDINAL` (4 bytes) over `LONGCARD` (8 bytes) for indices, counts, and IDs that will never exceed 2^32.
- Document the cache density: `(* 5 transitions per 64-byte cache line *)`.
- For hot-path records (transition tables, lookup entries), aim for 4, 8, 12, or 16 byte strides.

---

## 6. Fast-Path Optimisation in Core Procedures

Structure hot procedures (like `Step`) to minimise branches on the common path:

1. **Bounds check first** — single branch, cold path. Return immediately on error.
2. **Table lookup** — straight-line pointer arithmetic, no branch.
3. **Sentinel check** — compare against `NoState`/`NoEntry`/etc. One branch, fast reject.
4. **Guard/precondition** — gate behind a single sentinel check (`IF guardId # NoGuard`). The inner validation branches only fire when a guard exists.
5. **State mutation** — no branch.
6. **Action/callback** — gate behind sentinel check, then nil check.

The principle: the most common early exits (no-transition, guard-rejected) happen before any state mutation or callback dispatch.

---

## 7. Fix Qualified Import Shadowing in Examples

**Problem:** A module that exports a type with the same name as the module itself creates a shadowing issue.

```modula-2
(* Module "Fsm" exports type "Fsm" *)
FROM Fsm IMPORT Fsm, Transition, ...;
Fsm.ClearTable(...);  (* ERROR: Fsm is now the TYPE, not the module *)
```

In PIM4, `FROM M IMPORT x` brings `x` into scope but does NOT bring `M` into scope for qualified access. If the imported name shadows the module name, qualified calls like `M.Proc()` fail because the compiler sees `M` as the type.

**Fix:** FROM-import all needed procedures:
```modula-2
FROM Fsm IMPORT Fsm, Transition, ...,
                Init, SetActions, Step, ClearTable, SetTrans;

ClearTable(...);   (* unqualified — correct *)
```

This is common in mx libraries where the module and primary type share a name (e.g., `Fsm.Fsm`, `ByteBuf.Buf`). Check all examples and tests during refactoring.

---

## 8. Refactoring Checklist

For each library in `libs/m2xxx`:

- [ ] **Read the `.def` file.** Note every exported type, constant, and procedure. These are frozen.
- [ ] **Read the `.mod` file.** Identify all `ARRAY [0..N]` overlay types and `POINTER TO ARRAY` patterns.
- [ ] **Read the test file.** Understand what the tests verify and how they call the API.
- [ ] **Compile and run tests BEFORE changes** to establish a baseline.
- [ ] **Replace overlays** with `POINTER TO <element>` + `LONGCARD` arithmetic (sections 1-2).
- [ ] **Verify alignment** of any record used in pointer-arithmetic traversal (section 4).
- [ ] **Document cache density** for hot-path data structures (section 5).
- [ ] **Restructure hot procedures** for minimal branching (section 6).
- [ ] **Fix example import shadowing** if examples use qualified access on a shadowed module name (section 7).
- [ ] **Compile and run tests AFTER changes** — must produce identical results.
- [ ] **Run under ASan+UBSan** to catch pointer arithmetic errors:
  ```bash
  mx --emit-c tests/xxx_tests.mod -I src -o /tmp/xxx_tests.c
  cc -fsanitize=address,undefined -g -O0 -o /tmp/xxx_tests /tmp/xxx_tests.c
  /tmp/xxx_tests
  ```
- [ ] **Verify examples compile and produce expected output.**

---

## 9. Libraries to Refactor

Inspect each for overlay patterns and non-PIM4 pointer access:

| Library | Path | Notes |
|---|---|---|
| m2alloc | `libs/m2alloc` | Arena/Pool — likely heavy overlay use |
| m2bytes | `libs/m2bytes` | ByteBuf/Codec — byte-level pointer math |
| m2stream | `libs/m2stream` | Transport streams |
| m2fsm | `libs/m2fsm` | **Done** — reference implementation |
| m2http2 | `libs/m2http2` | HTTP/2 framing + HPACK |
| m2log | `libs/m2log` | Structured logging |
| m2cli | `libs/m2cli` | CLI parser |
| m2sys | `libs/m2sys` | C shim layer |
| m2auth | `libs/m2auth` | JWT/HMAC |
| m2futures | `libs/m2futures` | Async promises |
| m2sqlite | `libs/m2sqlite` | SQLite bindings |
| m2oidc | `libs/m2oidc` | OIDC/JWKS |

---

## Quick Reference: Type Transfer Pointer Arithmetic

```modula-2
(* Read element at index idx from a caller-provided ADDRESS *)
TYPE ElemPtr = POINTER TO Elem;
VAR p: ElemPtr;
p := ElemPtr(LONGCARD(base) + LONGCARD(idx * TSIZE(Elem)));
value := p^;

(* Write to element at index idx *)
p := ElemPtr(LONGCARD(base) + LONGCARD(idx * TSIZE(Elem)));
p^.field := newValue;

(* Iterate over n elements *)
i := 0;
WHILE i < n DO
  p := ElemPtr(LONGCARD(base) + LONGCARD(i * TSIZE(Elem)));
  (* use p^ *)
  INC(i)
END;
```
