# TRUNC

```modula2
TRUNC(r): INTEGER
```

Truncate the real number `r` to an integer by discarding the fractional part. The result is rounded toward zero.

`r` must be of type `REAL` or `LONGREAL`.

## Example

```modula2
VAR i: INTEGER;

BEGIN
  i := TRUNC(3.7);    (* i = 3 *)
  i := TRUNC(-2.9);   (* i = -2 *)
  i := TRUNC(0.5);    (* i = 0 *)
END
```

## Notes

- `TRUNC` rounds toward zero, not toward negative infinity.
- If the result exceeds the range of `INTEGER`, behavior is implementation-defined.
- See also `FLOAT` for the reverse conversion (INTEGER to REAL).
- PIM4 defines `TRUNC` as a pervasive standard function.
