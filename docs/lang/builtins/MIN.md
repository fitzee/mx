# MIN

```modula2
MIN(T): T
```

Return the minimum value of the scalar type `T`. The result is a compile-time constant.

`T` must be a scalar type: `INTEGER`, `CARDINAL`, `CHAR`, `BOOLEAN`, `REAL`, `LONGREAL`, or an enumeration type.

## Example

```modula2
TYPE Day = (Mon, Tue, Wed, Thu, Fri, Sat, Sun);
VAR n: INTEGER;

BEGIN
  n := MIN(INTEGER);    (* e.g., -2147483648 on 32-bit *)
  (* MIN(CARDINAL) = 0 *)
  (* MIN(Day) = Mon *)
END
```

## Notes

- `MIN(BOOLEAN)` returns `FALSE`.
- `MIN(CARDINAL)` is always `0`.
- `MIN(CHAR)` returns `CHR(0)` (the null character).
- For enumeration types, `MIN` returns the first declared constant.
- See also `MAX` for the corresponding maximum value.
