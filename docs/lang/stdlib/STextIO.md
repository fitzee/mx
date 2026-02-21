# STextIO

ISO-style text input/output module. Provides character and string
I/O following ISO 10514 conventions rather than PIM4.

## Exported Procedures

```modula2
PROCEDURE ReadChar(VAR ch: CHAR);
PROCEDURE WriteChar(ch: CHAR);
PROCEDURE ReadString(VAR s: ARRAY OF CHAR);
PROCEDURE WriteString(s: ARRAY OF CHAR);
PROCEDURE WriteLn;
PROCEDURE SkipLine;
```

## Notes

- `SkipLine` discards all remaining characters on the current
  input line, including the line terminator.
- `ReadString` reads characters until whitespace or end of input.
- This module mirrors the ISO Modula-2 STextIO interface for
  programs targeting ISO compatibility.

## Example

```modula2
MODULE STextDemo;
FROM STextIO IMPORT ReadString, WriteString, WriteLn, SkipLine;
VAR name: ARRAY [0..63] OF CHAR;
BEGIN
  WriteString("Enter your name: ");
  ReadString(name);
  SkipLine;
  WriteString("Hello, ");
  WriteString(name);
  WriteLn;
END STextDemo.
```
