# Conversions

Number-to-string and string-to-number conversion module. Useful for
building formatted output or parsing numeric input from strings.

## Exported Procedures

```modula2
PROCEDURE IntToStr(n: INTEGER; VAR s: ARRAY OF CHAR);
PROCEDURE StrToInt(s: ARRAY OF CHAR; VAR n: INTEGER;
                   VAR ok: BOOLEAN);
PROCEDURE CardToStr(n: CARDINAL; VAR s: ARRAY OF CHAR);
PROCEDURE StrToCard(s: ARRAY OF CHAR; VAR n: CARDINAL;
                    VAR ok: BOOLEAN);
```

## Notes

- `IntToStr` writes the decimal representation of `n` into `s`,
  including a leading minus sign for negative values.
- `StrToInt` parses the string `s` and sets `ok` to `TRUE` on
  success, `FALSE` if the string is not a valid integer.
- `CardToStr` and `StrToCard` work identically but for unsigned
  cardinal values.

## Example

```modula2
MODULE ConvDemo;
FROM Conversions IMPORT IntToStr, StrToInt;
FROM InOut IMPORT WriteString, WriteLn;
VAR
  buf: ARRAY [0..15] OF CHAR;
  n: INTEGER;
  ok: BOOLEAN;
BEGIN
  IntToStr(-42, buf);
  WriteString(buf); WriteLn;
  StrToInt("123", n, ok);
  IF ok THEN
    IntToStr(n, buf);
    WriteString(buf); WriteLn;
  END;
END ConvDemo.
```
