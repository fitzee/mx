# INC

```modula2
INC(x)
INC(x, n)
```

Increment the variable `x` by 1, or by `n` if the second argument is provided.

`x` must be a variable of type `INTEGER`, `CARDINAL`, or an enumeration type. `n` must be an integer-compatible expression.

## Example

```modula2
VAR count: CARDINAL;

BEGIN
  count := 0;
  INC(count);      (* count = 1 *)
  INC(count, 5);   (* count = 6 *)
END
```

## Notes

- `INC(x)` is equivalent to `x := x + 1` but may generate more efficient code.
- `INC(x, n)` is equivalent to `x := x + n`.
- For enumeration types, `INC` advances to the next value in declaration order.
- Overflow behavior is implementation-defined.
