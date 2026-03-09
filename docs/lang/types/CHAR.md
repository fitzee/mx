# CHAR

Single character type, representing one element of the character set.

## Properties

- **Size**: 8 bits
- **Range**: `CHR(0)`..`CHR(255)` (ordinal 0..255)
- **Operations**: none (not arithmetic)
- **Relational**: `=`, `#`, `<`, `>`, `<=`, `>=` (by ordinal value)
- **Standard functions**: `ORD`, `CHR`, `CAP`, `MIN`, `MAX`

## Syntax

```modula2
VAR
  ch: CHAR;
  code: CARDINAL;

ch := 'A';
code := ORD(ch);     (* code = 65 *)
ch := CHR(48);       (* ch = '0' *)
ch := CAP('z');       (* ch = 'Z' *)

IF (ch >= 'a') AND (ch <= 'z') THEN
  ch := CAP(ch)
END;
```

## Notes

- Character literals use single quotes: `'A'`. The null character is written as `0C`.
- Octal character constants are written as digits followed by `C` (e.g., `101C` for `'A'`).
- CHAR is an ordinal type and can be used in `CASE` selectors and subrange definitions.
- A string literal of length 1 (e.g., `"X"`) is compatible with CHAR.
- An empty string literal (`""` or `''`) is compatible with CHAR and represents the NUL character (`CHR(0)`).
- `CAP` converts lowercase to uppercase; non-letter characters are unchanged.
