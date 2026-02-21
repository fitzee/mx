# Timers

Min-heap timer queue for single-threaded event loops. Timers are pool-allocated with no heap allocation in the hot path. Expired timer callbacks are dispatched through a Scheduler, not called inline.

## Why a min-heap?

A min-heap provides O(log n) insert, O(1) peek at the nearest deadline, and O(k log n) tick for k expired timers. This is ideal for sparse timer workloads typical in event loops, where most iterations fire zero or one timer.

## Time Model

All time values are 32-bit `INTEGER` milliseconds from a monotonic clock. The value wraps at approximately 24.8 days. Comparisons use signed difference (`a - b < 0`) for correct wrap-around handling -- the same technique used by the Linux kernel's `time_after()` macro.

The Timers module is **pure Modula-2**: the caller passes the current time via `Poller.NowMs()`, keeping the module free of platform dependencies.

## Types

**`TimerId`** -- Handle for cancelling a timer:

```modula2
TYPE TimerId = INTEGER;
```

**`TimerQueue`** -- Opaque handle to a timer queue:

```modula2
TYPE TimerQueue = ADDRESS;
```

**`Status`** -- Operation result:

| Value           | Meaning                                      |
|-----------------|----------------------------------------------|
| `OK`            | Operation succeeded.                         |
| `Invalid`       | Bad argument (NIL queue).                    |
| `PoolExhausted` | Timer pool is full (256 concurrent timers).  |

## Procedures

### Create

```modula2
PROCEDURE Create(sched: Scheduler;
                 VAR out: TimerQueue): Status;
```

Create a timer queue that dispatches expired callbacks on `sched`. The queue supports up to 256 concurrent timers.

### Destroy

```modula2
PROCEDURE Destroy(VAR q: TimerQueue): Status;
```

Destroy the timer queue. Active timers are silently discarded.

### SetTimeout

```modula2
PROCEDURE SetTimeout(q: TimerQueue; now, delayMs: INTEGER;
                     cb: TaskProc; user: ADDRESS;
                     VAR id: TimerId): Status;
```

Schedule a one-shot timer that fires at `now + delayMs`. When fired, `cb(user)` is enqueued on the scheduler.

### SetInterval

```modula2
PROCEDURE SetInterval(q: TimerQueue; now, intervalMs: INTEGER;
                      cb: TaskProc; user: ADDRESS;
                      VAR id: TimerId): Status;
```

Schedule a repeating timer. Fires at `now + intervalMs`, then every `intervalMs` thereafter. Repeating timers reschedule from their deadline (not from current time), preventing drift.

### Cancel

```modula2
PROCEDURE Cancel(q: TimerQueue; id: TimerId): Status;
```

Cancel a pending timer. Idempotent: returns `OK` for already-fired or already-cancelled timers.

### ActiveCount

```modula2
PROCEDURE ActiveCount(q: TimerQueue): INTEGER;
```

Return the number of active (not yet fired or cancelled) timers.

### NextDeadline

```modula2
PROCEDURE NextDeadline(q: TimerQueue; now: INTEGER): INTEGER;
```

Return milliseconds until the next timer fires, or -1 if no timers are active. Used by the event loop to compute the poll timeout.

### Tick

```modula2
PROCEDURE Tick(q: TimerQueue; now: INTEGER): Status;
```

Fire all timers whose deadline is at or before `now`. Each expired timer's callback is enqueued on the scheduler. Repeating timers are automatically rescheduled.

## Notes

- The pool holds 256 timers. Cancelled timers free their pool slot immediately; one-shot timers free on fire.
- Repeating timers reschedule from `deadline + interval`, not from `now + interval`. This prevents cumulative drift when ticks run slightly late.
- The min-heap lazily skips cancelled entries during `Tick` and `NextDeadline`, so cancellation is O(n) in the worst case but O(1) amortized.

## Example

```modula2
FROM Timers IMPORT TimerQueue, TimerId, Create, Destroy,
                   SetTimeout, SetInterval, Cancel, Tick,
                   NextDeadline, ActiveCount;
FROM Timers IMPORT Status, OK;
FROM Scheduler IMPORT Scheduler, SchedulerCreate, SchedulerPump;
FROM Poller IMPORT NowMs;

VAR
  sched: Scheduler;
  q: TimerQueue;
  id1, id2: TimerId;
  now, timeout: INTEGER;
  didWork: BOOLEAN;

(* ... create scheduler ... *)
st := Create(sched, q);
now := NowMs();
st := SetTimeout(q, now, 1000, MyCallback, NIL, id1);
st := SetInterval(q, now, 500, TickCallback, NIL, id2);

(* In event loop: *)
now := NowMs();
timeout := NextDeadline(q, now);
(* ... poll with timeout ... *)
now := NowMs();
st := Tick(q, now);
st := SchedulerPump(sched, 256, didWork);
```
