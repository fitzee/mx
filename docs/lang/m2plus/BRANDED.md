# BRANDED

Branded reference type. Creates a distinct type from `REF T`,
preventing accidental type compatibility. Requires `--m2plus`.

## Syntax

```modula2
TYPE T = BRANDED REF MyRecord;
```

## Notes

- Two `BRANDED REF` types with the same target type are
  incompatible -- they cannot be assigned to each other.
- Without BRANDED, two `REF SameRecord` types are structurally
  compatible and freely interchangeable.
- Branding is useful for enforcing abstraction boundaries where
  different modules should not share reference types even if the
  underlying structure is identical.
- BRANDED REF values are still assignment-compatible with REFANY.

## Example

```modula2
TYPE
  Handle = BRANDED REF RECORD fd: INTEGER END;
  Token  = BRANDED REF RECORD fd: INTEGER END;
VAR
  h: Handle;
  t: Token;
BEGIN
  NEW(h);
  h^.fd := 1;
  (* t := h;  <-- compile error: incompatible branded types *)
END BrandedDemo.
```
