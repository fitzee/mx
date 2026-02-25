# EXCEPTION

Declares a named exception at module level. Named exceptions are used with RAISE and caught with EXCEPT. Requires `--m2plus`.

## Syntax

```modula2
EXCEPTION MyError;
```

## Notes

- Exception declarations appear in the declaration section of a module, alongside TYPE, VAR, and PROCEDURE declarations.
- Each EXCEPTION declaration creates a unique exception identity that can be matched by name in EXCEPT handlers.
- Exceptions can be exported from definition modules so that clients can catch them.
- The compiler generates a static exception descriptor for each declared exception.

## Example

```modula2
MODULE Errors;

EXCEPTION NotFound;
EXCEPTION OutOfRange;

PROCEDURE Check(i: INTEGER);
BEGIN
  IF i < 0 THEN
    RAISE OutOfRange;
  END;
END Check;

BEGIN
  TRY
    Check(-5);
  EXCEPT OutOfRange DO
    WriteString("Out of range"); WriteLn;
  END;
END Errors.
```
