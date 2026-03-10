MODULE Handshake;
(* Simple protocol handshake FSM demo.

   States: Init(0) -> SentHello(1) -> Established(2)
   Events: SendHello(0), RecvAck(1), Close(2)

   Build:
     m2c examples/handshake.mod -I src -o handshake
     ./handshake

   Expected output:
     [handshake] starting in state 0
     [action] sending HELLO
     FSM: 0 -> 1 ev=0 act=1 OK
     [action] received ACK, connection established
     FSM: 1 -> 2 ev=1 act=2 OK
     [action] closing connection
     FSM: 2 -> 0 ev=2 act=3 OK
     [handshake] done, steps=3  errors=0 *)

FROM SYSTEM IMPORT ADDRESS, ADR;
FROM InOut IMPORT WriteString, WriteLn, WriteCard;
FROM Fsm IMPORT Fsm, Transition, StepStatus,
                ActionProc, TraceProc,
                StateId, EventId, ActionId,
                NoState, NoAction, NoGuard,
                Init, SetActions, SetTrace, Step,
                CurrentState, StepCount, ErrorCount,
                SetTrans, ClearTable;
FROM FsmTrace IMPORT ConsoleTrace;

CONST
  (* States *)
  StInit = 0;
  StSentHello = 1;
  StEstablished = 2;
  NumStates = 3;

  (* Events *)
  EvSendHello = 0;
  EvRecvAck = 1;
  EvClose = 2;
  NumEvents = 3;

  (* Actions *)
  ActSendHello = 1;
  ActRecvAck = 2;
  ActClose = 3;
  NumActions = 4;

VAR
  f: Fsm;
  table: ARRAY [0..8] OF Transition;
  acts: ARRAY [0..3] OF ActionProc;
  status: StepStatus;

PROCEDURE DoSendHello(ctx: ADDRESS; ev: EventId;
                      payload: ADDRESS; VAR ok: BOOLEAN);
BEGIN
  WriteString("[action] sending HELLO"); WriteLn;
  ok := TRUE
END DoSendHello;

PROCEDURE DoRecvAck(ctx: ADDRESS; ev: EventId;
                    payload: ADDRESS; VAR ok: BOOLEAN);
BEGIN
  WriteString("[action] received ACK, connection established"); WriteLn;
  ok := TRUE
END DoRecvAck;

PROCEDURE DoClose(ctx: ADDRESS; ev: EventId;
                  payload: ADDRESS; VAR ok: BOOLEAN);
BEGIN
  WriteString("[action] closing connection"); WriteLn;
  ok := TRUE
END DoClose;

BEGIN
  (* Build transition table *)
  ClearTable(ADR(table), NumStates * NumEvents);
  SetTrans(table[StInit*NumEvents + EvSendHello],
               StSentHello, ActSendHello, NoGuard);
  SetTrans(table[StSentHello*NumEvents + EvRecvAck],
               StEstablished, ActRecvAck, NoGuard);
  SetTrans(table[StEstablished*NumEvents + EvClose],
               StInit, ActClose, NoGuard);

  (* Action table *)
  acts[0] := NIL;
  acts[ActSendHello] := DoSendHello;
  acts[ActRecvAck] := DoRecvAck;
  acts[ActClose] := DoClose;

  (* Init FSM *)
  Init(f, StInit, NIL, NumStates, NumEvents, ADR(table));
  SetActions(f, ADR(acts), NumActions);
  SetTrace(f, ConsoleTrace, NIL);

  WriteString("[handshake] starting in state ");
  WriteCard(CurrentState(f), 0); WriteLn;

  (* Run protocol *)
  Step(f, EvSendHello, NIL, status);
  Step(f, EvRecvAck, NIL, status);
  Step(f, EvClose, NIL, status);

  WriteString("[handshake] done, steps=");
  WriteCard(StepCount(f), 0);
  WriteString("  errors=");
  WriteCard(ErrorCount(f), 0);
  WriteLn
END Handshake.
