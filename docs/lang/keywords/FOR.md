# FOR

Counted loop. The control variable must be a local variable. The optional BY
clause sets the step; it defaults to 1.

```modula2
FOR i := start TO end BY step DO
  statements
END;
```

## Example

```modula2
FOR i := 1 TO 10 DO
  WriteCard(i * i, 4);
END;

FOR j := 20 TO 0 BY -2 DO
  WriteInt(j, 4);
END;
```
