# ROTATE

```modula2
ROTATE(val: CARDINAL; n: INTEGER): CARDINAL
```

Circular rotation of `val` by `n` bit positions. Positive `n` rotates left, negative `n` rotates right. Bits shifted out on one side re-enter on the other.

Based on ISO Modula-2 `SYSTEM.ROTATE`. Available as a pervasive builtin in m2c (no import required).

## Example

```modula2
VAR x: CARDINAL;

x := ROTATE(1, 4);      (* x = 16           — rotate left 4   *)
x := ROTATE(1, -4);     (* x = 10000000H    — rotate right 4  *)
x := ROTATE(0FF000000H, 8); (* x = 0000000FFH — MSB wraps to LSB *)
```

## Notes

- `val` is treated as an unsigned 32-bit value.
- The rotation amount is taken modulo 32, so `ROTATE(x, 33)` is the same as `ROTATE(x, 1)`.
- `ROTATE(x, 0)` returns `x` unchanged.
- Unlike `SHIFT`, no bits are lost -- they wrap around.
- Equivalent to ISO `SYSTEM.ROTATE` but does not require a `SYSTEM` import.
