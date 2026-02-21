# Scheduler

Microtask queue for single-threaded promise execution. All promise callbacks are dispatched through a Scheduler -- never called inline. The caller owns the Scheduler instance; there is no hidden global state.

## Why use a scheduler?

Promises and futures need a place to queue pending callbacks so they run at a controlled time rather than recursively during settlement. The Scheduler provides this: a simple ring-buffer task queue that the application drains via `SchedulerPump`. This keeps the execution model predictable and single-threaded, avoids unbounded recursion, and gives the caller full control over when continuations execute.

## Types

**`Status`** -- Enumeration returned by every procedure in both the Scheduler and Promise modules:

| Value            | Meaning                                              |
|------------------|------------------------------------------------------|
| `OK`             | Operation succeeded.                                 |
| `Invalid`        | Bad argument (e.g., NIL scheduler, zero capacity).   |
| `OutOfMemory`    | Pool or queue is full.                               |
| `AlreadySettled` | Promise was already resolved or rejected.            |

**`TaskProc`** -- Callback procedure type:

```modula2
TYPE TaskProc = PROCEDURE(ADDRESS);
```

A procedure that takes a single opaque `ADDRESS` argument. This is the unit of work the scheduler dispatches.

**`Scheduler`** -- Opaque handle to a scheduler instance. Internally a pointer to a heap-allocated ring-buffer record.

```modula2
TYPE Scheduler = ADDRESS;
```

## Procedures

### SchedulerCreate

```modula2
PROCEDURE SchedulerCreate(capacity: CARDINAL;
                          VAR out: Scheduler): Status;
```

Creates a scheduler with room for up to `capacity` queued tasks. Capacity is clamped to an internal maximum of 4096. Returns `Invalid` if `capacity` is 0. Returns `OutOfMemory` if heap allocation fails. On success, `out` receives a valid scheduler handle.

```modula2
VAR sched: Scheduler; st: Status;
...
st := SchedulerCreate(1024, sched);
IF st # OK THEN (* handle error *) END;
```

### SchedulerDestroy

```modula2
PROCEDURE SchedulerDestroy(VAR s: Scheduler): Status;
```

Destroys the scheduler and releases its memory. Sets `s` to `NIL`. Returns `Invalid` if `s` is already `NIL`. Any tasks still in the queue are silently discarded.

### SchedulerEnqueue

```modula2
PROCEDURE SchedulerEnqueue(s: Scheduler;
                           cb: TaskProc;
                           user: ADDRESS): Status;
```

Adds a callback to the tail of the queue. When later dispatched by `SchedulerPump`, the scheduler calls `cb(user)`. Returns `OutOfMemory` if the queue is full. Returns `Invalid` if `s` is `NIL`.

Callbacks may safely enqueue further tasks during execution -- the scheduler processes them in FIFO order across pump cycles.

### SchedulerPump

```modula2
PROCEDURE SchedulerPump(s: Scheduler;
                        maxSteps: CARDINAL;
                        VAR didWork: BOOLEAN): Status;
```

Runs up to `maxSteps` queued callbacks. Sets `didWork` to `TRUE` if at least one callback executed, `FALSE` if the queue was empty. Returns `Invalid` if `s` is `NIL`.

A typical "drain all pending work" loop:

```modula2
PROCEDURE PumpAll;
VAR dw: BOOLEAN;
BEGIN
  dw := TRUE;
  WHILE dw DO
    st := SchedulerPump(sched, 200, dw)
  END
END PumpAll;
```

## Notes

- The scheduler uses a fixed-size ring buffer internally (max 4096 entries). For most promise workloads this is more than sufficient -- each settlement enqueues one callback per attached continuation.
- Callbacks execute in strict FIFO order. A callback that enqueues more work does not cause that work to run immediately; it waits for the next iteration of the pump loop.
- The scheduler is not thread-safe. Use one scheduler per thread, or protect access with a mutex.

## Example

```modula2
MODULE SchedDemo;

FROM InOut IMPORT WriteString, WriteInt, WriteLn;
FROM SYSTEM IMPORT ADDRESS;
FROM Scheduler IMPORT Status, Scheduler, TaskProc, OK,
                      SchedulerCreate, SchedulerDestroy,
                      SchedulerEnqueue, SchedulerPump;

PROCEDURE PrintTask(data: ADDRESS);
BEGIN
  WriteString("Task executed"); WriteLn
END PrintTask;

VAR
  sched: Scheduler;
  st: Status;
  didWork: BOOLEAN;
BEGIN
  st := SchedulerCreate(64, sched);
  st := SchedulerEnqueue(sched, PrintTask, NIL);
  st := SchedulerEnqueue(sched, PrintTask, NIL);
  st := SchedulerPump(sched, 10, didWork);
  (* Output: Task executed (twice) *)
  st := SchedulerDestroy(sched)
END SchedDemo.
```
