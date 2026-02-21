# TSIZE

```modula2
TSIZE(T): CARDINAL
```

Return the number of storage units (bytes) required for a value of type `T`. The result is a compile-time constant.

`T` must be a type identifier.

## Example

```modula2
TYPE Rec = RECORD x, y: INTEGER END;

BEGIN
  WriteCard(TSIZE(INTEGER), 0);  (* typically 4 *)
  WriteCard(TSIZE(REAL), 0);     (* typically 4 or 8 *)
  WriteCard(TSIZE(Rec), 0);      (* typically 8 *)
END
```

## Notes

- `TSIZE` operates on types, while `SIZE` operates on variables.
- The result is a compile-time constant and may be used in constant expressions.
- Imported from the `SYSTEM` module in strict PIM4; many implementations make it pervasive.
- The value includes any internal padding for alignment.
