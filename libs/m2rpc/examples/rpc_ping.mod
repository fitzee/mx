(* rpc_ping.mod -- In-memory Ping/Pong RPC demo.
   Build: m2c rpc_ping.mod -I ../src -I ../../m2bytes/src \
          -I ../../m2futures/src -I ../../m2evloop/src \
          ../../m2evloop/src/poller_bridge.c *)
MODULE RpcPing;

FROM InOut IMPORT WriteString, WriteLn;
FROM SYSTEM IMPORT ADDRESS;
FROM ByteBuf IMPORT Buf, BytesView, Clear, AppendByte, Init, Free,
                     AsView, AppendView, ViewGetByte, AppendChars;
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
FROM RpcTest IMPORT Pipe, CreatePipe, DestroyPipe, ReadA, WriteA,
                     ReadB, WriteB;
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
  empty: BytesView;
  ok: BOOLEAN;
  st: CARDINAL;

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

PROCEDURE PumpSched(s: Scheduler);
VAR dw: BOOLEAN; tmp: CARDINAL;
BEGIN
  dw := TRUE;
  WHILE dw DO
    tmp := CARDINAL(SchedulerPump(s, 1000, dw))
  END
END PumpSched;

BEGIN
  WriteString("=== m2rpc Ping Demo ==="); WriteLn;

  CreatePipe(pipe, 0, 0);
  st := CARDINAL(SchedulerCreate(256, sched));
  empty.base := NIL;
  empty.len := 0;

  (* Set up server with Ping handler *)
  InitServer(srv, ReadB, pipe, WriteB, pipe);
  ok := RegisterHandler(srv, "Ping", 4, PingHandler, NIL);

  (* Set up client *)
  InitClient(cli, ReadA, pipe, WriteA, pipe, sched, NIL);

  (* Issue a Ping call *)
  WriteString("Client: calling Ping..."); WriteLn;
  Call(cli, "Ping", 4, empty, 0, f, ok);
  IF NOT ok THEN
    WriteString("ERROR: Call failed"); WriteLn
  END;

  (* Server processes the request *)
  ok := ServeOnce(srv);

  (* Client reads the response *)
  ok := OnReadable(cli);
  PumpSched(sched);

  (* Check the result *)
  st := CARDINAL(GetFate(f, fate));
  IF fate = Fulfilled THEN
    WriteString("Client: received Pong!"); WriteLn
  ELSE
    WriteString("Client: call failed"); WriteLn
  END;

  WriteString("Done."); WriteLn;

  FreeClient(cli);
  FreeServer(srv);
  st := CARDINAL(SchedulerDestroy(sched));
  DestroyPipe(pipe)
END RpcPing.
