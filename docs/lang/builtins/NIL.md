# NIL

```modula2
NIL
```

The null pointer constant. `NIL` is compatible with any pointer type and represents a pointer that does not reference any valid memory location.

## Example

```modula2
TYPE NodePtr = POINTER TO Node;
     Node = RECORD value: INTEGER; next: NodePtr END;

VAR head: NodePtr;

BEGIN
  head := NIL;
  NEW(head);
  head^.next := NIL;
  (* traverse until NIL *)
  WHILE head # NIL DO
    head := head^.next;
  END;
END
```

## Notes

- `NIL` is assignment-compatible with all pointer types.
- Dereferencing `NIL` (e.g., `NIL^`) is undefined behavior and typically causes a runtime crash.
- Use `p # NIL` to test whether a pointer is valid before dereferencing.
- `NIL` is a pervasive constant; it does not need to be imported.
