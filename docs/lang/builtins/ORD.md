# ORD

```modula2
ORD(x): CARDINAL
```

Return the ordinal (numeric) value of `x`.

`x` must be of type `CHAR`, `BOOLEAN`, or an enumeration type.

## Example

```modula2
VAR n: CARDINAL;

BEGIN
  n := ORD('A');     (* n = 65 *)
  n := ORD(TRUE);    (* n = 1 *)
  n := ORD(FALSE);   (* n = 0 *)
END
```

## Notes

- For `CHAR`, `ORD` returns the ASCII code of the character.
- For `BOOLEAN`, `ORD(FALSE) = 0` and `ORD(TRUE) = 1`.
- For enumeration types, `ORD` returns the zero-based position in the declaration.
- `ORD` is the inverse of `CHR` for characters: `CHR(ORD(c)) = c`.
