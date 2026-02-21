# CONST

Constant declaration. The expression must be evaluable at compile time. The
value is immutable and cannot be assigned to.

```modula2
CONST
  name = expression;
```

## Example

```modula2
CONST
  MaxSize = 256;
  Pi = 3.14159;
  Greeting = "Hello";
  LastChar = CHR(MaxSize - 1);
```
