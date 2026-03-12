MODULE PromiseTests;
(* Deterministic test suite for m2futures.
   Covers: scheduler basics, promise lifecycle, settlement,
   chaining (Map/OnReject/OnSettle), combinators (All/Race),
   cancellation tokens, and MapCancellable. *)

FROM SYSTEM IMPORT ADDRESS, ADR;
FROM InOut IMPORT WriteString, WriteLn, WriteInt;
FROM Scheduler IMPORT Scheduler, Status, TaskProc,
                      SchedulerCreate, SchedulerDestroy,
                      SchedulerEnqueue, SchedulerPump;
FROM Promise IMPORT Promise, Future, Fate, Value, Error, Result,
                    ThenFn, CatchFn, VoidFn,
                    AllResultPtr,
                    PromiseCreate, PromiseRelease, FutureRelease,
                    Resolve, Reject,
                    GetFate, GetResultIfSettled,
                    Map, OnReject, OnSettle,
                    All, Race,
                    CancelToken, CancelTokenCreate, CancelTokenDestroy,
                    Cancel, IsCancelled, OnCancel, MapCancellable,
                    MakeValue, MakeError, Ok, Fail;

VAR
  passed, failed, total: INTEGER;

  (* Shared scheduler for most tests *)
  sched: Scheduler;

  (* Callback tracking *)
  cbCount: CARDINAL;
  lastTag: INTEGER;
  lastCode: INTEGER;
  settleCount: CARDINAL;
  cancelCBCount: CARDINAL;

PROCEDURE Check(name: ARRAY OF CHAR; cond: BOOLEAN);
BEGIN
  INC(total);
  IF cond THEN
    INC(passed)
  ELSE
    INC(failed);
    WriteString("  FAIL: "); WriteString(name); WriteLn
  END
END Check;

(* Pump the scheduler until no more work remains. *)
PROCEDURE PumpAll(s: Scheduler);
VAR didWork: BOOLEAN; st: Status;
BEGIN
  LOOP
    st := SchedulerPump(s, 256, didWork);
    IF NOT didWork THEN EXIT END
  END
END PumpAll;

(* ================================================================
   Scheduler basics
   ================================================================ *)

PROCEDURE SimpleTask(ctx: ADDRESS);
BEGIN
  INC(cbCount)
END SimpleTask;

PROCEDURE TestSchedulerLifecycle;
VAR s: Scheduler; st: Status; didWork: BOOLEAN;
BEGIN
  WriteString("-- scheduler lifecycle"); WriteLn;

  st := SchedulerCreate(16, s);
  Check("sched: create ok", st = OK);
  Check("sched: not NIL", s # NIL);

  st := SchedulerDestroy(s);
  Check("sched: destroy ok", st = OK);

  st := SchedulerCreate(0, s);
  Check("sched: capacity 0 = Invalid", st = Invalid)
END TestSchedulerLifecycle;

PROCEDURE TestSchedulerEnqueuePump;
VAR s: Scheduler; st: Status; didWork: BOOLEAN;
BEGIN
  WriteString("-- scheduler enqueue/pump"); WriteLn;

  st := SchedulerCreate(8, s);
  Check("enq: create ok", st = OK);

  cbCount := 0;
  st := SchedulerEnqueue(s, SimpleTask, NIL);
  Check("enq: enqueue 1 ok", st = OK);
  st := SchedulerEnqueue(s, SimpleTask, NIL);
  Check("enq: enqueue 2 ok", st = OK);

  st := SchedulerPump(s, 10, didWork);
  Check("enq: pump ok", st = OK);
  Check("enq: didWork", didWork);
  Check("enq: ran 2 tasks", cbCount = 2);

  st := SchedulerPump(s, 10, didWork);
  Check("enq: empty pump no work", NOT didWork);

  st := SchedulerDestroy(s)
END TestSchedulerEnqueuePump;

(* ================================================================
   Promise creation and release
   ================================================================ *)

PROCEDURE TestPromiseCreateRelease;
VAR p: Promise; f: Future; st: Status; fate: Fate;
BEGIN
  WriteString("-- promise create/release"); WriteLn;

  st := PromiseCreate(sched, p, f);
  Check("create: ok", st = OK);
  Check("create: p not NIL", p # NIL);
  Check("create: f not NIL", f # NIL);
  Check("create: p = f (alias)", p = f);

  st := GetFate(f, fate);
  Check("create: fate Pending", fate = Pending);

  (* Release via FutureRelease — the alias pair has one ref *)
  FutureRelease(f);
  Check("release: f set NIL", f = NIL)
END TestPromiseCreateRelease;

PROCEDURE TestReleaseViaPromise;
VAR p: Promise; f: Future; st: Status;
BEGIN
  WriteString("-- release via PromiseRelease"); WriteLn;

  st := PromiseCreate(sched, p, f);
  Check("relp: create ok", st = OK);

  PromiseRelease(p);
  Check("relp: p set NIL", p = NIL)
END TestReleaseViaPromise;

(* ================================================================
   Settlement
   ================================================================ *)

PROCEDURE TestResolve;
VAR
  p: Promise; f: Future; st: Status;
  fate: Fate; settled: BOOLEAN; res: Result; v: Value;
BEGIN
  WriteString("-- resolve"); WriteLn;

  st := PromiseCreate(sched, p, f);
  Check("res: create ok", st = OK);

  MakeValue(42, NIL, v);
  st := Resolve(p, v);
  Check("res: resolve ok", st = OK);

  st := GetFate(f, fate);
  Check("res: Fulfilled", fate = Fulfilled);

  st := GetResultIfSettled(f, settled, res);
  Check("res: settled", settled);
  Check("res: isOk", res.isOk);
  Check("res: tag=42", res.v.tag = 42);

  (* Double settlement *)
  st := Resolve(p, v);
  Check("res: double = AlreadySettled", st = AlreadySettled);

  FutureRelease(f)
END TestResolve;

PROCEDURE TestReject;
VAR
  p: Promise; f: Future; st: Status;
  fate: Fate; settled: BOOLEAN; res: Result; e: Error;
BEGIN
  WriteString("-- reject"); WriteLn;

  st := PromiseCreate(sched, p, f);
  Check("rej: create ok", st = OK);

  MakeError(99, NIL, e);
  st := Reject(p, e);
  Check("rej: reject ok", st = OK);

  st := GetFate(f, fate);
  Check("rej: Rejected", fate = Rejected);

  st := GetResultIfSettled(f, settled, res);
  Check("rej: settled", settled);
  Check("rej: not isOk", NOT res.isOk);
  Check("rej: code=99", res.e.code = 99);

  FutureRelease(f)
END TestReject;

PROCEDURE TestPendingNotSettled;
VAR
  p: Promise; f: Future; st: Status;
  settled: BOOLEAN; res: Result;
BEGIN
  WriteString("-- pending not settled"); WriteLn;

  st := PromiseCreate(sched, p, f);
  Check("pend: create ok", st = OK);

  st := GetResultIfSettled(f, settled, res);
  Check("pend: not settled", NOT settled);

  FutureRelease(f)
END TestPendingNotSettled;

PROCEDURE TestNilArgs;
VAR
  p: Promise; f: Future; st: Status;
  fate: Fate; v: Value;
BEGIN
  WriteString("-- nil argument validation"); WriteLn;

  st := PromiseCreate(NIL, p, f);
  Check("nil: create NIL sched = Invalid", st = Invalid);

  st := GetFate(NIL, fate);
  Check("nil: GetFate NIL = Invalid", st = Invalid);

  st := Resolve(NIL, v);
  Check("nil: Resolve NIL = Invalid", st = Invalid)
END TestNilArgs;

(* ================================================================
   Chaining -- Map
   ================================================================ *)

PROCEDURE IncrementMap(inRes: Result; user: ADDRESS;
                       VAR outRes: Result);
VAR v: Value;
BEGIN
  INC(cbCount);
  IF inRes.isOk THEN
    MakeValue(inRes.v.tag + 1, NIL, v);
    Ok(v, outRes)
  ELSE
    outRes := inRes
  END
END IncrementMap;

PROCEDURE TestMapBeforeSettle;
VAR
  p: Promise; f1, f2: Future; st: Status;
  settled: BOOLEAN; res: Result; v: Value;
BEGIN
  WriteString("-- map before settlement"); WriteLn;

  st := PromiseCreate(sched, p, f1);
  Check("mapb: create ok", st = OK);

  cbCount := 0;
  st := Map(sched, f1, IncrementMap, NIL, f2);
  Check("mapb: attach ok", st = OK);

  (* Not yet settled *)
  st := GetResultIfSettled(f2, settled, res);
  Check("mapb: output not settled yet", NOT settled);

  (* Resolve input *)
  MakeValue(10, NIL, v);
  st := Resolve(p, v);
  Check("mapb: resolve ok", st = OK);

  PumpAll(sched);

  st := GetResultIfSettled(f2, settled, res);
  Check("mapb: output settled", settled);
  Check("mapb: cb ran", cbCount = 1);
  Check("mapb: value=11", res.isOk AND (res.v.tag = 11));

  FutureRelease(f1);
  FutureRelease(f2)
END TestMapBeforeSettle;

PROCEDURE TestMapAfterSettle;
VAR
  p: Promise; f1, f2: Future; st: Status;
  settled: BOOLEAN; res: Result; v: Value;
BEGIN
  WriteString("-- map after settlement (immediate enqueue)"); WriteLn;

  st := PromiseCreate(sched, p, f1);
  Check("mapa: create ok", st = OK);

  MakeValue(20, NIL, v);
  st := Resolve(p, v);
  Check("mapa: resolve ok", st = OK);

  cbCount := 0;
  st := Map(sched, f1, IncrementMap, NIL, f2);
  Check("mapa: attach ok", st = OK);

  PumpAll(sched);

  st := GetResultIfSettled(f2, settled, res);
  Check("mapa: output settled", settled);
  Check("mapa: cb ran", cbCount = 1);
  Check("mapa: value=21", res.isOk AND (res.v.tag = 21));

  FutureRelease(f1);
  FutureRelease(f2)
END TestMapAfterSettle;

PROCEDURE TestMapChain;
VAR
  p: Promise; f1, f2, f3: Future; st: Status;
  settled: BOOLEAN; res: Result; v: Value;
BEGIN
  WriteString("-- map chain (two deep)"); WriteLn;

  st := PromiseCreate(sched, p, f1);
  Check("chain: create ok", st = OK);

  cbCount := 0;
  st := Map(sched, f1, IncrementMap, NIL, f2);
  Check("chain: map1 ok", st = OK);
  st := Map(sched, f2, IncrementMap, NIL, f3);
  Check("chain: map2 ok", st = OK);

  MakeValue(0, NIL, v);
  st := Resolve(p, v);
  PumpAll(sched);

  st := GetResultIfSettled(f3, settled, res);
  Check("chain: output settled", settled);
  Check("chain: 2 callbacks", cbCount = 2);
  Check("chain: value=2", res.isOk AND (res.v.tag = 2));

  FutureRelease(f1);
  FutureRelease(f2);
  FutureRelease(f3)
END TestMapChain;

(* ================================================================
   Chaining -- OnReject
   ================================================================ *)

PROCEDURE RecoverCatch(inErr: Error; user: ADDRESS;
                       VAR outRes: Result);
VAR v: Value;
BEGIN
  INC(cbCount);
  lastCode := inErr.code;
  MakeValue(777, NIL, v);
  Ok(v, outRes)
END RecoverCatch;

PROCEDURE TestOnRejectTriggered;
VAR
  p: Promise; f1, f2: Future; st: Status;
  settled: BOOLEAN; res: Result; e: Error;
BEGIN
  WriteString("-- onReject triggered on rejection"); WriteLn;

  st := PromiseCreate(sched, p, f1);
  cbCount := 0;
  lastCode := 0;
  st := OnReject(sched, f1, RecoverCatch, NIL, f2);
  Check("catch: attach ok", st = OK);

  MakeError(55, NIL, e);
  st := Reject(p, e);
  PumpAll(sched);

  st := GetResultIfSettled(f2, settled, res);
  Check("catch: settled", settled);
  Check("catch: cb ran", cbCount = 1);
  Check("catch: saw code=55", lastCode = 55);
  Check("catch: recovered to ok", res.isOk AND (res.v.tag = 777));

  FutureRelease(f1);
  FutureRelease(f2)
END TestOnRejectTriggered;

PROCEDURE TestOnRejectPassthrough;
VAR
  p: Promise; f1, f2: Future; st: Status;
  settled: BOOLEAN; res: Result; v: Value;
BEGIN
  WriteString("-- onReject passthrough on fulfillment"); WriteLn;

  st := PromiseCreate(sched, p, f1);
  cbCount := 0;
  st := OnReject(sched, f1, RecoverCatch, NIL, f2);

  MakeValue(88, NIL, v);
  st := Resolve(p, v);
  PumpAll(sched);

  st := GetResultIfSettled(f2, settled, res);
  Check("pass: settled", settled);
  Check("pass: cb not called", cbCount = 0);
  Check("pass: value passed through", res.isOk AND (res.v.tag = 88));

  FutureRelease(f1);
  FutureRelease(f2)
END TestOnRejectPassthrough;

(* ================================================================
   Chaining -- OnSettle
   ================================================================ *)

PROCEDURE SettleObserver(inRes: Result; user: ADDRESS);
BEGIN
  INC(settleCount);
  IF inRes.isOk THEN
    lastTag := inRes.v.tag
  ELSE
    lastCode := inRes.e.code
  END
END SettleObserver;

PROCEDURE TestOnSettleFulfill;
VAR
  p: Promise; f1, f2: Future; st: Status;
  settled: BOOLEAN; res: Result; v: Value;
BEGIN
  WriteString("-- onSettle on fulfillment"); WriteLn;

  st := PromiseCreate(sched, p, f1);
  settleCount := 0;
  lastTag := 0;
  st := OnSettle(sched, f1, SettleObserver, NIL, f2);
  Check("sf: attach ok", st = OK);

  MakeValue(33, NIL, v);
  st := Resolve(p, v);
  PumpAll(sched);

  st := GetResultIfSettled(f2, settled, res);
  Check("sf: settled", settled);
  Check("sf: observer ran", settleCount = 1);
  Check("sf: saw tag=33", lastTag = 33);
  Check("sf: result passes through", res.isOk AND (res.v.tag = 33));

  FutureRelease(f1);
  FutureRelease(f2)
END TestOnSettleFulfill;

PROCEDURE TestOnSettleReject;
VAR
  p: Promise; f1, f2: Future; st: Status;
  settled: BOOLEAN; res: Result; e: Error;
BEGIN
  WriteString("-- onSettle on rejection"); WriteLn;

  st := PromiseCreate(sched, p, f1);
  settleCount := 0;
  lastCode := 0;
  st := OnSettle(sched, f1, SettleObserver, NIL, f2);

  MakeError(44, NIL, e);
  st := Reject(p, e);
  PumpAll(sched);

  st := GetResultIfSettled(f2, settled, res);
  Check("sr: settled", settled);
  Check("sr: observer ran", settleCount = 1);
  Check("sr: saw code=44", lastCode = 44);
  Check("sr: rejection passes through", (NOT res.isOk) AND (res.e.code = 44));

  FutureRelease(f1);
  FutureRelease(f2)
END TestOnSettleReject;

(* ================================================================
   Combinators -- All
   ================================================================ *)

PROCEDURE TestAllSuccess;
VAR
  p1, p2, p3: Promise;
  f1, f2, f3, fAll: Future;
  st: Status;
  settled: BOOLEAN;
  res: Result;
  v: Value;
  fs: ARRAY [0..2] OF Future;
  arp: AllResultPtr;
BEGIN
  WriteString("-- all success"); WriteLn;

  st := PromiseCreate(sched, p1, f1);
  st := PromiseCreate(sched, p2, f2);
  st := PromiseCreate(sched, p3, f3);

  fs[0] := f1;
  fs[1] := f2;
  fs[2] := f3;
  st := All(sched, fs, fAll);
  Check("all: create ok", st = OK);

  (* Resolve in non-sequential order *)
  MakeValue(10, NIL, v);
  st := Resolve(p2, v);
  MakeValue(20, NIL, v);
  st := Resolve(p3, v);
  MakeValue(30, NIL, v);
  st := Resolve(p1, v);

  PumpAll(sched);

  st := GetResultIfSettled(fAll, settled, res);
  Check("all: settled", settled);
  Check("all: isOk", res.isOk);
  Check("all: tag=3 (count)", res.v.tag = 3);

  arp := res.v.ptr;
  Check("all: result[0] ok", arp^[0].isOk AND (arp^[0].v.tag = 30));
  Check("all: result[1] ok", arp^[1].isOk AND (arp^[1].v.tag = 10));
  Check("all: result[2] ok", arp^[2].isOk AND (arp^[2].v.tag = 20));

  FutureRelease(f1);
  FutureRelease(f2);
  FutureRelease(f3);
  FutureRelease(fAll)
END TestAllSuccess;

PROCEDURE TestAllFirstRejection;
VAR
  p1, p2: Promise;
  f1, f2, fAll: Future;
  st: Status;
  settled: BOOLEAN;
  res: Result;
  v: Value;
  e: Error;
  fs: ARRAY [0..1] OF Future;
BEGIN
  WriteString("-- all first rejection"); WriteLn;

  st := PromiseCreate(sched, p1, f1);
  st := PromiseCreate(sched, p2, f2);

  fs[0] := f1;
  fs[1] := f2;
  st := All(sched, fs, fAll);
  Check("allf: create ok", st = OK);

  MakeError(77, NIL, e);
  st := Reject(p1, e);
  PumpAll(sched);

  st := GetResultIfSettled(fAll, settled, res);
  Check("allf: settled", settled);
  Check("allf: rejected", NOT res.isOk);
  Check("allf: code=77", res.e.code = 77);

  (* Resolve the other -- should be harmless *)
  MakeValue(1, NIL, v);
  st := Resolve(p2, v);
  PumpAll(sched);

  FutureRelease(f1);
  FutureRelease(f2);
  FutureRelease(fAll)
END TestAllFirstRejection;

(* ================================================================
   Combinators -- Race
   ================================================================ *)

PROCEDURE TestRaceFirstWins;
VAR
  p1, p2: Promise;
  f1, f2, fRace: Future;
  st: Status;
  settled: BOOLEAN;
  res: Result;
  v: Value;
  fs: ARRAY [0..1] OF Future;
BEGIN
  WriteString("-- race first wins"); WriteLn;

  st := PromiseCreate(sched, p1, f1);
  st := PromiseCreate(sched, p2, f2);

  fs[0] := f1;
  fs[1] := f2;
  st := Race(sched, fs, fRace);
  Check("race: create ok", st = OK);

  MakeValue(50, NIL, v);
  st := Resolve(p2, v);
  PumpAll(sched);

  st := GetResultIfSettled(fRace, settled, res);
  Check("race: settled", settled);
  Check("race: isOk", res.isOk);
  Check("race: tag=50 (from p2)", res.v.tag = 50);

  (* Late resolve of p1 -- harmless *)
  MakeValue(60, NIL, v);
  st := Resolve(p1, v);
  PumpAll(sched);

  FutureRelease(f1);
  FutureRelease(f2);
  FutureRelease(fRace)
END TestRaceFirstWins;

PROCEDURE TestRaceRejectWins;
VAR
  p1, p2: Promise;
  f1, f2, fRace: Future;
  st: Status;
  settled: BOOLEAN;
  res: Result;
  e: Error;
  v: Value;
  fs: ARRAY [0..1] OF Future;
BEGIN
  WriteString("-- race rejection wins"); WriteLn;

  st := PromiseCreate(sched, p1, f1);
  st := PromiseCreate(sched, p2, f2);

  fs[0] := f1;
  fs[1] := f2;
  st := Race(sched, fs, fRace);

  MakeError(66, NIL, e);
  st := Reject(p1, e);
  PumpAll(sched);

  st := GetResultIfSettled(fRace, settled, res);
  Check("racej: settled", settled);
  Check("racej: rejected", NOT res.isOk);
  Check("racej: code=66", res.e.code = 66);

  MakeValue(1, NIL, v);
  st := Resolve(p2, v);
  PumpAll(sched);

  FutureRelease(f1);
  FutureRelease(f2);
  FutureRelease(fRace)
END TestRaceRejectWins;

(* ================================================================
   Cancellation -- Token basics
   ================================================================ *)

PROCEDURE TestCancelTokenBasics;
VAR ct: CancelToken; st: Status;
BEGIN
  WriteString("-- cancel token basics"); WriteLn;

  st := CancelTokenCreate(sched, ct);
  Check("ct: create ok", st = OK);
  Check("ct: not NIL", ct # NIL);
  Check("ct: not cancelled", NOT IsCancelled(ct));

  Cancel(ct);
  Check("ct: now cancelled", IsCancelled(ct));

  CancelTokenDestroy(ct);
  Check("ct: destroy sets NIL", ct = NIL)
END TestCancelTokenBasics;

PROCEDURE TestCancelTokenIdempotent;
VAR ct: CancelToken; st: Status;
BEGIN
  WriteString("-- cancel is idempotent"); WriteLn;

  st := CancelTokenCreate(sched, ct);
  Cancel(ct);
  Cancel(ct);  (* second call should be harmless *)
  Check("ct2: still cancelled", IsCancelled(ct));

  CancelTokenDestroy(ct)
END TestCancelTokenIdempotent;

(* ================================================================
   Cancellation -- OnCancel callbacks
   ================================================================ *)

PROCEDURE CancelObserver(inRes: Result; user: ADDRESS);
BEGIN
  INC(cancelCBCount)
END CancelObserver;

PROCEDURE TestOnCancelBeforeCancel;
VAR ct: CancelToken; st: Status;
BEGIN
  WriteString("-- OnCancel registered before Cancel"); WriteLn;

  st := CancelTokenCreate(sched, ct);
  cancelCBCount := 0;

  OnCancel(ct, CancelObserver, NIL);
  OnCancel(ct, CancelObserver, NIL);

  (* Not yet cancelled -- callbacks should not fire *)
  PumpAll(sched);
  Check("ocb: no callbacks yet", cancelCBCount = 0);

  Cancel(ct);
  PumpAll(sched);
  Check("ocb: 2 callbacks fired", cancelCBCount = 2);

  CancelTokenDestroy(ct)
END TestOnCancelBeforeCancel;

PROCEDURE TestOnCancelAfterCancel;
VAR ct: CancelToken; st: Status;
BEGIN
  WriteString("-- OnCancel registered after Cancel"); WriteLn;

  st := CancelTokenCreate(sched, ct);
  cancelCBCount := 0;

  Cancel(ct);
  PumpAll(sched);

  OnCancel(ct, CancelObserver, NIL);
  PumpAll(sched);
  Check("oca: callback fired", cancelCBCount = 1);

  CancelTokenDestroy(ct)
END TestOnCancelAfterCancel;

(* ================================================================
   Cancellation -- Destroy safe after Cancel
   ================================================================ *)

PROCEDURE TestDestroyAfterCancel;
VAR ct: CancelToken; st: Status;
BEGIN
  WriteString("-- CancelTokenDestroy safe after Cancel"); WriteLn;

  st := CancelTokenCreate(sched, ct);
  cancelCBCount := 0;

  OnCancel(ct, CancelObserver, NIL);
  Cancel(ct);

  (* Destroy BEFORE pumping -- the dispatch ref must keep
     the token alive so ExecCancelCB doesn't use freed memory. *)
  CancelTokenDestroy(ct);
  Check("dac: ct set NIL", ct = NIL);

  PumpAll(sched);
  Check("dac: callback still fired", cancelCBCount = 1)
END TestDestroyAfterCancel;

(* ================================================================
   MapCancellable
   ================================================================ *)

PROCEDURE PassthroughMap(inRes: Result; user: ADDRESS;
                         VAR outRes: Result);
BEGIN
  INC(cbCount);
  outRes := inRes
END PassthroughMap;

PROCEDURE TestMapCancellableNotCancelled;
VAR
  p: Promise; f1, f2: Future; st: Status;
  ct: CancelToken;
  settled: BOOLEAN; res: Result; v: Value;
BEGIN
  WriteString("-- MapCancellable (not cancelled)"); WriteLn;

  st := PromiseCreate(sched, p, f1);
  st := CancelTokenCreate(sched, ct);
  cbCount := 0;

  st := MapCancellable(sched, f1, PassthroughMap, NIL, ct, f2);
  Check("mcnc: attach ok", st = OK);

  MakeValue(100, NIL, v);
  st := Resolve(p, v);
  PumpAll(sched);

  st := GetResultIfSettled(f2, settled, res);
  Check("mcnc: settled", settled);
  Check("mcnc: cb ran", cbCount = 1);
  Check("mcnc: value=100", res.isOk AND (res.v.tag = 100));

  FutureRelease(f1);
  FutureRelease(f2);
  CancelTokenDestroy(ct)
END TestMapCancellableNotCancelled;

PROCEDURE TestMapCancellableCancelled;
VAR
  p: Promise; f1, f2: Future; st: Status;
  ct: CancelToken;
  settled: BOOLEAN; res: Result; v: Value;
BEGIN
  WriteString("-- MapCancellable (cancelled before settle)"); WriteLn;

  st := PromiseCreate(sched, p, f1);
  st := CancelTokenCreate(sched, ct);
  cbCount := 0;

  st := MapCancellable(sched, f1, PassthroughMap, NIL, ct, f2);
  Check("mcc: attach ok", st = OK);

  Cancel(ct);

  MakeValue(200, NIL, v);
  st := Resolve(p, v);
  PumpAll(sched);

  st := GetResultIfSettled(f2, settled, res);
  Check("mcc: settled", settled);
  Check("mcc: user cb NOT called", cbCount = 0);
  Check("mcc: rejected with code -1",
        (NOT res.isOk) AND (res.e.code = -1));

  FutureRelease(f1);
  FutureRelease(f2);
  CancelTokenDestroy(ct)
END TestMapCancellableCancelled;

PROCEDURE TestMapCancellableDestroyBeforePump;
VAR
  p: Promise; f1, f2: Future; st: Status;
  ct: CancelToken;
  settled: BOOLEAN; res: Result; v: Value;
BEGIN
  WriteString("-- MapCancellable token destroyed before pump"); WriteLn;

  st := PromiseCreate(sched, p, f1);
  st := CancelTokenCreate(sched, ct);
  cbCount := 0;

  st := MapCancellable(sched, f1, PassthroughMap, NIL, ct, f2);
  Check("mcd: attach ok", st = OK);

  (* Destroy external token ref. Internal ref from MapCancellable
     still holds the token alive for CancellableThen. *)
  CancelTokenDestroy(ct);
  Check("mcd: ct set NIL", ct = NIL);

  MakeValue(300, NIL, v);
  st := Resolve(p, v);
  PumpAll(sched);

  st := GetResultIfSettled(f2, settled, res);
  Check("mcd: settled", settled);
  Check("mcd: cb ran (not cancelled)", cbCount = 1);
  Check("mcd: value=300", res.isOk AND (res.v.tag = 300));

  FutureRelease(f1);
  FutureRelease(f2)
END TestMapCancellableDestroyBeforePump;

(* ================================================================
   Main
   ================================================================ *)

BEGIN
  passed := 0;
  failed := 0;
  total := 0;

  SchedulerCreate(256, sched);

  WriteString("m2futures test suite"); WriteLn;
  WriteString("===================="); WriteLn;

  TestSchedulerLifecycle;
  TestSchedulerEnqueuePump;
  TestPromiseCreateRelease;
  TestReleaseViaPromise;
  TestResolve;
  TestReject;
  TestPendingNotSettled;
  TestNilArgs;
  TestMapBeforeSettle;
  TestMapAfterSettle;
  TestMapChain;
  TestOnRejectTriggered;
  TestOnRejectPassthrough;
  TestOnSettleFulfill;
  TestOnSettleReject;
  TestAllSuccess;
  TestAllFirstRejection;
  TestRaceFirstWins;
  TestRaceRejectWins;
  TestCancelTokenBasics;
  TestCancelTokenIdempotent;
  TestOnCancelBeforeCancel;
  TestOnCancelAfterCancel;
  TestDestroyAfterCancel;
  TestMapCancellableNotCancelled;
  TestMapCancellableCancelled;
  TestMapCancellableDestroyBeforePump;

  SchedulerDestroy(sched);

  WriteLn;
  WriteInt(total, 0); WriteString(" tests, ");
  WriteInt(passed, 0); WriteString(" passed, ");
  WriteInt(failed, 0); WriteString(" failed"); WriteLn;

  IF failed > 0 THEN
    WriteString("*** FAILURES ***"); WriteLn
  ELSE
    WriteString("*** ALL TESTS PASSED ***"); WriteLn
  END
END PromiseTests.
