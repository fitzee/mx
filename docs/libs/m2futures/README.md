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
Create a linked promise/future pair. `p` and `f` alias the same internal state and share one reference. The caller must call exactly one of `PromiseRelease` or `FutureRelease` — not both. Calling both is a double-release bug; calling neither is a leak.

**Lifetime**

```modula2
PROCEDURE PromiseRelease(VAR p: Promise);
PROCEDURE FutureRelease(VAR f: Future);
```
Release the creation reference. Sets the handle to `NIL`. The underlying state is freed when all references (external handle + continuations) are released. Both procedures are identical — two names exist for API clarity.

Output futures returned by chaining operations (`Map`, `OnReject`, `OnSettle`, `All`, `Race`) carry their own independent reference that the caller must release via `FutureRelease`.

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
Join: fulfills when every future in `fs` fulfills. Rejects immediately on the first rejection. On success, `Value.tag` = element count and `Value.ptr` = `AllResultPtr`. The result array is owned by the output future's internal state and remains valid until the output future is released via `FutureRelease` — copy any results you need before releasing.

Construction is best-effort: if scheduler enqueue fails partway through, already-attached continuations remain live and will execute normally. Remaining unattached continuations are freed. The caller receives `OutOfMemory` with `out = NIL`. The output state is reclaimed automatically once all live continuations have executed.

```modula2
PROCEDURE Race(s: Scheduler; fs: ARRAY OF Future;
               VAR out: Future): Status;
```
Race: settles as soon as the first future in `fs` settles. Same best-effort construction and failure semantics as `All`.

**Cancellation**

```modula2
PROCEDURE CancelTokenCreate(s: Scheduler; VAR ct: CancelToken): Status;
```
Create a new cancellation token.

```modula2
PROCEDURE CancelTokenDestroy(VAR ct: CancelToken);
```
Release the external reference to a cancel token. Safe to call immediately after `Cancel` — dispatched callbacks hold their own internal reference and will not outlive the token. Sets `ct` to `NIL`.

```modula2
PROCEDURE Cancel(ct: CancelToken);
```
Signal cancellation on the token. Callbacks are dispatched through the scheduler, not inline. The dispatch holds its own reference, so `CancelTokenDestroy` is safe immediately after `Cancel`. If the scheduler queue is full, the token is marked cancelled but pending callbacks may not fire.

```modula2
PROCEDURE IsCancelled(ct: CancelToken): BOOLEAN;
```
Check if a token has been cancelled.

```modula2
PROCEDURE OnCancel(ct: CancelToken; fn: VoidFn; ctx: ADDRESS);
```
Register a callback for when the token is cancelled. If already cancelled, `fn` is enqueued via the scheduler. At most 8 callbacks may be registered per token; calls beyond that limit are silently dropped. If the scheduler queue is full when dispatch is attempted, the affected callback will not fire.

```modula2
PROCEDURE MapCancellable(s: Scheduler; f: Future;
                          fn: ThenFn; user: ADDRESS;
                          ct: CancelToken;
                          VAR out: Future): Status;
```
Like `Map`, but checks the cancel token first. If cancelled, rejects with error code `-1`. Internally retains the cancel token; released when the continuation executes or construction fails. The caller's external reference is independent.

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
| `Release` | `Promise.FutureRelease` |
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
                    PromiseCreate, FutureRelease,
                    Resolve, Map,
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

  (* Create a promise/future pair -- p and f are aliases,
     sharing one reference. Release exactly one. *)
  st := PromiseCreate(s, p, f);

  (* Chain a transformation -- mapped has its own reference *)
  st := Map(s, f, Double, NIL, mapped);

  (* Resolve the promise *)
  MakeValue(21, NIL, v);
  st := Resolve(p, v);

  (* Pump the scheduler to execute continuations *)
  st := SchedulerPump(s, 100, didWork);
  (* mapped is now fulfilled with tag=42 *)

  (* Release handles *)
  FutureRelease(f);       (* creation ref *)
  FutureRelease(mapped);  (* chaining ref *)

  st := SchedulerDestroy(s);
END FuturesDemo.
```
