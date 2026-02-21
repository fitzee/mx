# ADR

```modula2
ADR(x): ADDRESS
```

Return the memory address of the variable `x`. The result is of type `ADDRESS`, which is defined in the `SYSTEM` module.

`x` must be a variable (not a constant or expression).

## Example

```modula2
FROM SYSTEM IMPORT ADR, ADDRESS;

VAR buf: ARRAY [0..255] OF CHAR;
    ptr: ADDRESS;

BEGIN
  ptr := ADR(buf);
END
```

## Notes

- `ADR` must be imported from the `SYSTEM` module.
- `ADDRESS` is compatible with all pointer types in the `SYSTEM` module.
- Used for low-level programming, FFI, and interfacing with system calls.
- Applying `ADR` to an open array parameter yields the pointer to the array data (the parameter is already a pointer internally).
- Code using `ADR` is inherently non-portable.
