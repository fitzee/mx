# INCL

```modula2
INCL(s, i)
```

Include element `i` in the set variable `s`. If `i` is already a member of `s`, the set is unchanged.

`s` must be a variable of a `SET OF` type, and `i` must be a value within the base type of that set.

## Example

```modula2
TYPE CharSet = SET OF CHAR;
VAR vowels: CharSet;

BEGIN
  vowels := CharSet{};
  INCL(vowels, 'a');
  INCL(vowels, 'e');
  (* vowels = CharSet{'a', 'e'} *)
END
```

## Notes

- `INCL(s, i)` is equivalent to `s := s + {i}` but may generate more efficient code.
- The element `i` must be within the valid range of the set's base type.
- See also `EXCL` for removing elements from a set.
