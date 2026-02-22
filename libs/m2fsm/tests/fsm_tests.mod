MODULE FsmTests;
(* Deterministic test suite for m2fsm.

   Tests:
     1.  basic          3-state, 3-event transition sequence
     2.  no_transition  Missing transition; state unchanged
     3.  guard_reject   Guard denies; rejected counter increments
     4.  hooks_order    onExit before state change, onEnter after
     5.  action_fail    Action returns ok=FALSE; Error status
     6.  trace          Trace receives correct tuples
     7.  reset          Reset restores start state and counters
     8.  bounds         Out-of-range event => Error
     9.  clear_table    ClearTable fills NoState entries
    10.  set_trans      SetTrans fills fields correctly *)

FROM SYSTEM IMPORT ADDRESS, ADR;
FROM InOut IMPORT WriteString, WriteLn, WriteInt, WriteCard;
FROM Fsm IMPORT Fsm, Transition, StepStatus,
                Ok, NoTransition, GuardRejected, Error,
                ActionProc, GuardProc, HookProc, TraceProc,
                StateId, EventId, ActionId, GuardId,
                NoState, NoAction, NoGuard;

VAR
  passed, failed, total: INTEGER;

  (* Action tracking *)
  actionCallCount: CARDINAL;
  lastActionEv: CARDINAL;

  (* Guard tracking *)
  guardCallCount: CARDINAL;
  guardShouldReject: BOOLEAN;

  (* Hook tracking *)
  hookLog: ARRAY [0..15] OF CARDINAL;
  hookIdx: CARDINAL;

  (* Hook ordering: FSM state seen during hook calls *)
  exitSeenState: CARDINAL;
  enterSeenState: CARDINAL;

  (* Trace tracking *)
  traceFrom, traceTo, traceEv, traceAct: CARDINAL;
  traceSt: StepStatus;
  traceCallCount: CARDINAL;

TYPE
  FsmPtr = POINTER TO Fsm;

PROCEDURE Check(name: ARRAY OF CHAR; cond: BOOLEAN);
BEGIN
  INC(total);
  IF cond THEN
    INC(passed)
  ELSE
    INC(failed);
    WriteString("FAIL: "); WriteString(name); WriteLn
  END
END Check;

(* ── Callback procedures ─────────────────────────────── *)

PROCEDURE GoodAction(ctx: ADDRESS; ev: EventId;
                     payload: ADDRESS; VAR ok: BOOLEAN);
BEGIN
  INC(actionCallCount);
  lastActionEv := ev;
  ok := TRUE
END GoodAction;

PROCEDURE BadAction(ctx: ADDRESS; ev: EventId;
                    payload: ADDRESS; VAR ok: BOOLEAN);
BEGIN
  INC(actionCallCount);
  ok := FALSE
END BadAction;

PROCEDURE TestGuard(ctx: ADDRESS; ev: EventId;
                    payload: ADDRESS; VAR allow: BOOLEAN);
BEGIN
  INC(guardCallCount);
  allow := NOT guardShouldReject
END TestGuard;

PROCEDURE ExitHook(ctx: ADDRESS; st: StateId; payload: ADDRESS);
VAR fp: FsmPtr;
BEGIN
  fp := ctx;
  exitSeenState := fp^.state;
  hookLog[hookIdx] := 200 + st;
  INC(hookIdx)
END ExitHook;

PROCEDURE EnterHook(ctx: ADDRESS; st: StateId; payload: ADDRESS);
VAR fp: FsmPtr;
BEGIN
  fp := ctx;
  enterSeenState := fp^.state;
  hookLog[hookIdx] := 100 + st;
  INC(hookIdx)
END EnterHook;

PROCEDURE TraceCb(ctx: ADDRESS; fromState, toState: StateId;
                  ev: EventId; action: ActionId;
                  status: StepStatus);
BEGIN
  traceFrom := fromState;
  traceTo := toState;
  traceEv := ev;
  traceAct := action;
  traceSt := status;
  INC(traceCallCount)
END TraceCb;

(* ── Test 1: Basic transitions ────────────────────── *)

PROCEDURE TestBasic;
CONST
  NS = 3; NE = 3;
VAR
  f: Fsm;
  table: ARRAY [0..8] OF Transition;
  acts: ARRAY [0..2] OF ActionProc;
  status: StepStatus;
BEGIN
  Fsm.ClearTable(ADR(table), NS * NE);

  (* S0+E0 -> S1, action 1 *)
  Fsm.SetTrans(table[0*NE+0], 1, 1, NoGuard);
  (* S1+E1 -> S2, action 2 *)
  Fsm.SetTrans(table[1*NE+1], 2, 2, NoGuard);
  (* S2+E2 -> S0, no action *)
  Fsm.SetTrans(table[2*NE+2], 0, NoAction, NoGuard);

  acts[0] := NIL;
  acts[1] := GoodAction;
  acts[2] := GoodAction;

  Fsm.Init(f, 0, NIL, NS, NE, ADR(table));
  Fsm.SetActions(f, ADR(acts), 3);
  actionCallCount := 0;

  Fsm.Step(f, 0, NIL, status);
  Check("basic: S0+E0 ok", status = Ok);
  Check("basic: state=1", Fsm.CurrentState(f) = 1);
  Check("basic: action called", actionCallCount = 1);

  Fsm.Step(f, 1, NIL, status);
  Check("basic: S1+E1 ok", status = Ok);
  Check("basic: state=2", Fsm.CurrentState(f) = 2);

  Fsm.Step(f, 2, NIL, status);
  Check("basic: S2+E2 ok", status = Ok);
  Check("basic: state=0", Fsm.CurrentState(f) = 0);

  Check("basic: steps=3", Fsm.StepCount(f) = 3);
  Check("basic: actions=2", actionCallCount = 2)
END TestBasic;

(* ── Test 2: NoTransition ─────────────────────────── *)

PROCEDURE TestNoTransition;
CONST
  NS = 3; NE = 3;
VAR
  f: Fsm;
  table: ARRAY [0..8] OF Transition;
  status: StepStatus;
BEGIN
  Fsm.ClearTable(ADR(table), NS * NE);
  (* Only S0+E0 -> S1 defined *)
  Fsm.SetTrans(table[0*NE+0], 1, NoAction, NoGuard);

  Fsm.Init(f, 0, NIL, NS, NE, ADR(table));

  (* E1 has no transition from S0 *)
  Fsm.Step(f, 1, NIL, status);
  Check("notrans: status", status = NoTransition);
  Check("notrans: state unchanged", Fsm.CurrentState(f) = 0);
  Check("notrans: invalid=1", Fsm.InvalidCount(f) = 1);

  (* E2 also has no transition from S0 *)
  Fsm.Step(f, 2, NIL, status);
  Check("notrans: invalid=2", Fsm.InvalidCount(f) = 2);
  Check("notrans: steps=0", Fsm.StepCount(f) = 0)
END TestNoTransition;

(* ── Test 3: Guard rejected ───────────────────────── *)

PROCEDURE TestGuardReject;
CONST
  NS = 2; NE = 2;
VAR
  f: Fsm;
  table: ARRAY [0..3] OF Transition;
  guards: ARRAY [0..1] OF GuardProc;
  status: StepStatus;
BEGIN
  Fsm.ClearTable(ADR(table), NS * NE);
  (* S0+E0 -> S1, guard 1 *)
  Fsm.SetTrans(table[0*NE+0], 1, NoAction, 1);

  guards[0] := NIL;
  guards[1] := TestGuard;

  Fsm.Init(f, 0, NIL, NS, NE, ADR(table));
  Fsm.SetGuards(f, ADR(guards), 2);

  (* Guard allows *)
  guardShouldReject := FALSE;
  guardCallCount := 0;
  Fsm.Step(f, 0, NIL, status);
  Check("guard: allow ok", status = Ok);
  Check("guard: state=1", Fsm.CurrentState(f) = 1);
  Check("guard: called once", guardCallCount = 1);

  (* Reset and try with rejection *)
  Fsm.Reset(f);
  guardShouldReject := TRUE;
  guardCallCount := 0;
  Fsm.Step(f, 0, NIL, status);
  Check("guard: reject status", status = GuardRejected);
  Check("guard: state unchanged", Fsm.CurrentState(f) = 0);
  Check("guard: reject count=1", Fsm.RejectCount(f) = 1);
  Check("guard: guard called", guardCallCount = 1)
END TestGuardReject;

(* ── Test 4: Hook ordering ────────────────────────── *)

PROCEDURE TestHooksOrder;
CONST
  NS = 3; NE = 2;
VAR
  f: Fsm;
  table: ARRAY [0..5] OF Transition;
  enter: ARRAY [0..2] OF HookProc;
  exitH: ARRAY [0..2] OF HookProc;
  status: StepStatus;
BEGIN
  Fsm.ClearTable(ADR(table), NS * NE);
  (* S0+E0 -> S1 *)
  Fsm.SetTrans(table[0*NE+0], 1, NoAction, NoGuard);
  (* S1+E1 -> S2 *)
  Fsm.SetTrans(table[1*NE+1], 2, NoAction, NoGuard);

  enter[0] := EnterHook;
  enter[1] := EnterHook;
  enter[2] := EnterHook;
  exitH[0] := ExitHook;
  exitH[1] := ExitHook;
  exitH[2] := ExitHook;

  Fsm.Init(f, 0, ADR(f), NS, NE, ADR(table));
  Fsm.SetHooks(f, ADR(enter), ADR(exitH));

  hookIdx := 0;
  exitSeenState := 999;
  enterSeenState := 999;

  (* S0 -> S1 *)
  Fsm.Step(f, 0, NIL, status);
  Check("hooks: ok", status = Ok);
  Check("hooks: exit first", hookLog[0] = 200);  (* exit S0 *)
  Check("hooks: enter second", hookLog[1] = 101);  (* enter S1 *)
  Check("hooks: exit sees old state", exitSeenState = 0);
  Check("hooks: enter sees new state", enterSeenState = 1);

  (* S1 -> S2 *)
  exitSeenState := 999;
  enterSeenState := 999;
  Fsm.Step(f, 1, NIL, status);
  Check("hooks: exit S1", hookLog[2] = 201);  (* exit S1 *)
  Check("hooks: enter S2", hookLog[3] = 102);  (* enter S2 *)
  Check("hooks: exit sees S1", exitSeenState = 1);
  Check("hooks: enter sees S2", enterSeenState = 2)
END TestHooksOrder;

(* ── Test 5: Action failure ───────────────────────── *)

PROCEDURE TestActionFail;
CONST
  NS = 2; NE = 1;
VAR
  f: Fsm;
  table: ARRAY [0..1] OF Transition;
  acts: ARRAY [0..1] OF ActionProc;
  status: StepStatus;
BEGIN
  Fsm.ClearTable(ADR(table), NS * NE);
  (* S0+E0 -> S1, action 1 (will fail) *)
  Fsm.SetTrans(table[0*NE+0], 1, 1, NoGuard);

  acts[0] := NIL;
  acts[1] := BadAction;

  Fsm.Init(f, 0, NIL, NS, NE, ADR(table));
  Fsm.SetActions(f, ADR(acts), 2);
  actionCallCount := 0;

  Fsm.Step(f, 0, NIL, status);
  Check("actfail: status=Error", status = Error);
  Check("actfail: action called", actionCallCount = 1);
  Check("actfail: errors=1", Fsm.ErrorCount(f) = 1);
  (* State remains changed on error (documented) *)
  Check("actfail: state=1", Fsm.CurrentState(f) = 1);
  Check("actfail: steps=0", Fsm.StepCount(f) = 0)
END TestActionFail;

(* ── Test 6: Trace correctness ────────────────────── *)

PROCEDURE TestTrace;
CONST
  NS = 2; NE = 2;
VAR
  f: Fsm;
  table: ARRAY [0..3] OF Transition;
  acts: ARRAY [0..1] OF ActionProc;
  status: StepStatus;
BEGIN
  Fsm.ClearTable(ADR(table), NS * NE);
  (* S0+E0 -> S1, action 1 *)
  Fsm.SetTrans(table[0*NE+0], 1, 1, NoGuard);

  acts[0] := NIL;
  acts[1] := GoodAction;

  Fsm.Init(f, 0, NIL, NS, NE, ADR(table));
  Fsm.SetActions(f, ADR(acts), 2);
  Fsm.SetTrace(f, TraceCb, NIL);
  traceCallCount := 0;

  (* Ok transition *)
  Fsm.Step(f, 0, NIL, status);
  Check("trace: called once", traceCallCount = 1);
  Check("trace: from=0", traceFrom = 0);
  Check("trace: to=1", traceTo = 1);
  Check("trace: ev=0", traceEv = 0);
  Check("trace: act=1", traceAct = 1);
  Check("trace: status=Ok", traceSt = Ok);

  (* NoTransition *)
  Fsm.Step(f, 1, NIL, status);
  Check("trace: notrans called", traceCallCount = 2);
  Check("trace: notrans from=1", traceFrom = 1);
  Check("trace: notrans to=1", traceTo = 1);
  Check("trace: notrans status", traceSt = NoTransition)
END TestTrace;

(* ── Test 7: Reset ────────────────────────────────── *)

PROCEDURE TestReset;
CONST
  NS = 2; NE = 1;
VAR
  f: Fsm;
  table: ARRAY [0..1] OF Transition;
  status: StepStatus;
BEGIN
  Fsm.ClearTable(ADR(table), NS * NE);
  Fsm.SetTrans(table[0*NE+0], 1, NoAction, NoGuard);

  Fsm.Init(f, 0, NIL, NS, NE, ADR(table));

  Fsm.Step(f, 0, NIL, status);
  Check("reset: state=1 before", Fsm.CurrentState(f) = 1);

  Fsm.Reset(f);
  Check("reset: state=0 after", Fsm.CurrentState(f) = 0);
  Check("reset: steps=0", Fsm.StepCount(f) = 0);
  Check("reset: invalid=0", Fsm.InvalidCount(f) = 0)
END TestReset;

(* ── Test 8: Bounds check ─────────────────────────── *)

PROCEDURE TestBounds;
CONST
  NS = 2; NE = 2;
VAR
  f: Fsm;
  table: ARRAY [0..3] OF Transition;
  status: StepStatus;
BEGIN
  Fsm.ClearTable(ADR(table), NS * NE);
  Fsm.Init(f, 0, NIL, NS, NE, ADR(table));

  (* Event out of range *)
  Fsm.Step(f, 99, NIL, status);
  Check("bounds: status=Error", status = Error);
  Check("bounds: errors=1", Fsm.ErrorCount(f) = 1);
  Check("bounds: state unchanged", Fsm.CurrentState(f) = 0)
END TestBounds;

(* ── Test 9: ClearTable ───────────────────────────── *)

PROCEDURE TestClearTable;
VAR
  table: ARRAY [0..3] OF Transition;
BEGIN
  (* Set some values *)
  table[0].next := 5;
  table[0].action := 3;
  table[0].guard := 2;

  (* Clear *)
  Fsm.ClearTable(ADR(table), 4);

  Check("clear: [0].next", table[0].next = NoState);
  Check("clear: [0].action", table[0].action = NoAction);
  Check("clear: [0].guard", table[0].guard = NoGuard);
  Check("clear: [3].next", table[3].next = NoState)
END TestClearTable;

(* ── Test 10: SetTrans ────────────────────────────── *)

PROCEDURE TestSetTrans;
VAR
  t: Transition;
BEGIN
  Fsm.SetTrans(t, 7, 3, 2);
  Check("settrans: next=7", t.next = 7);
  Check("settrans: action=3", t.action = 3);
  Check("settrans: guard=2", t.guard = 2)
END TestSetTrans;

BEGIN
  passed := 0;
  failed := 0;
  total := 0;

  TestBasic;
  TestNoTransition;
  TestGuardReject;
  TestHooksOrder;
  TestActionFail;
  TestTrace;
  TestReset;
  TestBounds;
  TestClearTable;
  TestSetTrans;

  WriteLn;
  WriteString("m2fsm: ");
  WriteInt(passed, 0); WriteString(" passed, ");
  WriteInt(failed, 0); WriteString(" failed, ");
  WriteInt(total, 0); WriteString(" total"); WriteLn;
  IF failed = 0 THEN
    WriteString("ALL TESTS PASSED"); WriteLn
  END
END FsmTests.
