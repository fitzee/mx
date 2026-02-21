# Terminal

Low-level terminal I/O module. Provides single-character and string
output without formatting. Simpler alternative to InOut for basic
terminal interaction.

## Exported Procedures

```modula2
PROCEDURE Read(VAR ch: CHAR);
PROCEDURE Write(ch: CHAR);
PROCEDURE WriteLn;
PROCEDURE WriteString(s: ARRAY OF CHAR);
```

## Notes

- `Read` reads a single character from standard input.
- `Write` writes a single character to standard output.
- Unlike InOut, Terminal does not provide formatted numeric output
  or the `Done` status variable.

## Example

```modula2
MODULE TermDemo;
FROM Terminal IMPORT Read, Write, WriteLn, WriteString;
VAR ch: CHAR;
BEGIN
  WriteString("Press a key: ");
  Read(ch);
  WriteLn;
  WriteString("You pressed: ");
  Write(ch);
  WriteLn;
END TermDemo.
```
