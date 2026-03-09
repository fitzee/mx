# ADDRESS

Untyped pointer type for low-level programming. Defined in the SYSTEM module.

## Properties

- **Size**: One machine word (pointer-sized)
- **Module**: Must import from `SYSTEM`
- **Compatibility**: Assignment compatible with any pointer type
- **Operations**: `+`, `-`, `DIV`, `MOD` (address arithmetic, implementation-defined)
- **Standard functions**: `ADR`, `SIZE`, `TSIZE`
- **Null value**: `NIL`

## Syntax

```modula2
FROM SYSTEM IMPORT ADDRESS, ADR;

VAR
  p: ADDRESS;
  n: INTEGER;
  arr: ARRAY [0..9] OF CHAR;

p := ADR(n);       (* address of n *)
p := ADR(arr);     (* address of arr *)
p := NIL;          (* null pointer *)
```

## Notes

- `ADR(x)` returns the memory address of variable x as an ADDRESS.
- ADDRESS is compatible with any typed pointer (`POINTER TO T`), allowing casts in both directions.
- Address arithmetic (adding/subtracting integers) is implementation-defined and not portable.
- `SIZE(x)` returns the storage size of variable x in bytes; `TSIZE(T)` returns the size of type T.
- Use ADDRESS only when interfacing with hardware, foreign code, or implementing allocators. Prefer typed pointers for normal use.
