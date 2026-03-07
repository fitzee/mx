# NEWPROCESS

```modula2
NEWPROCESS(proc, workspace, size, new)
```

Create a new coroutine from procedure `proc`, using a memory area `workspace` of `size` bytes, and store the resulting process descriptor in `new`.

- `proc` must be a parameterless procedure (`PROCEDURE`).
- `workspace` must be an `ADDRESS` pointing to a block of memory.
- `size` must be a `CARDINAL` giving the size of the workspace in bytes.
- `new` must be a `VAR` parameter of type `PROCESS` (also called `ADDRESS` in some implementations).

## Example

```modula2
FROM SYSTEM IMPORT ADDRESS, PROCESS, NEWPROCESS, TRANSFER;

VAR main, co: PROCESS;
    stack: ARRAY [0..4095] OF CHAR;

PROCEDURE Worker;
BEGIN
  (* coroutine body *)
  TRANSFER(co, main);
END Worker;

BEGIN
  NEWPROCESS(Worker, ADR(stack), SIZE(stack), co);
  TRANSFER(main, co);
END
```

## Notes

- `NEWPROCESS` is a PIM4 pervasive procedure imported from `SYSTEM`.
- The workspace must be large enough for the coroutine's stack usage; insufficient space causes undefined behavior.
- The created coroutine does not begin executing until control is transferred to it via `TRANSFER`.
- **Deprecated in m2c**: coroutine primitives are provided for PIM4 compatibility but their use is discouraged. Prefer the `Thread` module (with `--m2plus`) for concurrent programming.
