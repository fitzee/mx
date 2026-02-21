# BOR

```modula2
BOR(a: CARDINAL; b: CARDINAL): CARDINAL
```

Bitwise OR of `a` and `b`. Each result bit is 1 if the corresponding
bit of either operand is 1.

## Example

```modula2
VAR c: CARDINAL;

c := BOR(0F0H, 0FH);  (* c = 0FFH — combine nibbles *)
c := BOR(SHL(r, 24), BOR(SHL(g, 16), SHL(b, 8)));  (* pack RGB *)
```

## Notes

- Both arguments and the result are unsigned (CARDINAL).
- Commonly used to combine bit fields or set specific bits.
- See also `BAND`, `BXOR`, `BNOT`.
