# NEW

```modula2
NEW(p)
```

Allocate heap memory for the variable pointed to by `p`. After the call, `p` points to a newly allocated block of memory whose size matches the type that `p` references.

`p` must be a pointer type declared with `POINTER TO T`.

## Example

```modula2
TYPE NodePtr = POINTER TO Node;
     Node = RECORD value: INTEGER; next: NodePtr END;

VAR p: NodePtr;

BEGIN
  NEW(p);
  p^.value := 42;
  p^.next := NIL;
END
```

## Notes

- For `POINTER TO` types: equivalent to `malloc(sizeof(T))` in C.
- For M2+ `REF` and `OBJECT` types: calls `M2_ref_alloc` which prepends a hidden `M2_RefHeader` (type descriptor pointer) before the payload. The returned pointer points to the payload, not the header, so dereference with `^` works normally.
- The allocated memory is uninitialized. Fields should be assigned before use.
- Use `DISPOSE` to free memory allocated by `NEW`.
- If allocation fails, the program terminates with an error message.
