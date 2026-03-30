# STextIO

Text I/O following the ISO 10514 Modula-2 standard naming conventions. Provides character, string, line, and token I/O on stdin/stdout.

STextIO is functionally similar to InOut and Terminal but uses ISO-standard procedure names and does not use a global `Done` variable. You can use it in the default PIM4 mode -- no `--m2plus` flag is needed. Choose whichever API style you prefer; most PIM4 programs use InOut instead.

Available in PIM4 mode (the default). No special flags needed.

## Procedures

### Output

```modula2
PROCEDURE WriteChar(ch: CHAR);
```
Write a single character to stdout.

```modula2
PROCEDURE WriteString(s: ARRAY OF CHAR);
```
Write a NUL-terminated string to stdout.

```modula2
PROCEDURE WriteLn;
```
Write a newline character.

### Input

```modula2
PROCEDURE ReadChar(VAR ch: CHAR);
```
Read a single character from stdin. Returns whatever character is next, including whitespace and newlines.

```modula2
PROCEDURE ReadString(VAR s: ARRAY OF CHAR);
```
Read a line of text from stdin into `s`. Stops at newline or end of input.

```modula2
PROCEDURE ReadToken(VAR s: ARRAY OF CHAR);
```
Read a whitespace-delimited token from stdin. Leading whitespace is skipped, then characters are read until the next whitespace or EOF. This is useful for parsing space-separated input.

```modula2
PROCEDURE SkipLine;
```
Discard the remainder of the current input line (up to and including the newline). Use this to skip past input you do not need.

## Example

```modula2
MODULE STextIODemo;
FROM STextIO IMPORT WriteString, WriteChar, WriteLn;
BEGIN
  WriteString("Hello from STextIO");
  WriteChar('!');
  WriteLn
END STextIODemo.
```
