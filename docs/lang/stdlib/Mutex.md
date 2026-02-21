# Mutex

Mutual exclusion module. Provides procedures for creating and
operating on mutex locks for thread synchronization. Requires
`--m2plus` and links with pthreads.

## Exported Types

```modula2
TYPE Mutex;  (* opaque mutex handle *)
```

## Exported Procedures

```modula2
PROCEDURE Create(): Mutex;
PROCEDURE Lock(m: Mutex);
PROCEDURE Unlock(m: Mutex);
PROCEDURE Destroy(m: Mutex);
```

## Notes

- `Create` allocates and initializes a new mutex.
- `Lock` acquires the mutex, blocking if it is already held.
- `Unlock` releases the mutex.
- `Destroy` frees the mutex resources.
- Prefer the `LOCK` statement over manual Lock/Unlock calls, as
  LOCK guarantees the mutex is released even on exceptions.

## Example

```modula2
MODULE MutexDemo;
FROM Mutex IMPORT Mutex, Create, Lock, Unlock, Destroy;

VAR
  mu: Mutex;
  shared: INTEGER;

PROCEDURE SafeIncrement;
BEGIN
  Lock(mu);
  shared := shared + 1;
  Unlock(mu);
END SafeIncrement;

BEGIN
  mu := Create();
  shared := 0;
  SafeIncrement();
  Destroy(mu);
END MutexDemo.
```
