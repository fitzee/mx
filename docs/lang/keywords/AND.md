# AND

Logical AND operator (also written `&`). Short-circuit evaluation: the right
operand is not evaluated if the left is FALSE. Both operands must be BOOLEAN.

```modula2
expr1 AND expr2
```

## Example

```modula2
IF (i >= 0) AND (i < HIGH(a)) THEN
  Process(a[i]);
END;
```
