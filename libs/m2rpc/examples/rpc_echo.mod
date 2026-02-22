(* rpc_echo.mod -- In-memory Echo RPC demo.
   The Echo handler returns the request body unchanged.
   Build: m2c rpc_echo.mod -I ../src -I ../../m2bytes/src \
          -I ../../m2futures/src -I ../../m2evloop/src \
          ../../m2evloop/src/poller_bridge.c *)
MODULE RpcEcho;

FROM InOut IMPORT WriteString, WriteLn, Write;
FROM SYSTEM IMPORT ADDRESS;
FROM ByteBuf IMPORT Buf, BytesView, Init, Free, Clear,
                     AppendByte, AppendChars, AppendView,
                     AsView, ViewGetByte;
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
FROM RpcErrors IMPORT BadRequest, UnknownMethod, Timeout,
                       Internal, TooLarge, Closed;
FROM RpcTest IMPORT Pipe, CreatePipe, DestroyPipe,
                     ReadA, WriteA, ReadB, WriteB;
FROM RpcServer IMPORT Server, InitServer, RegisterHandler,
                       ServeOnce, FreeServer;
FROM RpcClient IMPORT Client, InitClient, Call, OnReadable,
                       FreeClient;
FROM Scheduler IMPORT Scheduler, SchedulerCreate, SchedulerDestroy,
                       SchedulerPump;
FROM Promise IMPORT Future, Fate, Value, Error, Result,
                     GetFate, GetResultIfSettled;
FROM Strings IMPORT Assign;
IMPORT EventLoop;
FROM Timers IMPORT TimerId;

VAR
  pipe: Pipe;
  srv: Server;
  cli: Client;
  sched: Scheduler;
  f: Future;
  fate: Fate;
  res: Result;
  settled: BOOLEAN;
  bodyBuf: Buf;
  body, empty: BytesView;
  ok: BOOLEAN;
  st: CARDINAL;
  respBuf: POINTER TO Buf;
  i: CARDINAL;

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

PROCEDURE PumpSched(s: Scheduler);
VAR dw: BOOLEAN; tmp: CARDINAL;
BEGIN
  dw := TRUE;
  WHILE dw DO
    tmp := CARDINAL(SchedulerPump(s, 1000, dw))
  END
END PumpSched;

BEGIN
  WriteString("=== m2rpc Echo Demo ==="); WriteLn;

  CreatePipe(pipe, 0, 0);
  st := CARDINAL(SchedulerCreate(256, sched));

  InitServer(srv, ReadB, pipe, WriteB, pipe);
  ok := RegisterHandler(srv, "Echo", 4, EchoHandler, NIL);

  InitClient(cli, ReadA, pipe, WriteA, pipe, sched, NIL);

  (* Build a request body: "Hello, RPC!" *)
  Init(bodyBuf, 64);
  AppendChars(bodyBuf, "Hello, RPC!", 11);
  body := AsView(bodyBuf);

  empty.base := NIL;
  empty.len := 0;

  WriteString("Client: Echo(Hello, RPC!)"); WriteLn;
  Call(cli, "Echo", 4, body, 0, f, ok);

  ok := ServeOnce(srv);
  ok := OnReadable(cli);
  PumpSched(sched);

  st := CARDINAL(GetFate(f, fate));
  IF fate = Fulfilled THEN
    WriteString("Client: response received"); WriteLn;
    st := CARDINAL(GetResultIfSettled(f, settled, res));
    IF settled AND res.isOk AND (res.v.ptr # NIL) THEN
      respBuf := res.v.ptr;
      WriteString("  body: ");
      body := AsView(respBuf^);
      i := 0;
      WHILE i < body.len DO
        Write(CHR(ViewGetByte(body, i)));
        INC(i)
      END;
      WriteLn
    END
  ELSE
    WriteString("Client: call failed!"); WriteLn
  END;

  WriteString("Done."); WriteLn;

  FreeClient(cli);
  FreeServer(srv);
  Free(bodyBuf);
  st := CARDINAL(SchedulerDestroy(sched));
  DestroyPipe(pipe)
END RpcEcho.
