# POINTER

Pointer type. Points to a dynamically allocated value. Dereferenced with `^`.
Created with NEW, released with DISPOSE, tested against NIL.

```modula2
TYPE P = POINTER TO T;
```

## Example

```modula2
TYPE IntPtr = POINTER TO INTEGER;
VAR p: IntPtr;

NEW(p);
p^ := 42;
WriteInt(p^, 1);
DISPOSE(p);
```
