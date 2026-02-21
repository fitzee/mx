# INTEGER

Signed whole number type. The fundamental numeric type in Modula-2.

## Properties

- **Size**: Platform-dependent (32 bits on most targets)
- **Range**: MIN(INTEGER)..MAX(INTEGER), typically -2147483648..2147483647 on 32-bit
- **Operations**: `+`, `-`, `*`, `DIV`, `MOD`, unary `-`
- **Relational**: `=`, `#`, `<`, `>`, `<=`, `>=`
- **Standard functions**: `ABS`, `MAX`, `MIN`, `INC`, `DEC`, `ODD`

## Syntax

```modula2
VAR
  i, j: INTEGER;
  result: INTEGER;

i := -42;
j := 17;
result := (i + j) DIV 3;

IF ODD(result) THEN
  DEC(result)
END;
```

## Notes

- `DIV` truncates toward zero in PIM4.
- `MOD` result has the sign of the divisor.
- Integer overflow is undefined; the compiler does not insert runtime checks unless range checking is enabled.
- Assignment compatible with expressions of type INTEGER. Not directly compatible with CARDINAL without explicit conversion via `INTEGER()` or `CARDINAL()`.
