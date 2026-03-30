# InOut

The primary text I/O module for PIM4 programs. This is the module you will use most often for printing output and reading user input. It reads from stdin and writes to stdout by default, with optional file redirection.

Available in PIM4 mode (the default). No special flags needed.

## Variables

| Variable | Type | Description |
|----------|------|-------------|
| `Done` | `BOOLEAN` | Set after every read operation. `TRUE` if the read succeeded, `FALSE` if input was invalid or unavailable (e.g., EOF). Write operations do not affect `Done`. |

## Procedures

### Output

```modula2
PROCEDURE WriteString(s: ARRAY OF CHAR);
```
Write a NUL-terminated string to stdout. This is the most common way to print text. The string is written up to the first NUL character (0C) or the end of the array, whichever comes first.

```modula2
PROCEDURE WriteLn;
```
Write a newline character. Call this after `WriteString` or `WriteInt` to end the line -- output is not automatically newline-terminated.

```modula2
PROCEDURE Write(ch: CHAR);
PROCEDURE WriteChar(ch: CHAR);
```
Write a single character to stdout. `WriteChar` is an alias for `Write` -- they do the same thing.

```modula2
PROCEDURE WriteInt(n: INTEGER; w: CARDINAL);
```
Write a signed integer right-justified in a field of width `w`. If the printed number has fewer digits than `w`, spaces are added on the left. If it has more digits, it is printed in full (not truncated). Use `w=1` for minimum-width output with no padding.

Example: `WriteInt(-42, 6)` prints `"   -42"` (4 characters, padded to width 6).

```modula2
PROCEDURE WriteCard(n: CARDINAL; w: CARDINAL);
```
Write an unsigned cardinal, right-justified in width `w`. Same padding rules as `WriteInt`.

```modula2
PROCEDURE WriteHex(n: CARDINAL; w: CARDINAL);
```
Write `n` as a hexadecimal number (uppercase A-F), right-justified in width `w`. No `0x` prefix is printed.

```modula2
PROCEDURE WriteOct(n: CARDINAL; w: CARDINAL);
```
Write `n` as an octal number, right-justified in width `w`.

### Input

```modula2
PROCEDURE Read(VAR ch: CHAR);
PROCEDURE ReadChar(VAR ch: CHAR);
```
Read a single character from stdin. Sets `Done` to `TRUE` on success, `FALSE` on EOF or error. `ReadChar` is an alias for `Read`.

```modula2
PROCEDURE ReadString(VAR s: ARRAY OF CHAR);
```
Read a whitespace-delimited token (word) from stdin into `s`. Leading whitespace is skipped. Reading stops at the next whitespace character or EOF. The result is NUL-terminated. Sets `Done`.

```modula2
PROCEDURE ReadInt(VAR n: INTEGER);
```
Read a signed integer from stdin. Skips leading whitespace, reads optional sign and digits. Sets `Done` to `FALSE` if the input is not a valid integer.

```modula2
PROCEDURE ReadCard(VAR n: CARDINAL);
```
Read an unsigned cardinal from stdin. Sets `Done` to `FALSE` if the input is not a valid number.

### File Redirection

These procedures redirect subsequent reads or writes to a file instead of stdin/stdout. This is useful for simple batch processing. For more control over file I/O, use `BinaryIO` or `FileSystem` instead.

```modula2
PROCEDURE OpenInput(ext: ARRAY OF CHAR);
```
Open a file for reading. After this call, `Read`, `ReadString`, `ReadInt`, and `ReadCard` read from the file instead of stdin. `ext` is the filename (not a file extension, despite the parameter name -- this is a PIM4 historical convention).

```modula2
PROCEDURE OpenOutput(ext: ARRAY OF CHAR);
```
Open a file for writing. Subsequent `Write`, `WriteString`, `WriteInt`, etc. go to the file.

```modula2
PROCEDURE CloseInput;
PROCEDURE CloseOutput;
```
Close the redirected file and restore stdin/stdout for subsequent I/O.

## Example

```modula2
MODULE InOutDemo;
FROM InOut IMPORT WriteString, WriteInt, WriteHex, WriteLn;
BEGIN
  WriteString("Decimal: ");
  WriteInt(255, 1); WriteLn;

  WriteString("Hex:     ");
  WriteHex(255, 1); WriteLn;

  WriteString("Padded:  [");
  WriteInt(42, 8);
  WriteString("]"); WriteLn
END InOutDemo.
```

Output:
```
Decimal: 255
Hex:     FF
Padded:  [      42]
```
