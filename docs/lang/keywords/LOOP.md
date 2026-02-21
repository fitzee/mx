# LOOP

Infinite loop. Must be terminated with an EXIT statement. Useful when the
termination condition is complex or occurs in the middle of the loop body.

```modula2
LOOP
  statements;
  IF condition THEN EXIT END;
  statements;
END;
```

## Example

```modula2
LOOP
  ReadChar(ch);
  IF ch = EOL THEN EXIT END;
  ProcessChar(ch);
END;
```
