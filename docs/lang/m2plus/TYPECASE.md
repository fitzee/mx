# TYPECASE

Runtime type dispatch on REFANY values. Branches based on the
concrete type of a reference. Requires `--m2plus`.

## Syntax

```modula2
TYPECASE expr OF
| Type1(var1) => statements
| Type2(var2) => statements
ELSE
  statements
END;
```

## Notes

- `expr` must be of type REFANY (or a compatible reference type).
- Each branch tests whether the runtime type matches the given REF
  type. If it matches, the value is bound to the named variable
  with the narrowed type.
- The ELSE branch handles values that do not match any listed type.
- Branches are tested in order; the first match executes.
- TYPECASE is the safe way to narrow REFANY to a concrete REF type.

## Example

```modula2
TYPE
  IntRef  = REF RECORD i: INTEGER END;
  RealRef = REF RECORD r: REAL END;

PROCEDURE Print(x: REFANY);
BEGIN
  TYPECASE x OF
  | IntRef(v)  => WriteInt(v^.i, 0); WriteLn;
  | RealRef(v) => WriteReal(v^.r, 10); WriteLn;
  ELSE
    WriteString("Unknown type"); WriteLn;
  END;
END Print;
```
