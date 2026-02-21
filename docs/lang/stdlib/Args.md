# Args

Command-line argument access module. Provides procedures to query
the number of arguments and retrieve individual argument strings.

## Exported Procedures

```modula2
PROCEDURE ArgCount(): CARDINAL;
PROCEDURE GetArg(n: CARDINAL; VAR s: ARRAY OF CHAR);
```

## Notes

- `ArgCount` returns the total number of command-line arguments,
  including the program name at index 0.
- `GetArg(0, s)` retrieves the program name.
- `GetArg(1, s)` retrieves the first user-supplied argument, etc.
- If `n` is out of range, `s` is set to an empty string.

## Example

```modula2
MODULE ArgsDemo;
FROM Args IMPORT ArgCount, GetArg;
FROM InOut IMPORT WriteString, WriteLn, WriteCard;
VAR
  i: CARDINAL;
  buf: ARRAY [0..255] OF CHAR;
BEGIN
  WriteString("Argument count: ");
  WriteCard(ArgCount(), 0);
  WriteLn;
  FOR i := 0 TO ArgCount() - 1 DO
    GetArg(i, buf);
    WriteString(buf); WriteLn;
  END;
END ArgsDemo.
```
