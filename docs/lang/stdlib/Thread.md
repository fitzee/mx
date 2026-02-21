# Thread

Thread creation and management module. Provides procedures for
spawning, joining, and identifying threads. Requires `--m2plus`
and links with pthreads.

## Exported Types

```modula2
TYPE Thread;  (* opaque thread handle *)
TYPE PROC = PROCEDURE;
```

## Exported Procedures

```modula2
PROCEDURE Fork(proc: PROC): Thread;
PROCEDURE Join(t: Thread);
PROCEDURE Self(): Thread;
```

## Notes

- `Fork` creates a new thread that executes the given parameterless
  procedure and returns a handle to the new thread.
- `Join` blocks the calling thread until thread `t` completes.
- `Self` returns the handle of the calling thread.
- Importing Thread causes the compiler to emit `M2_USE_THREADS`
  and link with pthreads.

## Example

```modula2
MODULE ThreadDemo;
FROM Thread IMPORT Fork, Join, Thread;
FROM InOut IMPORT WriteString, WriteLn;

PROCEDURE Worker;
BEGIN
  WriteString("Hello from worker"); WriteLn;
END Worker;

VAR t: Thread;
BEGIN
  t := Fork(Worker);
  WriteString("Hello from main"); WriteLn;
  Join(t);
END ThreadDemo.
```
