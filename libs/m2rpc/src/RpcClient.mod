IMPLEMENTATION MODULE RpcClient;

FROM SYSTEM IMPORT ADDRESS, ADR, TSIZE;
FROM ByteBuf IMPORT Buf, BytesView, Init, Free, Clear, AsView,
                     AppendView;
FROM RpcFrame IMPORT ReadFn, WriteFn, FrameReader, FrameStatus,
                      MaxFrame,
                      InitFrameReader, TryReadFrame, FreeFrameReader,
                      WriteFrame,
                      FrmOk, FrmNeedMore, FrmClosed, FrmTooLarge, FrmError;
FROM RpcCodec IMPORT MsgResponse, MsgError,
                      DecodeHeader, DecodeResponse, DecodeError,
                      EncodeRequest;
FROM RpcErrors IMPORT Timeout, Closed;
FROM Scheduler IMPORT Scheduler, TaskProc, OK;
FROM Promise IMPORT Future, Value,
                     PromiseCreate, Resolve, Reject,
                     PromiseRelease,
                     MakeValue, MakeError;
IMPORT Promise;
IMPORT EventLoop;
FROM Timers IMPORT TimerId;

VAR
  frameRdr: FrameReader;
  frameRdrInit: BOOLEAN;

(* ── Helpers ──────────────────────────────────────────── *)

PROCEDURE FindSlot(VAR c: Client): INTEGER;
VAR i: CARDINAL;
BEGIN
  i := 0;
  WHILE i < MaxInflight DO
    IF NOT c.pending[i].active THEN RETURN INTEGER(i) END;
    INC(i)
  END;
  RETURN -1
END FindSlot;

PROCEDURE FindPending(VAR c: Client; reqId: CARDINAL): INTEGER;
VAR i: CARDINAL;
BEGIN
  i := 0;
  WHILE i < MaxInflight DO
    IF c.pending[i].active AND (c.pending[i].requestId = reqId) THEN
      RETURN INTEGER(i)
    END;
    INC(i)
  END;
  RETURN -1
END FindPending;

PROCEDURE RejectPending(VAR c: Client; idx: CARDINAL; code: CARDINAL);
VAR
  e: Promise.Error;
  st: CARDINAL;
BEGIN
  MakeError(INTEGER(code), NIL, e);
  st := CARDINAL(Reject(c.pending[idx].promise, e));
  PromiseRelease(c.pending[idx].promise);
  c.pending[idx].promise := NIL;
  IF c.pending[idx].hasTimer AND (c.loop # NIL) THEN
    st := CARDINAL(EventLoop.CancelTimer(c.loop, c.pending[idx].timerId))
  END;
  c.pending[idx].active := FALSE
END RejectPending;

(* ── Timeout callback ─────────────────────────────────── *)

PROCEDURE OnTimeout(user: ADDRESS);
VAR
  tcp: POINTER TO TimeoutCtx;
  cp: POINTER TO Client;
  idx: INTEGER;
BEGIN
  tcp := user;
  cp := tcp^.clientPtr;
  idx := FindPending(cp^, tcp^.requestId);
  IF idx >= 0 THEN
    RejectPending(cp^, CARDINAL(idx), Timeout)
  END
END OnTimeout;

(* ── Client ───────────────────────────────────────────── *)

PROCEDURE InitClient(VAR c: Client;
                     readFn: ReadFn; readCtx: ADDRESS;
                     writeFn: WriteFn; writeCtx: ADDRESS;
                     sched: Scheduler;
                     loop: ADDRESS);
VAR i: CARDINAL;
BEGIN
  c.readFn := readFn;
  c.readCtx := readCtx;
  c.writeFn := writeFn;
  c.writeCtx := writeCtx;
  c.loop := loop;
  c.sched := sched;
  c.nextId := 1;
  c.alive := TRUE;
  i := 0;
  WHILE i < MaxInflight DO
    c.pending[i].active := FALSE;
    INC(i)
  END;
  Init(c.outBuf, 256);
  Init(c.respBuf, 256);
  InitFrameReader(frameRdr, MaxFrame, readFn, readCtx);
  frameRdrInit := TRUE
END InitClient;

PROCEDURE Call(VAR c: Client;
               method: ARRAY OF CHAR;
               methodLen: CARDINAL;
               body: BytesView;
               timeoutMs: CARDINAL;
               VAR out: Future;
               VAR ok: BOOLEAN);
VAR
  slot: INTEGER;
  reqId, st: CARDINAL;
  p: Promise.Promise;
  f: Future;
  pv: BytesView;
  tid: TimerId;
  wfn: WriteFn;
BEGIN
  ok := FALSE;
  out := NIL;

  slot := FindSlot(c);
  IF slot < 0 THEN RETURN END;

  reqId := c.nextId;
  INC(c.nextId);

  (* Create promise/future pair *)
  IF PromiseCreate(c.sched, p, f) # OK THEN RETURN END;

  (* Encode and send the request *)
  Clear(c.outBuf);
  EncodeRequest(c.outBuf, reqId, method, methodLen, body);
  pv := AsView(c.outBuf);
  wfn := c.writeFn;
  WriteFrame(wfn, c.writeCtx, pv, ok);
  IF NOT ok THEN RETURN END;

  (* Store pending call *)
  c.pending[slot].active := TRUE;
  c.pending[slot].requestId := reqId;
  c.pending[slot].promise := p;
  c.pending[slot].hasTimer := FALSE;
  c.pending[slot].timeoutCtx.clientPtr := ADR(c);
  c.pending[slot].timeoutCtx.requestId := reqId;

  (* Set timeout if requested *)
  IF (timeoutMs > 0) AND (c.loop # NIL) THEN
    st := CARDINAL(EventLoop.SetTimeout(c.loop, INTEGER(timeoutMs),
                                        OnTimeout,
                                        ADR(c.pending[slot].timeoutCtx),
                                        tid));
    IF st = CARDINAL(EventLoop.OK) THEN
      c.pending[slot].timerId := tid;
      c.pending[slot].hasTimer := TRUE
    END
  END;

  out := f;
  ok := TRUE
END Call;

PROCEDURE OnReadable(VAR c: Client): BOOLEAN;
VAR
  payload: BytesView;
  status: FrameStatus;
  ver, mt, reqId, errCode, st: CARDINAL;
  body, errMsg: BytesView;
  ok: BOOLEAN;
  idx: INTEGER;
  v: Value;
BEGIN
  IF NOT c.alive THEN RETURN FALSE END;

  LOOP
    TryReadFrame(frameRdr, payload, status);
    IF status = FrmOk THEN
      DecodeHeader(payload, ver, mt, reqId, ok);
      IF NOT ok THEN
        (* Skip malformed frame *)
      ELSIF mt = MsgResponse THEN
        DecodeResponse(payload, reqId, body, ok);
        IF ok THEN
          idx := FindPending(c, reqId);
          IF idx >= 0 THEN
            (* Store body for caller access *)
            Clear(c.respBuf);
            IF body.len > 0 THEN
              AppendView(c.respBuf, body)
            END;

            MakeValue(0, ADR(c.respBuf), v);
            st := CARDINAL(Resolve(c.pending[idx].promise, v));
            PromiseRelease(c.pending[idx].promise);
            c.pending[idx].promise := NIL;
            IF c.pending[idx].hasTimer AND (c.loop # NIL) THEN
              st := CARDINAL(EventLoop.CancelTimer(c.loop,
                             c.pending[idx].timerId))
            END;
            c.pending[idx].active := FALSE
          END
        END
      ELSIF mt = MsgError THEN
        DecodeError(payload, reqId, errCode, errMsg, body, ok);
        IF ok THEN
          idx := FindPending(c, reqId);
          IF idx >= 0 THEN
            RejectPending(c, CARDINAL(idx), errCode)
          END
        END
      END
    ELSIF status = FrmNeedMore THEN
      RETURN TRUE
    ELSIF status = FrmClosed THEN
      c.alive := FALSE;
      CancelAll(c);
      RETURN FALSE
    ELSE
      c.alive := FALSE;
      CancelAll(c);
      RETURN FALSE
    END
  END
END OnReadable;

PROCEDURE CancelAll(VAR c: Client);
VAR i: CARDINAL;
BEGIN
  i := 0;
  WHILE i < MaxInflight DO
    IF c.pending[i].active THEN
      RejectPending(c, i, Closed)
    END;
    INC(i)
  END
END CancelAll;

PROCEDURE FreeClient(VAR c: Client);
BEGIN
  IF frameRdrInit THEN
    FreeFrameReader(frameRdr);
    frameRdrInit := FALSE
  END;
  Free(c.outBuf);
  Free(c.respBuf)
END FreeClient;

BEGIN
  frameRdrInit := FALSE
END RpcClient.
