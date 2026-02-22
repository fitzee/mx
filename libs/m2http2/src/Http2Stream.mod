IMPLEMENTATION MODULE Http2Stream;

FROM SYSTEM IMPORT ADDRESS, ADR, TSIZE;
FROM Fsm IMPORT Fsm, Transition, StepStatus,
                NoState, NoAction, NoGuard;
FROM Http2Types IMPORT StIdle, StReservedLocal, StReservedRemote,
                       StOpen, StHalfClosedLocal, StHalfClosedRemote,
                       StClosed, NumStreamStates, NumStreamEvents,
                       EvSendH, EvSendHES, EvSendES, EvSendRst,
                       EvRecvH, EvRecvHES, EvRecvES, EvRecvRst,
                       EvRecvPP, DefaultWindowSize;

(* ── Stream transition table (RFC 7540 Section 5.1) ────── *)

PROCEDURE SetT(VAR table: StreamTransTable;
               state, event, next: CARDINAL);
BEGIN
  Fsm.SetTrans(table[state * NumStreamEvents + event],
               next, NoAction, NoGuard)
END SetT;

PROCEDURE InitStreamTable(VAR table: StreamTransTable);
BEGIN
  Fsm.ClearTable(ADR(table), StreamTableSize);

  (* Idle *)
  SetT(table, StIdle, EvSendH,   StOpen);
  SetT(table, StIdle, EvSendHES, StHalfClosedLocal);
  SetT(table, StIdle, EvRecvH,   StOpen);
  SetT(table, StIdle, EvRecvHES, StHalfClosedRemote);
  SetT(table, StIdle, EvRecvPP,  StReservedRemote);
  SetT(table, StIdle, EvSendRst, StClosed);
  SetT(table, StIdle, EvRecvRst, StClosed);

  (* Reserved (local) *)
  SetT(table, StReservedLocal, EvSendH,   StHalfClosedRemote);
  SetT(table, StReservedLocal, EvSendHES, StClosed);
  SetT(table, StReservedLocal, EvSendRst, StClosed);
  SetT(table, StReservedLocal, EvRecvRst, StClosed);

  (* Reserved (remote) *)
  SetT(table, StReservedRemote, EvRecvH,   StHalfClosedLocal);
  SetT(table, StReservedRemote, EvRecvHES, StClosed);
  SetT(table, StReservedRemote, EvSendRst, StClosed);
  SetT(table, StReservedRemote, EvRecvRst, StClosed);

  (* Open *)
  SetT(table, StOpen, EvSendES,  StHalfClosedLocal);
  SetT(table, StOpen, EvRecvES,  StHalfClosedRemote);
  SetT(table, StOpen, EvSendRst, StClosed);
  SetT(table, StOpen, EvRecvRst, StClosed);

  (* Half-closed (local) *)
  SetT(table, StHalfClosedLocal, EvRecvES,  StClosed);
  SetT(table, StHalfClosedLocal, EvSendRst, StClosed);
  SetT(table, StHalfClosedLocal, EvRecvRst, StClosed);

  (* Half-closed (remote) *)
  SetT(table, StHalfClosedRemote, EvSendES,  StClosed);
  SetT(table, StHalfClosedRemote, EvSendRst, StClosed);
  SetT(table, StHalfClosedRemote, EvRecvRst, StClosed);

  (* Closed: no transitions *)
END InitStreamTable;

(* ── Per-stream lifecycle ──────────────────────────────── *)

PROCEDURE InitStream(VAR s: H2Stream; streamId: CARDINAL;
                     initWindowSize: CARDINAL;
                     table: ADDRESS);
BEGIN
  s.id := streamId;
  Fsm.Init(s.fsm, StIdle, NIL,
           NumStreamStates, NumStreamEvents, table);
  s.sendWindow := VAL(INTEGER, initWindowSize);
  s.recvWindow := VAL(INTEGER, initWindowSize);
  s.rstCode := 0
END InitStream;

PROCEDURE StreamStep(VAR s: H2Stream; ev: CARDINAL;
                     VAR status: StepStatus);
BEGIN
  Fsm.Step(s.fsm, ev, NIL, status)
END StreamStep;

(* ── Flow control ──────────────────────────────────────── *)

PROCEDURE ConsumeSendWindow(VAR s: H2Stream; n: CARDINAL): BOOLEAN;
VAR needed: INTEGER;
BEGIN
  needed := VAL(INTEGER, n);
  IF s.sendWindow < needed THEN RETURN FALSE END;
  s.sendWindow := s.sendWindow - needed;
  RETURN TRUE
END ConsumeSendWindow;

PROCEDURE UpdateSendWindow(VAR s: H2Stream; increment: CARDINAL);
BEGIN
  s.sendWindow := s.sendWindow + VAL(INTEGER, increment)
END UpdateSendWindow;

PROCEDURE ConsumeRecvWindow(VAR s: H2Stream; n: CARDINAL): BOOLEAN;
VAR needed: INTEGER;
BEGIN
  needed := VAL(INTEGER, n);
  IF s.recvWindow < needed THEN RETURN FALSE END;
  s.recvWindow := s.recvWindow - needed;
  RETURN TRUE
END ConsumeRecvWindow;

PROCEDURE UpdateRecvWindow(VAR s: H2Stream; increment: CARDINAL);
BEGIN
  s.recvWindow := s.recvWindow + VAL(INTEGER, increment)
END UpdateRecvWindow;

(* ── Queries ───────────────────────────────────────────── *)

PROCEDURE StreamState(VAR s: H2Stream): CARDINAL;
BEGIN
  RETURN Fsm.CurrentState(s.fsm)
END StreamState;

PROCEDURE IsClosed(VAR s: H2Stream): BOOLEAN;
BEGIN
  RETURN Fsm.CurrentState(s.fsm) = StClosed
END IsClosed;

END Http2Stream.
