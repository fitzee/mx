# LFLOAT

```modula2
LFLOAT(n): LONGREAL
```

Convert an integer value `n` to type `LONGREAL` (double-precision floating point).

`n` must be of type `INTEGER` or `CARDINAL`.

## Example

```modula2
VAR d: LONGREAL;
    i: INTEGER;

BEGIN
  i := 42;
  d := LFLOAT(i);     (* d = 42.0 as LONGREAL *)
  d := LFLOAT(100);   (* d = 100.0 as LONGREAL *)
END
```

## Notes

- `LFLOAT` is the double-precision counterpart of `FLOAT`.
- `LONGREAL` (64-bit) can exactly represent all 32-bit integer values; precision loss only occurs for very large `LONGINT` values.
- The generated C code is `(double)(n)`.
- See also `FLOAT` for conversion to `REAL` and `TRUNC` for the reverse direction.
