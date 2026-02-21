# Strings

String manipulation module. Operates on `ARRAY OF CHAR` values
using the standard PIM4 null-terminated string convention.

## Exported Procedures

```modula2
PROCEDURE Length(s: ARRAY OF CHAR): CARDINAL;
PROCEDURE Assign(source: ARRAY OF CHAR; VAR dest: ARRAY OF CHAR);
PROCEDURE Concat(s1, s2: ARRAY OF CHAR; VAR result: ARRAY OF CHAR);
PROCEDURE Compare(s1, s2: ARRAY OF CHAR): INTEGER;
PROCEDURE Extract(source: ARRAY OF CHAR; pos, len: CARDINAL;
                  VAR dest: ARRAY OF CHAR);
PROCEDURE Insert(substr: ARRAY OF CHAR; VAR str: ARRAY OF CHAR;
                 pos: CARDINAL);
PROCEDURE Delete(VAR str: ARRAY OF CHAR; pos, len: CARDINAL);
PROCEDURE Pos(pattern, str: ARRAY OF CHAR): CARDINAL;
```

## Notes

- `Compare` returns -1 if s1 < s2, 0 if equal, +1 if s1 > s2.
- `Pos` returns the index of the first occurrence of `pattern` in
  `str`, or `HIGH(str) + 1` if not found.
- `Assign` truncates if `dest` is shorter than `source`.

## Example

```modula2
MODULE StringDemo;
FROM Strings IMPORT Length, Concat, Pos;
FROM InOut IMPORT WriteString, WriteLn, WriteCard;
VAR buf: ARRAY [0..63] OF CHAR;
BEGIN
  Concat("Hello, ", "world!", buf);
  WriteString(buf); WriteLn;
  WriteCard(Length(buf), 0); WriteLn;
  WriteCard(Pos("world", buf), 0); WriteLn;
END StringDemo.
```
