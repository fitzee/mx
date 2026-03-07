# Threads

## Why
Provides mutex, condition variable, and thread spawning primitives via a thin pthreads wrapper. All operations delegate to a C shim so the Modula-2 side stays minimal. Requires `-lpthread` at link time.

## Types

- **Mutex** (ADDRESS) -- Opaque mutex handle.
- **Cond** (ADDRESS) -- Opaque condition variable handle.
- **ThreadProc** -- `PROCEDURE(ADDRESS)` -- Entry point for a spawned thread.

## Procedures

### Mutex

- `PROCEDURE MutexInit(VAR m: Mutex)`
  Allocate and initialise a mutex.

- `PROCEDURE MutexDestroy(VAR m: Mutex)`
  Destroy a mutex and free its resources.

- `PROCEDURE MutexLock(m: Mutex)`
  Acquire the mutex. Blocks until available.

- `PROCEDURE MutexUnlock(m: Mutex)`
  Release the mutex.

### Condition Variable

- `PROCEDURE CondInit(VAR c: Cond)`
  Allocate and initialise a condition variable.

- `PROCEDURE CondDestroy(VAR c: Cond)`
  Destroy a condition variable and free its resources.

- `PROCEDURE CondWait(c: Cond; m: Mutex)`
  Atomically release the mutex and wait on the condition. Re-acquires the mutex on wake.

- `PROCEDURE CondSignal(c: Cond)`
  Wake one thread waiting on the condition.

- `PROCEDURE CondBroadcast(c: Cond)`
  Wake all threads waiting on the condition.

### Thread

- `PROCEDURE SpawnThread(proc: ThreadProc; arg: ADDRESS)`
  Create and start a new thread running proc(arg). The thread is detached.

## Example

```modula2
MODULE ThreadDemo;

FROM SYSTEM IMPORT ADDRESS;
FROM Threads IMPORT Mutex, MutexInit, MutexLock, MutexUnlock,
                     SpawnThread, ThreadProc;
FROM InOut IMPORT WriteString, WriteLn;

VAR
  mtx: Mutex;

  PROCEDURE Worker(arg: ADDRESS);
  BEGIN
    MutexLock(mtx);
    WriteString("hello from thread"); WriteLn;
    MutexUnlock(mtx)
  END Worker;

BEGIN
  MutexInit(mtx);
  SpawnThread(Worker, NIL);
END ThreadDemo.
```
