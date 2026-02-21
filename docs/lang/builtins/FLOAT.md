# FLOAT

```modula2
FLOAT(n): REAL
```

Convert an integer value `n` to type `REAL`.

`n` must be of type `INTEGER` or `CARDINAL`.

## Example

```modula2
VAR r: REAL;
    i: INTEGER;

BEGIN
  i := 42;
  r := FLOAT(i);       (* r = 42.0 *)
  r := FLOAT(100);     (* r = 100.0 *)
END
```

## Notes

- `FLOAT` is the standard way to convert integer values to floating point in PIM4.
- Precision may be lost for very large integer values that cannot be exactly represented in `REAL`.
- See also `TRUNC` for the reverse conversion (REAL to INTEGER).
- Some implementations also accept `CARDINAL` arguments.
