MODULE FuturesTests;
(* Unit-test driver for m2futures library. *)

FROM InOut IMPORT WriteString, WriteInt, WriteLn;
FROM SYSTEM IMPORT ADDRESS, ADR;
FROM Scheduler IMPORT Status, Scheduler, OK, Invalid, OutOfMemory,
                      AlreadySettled,
                      SchedulerCreate, SchedulerDestroy,
                      SchedulerEnqueue, SchedulerPump;
FROM Promise IMPORT Fate, Pending, Fulfilled, Rejected,
                    Value, Error, Result, Promise, Future,
                    ThenFn, CatchFn, VoidFn,
                    AllResultPtr, MAX_ALL_SIZE,
                    PromiseCreate, Resolve, Reject,
                    GetFate, GetResultIfSettled,
                    Map, OnReject, OnSettle, All, Race,
                    MakeValue, MakeError, Ok, Fail;

VAR
  passed, failed, total: INTEGER;
  sched: Scheduler;
  st: Status;

PROCEDURE PumpAll;
VAR dw: BOOLEAN;
BEGIN
  dw := TRUE;
  WHILE dw DO st := SchedulerPump(sched, 200, dw) END
END PumpAll;

PROCEDURE Check(name: ARRAY OF CHAR; cond: BOOLEAN);
BEGIN
  total := total + 1;
  IF cond THEN
    passed := passed + 1
  ELSE
    failed := failed + 1;
    WriteString("FAIL: "); WriteString(name); WriteLn
  END
END Check;

(* ---- Simple callbacks for testing ---- *)

PROCEDURE IdentityThen(res: Result; user: ADDRESS; VAR out: Result);
BEGIN
  out := res
END IdentityThen;

PROCEDURE AddTenThen(res: Result; user: ADDRESS; VAR out: Result);
VAR v: Value;
BEGIN
  IF res.isOk THEN
    v.tag := res.v.tag + 10;
    v.ptr := NIL;
    Ok(v, out)
  ELSE
    out := res
  END
END AddTenThen;

PROCEDURE RecoverCatch(err: Error; user: ADDRESS; VAR out: Result);
VAR v: Value;
BEGIN
  v.tag := err.code * -1;
  v.ptr := NIL;
  Ok(v, out)
END RecoverCatch;

PROCEDURE NopFinally(res: Result; user: ADDRESS);
BEGIN
  (* intentionally empty *)
END NopFinally;

(* ---- Tests ---- *)

PROCEDURE TestSchedulerBasics;
VAR s: Scheduler; dw: BOOLEAN;
BEGIN
  st := SchedulerCreate(0, s);
  Check("sched create cap=0 => Invalid", st = Invalid);

  st := SchedulerCreate(64, s);
  Check("sched create cap=64 => OK", st = OK);
  Check("sched create => non-nil", s # NIL);

  st := SchedulerPump(s, 10, dw);
  Check("pump empty => OK", st = OK);
  Check("pump empty => no work", dw = FALSE);

  st := SchedulerDestroy(s);
  Check("sched destroy => OK", st = OK)
END TestSchedulerBasics;

PROCEDURE TestCreateAndFate;
VAR p: Promise; f: Future; fate: Fate;
BEGIN
  st := PromiseCreate(sched, p, f);
  Check("create => OK", st = OK);
  Check("create => p non-nil", p # NIL);
  Check("create => f non-nil", f # NIL);

  st := GetFate(f, fate);
  Check("initial fate => Pending", fate = Pending)
END TestCreateAndFate;

PROCEDURE TestResolve;
VAR
  p: Promise; f: Future;
  fate: Fate;
  settled: BOOLEAN;
  res: Result;
  v: Value;
BEGIN
  st := PromiseCreate(sched, p, f);
  MakeValue(42, NIL, v);
  st := Resolve(p, v);
  Check("resolve => OK", st = OK);

  st := GetFate(f, fate);
  Check("after resolve => Fulfilled", fate = Fulfilled);

  st := GetResultIfSettled(f, settled, res);
  Check("settled => TRUE", settled);
  Check("result isOk", res.isOk);
  Check("result tag=42", res.v.tag = 42);

  (* Double resolve *)
  st := Resolve(p, v);
  Check("double resolve => AlreadySettled", st = AlreadySettled)
END TestResolve;

PROCEDURE TestReject;
VAR
  p: Promise; f: Future;
  fate: Fate;
  settled: BOOLEAN;
  res: Result;
  e: Error;
BEGIN
  st := PromiseCreate(sched, p, f);
  MakeError(7, NIL, e);
  st := Reject(p, e);
  Check("reject => OK", st = OK);

  st := GetFate(f, fate);
  Check("after reject => Rejected", fate = Rejected);

  st := GetResultIfSettled(f, settled, res);
  Check("settled", settled);
  Check("result NOT isOk", NOT res.isOk);
  Check("error code=7", res.e.code = 7)
END TestReject;

PROCEDURE TestThenChain;
VAR
  p: Promise; f, f2, f3: Future;
  v: Value;
  settled: BOOLEAN;
  res: Result;
BEGIN
  st := PromiseCreate(sched, p, f);
  st := Map(sched, f, AddTenThen, NIL, f2);
  st := Map(sched, f2, AddTenThen, NIL, f3);
  Check("then chain => OK", st = OK);

  MakeValue(5, NIL, v);
  st := Resolve(p, v);
  PumpAll;

  st := GetResultIfSettled(f3, settled, res);
  Check("chain settled", settled);
  Check("chain result isOk", res.isOk);
  Check("5 + 10 + 10 = 25", res.v.tag = 25)
END TestThenChain;

PROCEDURE TestCatchRecovery;
VAR
  p: Promise; f, f2, f3: Future;
  e: Error;
  settled: BOOLEAN;
  res: Result;
BEGIN
  st := PromiseCreate(sched, p, f);
  st := OnReject(sched, f, RecoverCatch, NIL, f2);
  st := Map(sched, f2, AddTenThen, NIL, f3);

  MakeError(3, NIL, e);
  st := Reject(p, e);
  PumpAll;

  st := GetResultIfSettled(f3, settled, res);
  Check("catch recovery settled", settled);
  Check("catch recovery isOk", res.isOk);
  Check("recovered tag = -3 + 10 = 7", res.v.tag = 7)
END TestCatchRecovery;

PROCEDURE TestCatchPassthrough;
VAR
  p: Promise; f, f2: Future;
  v: Value;
  settled: BOOLEAN;
  res: Result;
BEGIN
  (* Catch should pass through on fulfillment *)
  st := PromiseCreate(sched, p, f);
  st := OnReject(sched, f, RecoverCatch, NIL, f2);

  MakeValue(100, NIL, v);
  st := Resolve(p, v);
  PumpAll;

  st := GetResultIfSettled(f2, settled, res);
  Check("catch passthrough settled", settled);
  Check("catch passthrough isOk", res.isOk);
  Check("catch passthrough tag=100", res.v.tag = 100)
END TestCatchPassthrough;

PROCEDURE TestFinally;
VAR
  p: Promise; f, f2: Future;
  v: Value;
  settled: BOOLEAN;
  res: Result;
BEGIN
  st := PromiseCreate(sched, p, f);
  st := OnSettle(sched, f, NopFinally, NIL, f2);

  MakeValue(88, NIL, v);
  st := Resolve(p, v);
  PumpAll;

  st := GetResultIfSettled(f2, settled, res);
  Check("finally settled", settled);
  Check("finally passthrough isOk", res.isOk);
  Check("finally passthrough tag=88", res.v.tag = 88)
END TestFinally;

PROCEDURE TestLateAttach;
VAR
  p: Promise; f, f2: Future;
  v: Value;
  settled: BOOLEAN;
  res: Result;
BEGIN
  (* Resolve before attaching Then *)
  st := PromiseCreate(sched, p, f);
  MakeValue(33, NIL, v);
  st := Resolve(p, v);

  st := Map(sched, f, AddTenThen, NIL, f2);
  PumpAll;

  st := GetResultIfSettled(f2, settled, res);
  Check("late attach settled", settled);
  Check("late attach isOk", res.isOk);
  Check("late attach 33+10=43", res.v.tag = 43)
END TestLateAttach;

PROCEDURE TestAllFulfill;
VAR
  p1, p2, p3: Promise;
  f1, f2, f3, fAll: Future;
  fs: ARRAY [0..2] OF Future;
  v: Value;
  settled: BOOLEAN;
  res: Result;
  rp: AllResultPtr;
BEGIN
  st := PromiseCreate(sched, p1, f1);
  st := PromiseCreate(sched, p2, f2);
  st := PromiseCreate(sched, p3, f3);
  fs[0] := f1; fs[1] := f2; fs[2] := f3;
  st := All(sched, fs, fAll);
  Check("all create => OK", st = OK);

  MakeValue(1, NIL, v); st := Resolve(p1, v);
  MakeValue(2, NIL, v); st := Resolve(p2, v);
  MakeValue(3, NIL, v); st := Resolve(p3, v);
  PumpAll;

  st := GetResultIfSettled(fAll, settled, res);
  Check("all settled", settled);
  Check("all isOk", res.isOk);
  Check("all count=3", res.v.tag = 3);

  rp := res.v.ptr;
  Check("all[0] tag=1", rp^[0].v.tag = 1);
  Check("all[1] tag=2", rp^[1].v.tag = 2);
  Check("all[2] tag=3", rp^[2].v.tag = 3)
END TestAllFulfill;

PROCEDURE TestAllReject;
VAR
  p1, p2: Promise;
  f1, f2, fAll: Future;
  fs: ARRAY [0..1] OF Future;
  v: Value;
  e: Error;
  settled: BOOLEAN;
  res: Result;
BEGIN
  st := PromiseCreate(sched, p1, f1);
  st := PromiseCreate(sched, p2, f2);
  fs[0] := f1; fs[1] := f2;
  st := All(sched, fs, fAll);

  MakeValue(1, NIL, v);  st := Resolve(p1, v);
  MakeError(9, NIL, e);  st := Reject(p2, e);
  PumpAll;

  st := GetResultIfSettled(fAll, settled, res);
  Check("all-reject settled", settled);
  Check("all-reject NOT isOk", NOT res.isOk);
  Check("all-reject code=9", res.e.code = 9)
END TestAllReject;

PROCEDURE TestRace;
VAR
  p1, p2, p3: Promise;
  f1, f2, f3, fRace: Future;
  fs: ARRAY [0..2] OF Future;
  v: Value;
  settled: BOOLEAN;
  res: Result;
BEGIN
  st := PromiseCreate(sched, p1, f1);
  st := PromiseCreate(sched, p2, f2);
  st := PromiseCreate(sched, p3, f3);
  fs[0] := f1; fs[1] := f2; fs[2] := f3;
  st := Race(sched, fs, fRace);

  (* p2 settles first *)
  MakeValue(77, NIL, v); st := Resolve(p2, v);
  PumpAll;

  st := GetResultIfSettled(fRace, settled, res);
  Check("race settled", settled);
  Check("race isOk", res.isOk);
  Check("race winner=77", res.v.tag = 77)
END TestRace;

PROCEDURE TestReentrantResolve;
VAR
  p1, p2: Promise;
  f1, f2, fOut: Future;
  v: Value;
  settled: BOOLEAN;
  res: Result;
BEGIN
  (* A Then callback that resolves another promise *)
  st := PromiseCreate(sched, p1, f1);
  st := PromiseCreate(sched, p2, f2);
  (* p2Addr will be resolved inside the callback *)
  st := Map(sched, f1, ResolveOtherThen, p2, fOut);

  MakeValue(50, NIL, v);
  st := Resolve(p1, v);
  PumpAll;

  st := GetResultIfSettled(f2, settled, res);
  Check("reentrant resolve settled", settled);
  Check("reentrant resolve isOk", res.isOk);
  Check("reentrant resolve tag=50", res.v.tag = 50)
END TestReentrantResolve;

(* Callback that resolves promise at user address *)
PROCEDURE ResolveOtherThen(res: Result; user: ADDRESS; VAR out: Result);
VAR v: Value;
BEGIN
  IF res.isOk THEN
    MakeValue(res.v.tag, NIL, v);
    st := Resolve(user, v)
  END;
  out := res
END ResolveOtherThen;

PROCEDURE TestHelpers;
VAR v: Value; e: Error; r: Result;
BEGIN
  MakeValue(5, NIL, v);
  Check("MakeValue tag=5", v.tag = 5);
  Check("MakeValue ptr=NIL", v.ptr = NIL);

  MakeError(3, NIL, e);
  Check("MakeError code=3", e.code = 3);

  Ok(v, r);
  Check("Ok isOk", r.isOk);
  Check("Ok tag=5", r.v.tag = 5);

  Fail(e, r);
  Check("Fail NOT isOk", NOT r.isOk);
  Check("Fail code=3", r.e.code = 3)
END TestHelpers;

BEGIN
  passed := 0;
  failed := 0;
  total  := 0;
  WriteString("=== m2futures Tests ==="); WriteLn;

  st := SchedulerCreate(1024, sched);

  TestSchedulerBasics;
  TestCreateAndFate;
  TestResolve;
  TestReject;
  TestThenChain;
  TestCatchRecovery;
  TestCatchPassthrough;
  TestFinally;
  TestLateAttach;
  TestAllFulfill;
  TestAllReject;
  TestRace;
  TestReentrantResolve;
  TestHelpers;

  st := SchedulerDestroy(sched);

  WriteLn;
  WriteString("Results: ");
  WriteInt(passed, 1); WriteString(" passed, ");
  WriteInt(failed, 1); WriteString(" failed, ");
  WriteInt(total, 1); WriteString(" total"); WriteLn;

  IF failed = 0 THEN
    WriteString("ALL TESTS PASSED"); WriteLn
  ELSE
    WriteString("SOME TESTS FAILED"); WriteLn
  END
END FuturesTests.
