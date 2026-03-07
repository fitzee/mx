# Text

Text analysis utilities: UTF-8 validation, text-vs-binary detection, BOM detection, line counting and ending detection, and shebang parsing. Operates on raw byte buffers with no heap allocation.

## Why Text?

Tools that process source files, configuration, or arbitrary user input need to answer basic questions: Is this valid UTF-8? Is this a text file or binary? What line endings does it use? Does it have a shebang line? Text provides these checks as fast, single-pass scans over `ADDRESS`+length buffers. No heap allocation, no encoding conversion, no dependencies beyond SYSTEM.

## Constants

### Line Ending Types

```modula2
CONST
  LineEndNone  = 0;
  LineEndLF    = 1;
  LineEndCRLF  = 2;
  LineEndCR    = 3;
  LineEndMixed = 4;
```

| Value | Meaning |
|-------|---------|
| `LineEndNone` | No line endings found (single line) |
| `LineEndLF` | Unix-style (`\n` only) |
| `LineEndCRLF` | Windows-style (`\r\n` only) |
| `LineEndCR` | Classic Mac (`\r` only) |
| `LineEndMixed` | Multiple different line ending types present |

## Procedures

### IsValidUTF8

```modula2
PROCEDURE IsValidUTF8(buf: ADDRESS; len: CARDINAL): BOOLEAN;
```

Full UTF-8 validation. Returns `TRUE` if the buffer contains only valid UTF-8 sequences. Rejects overlong encodings, surrogates (U+D800..U+DFFF), and codepoints above U+10FFFF. Validates continuation bytes and multi-byte sequence lengths.

### IsASCII

```modula2
PROCEDURE IsASCII(buf: ADDRESS; len: CARDINAL): BOOLEAN;
```

Returns `TRUE` if every byte in the buffer is less than 128.

### IsText

```modula2
PROCEDURE IsText(buf: ADDRESS; len: CARDINAL): BOOLEAN;
```

Heuristic text detection. Scans the first min(`len`, 8192) bytes and returns `TRUE` if:

- No NUL bytes (0x00) are present, AND
- The ratio of control characters to total bytes is below 5%.

Control characters are bytes in ranges 0x01-0x08 and 0x0E-0x1F. TAB (0x09), LF (0x0A), and CR (0x0D) are excluded from the control character count since they appear in normal text.

### IsBinary

```modula2
PROCEDURE IsBinary(buf: ADDRESS; len: CARDINAL): BOOLEAN;
```

Convenience: returns `NOT IsText(buf, len)`.

### HasBOM

```modula2
PROCEDURE HasBOM(buf: ADDRESS; len: CARDINAL): INTEGER;
```

Check for a UTF-8 BOM (byte order mark: `EF BB BF`). Returns 3 if found (the BOM length in bytes), 0 otherwise. Requires `len >= 3` for a match.

### CountLines

```modula2
PROCEDURE CountLines(buf: ADDRESS; len: CARDINAL): INTEGER;
```

Count lines in the buffer. Returns 0 for an empty buffer, otherwise the number of LF bytes + 1 (so a file with no trailing newline still counts the last line).

### DetectLineEnding

```modula2
PROCEDURE DetectLineEnding(buf: ADDRESS; len: CARDINAL): INTEGER;
```

Scan the buffer and return one of the `LineEnd*` constants. Counts occurrences of CRLF, bare LF, and bare CR. If only one type is found, returns that type. If multiple types are present, returns `LineEndMixed`.

### ParseShebang

```modula2
PROCEDURE ParseShebang(buf: ADDRESS; len: CARDINAL;
                       VAR interp: ARRAY OF CHAR);
```

Extract the interpreter name from a `#!` shebang line at the start of the buffer. Handles both direct paths and the `/usr/bin/env` form:

| Input | interp |
|-------|--------|
| `"#!/bin/bash\n..."` | `"bash"` |
| `"#!/usr/bin/env python\n..."` | `"python"` |
| `"no shebang here"` | `""` |

Extracts the basename from direct paths (e.g., `/usr/bin/perl` yields `"perl"`).

## Example

```modula2
FROM Text IMPORT IsValidUTF8, IsText, CountLines, DetectLineEnding,
                 ParseShebang, LineEndLF, LineEndCRLF;
FROM SYSTEM IMPORT ADR;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

VAR
  src: ARRAY [0..511] OF CHAR;
  interp: ARRAY [0..63] OF CHAR;
  len: CARDINAL;

BEGIN
  (* Assume src is loaded with file contents, len bytes *)
  IF IsValidUTF8(ADR(src), len) THEN
    WriteString("valid UTF-8"); WriteLn
  END;

  IF IsText(ADR(src), len) THEN
    WriteString("lines: ");
    WriteInt(CountLines(ADR(src), len), 0); WriteLn;

    CASE DetectLineEnding(ADR(src), len) OF
      LineEndLF:   WriteString("unix line endings") |
      LineEndCRLF: WriteString("windows line endings")
    ELSE
      WriteString("other")
    END;
    WriteLn
  END;

  ParseShebang(ADR(src), len, interp);
  IF interp[0] # 0C THEN
    WriteString("interpreter: "); WriteString(interp); WriteLn
  END
END
```
