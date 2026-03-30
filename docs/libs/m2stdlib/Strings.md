# Strings

String manipulation on fixed-size character arrays. Modula-2 has no built-in string type -- all strings are `ARRAY [0..N] OF CHAR`, NUL-terminated within the array. This module provides the standard operations for working with these arrays: copying, concatenating, searching, comparing, inserting, and deleting.

All operations NUL-terminate their output and truncate silently if the destination array is too small. This is safe -- you will never get an unterminated string -- but you may lose characters if your buffer is undersized.

Available in PIM4 mode (the default). No special flags needed.

## Procedures

### Assign

```modula2
PROCEDURE Assign(s: ARRAY OF CHAR; VAR dst: ARRAY OF CHAR);
```
Copy the string `s` into `dst`, NUL-terminating the result. If `s` is longer than `dst` can hold, it is truncated to fit. This is the standard way to "assign" a string in Modula-2, since you cannot use `:=` to assign a string literal to an array variable.

```modula2
VAR name: ARRAY [0..31] OF CHAR;
Assign("hello", name);   (* name now contains "hello\0" *)
```

### Concat

```modula2
PROCEDURE Concat(s1: ARRAY OF CHAR; s2: ARRAY OF CHAR; VAR dst: ARRAY OF CHAR);
```
Concatenate `s1` and `s2` and store the result in `dst`. `dst` may be the same variable as `s1` (appending `s2` to an existing string). Truncates on overflow.

```modula2
Assign("hello", buf);
Concat(buf, " world", buf);   (* buf is now "hello world" *)
```

### Copy

```modula2
PROCEDURE Copy(src: ARRAY OF CHAR; pos: CARDINAL; len: CARDINAL;
               VAR dst: ARRAY OF CHAR);
```
Extract a substring: copy `len` characters from `src` starting at index `pos` into `dst`. NUL-terminates the result.

```modula2
Copy("hello world", 6, 5, part);   (* part is "world" *)
```

### Length

```modula2
PROCEDURE Length(s: ARRAY OF CHAR): CARDINAL;
```
Return the number of characters before the first NUL. This is the logical string length, not the array size.

### Pos

```modula2
PROCEDURE Pos(sub: ARRAY OF CHAR; s: ARRAY OF CHAR): CARDINAL;
```
Find the first occurrence of `sub` in `s`. Returns the zero-based index of the match. If `sub` is not found, returns `HIGH(s) + 1` (a value past the end of the string).

```modula2
pos := Pos("world", "hello world");   (* pos = 6 *)
pos := Pos("xyz", "hello world");     (* pos = HIGH("hello world") + 1 *)
```

### CompareStr

```modula2
PROCEDURE CompareStr(s1: ARRAY OF CHAR; s2: ARRAY OF CHAR): INTEGER;
```
Lexicographic comparison of two strings. Returns:
- `0` if the strings are equal
- A negative value if `s1 < s2`
- A positive value if `s1 > s2`

```modula2
IF CompareStr(a, b) = 0 THEN
  WriteString("equal")
END;
```

### Insert

```modula2
PROCEDURE Insert(sub: ARRAY OF CHAR; VAR dst: ARRAY OF CHAR; pos: CARDINAL);
```
Insert `sub` into `dst` at position `pos`, shifting existing characters to the right. Characters that would overflow the array are lost.

### Delete

```modula2
PROCEDURE Delete(VAR s: ARRAY OF CHAR; pos: CARDINAL; len: CARDINAL);
```
Remove `len` characters from `s` starting at position `pos`, shifting the remainder left. The result is NUL-terminated.

### CAPS

```modula2
PROCEDURE CAPS(VAR s: ARRAY OF CHAR);
```
Convert all lowercase letters in `s` to uppercase, in place. Non-letter characters are unchanged.

## Example

```modula2
MODULE StringDemo;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;
FROM Strings IMPORT Assign, Length, Concat, Pos, Copy, CompareStr;

VAR
  buf: ARRAY [0..255] OF CHAR;
  part: ARRAY [0..63] OF CHAR;
  pos: CARDINAL;

BEGIN
  (* Build a string *)
  Assign("hello", buf);
  Concat(buf, " world", buf);
  WriteString(buf); WriteLn;

  (* Query length *)
  WriteString("Length: ");
  WriteInt(INTEGER(Length(buf)), 1); WriteLn;

  (* Find a substring *)
  pos := Pos("world", buf);
  WriteString("'world' starts at index ");
  WriteInt(INTEGER(pos), 1); WriteLn;

  (* Extract a substring *)
  Copy(buf, 6, 5, part);
  WriteString("Extracted: ");
  WriteString(part); WriteLn;

  (* Compare strings *)
  IF CompareStr("abc", "def") < 0 THEN
    WriteString("abc comes before def")
  END;
  WriteLn
END StringDemo.
```
