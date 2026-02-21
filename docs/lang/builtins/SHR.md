# SHR

```modula2
SHR(x: CARDINAL; n: CARDINAL): CARDINAL
```

Shift `x` right by `n` bit positions. Vacated high bits are filled with zero
(logical shift, not arithmetic).

## Example

```modula2
VAR c: CARDINAL;

c := SHR(256, 8);       (* c = 1     *)
c := SHR(0FF00H, 8);    (* c = 0FFH  *)
```

## Notes

- Both arguments are unsigned (CARDINAL).
- This is a logical (unsigned) shift; the high bits are always zero-filled.
- `SHR(x, 0)` returns `x` unchanged.
- For bidirectional shifts (sign determines direction), see `SHIFT`.
