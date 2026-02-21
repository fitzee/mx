# LONGCARD

Extended-range unsigned integer type.

## Properties

- **Size**: Platform-dependent (typically 64 bits)
- **Range**: 0..MAX(LONGCARD)
- **Operations**: `+`, `-`, `*`, `DIV`, `MOD`
- **Relational**: `=`, `#`, `<`, `>`, `<=`, `>=`
- **Standard functions**: `MAX`, `MIN`, `INC`, `DEC`, `ODD`

## Syntax

```modula2
VAR
  big: LONGCARD;
  n: CARDINAL;

big := 4000000000;    (* exceeds 32-bit CARDINAL range *)
big := big + 1;
n := SHORT(big);       (* LONGCARD -> CARDINAL, may overflow *)
big := LONG(n);        (* CARDINAL -> LONGCARD *)
```

## Notes

- LONGCARD is not part of the original PIM4 standard but is a common implementation extension.
- `LONG` converts CARDINAL to LONGCARD; `SHORT` converts LONGCARD to CARDINAL.
- Subtraction that would produce a negative result is undefined.
- Use LONGCARD for large unsigned quantities such as file offsets, memory sizes, or hash values.
- Not directly assignment compatible with LONGINT; use explicit transfer functions to convert.
