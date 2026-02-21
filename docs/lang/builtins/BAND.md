# BAND

```modula2
BAND(a: CARDINAL; b: CARDINAL): CARDINAL
```

Bitwise AND of `a` and `b`. Each result bit is 1 only if the
corresponding bits of both operands are 1.

## Example

```modula2
VAR c: CARDINAL;

c := BAND(0FFH, 0FH);    (* c = 0FH  — mask low nibble *)
c := BAND(0ABCDH, 0FF00H); (* c = 0AB00H — mask high byte *)
```

## Notes

- Both arguments and the result are unsigned (CARDINAL).
- Commonly used to mask or extract specific bits from a value.
- See also `BOR`, `BXOR`, `BNOT`.
