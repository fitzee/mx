# HIGH

```modula2
HIGH(a): CARDINAL
```

Return the upper bound index of the open array parameter `a`. The value equals `LENGTH(a) - 1`.

`a` must be an open array formal parameter (declared as `ARRAY OF T`).

## Example

```modula2
PROCEDURE Sum(a: ARRAY OF INTEGER): INTEGER;
VAR i, total: INTEGER;
BEGIN
  total := 0;
  FOR i := 0 TO HIGH(a) DO
    total := total + a[i];
  END;
  RETURN total
END Sum;
```

## Notes

- `HIGH` is only valid on open array parameters, not on fixed-size arrays.
- Open arrays are zero-indexed: valid indices are `0` to `HIGH(a)`.
- For a one-element array, `HIGH` returns `0`.
- Passing an empty array is not permitted in standard PIM4.
