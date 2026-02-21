# SYSTEM

Low-level system interface module. Provides types and procedures
that bypass normal Modula-2 type safety. Use with caution.

## Exported Types

```modula2
TYPE WORD;    (* machine word, compatible with any single-word type *)
TYPE BYTE;    (* single byte *)
TYPE ADDRESS; (* untyped pointer, compatible with any pointer type *)
```

## Exported Procedures

```modula2
PROCEDURE ADR(x): ADDRESS;
PROCEDURE TSIZE(T): CARDINAL;
```

## Pervasive Bitwise Operations

The following bitwise operations are available as pervasive builtins
(no import required). They correspond to ISO `SYSTEM.SHIFT` /
`SYSTEM.ROTATE` and common compiler extensions.

```modula2
PROCEDURE SHL(x, n: CARDINAL): CARDINAL;    (* shift left  *)
PROCEDURE SHR(x, n: CARDINAL): CARDINAL;    (* shift right *)
PROCEDURE BAND(a, b: CARDINAL): CARDINAL;   (* bitwise AND *)
PROCEDURE BOR(a, b: CARDINAL): CARDINAL;    (* bitwise OR  *)
PROCEDURE BXOR(a, b: CARDINAL): CARDINAL;   (* bitwise XOR *)
PROCEDURE BNOT(x: CARDINAL): CARDINAL;      (* bitwise NOT *)
PROCEDURE SHIFT(val: CARDINAL; n: INTEGER): CARDINAL;  (* bidirectional shift  *)
PROCEDURE ROTATE(val: CARDINAL; n: INTEGER): CARDINAL; (* bidirectional rotate *)
```

`SHIFT` and `ROTATE` accept a signed shift count: positive values
shift/rotate left, negative values shift/rotate right. `SHIFT` fills
vacated bits with zero; `ROTATE` wraps bits around.

See the individual builtin reference pages for details.

## Notes

- `ADR(x)` returns the memory address of variable `x`.
- `TSIZE(T)` returns the size in bytes of type `T`.
- `ADDRESS` is assignment-compatible with all pointer types.
- Importing from `SYSTEM` bypasses normal type safety -- the
  compiler permits casts and operations that would otherwise be
  rejected.

## Example

```modula2
MODULE SysDemo;
FROM SYSTEM IMPORT ADR, TSIZE, ADDRESS;
FROM InOut IMPORT WriteCard, WriteLn;
VAR
  n: INTEGER;
  a: ADDRESS;
BEGIN
  a := ADR(n);
  WriteCard(TSIZE(INTEGER), 0); WriteLn;
END SysDemo.
```
