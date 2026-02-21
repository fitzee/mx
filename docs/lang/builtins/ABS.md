# ABS

```modula2
ABS(x): (same type as x)
```

Return the absolute value of `x`. The result type matches the type of the argument.

`x` must be of type `INTEGER`, `REAL`, or `LONGREAL`.

## Example

```modula2
VAR i: INTEGER;
    r: REAL;

BEGIN
  i := ABS(-7);     (* i = 7 *)
  r := ABS(-3.14);  (* r = 3.14 *)
END
```

## Notes

- `ABS` is a standard function procedure; it returns a value and cannot be used as a statement.
- The result type is the same as the argument type: `ABS(INTEGER)` returns `INTEGER`, `ABS(REAL)` returns `REAL`.
- `ABS(MIN(INTEGER))` may overflow since the positive range of `INTEGER` is one less than the negative range.
