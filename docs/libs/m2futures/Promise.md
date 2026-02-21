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

Both are opaque `ADDRESS` values pointing to shared internal state. They are created as a linked pair by `PromiseCreate`.

### AllResultPtr

```modula2
CONST MAX_ALL_SIZE = 32;
TYPE
  AllResultArray = ARRAY [0..MAX_ALL_SIZE-1] OF Result;
  AllResultPtr   = POINTER TO AllResultArray;
```

When `All` fulfills, `Value.tag` holds the element count and `Value.ptr` can be cast to `AllResultPtr` to access individual results.

## Creation

### PromiseCreate

```modula2
PROCEDURE PromiseCreate(s: Scheduler;
                        VAR p: Promise;
                        VAR f: Future): Status;
```

Creates a linked promise/future pair on scheduler `s`. Both start in `Pending` state. Returns `Invalid` if `s` is `NIL`. Returns `OutOfMemory` if the internal pool is exhausted.

```modula2
VAR p: Promise; f: Future; st: Status;
...
st := PromiseCreate(sched, p, f);
```

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

Returns `Invalid` if `s` is `NIL`, `fs` is empty, or the array exceeds `MAX_ALL_SIZE` (32). Returns `OutOfMemory` if allocation fails.

```modula2
VAR fs: ARRAY [0..2] OF Future;
...
fs[0] := f1; fs[1] := f2; fs[2] := f3;
st := All(sched, fs, fAll);
(* after pump: res.v.tag = 3, res.v.ptr^ has 3 Results *)
```

### Race

```modula2
PROCEDURE Race(s: Scheduler; fs: ARRAY OF Future;
               VAR out: Future): Status;
```

Settles as soon as the first future in `fs` settles. The winning result (whether fulfilled or rejected) becomes the output. Subsequent settlements of other futures are ignored.

```modula2
st := Race(sched, fs, fRace);
(* the first future to settle wins *)
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

- **Pool-based allocation**: The library pre-allocates 256 shared states and 512 continuation nodes. Normal promise operations (create, resolve, chain) use pool slots -- no `NEW`/`DISPOSE`. Only the `All` and `Race` combinators heap-allocate a small tracking record.
- **Late attachment**: Attaching a continuation to an already-settled future is safe. The continuation is enqueued immediately and fires on the next pump cycle.
- **Re-entrancy**: Callbacks may create new promises, resolve other promises, or attach new continuations. All such work is enqueued -- never executed inline -- so stack depth stays bounded.
- **No threads required**: The entire library is single-threaded. All progress happens through `SchedulerPump`. This makes it suitable for event-loop architectures, game loops, or cooperative multitasking.
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
                    PromiseCreate, Resolve, Reject,
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
  st := PromiseCreate(sched, p, f);
  st := Map(sched, f, DoubleVal, NIL, f2);
  st := Map(sched, f2, PrintVal, NIL, f3);

  MakeValue(21, NIL, v);
  st := Resolve(p, v);
  PumpAll;
  (* Output: => ok, tag=42 *)

  st := SchedulerDestroy(sched)
END FuturesDemo.
```
