# OR

Logical OR operator. Short-circuit evaluation: the right operand is not evaluated
if the left is TRUE. Both operands must be BOOLEAN.

```modula2
expr1 OR expr2
```

## Example

```modula2
IF (ch = ' ') OR (ch = CHR(9)) THEN
  WriteString("whitespace");
END;
```
