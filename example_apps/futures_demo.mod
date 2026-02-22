MODULE FuturesDemo;
(* Demonstrates the m2futures Promises/Futures library.
   Shows: resolve/reject, Then, Catch, Finally, All, Race,
   and chaining with captured state. *)

FROM InOut IMPORT WriteString, WriteInt, WriteLn;
FROM SYSTEM IMPORT ADDRESS, ADR;
FROM Scheduler IMPORT Status, Scheduler,
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
  didWork: BOOLEAN;

(* ---- Pump helper: drain all pending work ---- *)

PROCEDURE PumpAll;
VAR dw: BOOLEAN;
BEGIN
  dw := TRUE;
  WHILE dw DO
    st := SchedulerPump(sched, 100, dw)
  END
END PumpAll;

(* ---- Callback: double an integer value ---- *)

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

(* ---- Callback: print result in a Then ---- *)

PROCEDURE PrintVal(res: Result; user: ADDRESS; VAR out: Result);
BEGIN
  WriteString("  Then => ");
  IF res.isOk THEN
    WriteString("ok, tag="); WriteInt(res.v.tag, 1)
  ELSE
    WriteString("err, code="); WriteInt(res.e.code, 1)
  END;
  WriteLn;
  out := res
END PrintVal;

(* ---- Callback: recover from an error ---- *)

PROCEDURE RecoverErr(err: Error; user: ADDRESS; VAR out: Result);
VAR v: Value;
BEGIN
  WriteString("  Catch => recovering from code ");
  WriteInt(err.code, 1);
  WriteLn;
  MakeValue(0, NIL, v);
  Ok(v, out)
END RecoverErr;

(* ---- Callback: finally observer ---- *)

PROCEDURE FinallyObs(res: Result; user: ADDRESS);
BEGIN
  WriteString("  Finally => settled, isOk=");
  IF res.isOk THEN WriteString("TRUE") ELSE WriteString("FALSE") END;
  WriteLn
END FinallyObs;

(* ---- Callback: print All aggregate ---- *)

PROCEDURE PrintAll(res: Result; user: ADDRESS; VAR out: Result);
VAR
  rp: AllResultPtr;
  i, n: INTEGER;
BEGIN
  WriteString("  All => ");
  IF res.isOk THEN
    n := res.v.tag;
    rp := res.v.ptr;
    WriteString("fulfilled, count="); WriteInt(n, 1);
    WriteString(" tags=[");
    FOR i := 0 TO n - 1 DO
      WriteInt(rp^[i].v.tag, 1);
      IF i < n - 1 THEN WriteString(",") END
    END;
    WriteString("]")
  ELSE
    WriteString("rejected, code="); WriteInt(res.e.code, 1)
  END;
  WriteLn;
  out := res
END PrintAll;

(* ---- Callback: print Race winner ---- *)

PROCEDURE PrintRace(res: Result; user: ADDRESS; VAR out: Result);
BEGIN
  WriteString("  Race => ");
  IF res.isOk THEN
    WriteString("winner tag="); WriteInt(res.v.tag, 1)
  ELSE
    WriteString("winner err code="); WriteInt(res.e.code, 1)
  END;
  WriteLn;
  out := res
END PrintRace;

(* ---- Demo sections ---- *)

PROCEDURE DemoResolveChain;
VAR
  p: Promise; f, f2, f3, f4: Future;
  v: Value;
BEGIN
  WriteString("== Resolve + Then chain =="); WriteLn;
  st := PromiseCreate(sched, p, f);
  st := Map(sched, f, DoubleVal, NIL, f2);
  st := Map(sched, f2, PrintVal, NIL, f3);
  st := OnSettle(sched, f3, FinallyObs, NIL, f4);

  MakeValue(21, NIL, v);
  st := Resolve(p, v);
  PumpAll;
  WriteLn
END DemoResolveChain;

PROCEDURE DemoRejectCatch;
VAR
  p: Promise; f, f2, f3, f4: Future;
  e: Error;
BEGIN
  WriteString("== Reject + Catch =="); WriteLn;
  st := PromiseCreate(sched, p, f);
  st := OnReject(sched, f, RecoverErr, NIL, f2);
  st := Map(sched, f2, PrintVal, NIL, f3);
  st := OnSettle(sched, f3, FinallyObs, NIL, f4);

  MakeError(42, NIL, e);
  st := Reject(p, e);
  PumpAll;
  WriteLn
END DemoRejectCatch;

PROCEDURE DemoAll;
VAR
  p1, p2, p3: Promise;
  f1, f2, f3: Future;
  fAll, fOut: Future;
  fs: ARRAY [0..2] OF Future;
  v: Value;
BEGIN
  WriteString("== All (join) =="); WriteLn;
  st := PromiseCreate(sched, p1, f1);
  st := PromiseCreate(sched, p2, f2);
  st := PromiseCreate(sched, p3, f3);
  fs[0] := f1; fs[1] := f2; fs[2] := f3;
  st := All(sched, fs, fAll);
  st := Map(sched, fAll, PrintAll, NIL, fOut);

  MakeValue(10, NIL, v); st := Resolve(p1, v);
  MakeValue(20, NIL, v); st := Resolve(p2, v);
  MakeValue(30, NIL, v); st := Resolve(p3, v);
  PumpAll;
  WriteLn
END DemoAll;

PROCEDURE DemoAllReject;
VAR
  p1, p2, p3: Promise;
  f1, f2, f3: Future;
  fAll, fOut: Future;
  fs: ARRAY [0..2] OF Future;
  v: Value;
  e: Error;
BEGIN
  WriteString("== All (first reject) =="); WriteLn;
  st := PromiseCreate(sched, p1, f1);
  st := PromiseCreate(sched, p2, f2);
  st := PromiseCreate(sched, p3, f3);
  fs[0] := f1; fs[1] := f2; fs[2] := f3;
  st := All(sched, fs, fAll);
  st := Map(sched, fAll, PrintAll, NIL, fOut);

  MakeValue(10, NIL, v);  st := Resolve(p1, v);
  MakeError(99, NIL, e);  st := Reject(p2, e);
  MakeValue(30, NIL, v);  st := Resolve(p3, v);
  PumpAll;
  WriteLn
END DemoAllReject;

PROCEDURE DemoRace;
VAR
  p1, p2, p3: Promise;
  f1, f2, f3: Future;
  fRace, fOut: Future;
  fs: ARRAY [0..2] OF Future;
  v: Value;
BEGIN
  WriteString("== Race =="); WriteLn;
  st := PromiseCreate(sched, p1, f1);
  st := PromiseCreate(sched, p2, f2);
  st := PromiseCreate(sched, p3, f3);
  fs[0] := f1; fs[1] := f2; fs[2] := f3;
  st := Race(sched, fs, fRace);
  st := Map(sched, fRace, PrintRace, NIL, fOut);

  (* Resolve p2 first -- it should win the race *)
  MakeValue(77, NIL, v); st := Resolve(p2, v);
  MakeValue(10, NIL, v); st := Resolve(p1, v);
  MakeValue(30, NIL, v); st := Resolve(p3, v);
  PumpAll;
  WriteLn
END DemoRace;

PROCEDURE DemoLateAttach;
VAR
  p: Promise; f, f2: Future;
  v: Value;
BEGIN
  WriteString("== Late attachment (resolve before Then) =="); WriteLn;
  st := PromiseCreate(sched, p, f);
  MakeValue(55, NIL, v);
  st := Resolve(p, v);
  (* Attach Then AFTER settlement -- should still fire *)
  st := Map(sched, f, PrintVal, NIL, f2);
  PumpAll;
  WriteLn
END DemoLateAttach;

BEGIN
  WriteString("=== m2futures Demo ==="); WriteLn; WriteLn;
  st := SchedulerCreate(1024, sched);
  IF st # OK THEN
    WriteString("Failed to create scheduler"); WriteLn;
    RETURN
  END;

  DemoResolveChain;
  DemoRejectCatch;
  DemoAll;
  DemoAllReject;
  DemoRace;
  DemoLateAttach;

  st := SchedulerDestroy(sched);
  WriteString("Done."); WriteLn
END FuturesDemo.
