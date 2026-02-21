# RAISE

Raises a named exception, unwinding the call stack to the nearest
matching EXCEPT handler. Requires `--m2plus`.

## Syntax

```modula2
RAISE ExceptionName;
```

## Notes

- The exception name must have been declared with an `EXCEPTION`
  declaration at module level.
- RAISE transfers control via longjmp to the nearest enclosing
  TRY/EXCEPT frame that handles the named exception (or a
  catch-all handler).
- If no handler is found, the program terminates with an unhandled
  exception error.
- RAISE can appear in any statement context -- procedures, loops,
  conditionals, etc.

## Example

```modula2
EXCEPTION InvalidInput;

PROCEDURE Validate(n: INTEGER);
BEGIN
  IF n < 0 THEN
    RAISE InvalidInput;
  END;
END Validate;

BEGIN
  TRY
    Validate(-1);
  EXCEPT InvalidInput DO
    WriteString("Bad input"); WriteLn;
  END;
END Demo.
```
