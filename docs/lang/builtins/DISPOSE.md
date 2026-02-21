# DISPOSE

```modula2
DISPOSE(p)
```

Deallocate the heap memory pointed to by `p`. After the call, `p` becomes undefined and must not be dereferenced.

`p` must be a pointer type previously allocated with `NEW`.

## Example

```modula2
VAR p: POINTER TO INTEGER;

BEGIN
  NEW(p);
  p^ := 10;
  DISPOSE(p);
  (* p is now invalid; do not use *)
END
```

## Notes

- Equivalent to `free(p)` in C.
- Passing `NIL` to `DISPOSE` is implementation-defined in PIM4.
- Double-disposing the same pointer leads to undefined behavior.
- `DISPOSE` does not set `p` to `NIL` automatically.
