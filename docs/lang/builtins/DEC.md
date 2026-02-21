# DEC

```modula2
DEC(x)
DEC(x, n)
```

Decrement the variable `x` by 1, or by `n` if the second argument is provided.

`x` must be a variable of type `INTEGER`, `CARDINAL`, or an enumeration type. `n` must be an integer-compatible expression.

## Example

```modula2
VAR i: INTEGER;

BEGIN
  i := 10;
  DEC(i);       (* i = 9 *)
  DEC(i, 4);    (* i = 5 *)
END
```

## Notes

- `DEC(x)` is equivalent to `x := x - 1` but may generate more efficient code.
- `DEC(x, n)` is equivalent to `x := x - n`.
- For enumeration types, `DEC` moves to the previous value in declaration order.
- Underflow behavior is implementation-defined.
