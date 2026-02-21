# EXCEPT

Exception handler clause inside a TRY block. Catches exceptions
raised during execution of the TRY body. Requires `--m2plus`.

## Syntax

```modula2
(* Catch-all handler *)
TRY
  statements
EXCEPT
  handler-statements
END;

(* Named exception handler *)
TRY
  statements
EXCEPT ExceptionName DO
  handler-statements
END;
```

## Notes

- A catch-all `EXCEPT` (without a name) handles any exception.
- A named `EXCEPT ExName DO` handler catches only the specified
  exception; unmatched exceptions propagate to outer frames.
- The `DO` keyword is required after a named exception.
- Multiple named handlers are not supported in a single TRY block;
  use nested TRY blocks or a catch-all with conditional logic.

## Example

```modula2
EXCEPTION NotFound;
EXCEPTION Overflow;

TRY
  Lookup(key);
EXCEPT NotFound DO
  WriteString("Key not found"); WriteLn;
END;
```
