# BITSET

Set of small integers, representing a packed bit vector.

## Properties

- **Size**: One machine word (typically 32 bits)
- **Element range**: 0..W-1, where W is the word size in bits
- **Operations**: `+` (union), `-` (difference), `*` (intersection), `/` (symmetric difference)
- **Relational**: `=`, `#`, `<=` (subset), `>=` (superset)
- **Membership**: `IN` operator

## Syntax

```modula2
VAR
  s, t: BITSET;

s := {0, 2, 5};
t := {1, 2, 3};

s := s + t;            (* union: {0, 1, 2, 3, 5} *)
s := s * t;            (* intersection: {1, 2, 3} *)

IF 2 IN s THEN
  s := s - {2}         (* remove element 2 *)
END;

INCL(s, 7);            (* add element 7 *)
EXCL(s, 1);            (* remove element 1 *)
```

## Notes

- BITSET is the standard set type over the range of a machine word.
- `INCL(s, n)` and `EXCL(s, n)` add and remove a single element efficiently.
- The `IN` operator tests membership: `n IN s` is TRUE if n is an element of s.
- Including an element outside 0..W-1 is undefined behavior.
- BITSET is the only predefined set type; user-defined sets use `SET OF` with an enumeration or subrange base.
