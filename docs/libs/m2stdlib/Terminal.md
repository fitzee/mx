# Terminal

Minimal character-level I/O. Terminal provides just four procedures for reading and writing individual characters and strings. If you only need to print text and read characters, Terminal is simpler than InOut. For formatted number output, use InOut instead.

Available in PIM4 mode (the default). No special flags needed.

## Variables

| Variable | Type | Description |
|----------|------|-------------|
| `Done` | `BOOLEAN` | Set by `Read`. `TRUE` if a character was successfully read, `FALSE` on EOF. |

## Procedures

```modula2
PROCEDURE Read(VAR ch: CHAR);
```
Read a single character from stdin into `ch`. Sets `Done` to `FALSE` on EOF.

```modula2
PROCEDURE Write(ch: CHAR);
```
Write a single character to stdout.

```modula2
PROCEDURE WriteString(s: ARRAY OF CHAR);
```
Write a NUL-terminated string to stdout. Stops at the first NUL character or the end of the array.

```modula2
PROCEDURE WriteLn;
```
Write a newline character to stdout.

## Example

```modula2
MODULE TermDemo;
FROM Terminal IMPORT WriteString, Write, WriteLn;
BEGIN
  WriteString("Press any key: ");
  (* Note: Terminal has no WriteInt -- use InOut if you need numbers *)
  Write('!');
  WriteLn
END TermDemo.
```
