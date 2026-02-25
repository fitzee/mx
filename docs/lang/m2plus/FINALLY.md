# FINALLY

Cleanup clause in a TRY block. The FINALLY body is always executed regardless of whether an exception was raised. Requires `--m2plus`.

## Syntax

```modula2
TRY
  statements
FINALLY
  cleanup-statements
END;
```

## Notes

- FINALLY guarantees that cleanup code runs even if the TRY body raises an exception or returns early.
- A TRY block uses either EXCEPT or FINALLY, not both. To combine exception handling with cleanup, nest two TRY blocks.
- After the FINALLY body executes, if an exception was active it continues propagating to the next outer handler.

## Example

```modula2
PROCEDURE ProcessFile;
VAR f: File;
BEGIN
  f := Open("data.txt");
  TRY
    ReadData(f);
  FINALLY
    Close(f);
  END;
END ProcessFile;
```
