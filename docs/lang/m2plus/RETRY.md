# RETRY

Restarts the TRY block from the beginning. Only valid inside an
EXCEPT handler. Requires `--m2plus`.

## Syntax

```modula2
TRY
  statements
EXCEPT
  (* fix the problem, then: *)
  RETRY;
END;
```

## Notes

- RETRY transfers control back to the first statement of the
  enclosing TRY block.
- It is a compile-time error to use RETRY outside of an EXCEPT
  handler.
- Use RETRY for recovery patterns where the handler can correct
  the condition that caused the exception and re-attempt the
  protected operation.
- Be careful to avoid infinite loops -- ensure the retry condition
  will eventually succeed or add a retry counter.

## Example

```modula2
EXCEPTION Timeout;
VAR attempts: INTEGER;

BEGIN
  attempts := 0;
  TRY
    attempts := attempts + 1;
    Connect(server);
  EXCEPT Timeout DO
    IF attempts < 3 THEN
      RETRY;
    ELSE
      WriteString("Failed after 3 attempts"); WriteLn;
    END;
  END;
END Reconnect.
```
