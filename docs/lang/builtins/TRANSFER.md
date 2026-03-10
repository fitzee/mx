# TRANSFER

```modula2
TRANSFER(from, to)
```

Transfer control from the current coroutine to another. The current coroutine's state is saved in `from`, and execution resumes in the coroutine described by `to`.

- `from` must be a `VAR` parameter of type `PROCESS`.
- `to` must be a `PROCESS` value identifying the target coroutine.

## Example

```modula2
FROM SYSTEM IMPORT PROCESS, NEWPROCESS, TRANSFER;

VAR main, co: PROCESS;
    stack: ARRAY [0..4095] OF CHAR;

PROCEDURE Worker;
BEGIN
  (* do some work *)
  TRANSFER(co, main);   (* yield back to main *)
END Worker;

BEGIN
  NEWPROCESS(Worker, ADR(stack), SIZE(stack), co);
  TRANSFER(main, co);   (* start the coroutine *)
  (* execution continues here after Worker transfers back *)
END
```

## Notes

- `TRANSFER` is a PIM4 pervasive procedure imported from `SYSTEM`.
- After the call, `from` holds the saved state of the calling coroutine so that it can be resumed later.
- `TRANSFER` is the only way to switch between coroutines created with `NEWPROCESS`.
- **Deprecated in mx**: coroutine primitives are provided for PIM4 compatibility but their use is discouraged. Prefer the `Thread` module (with `--m2plus`) for concurrent programming.
- See also `NEWPROCESS` for creating coroutines and `IOTRANSFER` for interrupt-driven transfers.
