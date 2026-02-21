# BXOR

```modula2
BXOR(a: CARDINAL; b: CARDINAL): CARDINAL
```

Bitwise exclusive OR of `a` and `b`. Each result bit is 1 if the
corresponding bits of the two operands differ.

## Example

```modula2
VAR c: CARDINAL;

c := BXOR(0FFH, 0FH);  (* c = 0F0H — toggle low nibble *)
c := BXOR(c, c);        (* c = 0    — any value XOR itself is 0 *)
```

## Notes

- Both arguments and the result are unsigned (CARDINAL).
- XOR is its own inverse: `BXOR(BXOR(x, k), k) = x`.
- Commonly used in checksums, hashing, and toggle operations.
- See also `BAND`, `BOR`, `BNOT`.
