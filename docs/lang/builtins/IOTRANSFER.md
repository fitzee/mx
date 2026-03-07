# IOTRANSFER

```modula2
IOTRANSFER(from, to, vec)
```

Transfer control from the current coroutine to another, arranging for automatic re-transfer when the interrupt identified by `vec` occurs.

- `from` must be a `VAR` parameter of type `PROCESS`.
- `to` must be a `PROCESS` value identifying the target coroutine.
- `vec` must be a `CARDINAL` identifying the interrupt vector.

## Example

```modula2
FROM SYSTEM IMPORT PROCESS, NEWPROCESS, TRANSFER, IOTRANSFER;

VAR main, handler: PROCESS;
    stack: ARRAY [0..4095] OF CHAR;

PROCEDURE InterruptHandler;
BEGIN
  LOOP
    (* wait for interrupt on vector 4 *)
    IOTRANSFER(handler, main, 4);
    (* handle the interrupt *)
  END;
END InterruptHandler;

BEGIN
  NEWPROCESS(InterruptHandler, ADR(stack), SIZE(stack), handler);
  TRANSFER(main, handler);
END
```

## Notes

- `IOTRANSFER` is a PIM4 pervasive procedure imported from `SYSTEM`.
- After the call, control passes to the coroutine `to`. When interrupt `vec` fires, control automatically returns to the coroutine that called `IOTRANSFER`.
- The semantics of interrupt vectors are hardware- and OS-dependent.
- **Deprecated in m2c**: coroutine and interrupt primitives are provided for PIM4 compatibility but their use is discouraged. Modern systems should use signal handlers or the `Thread` module (with `--m2plus`) instead.
- See also `NEWPROCESS` and `TRANSFER` for the other coroutine primitives.
