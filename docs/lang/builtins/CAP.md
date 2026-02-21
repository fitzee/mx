# CAP

```modula2
CAP(c): CHAR
```

Return the uppercase equivalent of the character `c`. If `c` is not a lowercase letter (`'a'`..`'z'`), it is returned unchanged.

`c` must be of type `CHAR`.

## Example

```modula2
VAR ch: CHAR;

BEGIN
  ch := CAP('m');   (* ch = 'M' *)
  ch := CAP('Z');   (* ch = 'Z', already uppercase *)
  ch := CAP('5');   (* ch = '5', not a letter *)
END
```

## Notes

- `CAP` only converts ASCII lowercase letters (`'a'`..`'z'`) to uppercase.
- Characters outside the lowercase letter range are returned as-is.
- PIM4 does not define behavior for characters outside the ASCII range.
