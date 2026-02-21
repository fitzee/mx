# HALT

```modula2
HALT
```

Terminate the program immediately. Control does not return to the caller.

`HALT` takes no arguments and does not return a value.

## Example

```modula2
PROCEDURE CheckInput(n: INTEGER);
BEGIN
  IF n < 0 THEN
    HALT
  END;
END CheckInput;
```

## Notes

- `HALT` is a proper procedure (statement), not a function.
- Typically implemented as a call to `exit()` or `abort()` in C.
- The exit status returned to the operating system is implementation-defined.
- Use `HALT` for unrecoverable error conditions or assertion-style checks.
