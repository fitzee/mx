# MAX

```modula2
MAX(T): T
```

Return the maximum value of the scalar type `T`. The result is a compile-time constant.

`T` must be a scalar type: `INTEGER`, `CARDINAL`, `CHAR`, `BOOLEAN`, `REAL`, `LONGREAL`, or an enumeration type.

## Example

```modula2
TYPE Day = (Mon, Tue, Wed, Thu, Fri, Sat, Sun);
VAR n: INTEGER;

BEGIN
  n := MAX(INTEGER);    (* e.g., 2147483647 on 32-bit *)
  IF ch <= MAX(CHAR) THEN (* always TRUE *) END;
  (* MAX(Day) = Sun *)
END
```

## Notes

- `MAX(BOOLEAN)` returns `TRUE`.
- `MAX(CHAR)` returns `CHR(255)` (or the highest character in the implementation's character set).
- For enumeration types, `MAX` returns the last declared constant.
- See also `MIN` for the corresponding minimum value.
