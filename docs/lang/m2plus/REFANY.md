# REFANY

Universal reference type. Can hold any REF value regardless of its
target type. Requires `--m2plus`.

## Syntax

```modula2
VAR x: REFANY;
```

## Notes

- Any `REF T` value can be assigned to a `REFANY` variable.
- REFANY values carry a runtime type tag that identifies the
  original concrete type.
- Use TYPECASE to dispatch on the runtime type of a REFANY value.
- REFANY cannot be dereferenced directly -- you must narrow it
  to a specific REF type via TYPECASE first.
- Useful for building heterogeneous data structures such as
  collections that store mixed types.

## Example

```modula2
TYPE
  IntRef = REF RECORD i: INTEGER END;
  StrRef = REF RECORD s: ARRAY [0..31] OF CHAR END;
VAR
  any: REFANY;
  ir: IntRef;
BEGIN
  NEW(ir);
  ir^.i := 10;
  any := ir;
  TYPECASE any OF
  | IntRef(v) => WriteInt(v^.i, 0); WriteLn;
  | StrRef(v) => WriteString(v^.s); WriteLn;
  END;
END RefAnyDemo.
```
