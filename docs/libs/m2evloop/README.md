# m2evloop

## Why
Single-threaded event loop integrating I/O polling, timers, and microtask scheduling for non-blocking Modula-2 programs. Layered on top of a cross-platform poller (kqueue/epoll/poll), a min-heap timer queue, and a microtask scheduler.

## Modules

### EventLoop

The top-level loop that owns a Poller, TimerQueue, and Scheduler. Each iteration computes the nearest timer deadline, polls for I/O, dispatches watcher callbacks, ticks expired timers, and pumps the scheduler microtask queue.

#### Types

| Type | Description |
|------|-------------|
| `Loop` | Opaque event loop handle (`ADDRESS`). |
| `WatcherProc` | `PROCEDURE(INTEGER, INTEGER, ADDRESS)` -- callback for fd readiness events. Parameters are the file descriptor, a bitmask of ready events, and a user context pointer. |
| `Status` | `(OK, Invalid, SysError, PoolExhausted)` -- operation result. |

#### Procedures

```modula2
PROCEDURE Create(VAR out: Loop): Status;
```
Create an event loop, allocating a Poller, TimerQueue, and Scheduler internally.

```modula2
PROCEDURE Destroy(VAR lp: Loop): Status;
```
Destroy the event loop and all owned resources. Active watchers and timers are discarded.

```modula2
PROCEDURE SetTimeout(lp: Loop; delayMs: INTEGER;
                     cb: TaskProc; user: ADDRESS;
                     VAR id: TimerId): Status;
```
Schedule a one-shot timer. `cb(user)` is enqueued on the scheduler after `delayMs` milliseconds.

```modula2
PROCEDURE SetInterval(lp: Loop; intervalMs: INTEGER;
                      cb: TaskProc; user: ADDRESS;
                      VAR id: TimerId): Status;
```
Schedule a repeating timer that fires every `intervalMs` milliseconds.

```modula2
PROCEDURE CancelTimer(lp: Loop; id: TimerId): Status;
```
Cancel a timer by id. Idempotent.

```modula2
PROCEDURE WatchFd(lp: Loop; fd, events: INTEGER;
                  cb: WatcherProc; user: ADDRESS): Status;
```
Watch a file descriptor for readiness events. `events` is a bitmask of `Poller.EvRead` / `Poller.EvWrite`. The callback is invoked inline during `RunOnce`.

```modula2
PROCEDURE ModifyFd(lp: Loop; fd, events: INTEGER): Status;
```
Change the interest set for an already-watched fd.

```modula2
PROCEDURE UnwatchFd(lp: Loop; fd: INTEGER): Status;
```
Stop watching a file descriptor.

```modula2
PROCEDURE Enqueue(lp: Loop; cb: TaskProc; user: ADDRESS): Status;
```
Enqueue a microtask for execution during the next scheduler pump.

```modula2
PROCEDURE GetScheduler(lp: Loop): Scheduler;
```
Return the underlying Scheduler handle, for use with Promise/Future APIs.

```modula2
PROCEDURE RunOnce(lp: Loop): BOOLEAN;
```
Execute one iteration. Returns `TRUE` if work remains (watchers, timers, or queued tasks).

```modula2
PROCEDURE Run(lp: Loop);
```
Run the event loop until `Stop` is called or all work completes.

```modula2
PROCEDURE Stop(lp: Loop);
```
Signal the loop to stop after the current iteration.

---

### Poller

Cross-platform fd-readiness poller wrapping kqueue (macOS/BSD), epoll (Linux), or poll (fallback).

#### Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `EvRead` | 1 | Interest in read readiness. |
| `EvWrite` | 2 | Interest in write readiness. |
| `EvError` | 4 | Error condition on fd. |
| `EvHup` | 8 | Hangup on fd. |
| `MaxEvents` | 64 | Maximum events returned per `Wait` call. |

#### Types

| Type | Description |
|------|-------------|
| `Poller` | Poller handle (`INTEGER`). |
| `PollEvent` | Record with `fd: INTEGER` and `events: INTEGER` (bitmask of `Ev*` constants). |
| `EventBuf` | `ARRAY [0..MaxEvents-1] OF PollEvent` -- buffer for `Wait` results. |
| `Status` | `(OK, SysError, Invalid)` -- operation result. |

#### Procedures

```modula2
PROCEDURE Create(VAR out: Poller): Status;
```
Create a new poller instance.

```modula2
PROCEDURE Destroy(VAR p: Poller): Status;
```
Destroy a poller and release OS resources.

```modula2
PROCEDURE Add(p: Poller; fd, events: INTEGER): Status;
```
Register an fd for the given event interest set.

```modula2
PROCEDURE Modify(p: Poller; fd, events: INTEGER): Status;
```
Modify the interest set for an already-registered fd.

```modula2
PROCEDURE Remove(p: Poller; fd: INTEGER): Status;
```
Remove an fd from the poller.

```modula2
PROCEDURE Wait(p: Poller; timeoutMs: INTEGER;
               VAR buf: EventBuf;
               VAR count: INTEGER): Status;
```
Wait up to `timeoutMs` milliseconds for events. `-1` blocks indefinitely, `0` is non-blocking. On return, `count` holds the number of ready events in `buf`.

```modula2
PROCEDURE NowMs(): INTEGER;
```
Return current monotonic time in milliseconds. Wraps at ~24.8 days; use signed difference for comparisons.

---

### Timers

Min-heap timer queue for single-threaded event loops. Pool-allocated with no heap allocation in the hot path. Timer callbacks are enqueued via a Scheduler rather than called inline.

#### Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `MaxTimers` | 256 | Maximum number of concurrent timers. |

#### Types

| Type | Description |
|------|-------------|
| `TimerId` | Timer handle (`INTEGER`). |
| `TimerQueue` | Opaque timer queue handle (`ADDRESS`). |
| `Status` | `(OK, Invalid, PoolExhausted)` -- operation result. |

#### Procedures

```modula2
PROCEDURE Create(sched: Scheduler; VAR out: TimerQueue): Status;
```
Create a timer queue backed by the given Scheduler.

```modula2
PROCEDURE Destroy(VAR q: TimerQueue): Status;
```
Destroy a timer queue and free resources.

```modula2
PROCEDURE SetTimeout(q: TimerQueue; now, delayMs: INTEGER;
                     cb: TaskProc; user: ADDRESS;
                     VAR id: TimerId): Status;
```
Schedule a one-shot timer firing at `now + delayMs`.

```modula2
PROCEDURE SetInterval(q: TimerQueue; now, intervalMs: INTEGER;
                      cb: TaskProc; user: ADDRESS;
                      VAR id: TimerId): Status;
```
Schedule a repeating timer firing every `intervalMs`.

```modula2
PROCEDURE Cancel(q: TimerQueue; id: TimerId): Status;
```
Cancel a pending timer. Idempotent.

```modula2
PROCEDURE ActiveCount(q: TimerQueue): INTEGER;
```
Return the number of active timers.

```modula2
PROCEDURE NextDeadline(q: TimerQueue; now: INTEGER): INTEGER;
```
Milliseconds until the next timer fires, or `-1` if no timers are active. Used by the event loop to set the poll timeout.

```modula2
PROCEDURE Tick(q: TimerQueue; now: INTEGER): Status;
```
Fire all timers whose deadline has passed. Enqueues their callbacks on the scheduler. Repeating timers are automatically rescheduled.

## Example

```modula2
MODULE EvLoopDemo;

FROM EventLoop IMPORT Loop, Status, Create, Destroy,
                      SetTimeout, WatchFd, Run, Stop,
                      GetScheduler;
FROM Poller IMPORT EvRead;
FROM Scheduler IMPORT TaskProc;
FROM SYSTEM IMPORT ADDRESS;

VAR
  lp: Loop;
  st: Status;
  tid: INTEGER;

PROCEDURE OnTimer(user: ADDRESS);
BEGIN
  (* Timer fired -- stop the loop *)
  Stop(lp);
END OnTimer;

PROCEDURE OnReadable(fd, events: INTEGER; user: ADDRESS);
BEGIN
  (* fd is ready for reading *)
END OnReadable;

BEGIN
  st := Create(lp);
  (* Stop after 5 seconds *)
  st := SetTimeout(lp, 5000, OnTimer, NIL, tid);
  (* Watch stdin for readability *)
  st := WatchFd(lp, 0, EvRead, OnReadable, NIL);
  Run(lp);
  st := Destroy(lp);
END EvLoopDemo.
```
