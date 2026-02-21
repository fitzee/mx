# REPEAT

Repeat loop. The body executes at least once. The loop exits when the condition
becomes TRUE.

```modula2
REPEAT
  statements
UNTIL condition;
```

## Example

```modula2
REPEAT
  ReadChar(ch);
UNTIL ch = '.';
```
