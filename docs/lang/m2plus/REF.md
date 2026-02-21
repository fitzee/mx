# REF

Heap-allocated reference type. Similar to POINTER TO but with
managed semantics. Requires `--m2plus`.

## Syntax

```modula2
TYPE T = REF MyRecord;
```

## Notes

- `REF T` creates a reference to a heap-allocated value of type T.
- Values are created with `NEW(r)` which allocates via malloc.
- Dereference with `r^` to access the underlying value.
- REF types carry a runtime type tag, enabling TYPECASE dispatch.
- Optional Boehm GC support: compile with `-DM2_USE_GC` to use
  garbage collection instead of manual deallocation.
- REF is distinct from `POINTER TO` -- REF values are tagged and
  compatible with REFANY; pointers are not.

## Example

```modula2
TYPE
  IntRef = REF RECORD value: INTEGER END;
VAR
  r: IntRef;
BEGIN
  NEW(r);
  r^.value := 42;
  WriteInt(r^.value, 0); WriteLn;
END RefDemo.
```
