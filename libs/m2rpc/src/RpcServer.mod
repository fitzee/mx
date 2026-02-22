IMPLEMENTATION MODULE RpcServer;

FROM SYSTEM IMPORT ADDRESS, ADR;
FROM ByteBuf IMPORT Buf, BytesView, Init, Free, Clear, AsView,
                     AppendByte, AppendView, ViewGetByte;
FROM RpcFrame IMPORT ReadFn, WriteFn, FrameReader, FrameStatus,
                      MaxFrame,
                      InitFrameReader, TryReadFrame, FreeFrameReader,
                      WriteFrame,
                      FrmOk, FrmNeedMore, FrmClosed, FrmTooLarge, FrmError;
FROM RpcCodec IMPORT MsgRequest, MsgResponse, MsgError,
                      DecodeHeader, DecodeRequest,
                      EncodeResponse, EncodeError;
FROM RpcErrors IMPORT UnknownMethod, BadRequest;
FROM Strings IMPORT Assign;

(* ── Helpers ──────────────────────────────────────────── *)

PROCEDURE StrEqual(a: ARRAY OF CHAR; aLen: CARDINAL;
                   v: BytesView): BOOLEAN;
VAR i: CARDINAL;
BEGIN
  IF aLen # v.len THEN RETURN FALSE END;
  i := 0;
  WHILE i < aLen DO
    IF ORD(a[i]) # ViewGetByte(v, i) THEN RETURN FALSE END;
    INC(i)
  END;
  RETURN TRUE
END StrEqual;

(* ── Server ───────────────────────────────────────────── *)

PROCEDURE InitServer(VAR s: Server;
                     readFn: ReadFn; readCtx: ADDRESS;
                     writeFn: WriteFn; writeCtx: ADDRESS);
VAR i: CARDINAL;
BEGIN
  InitFrameReader(s.frameReader, MaxFrame, readFn, readCtx);
  s.writeFn := writeFn;
  s.writeCtx := writeCtx;
  s.handlerCount := 0;
  i := 0;
  WHILE i < MaxHandlers DO
    s.handlers[i].active := FALSE;
    INC(i)
  END;
  Init(s.outBuf, 256);
  Init(s.respBuf, 256)
END InitServer;

PROCEDURE RegisterHandler(VAR s: Server;
                          method: ARRAY OF CHAR;
                          methodLen: CARDINAL;
                          handler: Handler;
                          ctx: ADDRESS): BOOLEAN;
VAR idx, ml: CARDINAL;
BEGIN
  IF s.handlerCount >= MaxHandlers THEN RETURN FALSE END;
  idx := s.handlerCount;
  ml := methodLen;
  IF ml > MaxMethodLen THEN ml := MaxMethodLen END;
  Assign(method, s.handlers[idx].method);
  s.handlers[idx].methodLen := ml;
  s.handlers[idx].handler := handler;
  s.handlers[idx].ctx := ctx;
  s.handlers[idx].active := TRUE;
  INC(s.handlerCount);
  RETURN TRUE
END RegisterHandler;

PROCEDURE FindHandler(VAR s: Server; methodView: BytesView): INTEGER;
VAR i: CARDINAL;
BEGIN
  i := 0;
  WHILE i < s.handlerCount DO
    IF s.handlers[i].active THEN
      IF StrEqual(s.handlers[i].method,
                  s.handlers[i].methodLen,
                  methodView) THEN
        RETURN INTEGER(i)
      END
    END;
    INC(i)
  END;
  RETURN -1
END FindHandler;

PROCEDURE DispatchHandler(VAR s: Server; idx: CARDINAL;
                          reqId: CARDINAL;
                          method: BytesView;
                          body: BytesView;
                          VAR errCode: CARDINAL;
                          VAR handlerOk: BOOLEAN);
VAR h: Handler;
BEGIN
  h := s.handlers[idx].handler;
  h(s.handlers[idx].ctx, reqId, method.base, method.len, body,
    s.respBuf, errCode, handlerOk)
END DispatchHandler;

PROCEDURE HandleRequest(VAR s: Server; payload: BytesView);
VAR
  reqId, errCode: CARDINAL;
  method, body: BytesView;
  ok, handlerOk: BOOLEAN;
  idx: INTEGER;
  respView: BytesView;
  emptyView: BytesView;
BEGIN
  emptyView.base := NIL;
  emptyView.len := 0;

  DecodeRequest(payload, reqId, method, body, ok);
  IF NOT ok THEN
    Clear(s.outBuf);
    EncodeError(s.outBuf, 0, BadRequest, "bad request", 11, emptyView);
    respView := AsView(s.outBuf);
    WriteFrame(s.writeFn, s.writeCtx, respView, ok);
    RETURN
  END;

  idx := FindHandler(s, method);
  IF idx < 0 THEN
    Clear(s.outBuf);
    EncodeError(s.outBuf, reqId, UnknownMethod,
                "unknown method", 14, emptyView);
    respView := AsView(s.outBuf);
    WriteFrame(s.writeFn, s.writeCtx, respView, ok);
    RETURN
  END;

  Clear(s.respBuf);
  errCode := 0;
  handlerOk := TRUE;
  DispatchHandler(s, CARDINAL(idx), reqId, method, body,
                  errCode, handlerOk);

  Clear(s.outBuf);
  IF handlerOk THEN
    respView := AsView(s.respBuf);
    EncodeResponse(s.outBuf, reqId, respView)
  ELSE
    respView := AsView(s.respBuf);
    EncodeError(s.outBuf, reqId, errCode, "", 0, respView)
  END;

  respView := AsView(s.outBuf);
  WriteFrame(s.writeFn, s.writeCtx, respView, ok)
END HandleRequest;

PROCEDURE ServeOnce(VAR s: Server): BOOLEAN;
VAR
  payload: BytesView;
  status: FrameStatus;
  ver, mt, reqId: CARDINAL;
  ok: BOOLEAN;
  respView, emptyView: BytesView;
BEGIN
  emptyView.base := NIL;
  emptyView.len := 0;

  LOOP
    TryReadFrame(s.frameReader, payload, status);
    IF status = FrmOk THEN
      DecodeHeader(payload, ver, mt, reqId, ok);
      IF ok AND (mt = MsgRequest) THEN
        HandleRequest(s, payload)
      ELSE
        Clear(s.outBuf);
        EncodeError(s.outBuf, reqId, BadRequest,
                    "expected request", 16, emptyView);
        respView := AsView(s.outBuf);
        WriteFrame(s.writeFn, s.writeCtx, respView, ok)
      END
    ELSIF status = FrmNeedMore THEN
      RETURN TRUE
    ELSIF status = FrmClosed THEN
      RETURN FALSE
    ELSE
      RETURN FALSE
    END
  END
END ServeOnce;

PROCEDURE FreeServer(VAR s: Server);
BEGIN
  FreeFrameReader(s.frameReader);
  Free(s.outBuf);
  Free(s.respBuf)
END FreeServer;

END RpcServer.
