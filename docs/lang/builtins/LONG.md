# LONG

```modula2
LONG(x): LONGINT
```

Convert an integer value `x` to type `LONGINT` (64-bit signed integer).

`x` must be of type `INTEGER` or `CARDINAL`.

## Example

```modula2
VAR n: INTEGER;
    big: LONGINT;

BEGIN
  n := 1000000;
  big := LONG(n);          (* big = 1000000 as LONGINT *)
  big := LONG(2147483647); (* maximum INTEGER promoted to LONGINT *)
END
```

## Notes

- `LONG` widens the value without loss of information since `LONGINT` can represent every `INTEGER` and `CARDINAL` value.
- The generated C code is `(int64_t)(x)`.
- See also `SHORT` for the reverse conversion (LONGINT to INTEGER).
- Defined in PIM4 as a pervasive standard function.
