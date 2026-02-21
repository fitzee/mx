# Storage

Dynamic memory allocation module. Provides the low-level allocation
and deallocation procedures used internally by `NEW` and `DISPOSE`.

## Exported Procedures

```modula2
PROCEDURE ALLOCATE(VAR p: ADDRESS; size: CARDINAL);
PROCEDURE DEALLOCATE(VAR p: ADDRESS; size: CARDINAL);
```

## Notes

- `ALLOCATE` allocates `size` bytes and stores the pointer in `p`.
- `DEALLOCATE` frees the memory at `p` and sets `p` to `NIL`.
- The built-in `NEW(p)` expands to `ALLOCATE(p, TSIZE(T))` where
  `T` is the base type of the pointer `p`.
- The built-in `DISPOSE(p)` expands to `DEALLOCATE(p, TSIZE(T))`.
- Import `Storage` (or at least `ALLOCATE`) to use `NEW`/`DISPOSE`.

## Example

```modula2
MODULE StorageDemo;
FROM Storage IMPORT ALLOCATE, DEALLOCATE;
TYPE
  NodePtr = POINTER TO Node;
  Node = RECORD
    value: INTEGER;
    next: NodePtr;
  END;
VAR p: NodePtr;
BEGIN
  NEW(p);
  p^.value := 42;
  p^.next := NIL;
  DISPOSE(p);
END StorageDemo.
```
