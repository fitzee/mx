# TYPECASE

Runtime type dispatch on REFANY values. Branches based on the concrete type of a reference. Requires `--m2plus`.

## Syntax

```modula2
TYPECASE expr OF
  Type1 (var1):
    statements
| Type2:
    statements
ELSE
  statements
END;
```

## Notes

- `expr` must be of type REFANY (or a compatible reference type).
- Each branch tests whether the runtime type is or inherits from the given type. If it matches, the optional variable is bound with the narrowed type.
- The variable binding `(var)` is optional. Without it, the branch still matches but no named binding is introduced.
- The ELSE branch handles values that do not match any listed type, including NIL.
- Branches are tested in order; the first match executes.
- TYPECASE is the safe way to narrow REFANY to a concrete REF or OBJECT type.
- Subtype matching: a value whose type inherits from the branch type will match. Place more specific subtypes before parent types.

## Implementation

Each REF and OBJECT allocation is prefixed with an `M2_RefHeader` containing a pointer to a `M2_TypeDesc` descriptor. TYPECASE uses `M2_ISA` to walk the parent chain for subtype-aware matching, with a depth field for early-out when the value cannot possibly be a subtype.

## Example

```modula2
TYPE
  IntRef  = REF INTEGER;
  RealRef = REF REAL;

PROCEDURE Print(x: REFANY);
BEGIN
  TYPECASE x OF
    IntRef (v):
      WriteInt(v^, 0); WriteLn
  | RealRef:
      WriteString("a real"); WriteLn
  ELSE
    WriteString("unknown type"); WriteLn
  END;
END Print;
```
