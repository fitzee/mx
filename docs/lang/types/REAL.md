# REAL

Single-precision floating-point type.

## Properties

- **Size**: 32 bits (IEEE 754 single precision)
- **Range**: Approximately 1.18E-38..3.40E+38
- **Precision**: ~7 significant decimal digits
- **Operations**: `+`, `-`, `*`, `/`, unary `-`
- **Relational**: `=`, `#`, `<`, `>`, `<=`, `>=`
- **Standard functions**: `ABS`, `FLOAT`, `TRUNC`, `MAX`, `MIN`

## Syntax

```modula2
VAR
  x, y: REAL;
  n: INTEGER;

x := 3.14;
y := 1.0E5;       (* 100000.0 *)
n := TRUNC(x);     (* n = 3 *)
x := FLOAT(n);     (* x = 3.0 *)
```

## Notes

- Real literals must contain a decimal point. The exponent marker is `E` (e.g., `1.5E-3`).
- `/` is real division; integer types use `DIV` instead.
- `TRUNC` converts REAL to INTEGER by truncating toward zero.
- `FLOAT` converts INTEGER to REAL.
- REAL and LONGREAL are not directly assignment compatible; use explicit conversion.
