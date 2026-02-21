# NOT

Logical NOT operator (also written `~`). Negates a BOOLEAN expression.

```modula2
NOT expr
```

## Example

```modula2
IF NOT done THEN
  Continue();
END;

WHILE NOT EOF() DO
  ReadLine(buf);
END;
```
