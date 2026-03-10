# Async Architecture

Overview of the mx single-threaded async runtime, from OS primitives up to application-level Promises.

## Layer Diagram

```
┌───────────────────────────────────────────────────┐
│  Application Code                                 │
│  (async_tcp_echo_future.mod, etc.)                │
├───────────────────────────────────────────────────┤
│  Promise / Future          (m2futures)            │
│  Composable async values: resolve, reject, chain  │
├───────────────────────────────────────────────────┤
│  Scheduler                 (m2futures)            │
│  Microtask ring-buffer queue, FIFO dispatch       │
├───────────────────────────────────────────────────┤
│  EventLoop                 (m2evloop)             │
│  Orchestrator: poll → dispatch → tick → pump      │
├──────────────────────┬────────────────────────────┤
│  Poller              │  Timers                    │
│  fd readiness        │  min-heap deadline queue   │
│  (kqueue/epoll/poll) │  (pure M2, pool-alloc)     │
├──────────────────────┴────────────────────────────┤
│  PollerBridge (C FFI)                             │
│  poller_bridge.c — platform syscalls              │
├───────────────────────────────────────────────────┤
│  OS Kernel (kqueue / epoll / poll)                │
└───────────────────────────────────────────────────┘
```

## Execution Model

The runtime is **single-threaded**. All concurrency comes from interleaving: the event loop multiplexes I/O readiness and timer expiry, dispatching callbacks that run to completion before the next callback starts.

### RunOnce Cycle

```
1. now = NowMs()
2. timeout = min(NextTimerDeadline, DefaultTimeout)
3. events = Poller.Wait(timeout)
4. for each ready fd:
     call watcher callback inline
5. now = NowMs()
6. Timers.Tick(now)          -- enqueues callbacks on Scheduler
7. while Scheduler has work:
     SchedulerPump(256)      -- drains microtask queue
```

### Callback Ordering

- **Watcher callbacks** (I/O readiness) run inline during step 4. They execute before any timer or scheduled callbacks in the same iteration.
- **Timer callbacks** are enqueued on the Scheduler during step 6, then execute during step 7. This ensures consistent ordering with Promise continuations.
- **Microtasks** (direct `Enqueue` calls, Promise continuations) also execute during step 7, interleaved with timer callbacks in FIFO order.

## Integration with Promises

Use `EventLoop.GetScheduler()` to obtain the Scheduler handle needed by the Promise API:

```modula2
FROM EventLoop IMPORT Loop, Create, GetScheduler, Run;
FROM Scheduler IMPORT Scheduler;
FROM Promise IMPORT Future, Promise, PromiseCreate, Resolve;

VAR
  loop: Loop;
  sched: Scheduler;
  p: Promise;
  f: Future;

st := Create(loop);
sched := GetScheduler(loop);
st := PromiseCreate(sched, p, f);
(* ... resolve p from a watcher or timer callback ... *)
Run(loop);
```

Timer-based resolution:

```modula2
PROCEDURE ResolveAfterDelay(user: ADDRESS);
BEGIN
  st := Resolve(myPromise, someValue)
END ResolveAfterDelay;

st := SetTimeout(loop, 1000, ResolveAfterDelay, NIL, tid);
```

## Resource Limits

| Resource     | Limit | Module     |
|--------------|-------|------------|
| Pollers      | 16    | C bridge   |
| Watchers     | 64    | EventLoop  |
| Timers       | 256   | Timers     |
| Sched queue  | 1024  | Scheduler  |
| Promises     | 256   | Promise    |
| Continuations| 512   | Promise    |

## Time Representation

All timestamps are 32-bit signed `INTEGER` milliseconds from a monotonic clock. The value wraps approximately every 24.8 days. Comparisons always use signed difference: `(a - b) < 0` means "a is before b". This handles wrap-around correctly for any two times within ~12.4 days of each other.

## Platform Support

| Platform | I/O Backend | Timer Clock        |
|----------|-------------|--------------------|
| macOS    | kqueue      | CLOCK_MONOTONIC    |
| Linux    | epoll       | CLOCK_MONOTONIC    |
| Others   | poll        | CLOCK_MONOTONIC    |

## See Also

- [EventLoop](EventLoop.md) -- Main orchestrator API
- [Timers](Timers.md) -- Timer queue API
- [Poller](Poller.md) -- I/O polling API
- [Scheduler](../m2futures/Scheduler.md) -- Microtask queue
- [Promise](../m2futures/Promise.md) -- Composable async values
