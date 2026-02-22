MODULE StreamState;
(* Simplified stream lifecycle FSM with guards and hooks.

   States: Idle(0) -> Reading(1) / Writing(2) -> Closed(3)
   Events: StartRead(0), StartWrite(1), Done(2), CloseEv(3)

   Guard: only allow StartRead/StartWrite when stream is open.
   Hooks: print entry/exit messages.

   Build:
     m2c examples/stream_state.mod -I src -o stream_state
     ./stream_state

   Expected output:
     [guard] checking if stream is open: yes
     [exit] leaving state 0
     [enter] entering state 1
     [action] starting read
     FSM: 0 -> 1 ev=0 act=1 OK
     [exit] leaving state 1
     [enter] entering state 0
     FSM: 1 -> 0 ev=2 act=0 OK
     [guard] checking if stream is open: yes
     [exit] leaving state 0
     [enter] entering state 2
     [action] starting write
     FSM: 0 -> 2 ev=1 act=2 OK
     [exit] leaving state 2
     [enter] entering state 3
     [action] closing stream
     FSM: 2 -> 3 ev=3 act=3 OK
     [guard] checking if stream is open: no
     FSM: 3 -> 3 ev=0 act=1 REJECTED
     stream closed after 4 steps, 1 rejected *)

FROM SYSTEM IMPORT ADDRESS, ADR;
FROM InOut IMPORT WriteString, WriteLn, WriteCard;
FROM Fsm IMPORT Fsm, Transition, StepStatus,
                ActionProc, GuardProc, HookProc, TraceProc,
                StateId, EventId, ActionId,
                NoState, NoAction, NoGuard;
FROM FsmTrace IMPORT ConsoleTrace;

CONST
  (* States *)
  StIdle = 0;
  StReading = 1;
  StWriting = 2;
  StClosed = 3;
  NumStates = 4;

  (* Events *)
  EvStartRead = 0;
  EvStartWrite = 1;
  EvDone = 2;
  EvClose = 3;
  NumEvents = 4;

  (* Actions *)
  ActStartRead = 1;
  ActStartWrite = 2;
  ActClose = 3;
  NumActions = 4;

VAR
  f: Fsm;
  table: ARRAY [0..15] OF Transition;
  acts: ARRAY [0..3] OF ActionProc;
  guards: ARRAY [0..1] OF GuardProc;
  enter: ARRAY [0..3] OF HookProc;
  exitH: ARRAY [0..3] OF HookProc;
  status: StepStatus;

(* ── Callbacks ───────────────────────────────────────── *)

PROCEDURE DoStartRead(ctx: ADDRESS; ev: EventId;
                      payload: ADDRESS; VAR ok: BOOLEAN);
BEGIN
  WriteString("[action] starting read"); WriteLn;
  ok := TRUE
END DoStartRead;

PROCEDURE DoStartWrite(ctx: ADDRESS; ev: EventId;
                       payload: ADDRESS; VAR ok: BOOLEAN);
BEGIN
  WriteString("[action] starting write"); WriteLn;
  ok := TRUE
END DoStartWrite;

PROCEDURE DoClose(ctx: ADDRESS; ev: EventId;
                  payload: ADDRESS; VAR ok: BOOLEAN);
BEGIN
  WriteString("[action] closing stream"); WriteLn;
  ok := TRUE
END DoClose;

PROCEDURE CheckOpen(ctx: ADDRESS; ev: EventId;
                    payload: ADDRESS; VAR allow: BOOLEAN);
VAR fp: POINTER TO Fsm;
BEGIN
  fp := ctx;
  allow := fp^.state # StClosed;
  WriteString("[guard] checking if stream is open: ");
  IF allow THEN
    WriteString("yes")
  ELSE
    WriteString("no")
  END;
  WriteLn
END CheckOpen;

PROCEDURE OnEnter(ctx: ADDRESS; st: StateId; payload: ADDRESS);
BEGIN
  WriteString("[enter] entering state ");
  WriteCard(st, 0); WriteLn
END OnEnter;

PROCEDURE OnExit(ctx: ADDRESS; st: StateId; payload: ADDRESS);
BEGIN
  WriteString("[exit] leaving state ");
  WriteCard(st, 0); WriteLn
END OnExit;

BEGIN
  (* Build transition table *)
  Fsm.ClearTable(ADR(table), NumStates * NumEvents);

  (* Idle transitions *)
  Fsm.SetTrans(table[StIdle*NumEvents + EvStartRead],
               StReading, ActStartRead, 1);
  Fsm.SetTrans(table[StIdle*NumEvents + EvStartWrite],
               StWriting, ActStartWrite, 1);
  Fsm.SetTrans(table[StIdle*NumEvents + EvClose],
               StClosed, ActClose, NoGuard);

  (* Reading -> Idle on Done *)
  Fsm.SetTrans(table[StReading*NumEvents + EvDone],
               StIdle, NoAction, NoGuard);
  Fsm.SetTrans(table[StReading*NumEvents + EvClose],
               StClosed, ActClose, NoGuard);

  (* Writing -> Idle on Done *)
  Fsm.SetTrans(table[StWriting*NumEvents + EvDone],
               StIdle, NoAction, NoGuard);
  Fsm.SetTrans(table[StWriting*NumEvents + EvClose],
               StClosed, ActClose, NoGuard);

  (* Closed: StartRead guarded -> rejected *)
  Fsm.SetTrans(table[StClosed*NumEvents + EvStartRead],
               StReading, ActStartRead, 1);

  (* Action table *)
  acts[0] := NIL;
  acts[ActStartRead] := DoStartRead;
  acts[ActStartWrite] := DoStartWrite;
  acts[ActClose] := DoClose;

  (* Guard table *)
  guards[0] := NIL;
  guards[1] := CheckOpen;

  (* Hook tables *)
  enter[0] := OnEnter;
  enter[1] := OnEnter;
  enter[2] := OnEnter;
  enter[3] := OnEnter;
  exitH[0] := OnExit;
  exitH[1] := OnExit;
  exitH[2] := OnExit;
  exitH[3] := OnExit;

  (* Init FSM with ctx = ADR(f) so guard can read state *)
  Fsm.Init(f, StIdle, ADR(f), NumStates, NumEvents, ADR(table));
  Fsm.SetActions(f, ADR(acts), NumActions);
  Fsm.SetGuards(f, ADR(guards), 2);
  Fsm.SetHooks(f, ADR(enter), ADR(exitH));
  Fsm.SetTrace(f, ConsoleTrace, NIL);

  (* Start read, finish, start write, close *)
  Fsm.Step(f, EvStartRead, NIL, status);
  Fsm.Step(f, EvDone, NIL, status);
  Fsm.Step(f, EvStartWrite, NIL, status);
  Fsm.Step(f, EvClose, NIL, status);

  (* Try to read on closed stream *)
  Fsm.Step(f, EvStartRead, NIL, status);

  WriteString("stream closed after ");
  WriteCard(Fsm.StepCount(f), 0);
  WriteString(" steps, ");
  WriteCard(Fsm.RejectCount(f), 0);
  WriteString(" rejected"); WriteLn
END StreamState.
