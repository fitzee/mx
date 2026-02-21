# EventLoop

Single-threaded event loop integrating I/O polling, timers, and microtask scheduling. This is the main orchestrator for asynchronous Modula-2 applications.

## Why use an event loop?

An event loop provides a single-threaded concurrency model: instead of blocking on I/O or spinning in sleep loops, the application registers interest in events (fd readiness, timer expiry) and the loop dispatches callbacks when those events occur. This avoids the complexity of threads while supporting high-concurrency workloads like network servers.

## Architecture

Each `RunOnce` iteration performs these steps in order:

1. Compute poll timeout from the nearest timer deadline
2. Poll for I/O events (or sleep until timeout)
3. Dispatch watcher callbacks inline (fd readiness)
4. Tick expired timers (enqueue callbacks on scheduler)
5. Pump scheduler (drain microtask queue)

The loop exits when `Stop` is called or when no watchers, timers, or scheduled tasks remain.

## Types

**`Loop`** -- Opaque handle to an event loop instance.

```modula2
TYPE Loop = ADDRESS;
```

**`WatcherProc`** -- Callback for fd readiness events:

```modula2
TYPE WatcherProc = PROCEDURE(INTEGER, INTEGER, ADDRESS);
(* Parameters: fd, readyEvents, userData *)
```

**`Status`** -- Operation result:

| Value           | Meaning                                    |
|-----------------|--------------------------------------------|
| `OK`            | Operation succeeded.                       |
| `Invalid`       | Bad argument (NIL loop, unknown fd).       |
| `SysError`      | OS-level error from poller.                |
| `PoolExhausted` | No room for more watchers or timers.       |

## Procedures

### Create

```modula2
PROCEDURE Create(VAR out: Loop): Status;
```

Creates an event loop with an internal Poller, TimerQueue (256 slots), and Scheduler (1024-entry ring buffer). Returns `PoolExhausted` if allocation fails.

### Destroy

```modula2
PROCEDURE Destroy(VAR lp: Loop): Status;
```

Destroys the event loop and all owned resources. Sets `lp` to `NIL`.

### SetTimeout

```modula2
PROCEDURE SetTimeout(lp: Loop; delayMs: INTEGER;
                     cb: TaskProc; user: ADDRESS;
                     VAR id: TimerId): Status;
```

Schedule a one-shot timer. After `delayMs` milliseconds, `cb(user)` is enqueued on the internal scheduler. The returned `id` can be used with `CancelTimer`.

### SetInterval

```modula2
PROCEDURE SetInterval(lp: Loop; intervalMs: INTEGER;
                      cb: TaskProc; user: ADDRESS;
                      VAR id: TimerId): Status;
```

Schedule a repeating timer. Fires every `intervalMs` milliseconds.

### CancelTimer

```modula2
PROCEDURE CancelTimer(lp: Loop; id: TimerId): Status;
```

Cancel a pending timer. Idempotent -- cancelling an already-fired timer returns `OK`.

### WatchFd

```modula2
PROCEDURE WatchFd(lp: Loop; fd, events: INTEGER;
                  cb: WatcherProc; user: ADDRESS): Status;
```

Register a file descriptor for readiness events. `events` is a bitmask of `Poller.EvRead` and `Poller.EvWrite`. The callback `cb(fd, readyEvents, user)` is called **inline** during `RunOnce` -- not via the scheduler.

### ModifyFd

```modula2
PROCEDURE ModifyFd(lp: Loop; fd, events: INTEGER): Status;
```

Change the interest set for an already-watched fd.

### UnwatchFd

```modula2
PROCEDURE UnwatchFd(lp: Loop; fd: INTEGER): Status;
```

Stop watching a file descriptor. The fd is removed from the poller.

### Enqueue

```modula2
PROCEDURE Enqueue(lp: Loop; cb: TaskProc; user: ADDRESS): Status;
```

Enqueue a microtask for execution during the next scheduler pump.

### GetScheduler

```modula2
PROCEDURE GetScheduler(lp: Loop): Scheduler;
```

Return the underlying Scheduler handle. Use this when creating Promises that need a scheduler reference.

### RunOnce

```modula2
PROCEDURE RunOnce(lp: Loop): BOOLEAN;
```

Execute one iteration of the event loop. Returns `TRUE` if there is still work to do.

### Run

```modula2
PROCEDURE Run(lp: Loop);
```

Run the event loop until `Stop` is called or all work completes.

### Stop

```modula2
PROCEDURE Stop(lp: Loop);
```

Signal the loop to exit after the current `RunOnce` iteration.

## Notes

- Maximum 64 concurrent fd watchers and 256 timers per loop.
- Watcher callbacks are called inline (during `RunOnce`). Timer callbacks go through the scheduler to maintain consistent ordering with Promise continuations.
- The scheduler is pumped to completion each iteration, so microtasks enqueued by timer callbacks execute in the same iteration.
- Timer timestamps use 32-bit signed integers with wrap-safe comparison, giving correct behavior for up to ~12.4 days between any two events.

## Example

```modula2
MODULE TimerDemo;

FROM SYSTEM IMPORT ADDRESS;
FROM InOut IMPORT WriteString, WriteLn;
FROM EventLoop IMPORT Loop, Create, Destroy, SetTimeout,
                      SetInterval, CancelTimer, Run, Stop;
FROM EventLoop IMPORT Status, OK;
FROM Timers IMPORT TimerId;
FROM Scheduler IMPORT TaskProc;

VAR
  loop: Loop;
  st: Status;
  tickId, stopId: TimerId;

PROCEDURE OnTick(user: ADDRESS);
BEGIN
  WriteString("tick"); WriteLn
END OnTick;

PROCEDURE OnStop(user: ADDRESS);
BEGIN
  WriteString("stopping"); WriteLn;
  Stop(loop)
END OnStop;

BEGIN
  st := Create(loop);
  st := SetInterval(loop, 500, OnTick, NIL, tickId);
  st := SetTimeout(loop, 2500, OnStop, NIL, stopId);
  Run(loop);
  st := Destroy(loop)
END TimerDemo.
```
