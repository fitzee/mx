# LONGREAL

Double-precision floating-point type.

## Properties

- **Size**: 64 bits (IEEE 754 double precision)
- **Range**: Approximately 2.23D-308..1.80D+308
- **Precision**: ~15 significant decimal digits
- **Operations**: `+`, `-`, `*`, `/`, unary `-`
- **Relational**: `=`, `#`, `<`, `>`, `<=`, `>=`
- **Standard functions**: `ABS`, `FLOAT`, `TRUNC`, `LONG`, `SHORT`, `MAX`, `MIN`

## Syntax

```modula2
VAR
  d: LONGREAL;
  r: REAL;

d := 1.0D5;         (* 100000.0 as LONGREAL *)
d := 2.718281828D0;
r := SHORT(d);       (* LONGREAL -> REAL, may lose precision *)
d := LONG(r);        (* REAL -> LONGREAL *)
```

## Notes

- LONGREAL literals use the `D` exponent marker instead of `E` (e.g., `1.0D-10`).
- `LONG` converts REAL to LONGREAL; `SHORT` converts LONGREAL to REAL.
- Mixing REAL and LONGREAL in expressions requires explicit conversion.
- `TRUNC` on LONGREAL yields INTEGER, truncating toward zero.
- Preferred over REAL when precision matters (scientific computation, accumulators).
