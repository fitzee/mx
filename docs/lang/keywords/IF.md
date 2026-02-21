# IF

Conditional statement. Supports chained conditions with ELSIF and a final ELSE.

```modula2
IF condition THEN
  statements
ELSIF condition THEN
  statements
ELSE
  statements
END;
```

## Example

```modula2
IF x > 0 THEN
  WriteString("positive");
ELSIF x = 0 THEN
  WriteString("zero");
ELSE
  WriteString("negative");
END;
```
