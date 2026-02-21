# Condition

Condition variable module. Provides procedures for thread
synchronization using condition variables paired with mutexes.
Requires `--m2plus` and links with pthreads.

## Exported Types

```modula2
TYPE Condition;  (* opaque condition variable handle *)
```

## Exported Procedures

```modula2
PROCEDURE Create(): Condition;
PROCEDURE Wait(c: Condition; m: Mutex);
PROCEDURE Signal(c: Condition);
PROCEDURE Broadcast(c: Condition);
PROCEDURE Destroy(c: Condition);
```

## Notes

- `Wait` atomically releases the mutex `m` and blocks until the
  condition is signaled, then reacquires `m` before returning.
- `Signal` wakes one thread waiting on the condition.
- `Broadcast` wakes all threads waiting on the condition.
- Always check the wait predicate in a loop, as spurious wakeups
  are possible.

## Example

```modula2
MODULE CondDemo;
FROM Mutex IMPORT Mutex, Create, Lock, Unlock;
FROM Condition IMPORT Condition, Wait, Signal;

VAR
  mu: Mutex;
  cv: Condition;
  ready: BOOLEAN;

PROCEDURE Producer;
BEGIN
  Lock(mu);
  ready := TRUE;
  Signal(cv);
  Unlock(mu);
END Producer;

PROCEDURE Consumer;
BEGIN
  Lock(mu);
  WHILE NOT ready DO Wait(cv, mu) END;
  Unlock(mu);
END Consumer;

END CondDemo.
```
