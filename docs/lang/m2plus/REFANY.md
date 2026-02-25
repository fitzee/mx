# REFANY

Universal reference type. Can hold any REF value regardless of its target type. Requires `--m2plus`.

## Syntax

```modula2
VAR x: REFANY;
```

## Notes

- Any `REF T` or OBJECT value can be assigned to a `REFANY` variable.
- REFANY values carry a runtime type descriptor (`M2_TypeDesc`) stored in a hidden header prepended to the allocation. This identifies the concrete type and its parent chain.
- Use TYPECASE to dispatch on the runtime type of a REFANY value.
- REFANY cannot be dereferenced directly -- you must narrow it to a specific REF type via TYPECASE first.
- NIL is a valid REFANY value. TYPECASE handles it safely (falls through to ELSE).
- Useful for building heterogeneous data structures such as collections that store mixed types.
- REFANY can be used as a formal parameter type.

## Example

```modula2
TYPE
  IntRef = REF INTEGER;
  StrRef = REF ARRAY [0..31] OF CHAR;
VAR
  any: REFANY;
  ir: IntRef;
BEGIN
  NEW(ir);
  ir^ := 10;
  any := ir;
  TYPECASE any OF
    IntRef (v):
      WriteInt(v^, 0); WriteLn
  | StrRef:
      WriteString("a string"); WriteLn
  END;
END RefAnyDemo.
```
