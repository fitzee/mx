# SHIFT

```modula2
SHIFT(val: CARDINAL; n: INTEGER): CARDINAL
```

Logical shift of `val` by `n` bit positions. Positive `n` shifts left,
negative `n` shifts right. Vacated bits are filled with zero.

Based on ISO Modula-2 `SYSTEM.SHIFT`. Available as a pervasive builtin
in m2c (no import required).

## Example

```modula2
VAR x: CARDINAL;

x := SHIFT(1, 8);    (* x = 256  — shift left 8  *)
x := SHIFT(256, -8); (* x = 1    — shift right 8  *)
x := SHIFT(0FFH, 4); (* x = 0FF0H                 *)
```

## Notes

- `val` is treated as an unsigned 32-bit value.
- If `|n| >= 32`, the result is 0 (all bits shifted out).
- `SHIFT(x, 0)` returns `x` unchanged.
- For fixed-direction shifts, the simpler `SHL` and `SHR` builtins
  may be preferred.
- Equivalent to ISO `SYSTEM.SHIFT` but does not require a `SYSTEM` import.
