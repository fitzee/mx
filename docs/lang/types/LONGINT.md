# LONGINT

Extended-range signed integer type.

## Properties

- **Size**: Platform-dependent (typically 64 bits)
- **Range**: MIN(LONGINT)..MAX(LONGINT), typically -2^63..2^63-1
- **Operations**: `+`, `-`, `*`, `DIV`, `MOD`, unary `-`
- **Relational**: `=`, `#`, `<`, `>`, `<=`, `>=`
- **Standard functions**: `ABS`, `MAX`, `MIN`, `INC`, `DEC`, `ODD`

## Syntax

```modula2
VAR
  big: LONGINT;
  n: INTEGER;

big := 1000000000;
big := big * 1000;    (* 1000000000000, beyond INTEGER range *)
n := SHORT(big);       (* LONGINT -> INTEGER, may overflow *)
big := LONG(n);        (* INTEGER -> LONGINT *)
```

## Notes

- LONGINT is not part of the original PIM4 standard but is a common implementation extension.
- `LONG` converts INTEGER to LONGINT; `SHORT` converts LONGINT to INTEGER.
- Mixing INTEGER and LONGINT in expressions may require explicit conversion depending on the implementation.
- Use LONGINT when values may exceed the range of INTEGER (e.g., file sizes, timestamps, large counters).
- Overflow behavior is undefined unless runtime range checking is enabled.
