# CHR

```modula2
CHR(n): CHAR
```

Return the character whose ordinal value is `n`.

`n` must be of type `CARDINAL` or `INTEGER`.

## Example

```modula2
VAR ch: CHAR;

BEGIN
  ch := CHR(65);    (* ch = 'A' *)
  ch := CHR(48);    (* ch = '0' *)
  ch := CHR(10);    (* ch = newline *)
END
```

## Notes

- `CHR` is the inverse of `ORD` for characters: `ORD(CHR(n)) = n` for valid values.
- The argument must be in the range `0..255` (or the implementation's character range).
- Values outside the valid character range produce undefined results.
