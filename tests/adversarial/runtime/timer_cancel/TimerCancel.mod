MODULE TimerCancel;

(* Tests EventLoop timer creation, firing, and cancellation.
   Deterministic: no network, no timing sensitivity. *)

FROM SYSTEM IMPORT ADDRESS;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;
FROM EventLoop IMPORT Loop, Create, Destroy, GetScheduler,
                     SetTimeout, CancelTimer, RunOnce;
FROM Scheduler IMPORT Scheduler, TaskProc;
FROM Timers IMPORT TimerId;

VAR
  lp: Loop;
  sched: Scheduler;
  firedA, firedB, firedC: INTEGER;
  pass, fail, total: INTEGER;

PROCEDURE Check(cond: BOOLEAN; name: ARRAY OF CHAR);
BEGIN
  total := total + 1;
  IF cond THEN
    pass := pass + 1;
    WriteString("  PASS: "); WriteString(name); WriteLn
  ELSE
    fail := fail + 1;
    WriteString("  FAIL: "); WriteString(name); WriteLn
  END
END Check;

PROCEDURE OnTimerA(user: ADDRESS);
BEGIN firedA := firedA + 1 END OnTimerA;

PROCEDURE OnTimerB(user: ADDRESS);
BEGIN firedB := firedB + 1 END OnTimerB;

PROCEDURE OnTimerC(user: ADDRESS);
BEGIN firedC := firedC + 1 END OnTimerC;

VAR
  est: INTEGER;
  idA, idB, idC: TimerId;
  hasMore: BOOLEAN;
  i: INTEGER;

BEGIN
  pass := 0; fail := 0; total := 0;
  firedA := 0; firedB := 0; firedC := 0;

  WriteString("--- Timer cancel tests ---"); WriteLn;

  est := ORD(Create(lp));
  Check(est = 0, "create loop");
  sched := GetScheduler(lp);

  (* Set three timers: 10ms, 20ms, 30ms *)
  est := ORD(SetTimeout(lp, 10, OnTimerA, NIL, idA));
  Check(est = 0, "set timer A");

  est := ORD(SetTimeout(lp, 20, OnTimerB, NIL, idB));
  Check(est = 0, "set timer B");

  est := ORD(SetTimeout(lp, 30, OnTimerC, NIL, idC));
  Check(est = 0, "set timer C");

  (* Cancel timer B before it fires *)
  est := ORD(CancelTimer(lp, idB));
  Check(est = 0, "cancel timer B");

  (* Pump the loop to let A and C fire *)
  FOR i := 0 TO 20 DO
    hasMore := RunOnce(lp)
  END;

  Check(firedA >= 1, "timer A fired");
  Check(firedB = 0, "timer B did NOT fire");
  Check(firedC >= 1, "timer C fired");

  est := ORD(Destroy(lp));

  WriteLn;
  WriteString("Results: ");
  WriteInt(pass, 0); WriteString(" passed, ");
  WriteInt(fail, 0); WriteString(" failed / ");
  WriteInt(total, 0); WriteString(" total"); WriteLn;

  IF fail > 0 THEN HALT END
END TimerCancel.
