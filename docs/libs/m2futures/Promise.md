# Promise

Composable Promises/Futures for single-threaded Modula-2+. A **Promise** is the write handle (resolve/reject); a **Future** is the read handle (inspect/chain/combine). Both share internal state and are created together via `PromiseCreate`. All continuations dispatch through a Scheduler -- never inline.

## Why use promises?

Promises bring structured asynchronous composition to Modula-2 without threads, callbacks-within-callbacks, or manual state machines. They let you:

- **Chain transformations** (`Map`) so that the output of one operation feeds into the next, without nesting.
- **Recover from errors** (`OnReject`) at any point in a chain, converting failures back into successes.
- **Observe completion** (`OnSettle`) for cleanup or logging without altering the result.
- **Join multiple results** (`All`) -- wait for N independent operations and collect all results into a single array.
- **Race N operations** (`Race`) -- use whichever completes first.
- **Attach late** -- attach a continuation to an already-settled future and it still fires on the next pump cycle.

The library uses pool-based allocation (no heap allocation for normal promise operations) and a scheduler-driven execution model that keeps stack depth bounded and behavior predictable.

## Types

### Fate

Settlement state of a promise/future pair:

| Value       | Meaning                            |
|-------------|------------------------------------|
| `Pending`   | Not yet settled.                   |
| `Fulfilled` | Resolved successfully with a value.|
| `Rejected`  | Rejected with an error.            |

### Value

Success payload record:

| Field | Type      | Description                                    |
|-------|-----------|------------------------------------------------|
| `tag` | `INTEGER` | User-defined integer tag (e.g. result code).   |
| `ptr` | `ADDRESS` | User-defined pointer (e.g. to a heap record).  |

### Error

Failure payload record:

| Field  | Type      | Description                                    |
|--------|-----------|------------------------------------------------|
| `code` | `INTEGER` | User-defined error code.                       |
| `ptr`  | `ADDRESS` | Optional pointer to error details.             |

### Result

Discriminated result -- either a success value or a failure error:

| Field  | Type      | Description                                |
|--------|-----------|--------------------------------------------|
| `isOk` | `BOOLEAN` | `TRUE` if fulfilled, `FALSE` if rejected.  |
| `v`    | `Value`   | The success payload (meaningful when isOk). |
| `e`    | `Error`   | The failure payload (meaningful when not isOk). |

### Callback types

```modula2
(* Transform a result into a new result. *)
ThenFn  = PROCEDURE(Result, ADDRESS, VAR Result);

(* Handle an error, producing a recovery result. *)
CatchFn = PROCEDURE(Error, ADDRESS, VAR Result);

(* Observe a result without affecting the chain. *)
VoidFn  = PROCEDURE(Result, ADDRESS);
```

- **`ThenFn`**: receives the input `Result` by value and a user-data `ADDRESS`; fills the output `VAR Result` to settle the next link in the chain.
- **`CatchFn`**: receives only the `Error` (called only on rejection); fills a `VAR Result` to recover (or re-reject).
- **`VoidFn`**: observes the `Result` for side effects; the original result passes through unchanged.

### Handles

```modula2
Promise = ADDRESS;  (* write handle *)
Future  = ADDRESS;  (* read handle  *)
```

Both are opaque `ADDRESS` values pointing to shared internal state. They are created as a linked pair by `PromiseCreate`. **Promise and Future alias the same state** — they are not independent handles. See [Ownership](#ownership) below.

### AllResultPtr

```modula2
CONST MAX_ALL_SIZE = 32;
TYPE
  AllResultArray = ARRAY [0..MAX_ALL_SIZE-1] OF Result;
  AllResultPtr   = POINTER TO AllResultArray;
```

When `All` fulfills, `Value.tag` holds the element count and `Value.ptr` can be cast to `AllResultPtr` to access individual results.

## Ownership

`PromiseCreate` returns a handle pair sharing **one reference**. Promise and Future alias the same internal state — they are **not** independent handles. The caller must call exactly one of `PromiseRelease` or `FutureRelease` to drop the creation reference:

- Calling both is a **double-release bug**.
- Calling neither is a **leak**.

Continuations attached via `Map`, `OnReject`, `OnSettle`, `All`, and `Race` hold their own internal references and release them when they execute. The **output future** returned by each chaining operation carries its own independent reference that the caller must release via `FutureRelease`.

## Creation

### PromiseCreate

```modula2
PROCEDURE PromiseCreate(s: Scheduler;
                        VAR p: Promise;
                        VAR f: Future): Status;
```

Creates a linked promise/future pair on scheduler `s`. `p` and `f` alias the same state; `refCount = 1`. Both start in `Pending` state. The caller must call exactly one of `PromiseRelease` or `FutureRelease` — not both. Returns `Invalid` if `s` is `NIL`. Returns `OutOfMemory` if the internal pool is exhausted.

```modula2
VAR p: Promise; f: Future; st: Status;
...
st := PromiseCreate(sched, p, f);
(* p and f are aliases — release exactly one when done *)
```

## Lifetime

### PromiseRelease

```modula2
PROCEDURE PromiseRelease(VAR p: Promise);
```

Release the promise handle. Sets `p` to `NIL`. The underlying state is freed when all references (external handle + continuations) are released.

### FutureRelease

```modula2
PROCEDURE FutureRelease(VAR f: Future);
```

Release the future handle. Sets `f` to `NIL`. Same operation as `PromiseRelease`; both names exist for API clarity since Promise and Future share state.

## Settlement

### Resolve

```modula2
PROCEDURE Resolve(p: Promise; v: Value): Status;
```

Fulfills the promise with value `v`. All attached continuations are enqueued on the scheduler. Returns `AlreadySettled` if the promise was already resolved or rejected. Returns `Invalid` if `p` is `NIL`.

```modula2
VAR v: Value;
MakeValue(42, NIL, v);
st := Resolve(p, v);
```

### Reject

```modula2
PROCEDURE Reject(p: Promise; e: Error): Status;
```

Rejects the promise with error `e`. All attached continuations are enqueued. Returns `AlreadySettled` if already settled. Returns `Invalid` if `p` is `NIL`.

```modula2
VAR e: Error;
MakeError(404, NIL, e);
st := Reject(p, e);
```

## Inspection

### GetFate

```modula2
PROCEDURE GetFate(f: Future; VAR fate: Fate): Status;
```

Returns the current fate of the future without blocking. Returns `Invalid` if `f` is `NIL`.

### GetResultIfSettled

```modula2
PROCEDURE GetResultIfSettled(f: Future;
                             VAR settled: BOOLEAN;
                             VAR res: Result): Status;
```

If the future is settled, sets `settled` to `TRUE` and fills `res` with the result. If still pending, sets `settled` to `FALSE` and leaves `res` unchanged. Returns `Invalid` if `f` is `NIL`.

## Chaining

### Map

```modula2
PROCEDURE Map(s: Scheduler; f: Future;
              fn: ThenFn; user: ADDRESS;
              VAR out: Future): Status;
```

Attaches a transformation to future `f`. When `f` settles, `fn` is called with the result and `user` data; `fn`'s output becomes the settlement of the returned future `out`. If `f` is already settled, the continuation is enqueued immediately (fires on the next pump cycle).

```modula2
PROCEDURE Double(res: Result; user: ADDRESS; VAR out: Result);
VAR v: Value;
BEGIN
  IF res.isOk THEN
    MakeValue(res.v.tag * 2, NIL, v);
    Ok(v, out)
  ELSE
    out := res
  END
END Double;
...
st := Map(sched, f, Double, NIL, f2);
```

### OnReject

```modula2
PROCEDURE OnReject(s: Scheduler; f: Future;
                   fn: CatchFn; user: ADDRESS;
                   VAR out: Future): Status;
```

Attaches an error handler. `fn` is called only when `f` rejects; on fulfillment the value passes through unchanged. Use this to recover from errors in a chain.

```modula2
PROCEDURE Recover(err: Error; user: ADDRESS; VAR out: Result);
VAR v: Value;
BEGIN
  MakeValue(0, NIL, v);
  Ok(v, out)  (* convert error into success *)
END Recover;
...
st := OnReject(sched, f, Recover, NIL, f2);
```

### OnSettle

```modula2
PROCEDURE OnSettle(s: Scheduler; f: Future;
                   fn: VoidFn; user: ADDRESS;
                   VAR out: Future): Status;
```

Attaches a side-effect observer. `fn` is called on settlement (whether fulfilled or rejected) but does not alter the chain -- the original result passes through to `out`.

```modula2
PROCEDURE Log(res: Result; user: ADDRESS);
BEGIN
  IF res.isOk THEN WriteString("ok") ELSE WriteString("err") END;
  WriteLn
END Log;
...
st := OnSettle(sched, f, Log, NIL, f2);
```

## Combinators

### All

```modula2
PROCEDURE All(s: Scheduler; fs: ARRAY OF Future;
              VAR out: Future): Status;
```

Joins N futures. Fulfills when every future in `fs` fulfills. Rejects immediately on the first rejection.

On success:
- `Value.tag` = element count
- `Value.ptr` points to an `AllResultArray` containing each element's `Result` in order

**Result pointer lifetime**: the `AllResultPtr` in `Value.ptr` points to an array owned by the output future's internal state. It remains valid as long as the output future has not been released via `FutureRelease`. Copy any results you need before releasing.

**Best-effort construction**: continuations are pre-allocated up front. If pre-allocation fails, full cleanup occurs and the caller receives `OutOfMemory` with `out = NIL`. If scheduler enqueue fails partway through attachment, already-attached continuations remain live (they hold refs and will execute normally when their input settles). Remaining unattached continuations are freed, and the caller receives `OutOfMemory` with `out = NIL`. The output state is reclaimed automatically once all live continuations have executed and released.

Returns `Invalid` if `s` is `NIL`, `fs` is empty, any element is `NIL`, or the array exceeds `MAX_ALL_SIZE` (32). Returns `OutOfMemory` if allocation fails.

```modula2
VAR fs: ARRAY [0..2] OF Future;
...
fs[0] := f1; fs[1] := f2; fs[2] := f3;
st := All(sched, fs, fAll);
(* after pump: res.v.tag = 3, res.v.ptr^ has 3 Results *)
(* copy results before calling FutureRelease(fAll) *)
```

### Race

```modula2
PROCEDURE Race(s: Scheduler; fs: ARRAY OF Future;
               VAR out: Future): Status;
```

Settles as soon as the first future in `fs` settles. The winning result (whether fulfilled or rejected) becomes the output. Subsequent settlements of other futures are ignored. Same best-effort construction and failure semantics as `All`.

```modula2
st := Race(sched, fs, fRace);
(* the first future to settle wins *)
```

## Cancellation

### CancelTokenCreate

```modula2
PROCEDURE CancelTokenCreate(s: Scheduler; VAR ct: CancelToken): Status;
```

Create a new cancellation token on scheduler `s`. Returns `Invalid` if `s` is `NIL`. Returns `OutOfMemory` if the token pool (64 slots) is exhausted.

### CancelTokenDestroy

```modula2
PROCEDURE CancelTokenDestroy(VAR ct: CancelToken);
```

Release the external reference to a cancel token. Sets `ct` to `NIL`. Safe to call immediately after `Cancel` — dispatched callbacks hold their own internal reference and will not outlive the token. The pool slot is freed when all references (external + internal from `MapCancellable` + dispatch) are released.

### Cancel

```modula2
PROCEDURE Cancel(ct: CancelToken);
```

Signal cancellation on the token. Callbacks registered via `OnCancel` are dispatched through the scheduler, not inline. The dispatch holds its own reference to the token, so `CancelTokenDestroy` is safe immediately after `Cancel`. If the scheduler queue is full, the token is marked cancelled but pending callbacks may not fire. Calling `Cancel` on an already-cancelled token is a no-op.

### IsCancelled

```modula2
PROCEDURE IsCancelled(ct: CancelToken): BOOLEAN;
```

Returns `TRUE` if the token has been cancelled.

### OnCancel

```modula2
PROCEDURE OnCancel(ct: CancelToken; fn: VoidFn; ctx: ADDRESS);
```

Register a callback to be called when the token is cancelled. If already cancelled, `fn` is enqueued via the scheduler.

**Limits**: at most 8 callbacks may be registered per token. Calls beyond that limit are silently dropped. If the scheduler queue is full when dispatch is attempted, the token remains marked cancelled but the affected callback will not fire.

### MapCancellable

```modula2
PROCEDURE MapCancellable(s: Scheduler; f: Future;
                         fn: ThenFn; user: ADDRESS;
                         ct: CancelToken;
                         VAR out: Future): Status;
```

Like `Map`, but checks the cancel token before invoking `fn`. If cancelled, rejects with error code `-1`. Internally retains the cancel token; released when the continuation executes or construction fails. The caller's external reference is independent.

```modula2
st := CancelTokenCreate(sched, ct);
st := MapCancellable(sched, f, MyFn, NIL, ct, f2);
Cancel(ct);
(* safe to destroy immediately -- internal ref keeps token alive *)
CancelTokenDestroy(ct);
PumpAll;
(* f2 is now rejected with code -1 *)
FutureRelease(f2);
```

## Helpers

### MakeValue

```modula2
PROCEDURE MakeValue(tag: INTEGER; ptr: ADDRESS; VAR v: Value);
```

Convenience: fills a `Value` record.

### MakeError

```modula2
PROCEDURE MakeError(code: INTEGER; ptr: ADDRESS; VAR e: Error);
```

Convenience: fills an `Error` record.

### Ok

```modula2
PROCEDURE Ok(v: Value; VAR r: Result);
```

Convenience: fills a `Result` as fulfilled with value `v`.

### Fail

```modula2
PROCEDURE Fail(e: Error; VAR r: Result);
```

Convenience: fills a `Result` as rejected with error `e`.

## Notes

- **Pool-based allocation**: The library pre-allocates 256 shared states, 512 continuation nodes, and 64 cancel token slots. Normal promise operations (create, resolve, chain) use pool slots — no `NEW`/`DISPOSE`. Only `All` and `Race` heap-allocate a small tracking record, and `MapCancellable` heap-allocates a small wrapper record.
- **Alias-pair ownership**: `PromiseCreate` returns two handles that alias the same state with one shared reference. Release exactly one. Each chaining output future has its own independent reference.
- **Late attachment**: Attaching a continuation to an already-settled future is safe. The continuation is enqueued immediately and fires on the next pump cycle.
- **Re-entrancy**: Callbacks may create new promises, resolve other promises, or attach new continuations. All such work is enqueued — never executed inline — so stack depth stays bounded.
- **No threads required**: The entire library is single-threaded. All progress happens through `SchedulerPump`. This makes it suitable for event-loop architectures, game loops, or cooperative multitasking.
- **Cancel token dispatch safety**: `Cancel` acquires a dispatch reference before enqueuing callbacks, so `CancelTokenDestroy` is safe immediately after `Cancel`. The dispatch reference is released when all callbacks have been dispatched.
- **Lossy cancellation**: At most 8 `OnCancel` callbacks per token; excess registrations are silently dropped. If the scheduler queue is full during dispatch, the token is marked cancelled but remaining callbacks will not fire.
- **`THEN` is reserved**: The chaining procedure is named `Map` (not `Then`) because `THEN` is a reserved keyword in Modula-2. Similarly, `Catch` becomes `OnReject` and `Finally` becomes `OnSettle`.

## Complete Example

Demonstrates resolve/reject, chaining, catch/recovery, All, Race, and late attachment.

```modula2
MODULE FuturesDemo;

FROM InOut IMPORT WriteString, WriteInt, WriteLn;
FROM SYSTEM IMPORT ADDRESS;
FROM Scheduler IMPORT Status, Scheduler, OK,
                      SchedulerCreate, SchedulerDestroy,
                      SchedulerPump;
FROM Promise IMPORT Fate, Value, Error, Result, Promise, Future,
                    ThenFn, CatchFn, VoidFn,
                    AllResultPtr, MAX_ALL_SIZE,
                    PromiseCreate, FutureRelease,
                    Resolve, Reject,
                    GetFate, Map, OnReject, OnSettle, All, Race,
                    MakeValue, MakeError, Ok, Fail;

VAR
  sched: Scheduler;
  st: Status;

PROCEDURE PumpAll;
VAR dw: BOOLEAN;
BEGIN
  dw := TRUE;
  WHILE dw DO st := SchedulerPump(sched, 100, dw) END
END PumpAll;

(* Double an integer value *)
PROCEDURE DoubleVal(res: Result; user: ADDRESS; VAR out: Result);
VAR v: Value;
BEGIN
  IF res.isOk THEN
    MakeValue(res.v.tag * 2, NIL, v);
    Ok(v, out)
  ELSE
    out := res
  END
END DoubleVal;

(* Print a result *)
PROCEDURE PrintVal(res: Result; user: ADDRESS; VAR out: Result);
BEGIN
  WriteString("  => ");
  IF res.isOk THEN
    WriteString("ok, tag="); WriteInt(res.v.tag, 1)
  ELSE
    WriteString("err, code="); WriteInt(res.e.code, 1)
  END;
  WriteLn;
  out := res
END PrintVal;

VAR
  p: Promise; f, f2, f3: Future;
  v: Value;
BEGIN
  st := SchedulerCreate(1024, sched);

  (* Chain: 21 -> double -> print *)
  (* p and f are aliases sharing one reference *)
  st := PromiseCreate(sched, p, f);
  st := Map(sched, f, DoubleVal, NIL, f2);
  st := Map(sched, f2, PrintVal, NIL, f3);

  MakeValue(21, NIL, v);
  st := Resolve(p, v);
  PumpAll;
  (* Output: => ok, tag=42 *)

  (* Release handles: one for the creation pair, one for each Map output *)
  FutureRelease(f);
  FutureRelease(f2);
  FutureRelease(f3);

  st := SchedulerDestroy(sched)
END FuturesDemo.
```
