# m2futures

## Why
Composable Promises and Futures for single-threaded asynchronous programming in Modula-2+. All continuations dispatch through a Scheduler (never inline), enabling deterministic, cooperative concurrency without threads.

## Modules

### Scheduler

Microtask queue that drives all promise settlement and continuation dispatch. The caller owns the instance; there is no hidden global state.

#### Types

| Type | Description |
|------|-------------|
| `Scheduler` | Opaque scheduler handle (`ADDRESS`). |
| `TaskProc` | `PROCEDURE(ADDRESS)` -- callback for enqueued tasks. |
| `Status` | `(OK, Invalid, OutOfMemory, AlreadySettled)` -- shared status across Scheduler and Promise modules. |

#### Procedures

```modula2
PROCEDURE SchedulerCreate(capacity: CARDINAL;
                           VAR out: Scheduler): Status;
```
Create a scheduler with room for up to `capacity` queued tasks.

```modula2
PROCEDURE SchedulerDestroy(VAR s: Scheduler): Status;
```
Destroy a scheduler and free its resources.

```modula2
PROCEDURE SchedulerEnqueue(s: Scheduler;
                            cb: TaskProc;
                            user: ADDRESS): Status;
```
Enqueue a callback for execution on the next pump cycle. Returns `OutOfMemory` if the queue is full.

```modula2
PROCEDURE SchedulerPump(s: Scheduler;
                         maxSteps: CARDINAL;
                         VAR didWork: BOOLEAN): Status;
```
Run up to `maxSteps` queued callbacks. Callbacks may enqueue further work during execution. `didWork` is `TRUE` if at least one callback ran.

---

### Promise

The write-side handle for creating and settling futures. A Promise/Future pair shares internal state: Promise resolves or rejects, Future is used to inspect, chain, and combine results.

#### Types

| Type | Description |
|------|-------------|
| `Promise` | Opaque write handle (`ADDRESS`). |
| `Future` | Opaque read handle (`ADDRESS`). |
| `Fate` | `(Pending, Fulfilled, Rejected)` -- settlement state. |
| `Value` | Record with `tag: INTEGER` and `ptr: ADDRESS` -- success payload. |
| `Error` | Record with `code: INTEGER` and `ptr: ADDRESS` -- failure payload. |
| `Result` | Discriminated result with `isOk: BOOLEAN`, `v: Value`, `e: Error`. |
| `ThenFn` | `PROCEDURE(Result, ADDRESS, VAR Result)` -- transform a result into a new result. |
| `CatchFn` | `PROCEDURE(Error, ADDRESS, VAR Result)` -- handle an error, producing a recovery result. |
| `VoidFn` | `PROCEDURE(Result, ADDRESS)` -- observe a result without altering the chain. |
| `CancelToken` | Opaque cancellation token (`ADDRESS`). |
| `AllResultArray` | `ARRAY [0..MAX_ALL_SIZE-1] OF Result` -- results from `All()`. |
| `AllResultPtr` | `POINTER TO AllResultArray`. |

#### Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `MAX_ALL_SIZE` | 32 | Maximum number of futures in an `All()` group. |

#### Procedures

**Creation**

```modula2
PROCEDURE PromiseCreate(s: Scheduler;
                         VAR p: Promise;
                         VAR f: Future): Status;
```
Create a linked promise/future pair. `p` is the write handle, `f` is the read handle.

**Settlement**

```modula2
PROCEDURE Resolve(p: Promise; v: Value): Status;
```
Fulfill the promise with value `v`. Enqueues all attached continuations. Returns `AlreadySettled` if already resolved or rejected.

```modula2
PROCEDURE Reject(p: Promise; e: Error): Status;
```
Reject the promise with error `e`.

**Inspection**

```modula2
PROCEDURE GetFate(f: Future; VAR fate: Fate): Status;
```
Query the current fate of a future.

```modula2
PROCEDURE GetResultIfSettled(f: Future;
                              VAR settled: BOOLEAN;
                              VAR res: Result): Status;
```
Query the result if settled. `settled` is `FALSE` if still pending.

**Chaining**

```modula2
PROCEDURE Map(s: Scheduler; f: Future;
              fn: ThenFn; user: ADDRESS;
              VAR out: Future): Status;
```
Attach a transformation. `fn` receives the result when `f` settles; its output becomes the settlement of `out`.

```modula2
PROCEDURE OnReject(s: Scheduler; f: Future;
                   fn: CatchFn; user: ADDRESS;
                   VAR out: Future): Status;
```
Attach an error handler. `fn` is called only on rejection; fulfillment passes through unchanged.

```modula2
PROCEDURE OnSettle(s: Scheduler; f: Future;
                   fn: VoidFn; user: ADDRESS;
                   VAR out: Future): Status;
```
Attach a side-effect observer. Does not alter the chain; the original result passes through.

**Combinators**

```modula2
PROCEDURE All(s: Scheduler; fs: ARRAY OF Future;
              VAR out: Future): Status;
```
Join: fulfills when every future in `fs` fulfills. Rejects immediately on the first rejection. On success, `Value.tag` = element count and `Value.ptr` = `AllResultPtr`.

```modula2
PROCEDURE Race(s: Scheduler; fs: ARRAY OF Future;
               VAR out: Future): Status;
```
Race: settles as soon as the first future in `fs` settles.

**Cancellation**

```modula2
PROCEDURE CancelTokenCreate(s: Scheduler; VAR ct: CancelToken): Status;
```
Create a new cancellation token.

```modula2
PROCEDURE Cancel(ct: CancelToken);
```
Signal cancellation on the token.

```modula2
PROCEDURE IsCancelled(ct: CancelToken): BOOLEAN;
```
Check if a token has been cancelled.

```modula2
PROCEDURE OnCancel(ct: CancelToken; fn: VoidFn; ctx: ADDRESS);
```
Register a callback for when the token is cancelled. If already cancelled, `fn` is enqueued immediately.

```modula2
PROCEDURE MapCancellable(s: Scheduler; f: Future;
                          fn: ThenFn; user: ADDRESS;
                          ct: CancelToken;
                          VAR out: Future): Status;
```
Like `Map`, but checks the cancel token first. If cancelled, rejects with error code `-1`.

**Helpers**

```modula2
PROCEDURE MakeValue(tag: INTEGER; ptr: ADDRESS; VAR v: Value);
PROCEDURE MakeError(code: INTEGER; ptr: ADDRESS; VAR e: Error);
PROCEDURE Ok(v: Value; VAR r: Result);
PROCEDURE Fail(e: Error; VAR r: Result);
```
Convenience constructors for `Value`, `Error`, and `Result` records.

---

### Future

Read-side convenience namespace that re-exports chaining and combinator procedures from `Promise` under `F`-prefixed names. Allows callers to import creation/settlement from `Promise` and chaining from `Future` without name collisions.

#### Procedures

| Procedure | Delegates to |
|-----------|-------------|
| `GetFate` | `Promise.GetFate` |
| `GetResultIfSettled` | `Promise.GetResultIfSettled` |
| `FMap` | `Promise.Map` |
| `FOnReject` | `Promise.OnReject` |
| `FOnSettle` | `Promise.OnSettle` |
| `FAll` | `Promise.All` |
| `FRace` | `Promise.Race` |

## Example

```modula2
MODULE FuturesDemo;

FROM SYSTEM IMPORT ADDRESS;
FROM Scheduler IMPORT Scheduler, Status, SchedulerCreate,
                      SchedulerPump, SchedulerDestroy;
FROM Promise IMPORT Promise, Future, Value, Error, Result,
                    Fate, ThenFn,
                    PromiseCreate, Resolve, Map,
                    MakeValue, Ok;

VAR
  s: Scheduler;
  p: Promise;
  f, mapped: Future;
  v: Value;
  st: Status;
  didWork: BOOLEAN;

PROCEDURE Double(input: Result; user: ADDRESS; VAR out: Result);
VAR
  doubled: Value;
BEGIN
  MakeValue(input.v.tag * 2, NIL, doubled);
  Ok(doubled, out);
END Double;

BEGIN
  st := SchedulerCreate(64, s);

  (* Create a promise/future pair *)
  st := PromiseCreate(s, p, f);

  (* Chain a transformation *)
  st := Map(s, f, Double, NIL, mapped);

  (* Resolve the promise *)
  MakeValue(21, NIL, v);
  st := Resolve(p, v);

  (* Pump the scheduler to execute continuations *)
  st := SchedulerPump(s, 100, didWork);
  (* mapped is now fulfilled with tag=42 *)

  st := SchedulerDestroy(s);
END FuturesDemo.
```
