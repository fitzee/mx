# SIZE

```modula2
SIZE(x): CARDINAL
```

Return the number of storage units (bytes) occupied by the variable `x`.

`x` must be a variable of any type.

## Example

```modula2
VAR n: INTEGER;
    r: REAL;
    a: ARRAY [0..9] OF CHAR;

BEGIN
  WriteCard(SIZE(n), 0);   (* typically 4 *)
  WriteCard(SIZE(r), 0);   (* typically 4 or 8 *)
  WriteCard(SIZE(a), 0);   (* typically 10 *)
END
```

## Notes

- `SIZE` takes a variable as its argument. For type-level sizing, use `TSIZE`.
- The result is implementation-dependent and reflects the target platform's data sizes.
- `SIZE` may include padding bytes for alignment in record types.
- Defined in the PIM4 standard as a pervasive (always available) function.
