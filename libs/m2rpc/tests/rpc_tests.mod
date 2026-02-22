MODULE RpcTests;
(* Deterministic test suite for m2rpc.
   Tests framing, codec, pipe, server, client/server integration,
   and 20 concurrent calls. *)

FROM InOut IMPORT WriteString, WriteLn, WriteInt, WriteCard;
FROM SYSTEM IMPORT ADDRESS, ADR;
FROM ByteBuf IMPORT Buf, BytesView, Init, Free, Clear, AsView,
                     AppendByte, AppendChars, GetByte, ViewGetByte,
                     AppendView;
FROM Codec IMPORT Writer, InitWriter, WriteU32BE, WriteU8;
FROM RpcFrame IMPORT ReadFn, WriteFn, FrameReader, FrameStatus,
                      MaxFrame,
                      InitFrameReader, TryReadFrame, FreeFrameReader,
                      WriteFrame, ResetFrameReader,
                      TsOk, TsWouldBlock, TsClosed, TsError,
                      FrmOk, FrmNeedMore, FrmClosed, FrmTooLarge, FrmError;
FROM RpcCodec IMPORT MsgRequest, MsgResponse, MsgError, Version,
                      EncodeRequest, EncodeResponse, EncodeError,
                      DecodeRequest, DecodeResponse, DecodeError,
                      DecodeHeader;
FROM RpcErrors IMPORT Ok, BadRequest, UnknownMethod, Timeout,
                       Internal, TooLarge, Closed, ToString;
FROM RpcTest IMPORT Pipe, CreatePipe, DestroyPipe,
                     ReadA, WriteA, ReadB, WriteB,
                     CloseA, CloseB, PendingAtoB, PendingBtoA;
FROM RpcServer IMPORT Server, InitServer, RegisterHandler,
                       ServeOnce, FreeServer, Handler;
FROM RpcClient IMPORT Client, InitClient, Call, OnReadable,
                       CancelAll, FreeClient;
FROM Scheduler IMPORT Scheduler, SchedulerCreate, SchedulerDestroy,
                       SchedulerPump, OK;
FROM Promise IMPORT Future, Fate, Value, Error, Result,
                     GetFate, GetResultIfSettled;

VAR
  passed, failed, total: INTEGER;

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

(* ── Test helpers ─────────────────────────────────────── *)

PROCEDURE PumpSched(s: Scheduler);
VAR didWork: BOOLEAN; st: CARDINAL;
BEGIN
  didWork := TRUE;
  WHILE didWork DO
    st := CARDINAL(SchedulerPump(s, 1000, didWork))
  END
END PumpSched;

(* Write raw bytes into pipe A→B *)
PROCEDURE WriteBytesToPipe(pipe: Pipe; VAR buf: Buf);
VAR v: BytesView; sent: CARDINAL; ts: CARDINAL;
BEGIN
  v := AsView(buf);
  IF v.len > 0 THEN
    ts := WriteA(pipe, v.base, v.len, sent)
  END
END WriteBytesToPipe;

(* ── Test 1: FrameReader complete read ────────────────── *)

PROCEDURE TestFrameComplete;
VAR
  pipe: Pipe;
  fr: FrameReader;
  frameBuf: Buf;
  payload: BytesView;
  status: FrameStatus;
  sent: CARDINAL;
BEGIN
  CreatePipe(pipe, 0, 0);
  Init(frameBuf, 64);

  (* Build raw frame: length=5, payload="Hello" *)
  AppendByte(frameBuf, 0);
  AppendByte(frameBuf, 0);
  AppendByte(frameBuf, 0);
  AppendByte(frameBuf, 5);
  AppendByte(frameBuf, ORD('H'));
  AppendByte(frameBuf, ORD('e'));
  AppendByte(frameBuf, ORD('l'));
  AppendByte(frameBuf, ORD('l'));
  AppendByte(frameBuf, ORD('o'));

  WriteBytesToPipe(pipe, frameBuf);

  InitFrameReader(fr, MaxFrame, ReadB, pipe);
  TryReadFrame(fr, payload, status);
  Check("frame.complete: status=Ok", status = FrmOk);
  Check("frame.complete: len=5", payload.len = 5);
  Check("frame.complete: byte0=H", ViewGetByte(payload, 0) = ORD('H'));
  Check("frame.complete: byte4=o", ViewGetByte(payload, 4) = ORD('o'));

  FreeFrameReader(fr);
  Free(frameBuf);
  DestroyPipe(pipe)
END TestFrameComplete;

(* ── Test 2: FrameReader split header ─────────────────── *)

PROCEDURE TestFrameSplitHeader;
VAR
  pipe: Pipe;
  fr: FrameReader;
  payload: BytesView;
  status: FrameStatus;
  b: ARRAY [0..0] OF CHAR;
  sent: CARDINAL;
BEGIN
  (* Feed bytes one at a time to exercise NeedMore transitions.
     Frame: [0,0,0,3] + "abc" = 7 bytes total *)
  CreatePipe(pipe, 0, 0);
  InitFrameReader(fr, MaxFrame, ReadB, pipe);

  (* Feed header byte 0 *)
  b[0] := CHR(0);
  sent := WriteA(pipe, ADR(b), 1, sent);
  TryReadFrame(fr, payload, status);
  Check("frame.split: need1", status = FrmNeedMore);

  (* Feed header byte 1 *)
  sent := WriteA(pipe, ADR(b), 1, sent);
  TryReadFrame(fr, payload, status);
  Check("frame.split: need2", status = FrmNeedMore);

  (* Feed header byte 2 *)
  sent := WriteA(pipe, ADR(b), 1, sent);
  TryReadFrame(fr, payload, status);
  Check("frame.split: need3", status = FrmNeedMore);

  (* Feed header byte 3 (length=3) *)
  b[0] := CHR(3);
  sent := WriteA(pipe, ADR(b), 1, sent);
  TryReadFrame(fr, payload, status);
  Check("frame.split: need4", status = FrmNeedMore);

  (* Feed payload byte 'a' *)
  b[0] := 'a';
  sent := WriteA(pipe, ADR(b), 1, sent);
  TryReadFrame(fr, payload, status);
  Check("frame.split: need5", status = FrmNeedMore);

  (* Feed payload byte 'b' *)
  b[0] := 'b';
  sent := WriteA(pipe, ADR(b), 1, sent);
  TryReadFrame(fr, payload, status);
  Check("frame.split: need6", status = FrmNeedMore);

  (* Feed payload byte 'c' — frame complete *)
  b[0] := 'c';
  sent := WriteA(pipe, ADR(b), 1, sent);
  TryReadFrame(fr, payload, status);
  Check("frame.split: ok", status = FrmOk);
  Check("frame.split: len=3", payload.len = 3);
  Check("frame.split: byte0=a", ViewGetByte(payload, 0) = ORD('a'));

  FreeFrameReader(fr);
  DestroyPipe(pipe)
END TestFrameSplitHeader;

(* ── Test 3: FrameReader payload split ────────────────── *)

PROCEDURE TestFrameSplitPayload;
VAR
  pipe: Pipe;
  fr: FrameReader;
  frameBuf: Buf;
  payload: BytesView;
  status: FrameStatus;
  i: CARDINAL;
BEGIN
  CreatePipe(pipe, 3, 0);
  Init(frameBuf, 128);

  AppendByte(frameBuf, 0);
  AppendByte(frameBuf, 0);
  AppendByte(frameBuf, 0);
  AppendByte(frameBuf, 10);
  i := 0;
  WHILE i < 10 DO
    AppendByte(frameBuf, 65 + i);
    INC(i)
  END;

  WriteBytesToPipe(pipe, frameBuf);

  InitFrameReader(fr, MaxFrame, ReadB, pipe);

  TryReadFrame(fr, payload, status);
  WHILE status = FrmNeedMore DO
    TryReadFrame(fr, payload, status)
  END;

  Check("frame.payload_split: ok", status = FrmOk);
  Check("frame.payload_split: len=10", payload.len = 10);
  Check("frame.payload_split: first=A", ViewGetByte(payload, 0) = 65);
  Check("frame.payload_split: last=J", ViewGetByte(payload, 9) = 74);

  FreeFrameReader(fr);
  Free(frameBuf);
  DestroyPipe(pipe)
END TestFrameSplitPayload;

(* ── Test 4: TooLarge frame ───────────────────────────── *)

PROCEDURE TestFrameTooLarge;
VAR
  pipe: Pipe;
  fr: FrameReader;
  frameBuf: Buf;
  payload: BytesView;
  status: FrameStatus;
BEGIN
  CreatePipe(pipe, 0, 0);
  Init(frameBuf, 16);

  (* Header claiming 99999 bytes *)
  AppendByte(frameBuf, 0);
  AppendByte(frameBuf, 1);
  AppendByte(frameBuf, 134);
  AppendByte(frameBuf, 160);

  WriteBytesToPipe(pipe, frameBuf);

  InitFrameReader(fr, 100, ReadB, pipe);
  TryReadFrame(fr, payload, status);
  Check("frame.toolarge: rejected", status = FrmTooLarge);

  FreeFrameReader(fr);
  Free(frameBuf);
  DestroyPipe(pipe)
END TestFrameTooLarge;

(* ── Test 5: Zero-length payload ──────────────────────── *)

PROCEDURE TestFrameZeroLen;
VAR
  pipe: Pipe;
  fr: FrameReader;
  frameBuf: Buf;
  payload: BytesView;
  status: FrameStatus;
BEGIN
  CreatePipe(pipe, 0, 0);
  Init(frameBuf, 16);

  AppendByte(frameBuf, 0);
  AppendByte(frameBuf, 0);
  AppendByte(frameBuf, 0);
  AppendByte(frameBuf, 0);

  WriteBytesToPipe(pipe, frameBuf);

  InitFrameReader(fr, MaxFrame, ReadB, pipe);
  TryReadFrame(fr, payload, status);
  Check("frame.zerolen: ok", status = FrmOk);
  Check("frame.zerolen: len=0", payload.len = 0);

  FreeFrameReader(fr);
  Free(frameBuf);
  DestroyPipe(pipe)
END TestFrameZeroLen;

(* ── Test 6: Closed during header ─────────────────────── *)

PROCEDURE TestFrameClosedHeader;
VAR
  pipe: Pipe;
  fr: FrameReader;
  payload: BytesView;
  status: FrameStatus;
BEGIN
  CreatePipe(pipe, 0, 0);
  CloseA(pipe);

  InitFrameReader(fr, MaxFrame, ReadB, pipe);
  TryReadFrame(fr, payload, status);
  Check("frame.closed: detected", status = FrmClosed);

  FreeFrameReader(fr);
  DestroyPipe(pipe)
END TestFrameClosedHeader;

(* ── Test 7: WriteFrame roundtrip ─────────────────────── *)

PROCEDURE TestWriteFrameRoundtrip;
VAR
  pipe: Pipe;
  fr: FrameReader;
  buf: Buf;
  payload, out: BytesView;
  status: FrameStatus;
  ok: BOOLEAN;
BEGIN
  CreatePipe(pipe, 0, 0);
  Init(buf, 64);

  AppendByte(buf, ORD('T'));
  AppendByte(buf, ORD('e'));
  AppendByte(buf, ORD('s'));
  AppendByte(buf, ORD('t'));
  payload := AsView(buf);
  WriteFrame(WriteA, pipe, payload, ok);
  Check("writeframe: write ok", ok);

  InitFrameReader(fr, MaxFrame, ReadB, pipe);
  TryReadFrame(fr, out, status);
  Check("writeframe: read ok", status = FrmOk);
  Check("writeframe: len=4", out.len = 4);
  Check("writeframe: byte0=T", ViewGetByte(out, 0) = ORD('T'));
  Check("writeframe: byte3=t", ViewGetByte(out, 3) = ORD('t'));

  FreeFrameReader(fr);
  Free(buf);
  DestroyPipe(pipe)
END TestWriteFrameRoundtrip;

(* ── Test 8: Codec request roundtrip ──────────────────── *)

PROCEDURE TestCodecRequest;
VAR
  buf, bodyBuf: Buf;
  payload, method, body: BytesView;
  reqId: CARDINAL;
  ok: BOOLEAN;
BEGIN
  Init(buf, 256);
  Init(bodyBuf, 64);

  AppendByte(bodyBuf, ORD('x'));
  AppendByte(bodyBuf, ORD('y'));
  body := AsView(bodyBuf);

  EncodeRequest(buf, 42, "Echo", 4, body);
  payload := AsView(buf);

  DecodeRequest(payload, reqId, method, body, ok);
  Check("codec.req: decode ok", ok);
  Check("codec.req: reqId=42", reqId = 42);
  Check("codec.req: method len=4", method.len = 4);
  Check("codec.req: method[0]=E", ViewGetByte(method, 0) = ORD('E'));
  Check("codec.req: body len=2", body.len = 2);
  Check("codec.req: body[0]=x", ViewGetByte(body, 0) = ORD('x'));

  Free(buf);
  Free(bodyBuf)
END TestCodecRequest;

(* ── Test 9: Codec response roundtrip ─────────────────── *)

PROCEDURE TestCodecResponse;
VAR
  buf, bodyBuf: Buf;
  payload, body: BytesView;
  reqId: CARDINAL;
  ok: BOOLEAN;
BEGIN
  Init(buf, 256);
  Init(bodyBuf, 64);

  AppendByte(bodyBuf, ORD('O'));
  AppendByte(bodyBuf, ORD('K'));
  body := AsView(bodyBuf);

  EncodeResponse(buf, 99, body);
  payload := AsView(buf);

  DecodeResponse(payload, reqId, body, ok);
  Check("codec.resp: decode ok", ok);
  Check("codec.resp: reqId=99", reqId = 99);
  Check("codec.resp: body len=2", body.len = 2);
  Check("codec.resp: body[0]=O", ViewGetByte(body, 0) = ORD('O'));

  Free(buf);
  Free(bodyBuf)
END TestCodecResponse;

(* ── Test 10: Codec error roundtrip ───────────────────── *)

PROCEDURE TestCodecError;
VAR
  buf: Buf;
  payload, errMsg, body: BytesView;
  empty: BytesView;
  reqId, errCode: CARDINAL;
  ok: BOOLEAN;
BEGIN
  Init(buf, 256);
  empty.base := NIL;
  empty.len := 0;

  EncodeError(buf, 7, UnknownMethod, "not found", 9, empty);
  payload := AsView(buf);

  DecodeError(payload, reqId, errCode, errMsg, body, ok);
  Check("codec.err: decode ok", ok);
  Check("codec.err: reqId=7", reqId = 7);
  Check("codec.err: code=UnknownMethod", errCode = UnknownMethod);
  Check("codec.err: msg len=9", errMsg.len = 9);
  Check("codec.err: body empty", body.len = 0);

  Free(buf)
END TestCodecError;

(* ── Test 11: Codec truncated ─────────────────────────── *)

PROCEDURE TestCodecTruncated;
VAR
  buf: Buf;
  payload, method, body: BytesView;
  reqId: CARDINAL;
  ok: BOOLEAN;
BEGIN
  Init(buf, 16);
  AppendByte(buf, 1);
  AppendByte(buf, 0);
  AppendByte(buf, 0);
  payload := AsView(buf);

  DecodeRequest(payload, reqId, method, body, ok);
  Check("codec.trunc: rejected", NOT ok);

  Free(buf)
END TestCodecTruncated;

(* ── Test 12: Codec wrong version ─────────────────────── *)

PROCEDURE TestCodecBadVersion;
VAR
  buf: Buf;
  payload, method, body: BytesView;
  reqId: CARDINAL;
  ok: BOOLEAN;
BEGIN
  Init(buf, 64);
  AppendByte(buf, 99);  (* bad version *)
  AppendByte(buf, 0);
  AppendByte(buf, 0); AppendByte(buf, 0);
  AppendByte(buf, 0); AppendByte(buf, 1);
  AppendByte(buf, 0); AppendByte(buf, 0);
  AppendByte(buf, 0); AppendByte(buf, 0);
  AppendByte(buf, 0); AppendByte(buf, 0);
  payload := AsView(buf);

  DecodeRequest(payload, reqId, method, body, ok);
  Check("codec.badver: rejected", NOT ok);

  Free(buf)
END TestCodecBadVersion;

(* ── Test 13: Codec wrong msg_type ────────────────────── *)

PROCEDURE TestCodecBadType;
VAR
  buf: Buf;
  payload, body: BytesView;
  reqId: CARDINAL;
  ok: BOOLEAN;
BEGIN
  Init(buf, 64);
  AppendByte(buf, 1);   (* version 1 *)
  AppendByte(buf, 0);   (* Request *)
  AppendByte(buf, 0); AppendByte(buf, 0);
  AppendByte(buf, 0); AppendByte(buf, 1);
  AppendByte(buf, 0); AppendByte(buf, 0);
  AppendByte(buf, 0); AppendByte(buf, 0);
  AppendByte(buf, 0); AppendByte(buf, 0);
  payload := AsView(buf);

  (* Try to decode as Response -- should fail *)
  DecodeResponse(payload, reqId, body, ok);
  Check("codec.badtype: rejected", NOT ok);

  Free(buf)
END TestCodecBadType;

(* ── Test 14: Codec empty body ────────────────────────── *)

PROCEDURE TestCodecEmptyBody;
VAR
  buf: Buf;
  payload, body: BytesView;
  empty: BytesView;
  reqId: CARDINAL;
  ok: BOOLEAN;
BEGIN
  Init(buf, 64);
  empty.base := NIL;
  empty.len := 0;

  EncodeResponse(buf, 55, empty);
  payload := AsView(buf);

  DecodeResponse(payload, reqId, body, ok);
  Check("codec.empty: decode ok", ok);
  Check("codec.empty: reqId=55", reqId = 55);
  Check("codec.empty: body empty", body.len = 0);

  Free(buf)
END TestCodecEmptyBody;

(* ── Test 15: PipeStream basic ────────────────────────── *)

PROCEDURE TestPipeBasic;
VAR
  pipe: Pipe;
  wbuf: ARRAY [0..7] OF CHAR;
  rbuf: ARRAY [0..7] OF CHAR;
  sent, got, ts: CARDINAL;
BEGIN
  CreatePipe(pipe, 0, 0);

  wbuf[0] := 'H'; wbuf[1] := 'i';
  ts := WriteA(pipe, ADR(wbuf), 2, sent);
  Check("pipe.basic: write ok", ts = TsOk);
  Check("pipe.basic: sent=2", sent = 2);
  Check("pipe.basic: pending=2", PendingAtoB(pipe) = 2);

  ts := ReadB(pipe, ADR(rbuf), 8, got);
  Check("pipe.basic: read ok", ts = TsOk);
  Check("pipe.basic: got=2", got = 2);
  Check("pipe.basic: byte0=H", rbuf[0] = 'H');

  DestroyPipe(pipe)
END TestPipeBasic;

(* ── Test 16: PipeStream partial read ─────────────────── *)

PROCEDURE TestPipePartialRead;
VAR
  pipe: Pipe;
  wbuf: ARRAY [0..7] OF CHAR;
  rbuf: ARRAY [0..7] OF CHAR;
  sent, got, ts: CARDINAL;
BEGIN
  CreatePipe(pipe, 2, 0);

  wbuf[0] := 'A'; wbuf[1] := 'B';
  wbuf[2] := 'C'; wbuf[3] := 'D';
  ts := WriteA(pipe, ADR(wbuf), 4, sent);
  Check("pipe.partial_read: write 4", sent = 4);

  ts := ReadB(pipe, ADR(rbuf), 8, got);
  Check("pipe.partial_read: got=2", got = 2);
  Check("pipe.partial_read: byte0=A", rbuf[0] = 'A');

  ts := ReadB(pipe, ADR(rbuf), 8, got);
  Check("pipe.partial_read: got=2b", got = 2);
  Check("pipe.partial_read: byte0=C", rbuf[0] = 'C');

  DestroyPipe(pipe)
END TestPipePartialRead;

(* ── Test 17: PipeStream partial write ────────────────── *)

PROCEDURE TestPipePartialWrite;
VAR
  pipe: Pipe;
  wbuf: ARRAY [0..7] OF CHAR;
  rbuf: ARRAY [0..7] OF CHAR;
  sent, got, ts: CARDINAL;
BEGIN
  CreatePipe(pipe, 0, 3);

  wbuf[0] := '1'; wbuf[1] := '2';
  wbuf[2] := '3'; wbuf[3] := '4'; wbuf[4] := '5';
  ts := WriteA(pipe, ADR(wbuf), 5, sent);
  Check("pipe.partial_write: sent=3", sent = 3);

  ts := ReadB(pipe, ADR(rbuf), 8, got);
  Check("pipe.partial_write: got=3", got = 3);
  Check("pipe.partial_write: byte0=1", rbuf[0] = '1');

  DestroyPipe(pipe)
END TestPipePartialWrite;

(* ── Test 18: PipeStream close ────────────────────────── *)

PROCEDURE TestPipeClose;
VAR
  pipe: Pipe;
  rbuf: ARRAY [0..7] OF CHAR;
  got, ts: CARDINAL;
BEGIN
  CreatePipe(pipe, 0, 0);
  CloseA(pipe);

  ts := ReadB(pipe, ADR(rbuf), 8, got);
  Check("pipe.close: closed", ts = TsClosed);

  DestroyPipe(pipe)
END TestPipeClose;

(* ── Test 19: PipeStream bidirectional ────────────────── *)

PROCEDURE TestPipeBidir;
VAR
  pipe: Pipe;
  wbuf, rbuf: ARRAY [0..7] OF CHAR;
  sent, got, ts: CARDINAL;
BEGIN
  CreatePipe(pipe, 0, 0);

  wbuf[0] := 'X';
  ts := WriteA(pipe, ADR(wbuf), 1, sent);
  Check("pipe.bidir: A->B write", sent = 1);

  wbuf[0] := 'Y';
  ts := WriteB(pipe, ADR(wbuf), 1, sent);
  Check("pipe.bidir: B->A write", sent = 1);

  ts := ReadB(pipe, ADR(rbuf), 8, got);
  Check("pipe.bidir: B reads X", (got = 1) AND (rbuf[0] = 'X'));

  ts := ReadA(pipe, ADR(rbuf), 8, got);
  Check("pipe.bidir: A reads Y", (got = 1) AND (rbuf[0] = 'Y'));

  DestroyPipe(pipe)
END TestPipeBidir;

(* ── Handlers ─────────────────────────────────────────── *)

(* Handler: always returns "Pong" *)
PROCEDURE PingHandler(ctx: ADDRESS; reqId: CARDINAL;
                      methodPtr: ADDRESS; methodLen: CARDINAL;
                      body: BytesView;
                      VAR outBody: Buf; VAR errCode: CARDINAL;
                      VAR ok: BOOLEAN);
BEGIN
  Clear(outBody);
  AppendByte(outBody, ORD('P'));
  AppendByte(outBody, ORD('o'));
  AppendByte(outBody, ORD('n'));
  AppendByte(outBody, ORD('g'));
  errCode := 0;
  ok := TRUE
END PingHandler;

(* Handler: echoes the request body back *)
PROCEDURE EchoHandler(ctx: ADDRESS; reqId: CARDINAL;
                      methodPtr: ADDRESS; methodLen: CARDINAL;
                      body: BytesView;
                      VAR outBody: Buf; VAR errCode: CARDINAL;
                      VAR ok: BOOLEAN);
BEGIN
  Clear(outBody);
  IF body.len > 0 THEN
    AppendView(outBody, body)
  END;
  errCode := 0;
  ok := TRUE
END EchoHandler;

(* ── Test 20: Server Ping ─────────────────────────────── *)

PROCEDURE TestServerPing;
VAR
  pipe: Pipe;
  srv: Server;
  reqBuf: Buf;
  reqView, respPayload, body, method: BytesView;
  fr: FrameReader;
  status: FrameStatus;
  empty: BytesView;
  ok: BOOLEAN;
  reqId: CARDINAL;
BEGIN
  CreatePipe(pipe, 0, 0);
  Init(reqBuf, 256);
  empty.base := NIL;
  empty.len := 0;

  InitServer(srv, ReadB, pipe, WriteB, pipe);
  ok := RegisterHandler(srv, "Ping", 4, PingHandler, NIL);
  Check("srv.ping: register ok", ok);

  EncodeRequest(reqBuf, 1, "Ping", 4, empty);
  reqView := AsView(reqBuf);
  WriteFrame(WriteA, pipe, reqView, ok);
  Check("srv.ping: write ok", ok);

  ok := ServeOnce(srv);

  InitFrameReader(fr, MaxFrame, ReadA, pipe);
  TryReadFrame(fr, respPayload, status);
  Check("srv.ping: resp frame ok", status = FrmOk);

  DecodeResponse(respPayload, reqId, body, ok);
  Check("srv.ping: decode ok", ok);
  Check("srv.ping: reqId=1", reqId = 1);
  Check("srv.ping: body len=4", body.len = 4);
  Check("srv.ping: body=Pong",
        (ViewGetByte(body, 0) = ORD('P')) AND
        (ViewGetByte(body, 3) = ORD('g')));

  FreeFrameReader(fr);
  FreeServer(srv);
  Free(reqBuf);
  DestroyPipe(pipe)
END TestServerPing;

(* ── Test 21: Server unknown method ───────────────────── *)

PROCEDURE TestServerUnknown;
VAR
  pipe: Pipe;
  srv: Server;
  reqBuf: Buf;
  reqView, respPayload, errMsg, body: BytesView;
  fr: FrameReader;
  status: FrameStatus;
  empty: BytesView;
  ok: BOOLEAN;
  reqId, errCode: CARDINAL;
BEGIN
  CreatePipe(pipe, 0, 0);
  Init(reqBuf, 256);
  empty.base := NIL;
  empty.len := 0;

  InitServer(srv, ReadB, pipe, WriteB, pipe);

  EncodeRequest(reqBuf, 5, "NoSuch", 6, empty);
  reqView := AsView(reqBuf);
  WriteFrame(WriteA, pipe, reqView, ok);

  ok := ServeOnce(srv);

  InitFrameReader(fr, MaxFrame, ReadA, pipe);
  TryReadFrame(fr, respPayload, status);
  Check("srv.unknown: frame ok", status = FrmOk);

  DecodeError(respPayload, reqId, errCode, errMsg, body, ok);
  Check("srv.unknown: decode ok", ok);
  Check("srv.unknown: reqId=5", reqId = 5);
  Check("srv.unknown: code=UnknownMethod", errCode = UnknownMethod);

  FreeFrameReader(fr);
  FreeServer(srv);
  Free(reqBuf);
  DestroyPipe(pipe)
END TestServerUnknown;

(* ── Test 22: Client/Server basic roundtrip ───────────── *)

PROCEDURE TestClientServerBasic;
VAR
  pipe: Pipe;
  srv: Server;
  cli: Client;
  sched: Scheduler;
  f: Future;
  fate: Fate;
  empty: BytesView;
  ok: BOOLEAN;
  st: CARDINAL;
BEGIN
  CreatePipe(pipe, 0, 0);
  st := CARDINAL(SchedulerCreate(256, sched));
  empty.base := NIL;
  empty.len := 0;

  InitServer(srv, ReadB, pipe, WriteB, pipe);
  ok := RegisterHandler(srv, "Ping", 4, PingHandler, NIL);

  InitClient(cli, ReadA, pipe, WriteA, pipe, sched, NIL);

  Call(cli, "Ping", 4, empty, 0, f, ok);
  Check("cs.basic: call ok", ok);

  ok := ServeOnce(srv);
  ok := OnReadable(cli);
  Check("cs.basic: alive", ok);

  PumpSched(sched);

  st := CARDINAL(GetFate(f, fate));
  Check("cs.basic: fulfilled", fate = Fulfilled);

  FreeClient(cli);
  FreeServer(srv);
  st := CARDINAL(SchedulerDestroy(sched));
  DestroyPipe(pipe)
END TestClientServerBasic;

(* ── Test 23: Multiple sequential calls ───────────────── *)

PROCEDURE TestClientServerSequential;
VAR
  pipe: Pipe;
  srv: Server;
  cli: Client;
  sched: Scheduler;
  f1, f2, f3: Future;
  fate: Fate;
  empty: BytesView;
  ok: BOOLEAN;
  st: CARDINAL;
BEGIN
  CreatePipe(pipe, 0, 0);
  st := CARDINAL(SchedulerCreate(256, sched));
  empty.base := NIL;
  empty.len := 0;

  InitServer(srv, ReadB, pipe, WriteB, pipe);
  ok := RegisterHandler(srv, "Ping", 4, PingHandler, NIL);

  InitClient(cli, ReadA, pipe, WriteA, pipe, sched, NIL);

  Call(cli, "Ping", 4, empty, 0, f1, ok);
  Check("cs.seq: call1 ok", ok);
  ok := ServeOnce(srv);
  ok := OnReadable(cli);
  PumpSched(sched);

  Call(cli, "Ping", 4, empty, 0, f2, ok);
  Check("cs.seq: call2 ok", ok);
  ok := ServeOnce(srv);
  ok := OnReadable(cli);
  PumpSched(sched);

  Call(cli, "Ping", 4, empty, 0, f3, ok);
  Check("cs.seq: call3 ok", ok);
  ok := ServeOnce(srv);
  ok := OnReadable(cli);
  PumpSched(sched);

  st := CARDINAL(GetFate(f1, fate));
  Check("cs.seq: f1 fulfilled", fate = Fulfilled);
  st := CARDINAL(GetFate(f2, fate));
  Check("cs.seq: f2 fulfilled", fate = Fulfilled);
  st := CARDINAL(GetFate(f3, fate));
  Check("cs.seq: f3 fulfilled", fate = Fulfilled);

  FreeClient(cli);
  FreeServer(srv);
  st := CARDINAL(SchedulerDestroy(sched));
  DestroyPipe(pipe)
END TestClientServerSequential;

(* ── Test 24: 20 concurrent calls ─────────────────────── *)

PROCEDURE TestConcurrent20;
VAR
  pipe: Pipe;
  srv: Server;
  cli: Client;
  sched: Scheduler;
  futures: ARRAY [0..19] OF Future;
  bodyBuf: Buf;
  body, empty: BytesView;
  ok: BOOLEAN;
  fate: Fate;
  st: CARDINAL;
  i, fulfilled: CARDINAL;
BEGIN
  CreatePipe(pipe, 0, 0);
  st := CARDINAL(SchedulerCreate(512, sched));
  Init(bodyBuf, 64);
  empty.base := NIL;
  empty.len := 0;

  InitServer(srv, ReadB, pipe, WriteB, pipe);
  ok := RegisterHandler(srv, "Echo", 4, EchoHandler, NIL);

  InitClient(cli, ReadA, pipe, WriteA, pipe, sched, NIL);

  (* Issue 20 calls *)
  i := 0;
  WHILE i < 20 DO
    Clear(bodyBuf);
    AppendByte(bodyBuf, i);
    body := AsView(bodyBuf);
    Call(cli, "Echo", 4, body, 0, futures[i], ok);
    Check("concurrent: call ok", ok);
    INC(i)
  END;

  (* Server processes all *)
  i := 0;
  WHILE i < 20 DO
    ok := ServeOnce(srv);
    INC(i)
  END;

  (* Client reads all responses *)
  ok := OnReadable(cli);
  PumpSched(sched);

  (* Verify *)
  fulfilled := 0;
  i := 0;
  WHILE i < 20 DO
    st := CARDINAL(GetFate(futures[i], fate));
    IF fate = Fulfilled THEN INC(fulfilled) END;
    INC(i)
  END;
  Check("concurrent: all 20 fulfilled", fulfilled = 20);

  FreeClient(cli);
  FreeServer(srv);
  Free(bodyBuf);
  st := CARDINAL(SchedulerDestroy(sched));
  DestroyPipe(pipe)
END TestConcurrent20;

(* ── Test 25: RpcErrors ToString ──────────────────────── *)

PROCEDURE TestErrorStrings;
VAR s: ARRAY [0..31] OF CHAR;
BEGIN
  ToString(Ok, s);
  Check("errors: Ok", s[0] = 'O');
  ToString(BadRequest, s);
  Check("errors: BadRequest", s[0] = 'B');
  ToString(UnknownMethod, s);
  Check("errors: UnknownMethod", s[0] = 'U');
  ToString(Timeout, s);
  Check("errors: Timeout", s[0] = 'T');
  ToString(Internal, s);
  Check("errors: Internal", s[0] = 'I');
  ToString(TooLarge, s);
  Check("errors: TooLarge", s[0] = 'T');
  ToString(Closed, s);
  Check("errors: Closed", s[0] = 'C');
  ToString(99, s);
  Check("errors: Unknown", s[0] = 'U')
END TestErrorStrings;

BEGIN
  passed := 0;
  failed := 0;
  total := 0;

  TestFrameComplete;
  TestFrameSplitHeader;
  TestFrameSplitPayload;
  TestFrameTooLarge;
  TestFrameZeroLen;
  TestFrameClosedHeader;
  TestWriteFrameRoundtrip;
  TestCodecRequest;
  TestCodecResponse;
  TestCodecError;
  TestCodecTruncated;
  TestCodecBadVersion;
  TestCodecBadType;
  TestCodecEmptyBody;
  TestPipeBasic;
  TestPipePartialRead;
  TestPipePartialWrite;
  TestPipeClose;
  TestPipeBidir;
  TestServerPing;
  TestServerUnknown;
  TestClientServerBasic;
  TestClientServerSequential;
  TestConcurrent20;
  TestErrorStrings;

  WriteLn;
  WriteString("m2rpc tests: ");
  WriteInt(passed, 0);
  WriteString(" passed, ");
  WriteInt(failed, 0);
  WriteString(" failed out of ");
  WriteInt(total, 0);
  WriteLn;

  IF failed > 0 THEN
    WriteString("SOME TESTS FAILED"); WriteLn
  ELSE
    WriteString("ALL TESTS PASSED"); WriteLn
  END
END RpcTests.
