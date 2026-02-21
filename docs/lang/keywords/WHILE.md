# WHILE

While loop. The condition is checked before each iteration. The body may execute
zero times if the condition is initially FALSE.

```modula2
WHILE condition DO
  statements
END;
```

## Example

```modula2
WHILE i < 10 DO
  sum := sum + i;
  INC(i);
END;
```
