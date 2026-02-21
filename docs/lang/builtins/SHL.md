# SHL

```modula2
SHL(x: CARDINAL; n: CARDINAL): CARDINAL
```

Shift `x` left by `n` bit positions. Vacated low bits are filled with zero.

## Example

```modula2
VAR c: CARDINAL;

c := SHL(1, 8);     (* c = 256   *)
c := SHL(0FFH, 16); (* c = 0FF0000H *)
```

## Notes

- Both arguments are unsigned (CARDINAL).
- Bits shifted beyond bit 31 are lost.
- `SHL(x, 0)` returns `x` unchanged.
- For bidirectional shifts (sign determines direction), see `SHIFT`.
