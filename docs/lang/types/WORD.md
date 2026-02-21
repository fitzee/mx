# WORD

Untyped machine word for low-level programming. Defined in the SYSTEM module.

## Properties

- **Size**: One machine word (same as ADDRESS, typically 32 or 64 bits)
- **Module**: Must import from `SYSTEM`
- **Compatibility**: Compatible with any type of the same size when passed as a parameter
- **Operations**: None defined by the language

## Syntax

```modula2
FROM SYSTEM IMPORT WORD;

PROCEDURE RawWrite(w: WORD);
(* Accepts any word-sized value *)

VAR
  n: INTEGER;
  c: CARDINAL;

n := 42;
RawWrite(n);   (* INTEGER passed as WORD *)
RawWrite(c);   (* CARDINAL passed as WORD *)
```

## Notes

- WORD exists to allow generic low-level routines that accept any word-sized argument.
- Type compatibility with WORD applies only at parameter passing boundaries, not in assignments.
- WORD parameters defeat type checking; use sparingly and only for system-level code.
- Not all PIM4 implementations provide WORD; its availability is implementation-defined.
- Prefer typed interfaces where possible; WORD is a last resort for hardware interaction or foreign function interfaces.
