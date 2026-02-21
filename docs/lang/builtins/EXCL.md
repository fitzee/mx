# EXCL

```modula2
EXCL(s, i)
```

Exclude element `i` from the set variable `s`. If `i` is not a member of `s`, the set is unchanged.

`s` must be a variable of a `SET OF` type, and `i` must be a value within the base type of that set.

## Example

```modula2
TYPE Digits = SET OF [0..9];
VAR d: Digits;

BEGIN
  d := Digits{0, 1, 2, 3};
  EXCL(d, 2);
  (* d = Digits{0, 1, 3} *)
END
```

## Notes

- `EXCL(s, i)` is equivalent to `s := s - {i}` but may generate more efficient code.
- The element `i` must be within the valid range of the set's base type.
- See also `INCL` for adding elements to a set.
