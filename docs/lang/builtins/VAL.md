# VAL

```modula2
VAL(T, x): T
```

Convert the value `x` to the scalar type `T`. This is a type transfer function that reinterprets or converts `x` as a value of type `T`.

`T` must be a scalar type (integer, cardinal, enumeration, char, or boolean). `x` must be a compatible scalar value.

## Example

```modula2
TYPE Color = (Red, Green, Blue);
VAR c: Color;
    n: CARDINAL;

BEGIN
  c := VAL(Color, 2);     (* c = Blue *)
  n := VAL(CARDINAL, -1); (* implementation-defined *)
END
```

## Notes

- `VAL` generalizes `ORD` and `CHR`: `CHR(n)` is equivalent to `VAL(CHAR, n)`.
- For enumeration types, `VAL(EnumType, n)` returns the `n`-th value (zero-based).
- If the value is out of range for the target type, behavior is implementation-defined.
- `VAL` is a compile-time type transfer; it does not necessarily generate runtime code.
