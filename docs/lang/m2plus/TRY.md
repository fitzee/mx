# TRY

Exception handling block. Begins a protected region of code whose
exceptions can be caught by an `EXCEPT` clause or cleaned up by a
`FINALLY` clause. Requires the `--m2plus` compiler flag.

## Syntax

```modula2
TRY
  statements
EXCEPT
  handler
END;

TRY
  statements
FINALLY
  cleanup
END;
```

## Notes

- A TRY block must be paired with either `EXCEPT` or `FINALLY`,
  not both in the same block.
- If an exception is raised inside the TRY body, control transfers
  to the matching EXCEPT handler.
- Implementation uses setjmp/longjmp with a stack-based exception
  frame (M2_TRY/M2_CATCH/M2_ENDTRY macros).

## Example

```modula2
EXCEPTION DivByZero;

PROCEDURE SafeDiv(a, b: INTEGER): INTEGER;
VAR result: INTEGER;
BEGIN
  TRY
    IF b = 0 THEN RAISE DivByZero END;
    result := a DIV b;
  EXCEPT DivByZero DO
    result := 0;
  END;
  RETURN result;
END SafeDiv;
```
