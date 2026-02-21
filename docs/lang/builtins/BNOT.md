# BNOT

```modula2
BNOT(x: CARDINAL): CARDINAL
```

Bitwise complement (NOT) of `x`. Each bit in the result is the inverse
of the corresponding bit in `x`.

## Example

```modula2
VAR c: CARDINAL;

c := BNOT(0);        (* c = 0FFFFFFFFH — all bits set *)
c := BNOT(0FFH);     (* c = 0FFFFFF00H *)
```

## Notes

- Takes a single CARDINAL argument (unlike the other bitwise functions
  which take two).
- The result is an unsigned 32-bit complement.
- Useful for creating masks: `BAND(x, BNOT(mask))` clears the bits in `mask`.
- See also `BAND`, `BOR`, `BXOR`.
