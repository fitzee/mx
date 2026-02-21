# LOCK

Mutex lock statement. Acquires a mutex, executes the body, and
releases the mutex on exit, even if an exception is raised.
Requires `--m2plus`.

## Syntax

```modula2
LOCK mutex DO
  statements
END;
```

## Notes

- `mutex` must be a value of type `Mutex.Mutex` (from the Mutex
  stdlib module).
- The mutex is acquired before the body executes and released
  after, providing safe critical sections.
- If an exception is raised inside the LOCK body, the mutex is
  still released before the exception propagates.
- Using LOCK automatically causes the compiler to emit the
  `M2_USE_THREADS` define and link with pthreads.

## Example

```modula2
FROM Mutex IMPORT Mutex, Create, Destroy;

VAR
  mu: Mutex;
  counter: INTEGER;

PROCEDURE Increment;
BEGIN
  LOCK mu DO
    counter := counter + 1;
  END;
END Increment;

BEGIN
  mu := Create();
  counter := 0;
  Increment();
  Destroy(mu);
END LockDemo.
```
