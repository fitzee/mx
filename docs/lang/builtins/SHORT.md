# SHORT

```modula2
SHORT(x): INTEGER
```

Convert a `LONGINT` value `x` to type `INTEGER` (32-bit signed integer).

`x` must be of type `LONGINT`.

## Example

```modula2
VAR big: LONGINT;
    n: INTEGER;

BEGIN
  big := LONG(42);
  n := SHORT(big);   (* n = 42 *)
END
```

## Notes

- If the value of `x` exceeds the range of `INTEGER`, the result is implementation-defined (truncation to 32 bits).
- The generated C code is `(int32_t)(x)`.
- See also `LONG` for the reverse conversion (INTEGER to LONGINT).
- Defined in PIM4 as a pervasive standard function.
