# REF

Heap-allocated reference type. Similar to POINTER TO but with managed semantics. Requires `--m2plus`.

## Syntax

```modula2
TYPE T = REF MyRecord;
```

## Notes

- `REF T` creates a reference to a heap-allocated value of type T.
- Values are created with `NEW(r)` which allocates via `M2_ref_alloc`. This prepends a hidden `M2_RefHeader` containing a type descriptor pointer before the payload, then returns a pointer to the payload.
- Dereference with `r^` to access the underlying value.
- Each REF type gets a unique `M2_TypeDesc` with a type ID, name, parent pointer, and depth. This enables TYPECASE dispatch via `M2_ISA` (parent-chain walk with depth early-out).
- Optional Boehm GC support: compile with `-DM2_USE_GC` to use garbage collection instead of manual deallocation. Falls back to `malloc`/`free` automatically if `gc/gc.h` is not installed.
- REF is distinct from `POINTER TO` -- REF values carry a type descriptor header and are compatible with REFANY; plain pointers are not.

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
