# CARDINAL

Unsigned whole number type. Used where negative values are not meaningful.

## Properties

- **Size**: Platform-dependent (32 bits on most targets)
- **Range**: 0..MAX(CARDINAL)
- **Operations**: `+`, `-`, `*`, `DIV`, `MOD`
- **Relational**: `=`, `#`, `<`, `>`, `<=`, `>=`
- **Standard functions**: `MAX`, `MIN`, `INC`, `DEC`, `ODD`, `HIGH`
- **Bitwise functions**: `SHL`, `SHR`, `BAND`, `BOR`, `BXOR`, `BNOT`, `SHIFT`, `ROTATE`

## Syntax

```modula2
VAR
  count: CARDINAL;
  index: CARDINAL;

count := 100;
index := 0;

WHILE index < count DO
  INC(index)
END;
```

## Notes

- CARDINAL is the natural type for array indices, loop counters, and sizes.
- Subtraction that would produce a negative result is undefined.
- Not directly assignment compatible with INTEGER; use transfer functions `INTEGER()` or `CARDINAL()` to convert.
- `HIGH(a)` returns CARDINAL for the upper bound of an open array parameter.
- Mixed INTEGER/CARDINAL expressions require explicit conversion in strict PIM4.
