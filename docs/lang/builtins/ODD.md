# ODD

```modula2
ODD(x): BOOLEAN
```

Return `TRUE` if `x` is an odd number, `FALSE` otherwise.

`x` must be of type `INTEGER` or `CARDINAL`.

## Example

```modula2
VAR n: INTEGER;

BEGIN
  n := 7;
  IF ODD(n) THEN
    (* this branch is taken *)
  END;
END
```

## Notes

- `ODD(x)` is equivalent to `(x MOD 2) # 0`, but is the idiomatic PIM4 way to test parity.
- `ODD(0)` returns `FALSE`.
- Typically compiles to a bitwise AND with 1.
