# WebSocket

## Why
Provides a WebSocket client implementing RFC 6455 over m2stream. Async, Future-based API that runs on m2evloop. Supports text, binary, ping/pong, and close frames with proper masking. Single-threaded, no hidden globals.

## Dependencies

m2stream, m2http, m2bytes, m2evloop, m2futures.

## Modules

### WebSocket (client API)

#### Types

- **WebSocket** (ADDRESS) -- Opaque connection handle.
- **State** -- `Connecting`, `Open`, `Closing`, `Closed`.
- **Status** -- `Ok`, `Invalid`, `ConnectFailed`, `ProtocolError`, `Closed`.
- **MessageProc** -- `PROCEDURE(ADDRESS, Opcode, ADDRESS, CARDINAL, ADDRESS)` -- Callback for incoming messages. Parameters: ws handle, opcode, payload pointer, payload length, user context.

#### Procedures

- `PROCEDURE Connect(lp: Loop; sched: Scheduler; url: ARRAY OF CHAR; VAR ws: WebSocket): Future`
  Open a WebSocket connection to url. Returns a Future that resolves when the handshake completes.

- `PROCEDURE Send(ws: WebSocket; opcode: Opcode; data: ADDRESS; len: CARDINAL): Future`
  Send a frame with the given opcode and payload.

- `PROCEDURE SendText(ws: WebSocket; text: ARRAY OF CHAR): Future`
  Send a text frame.

- `PROCEDURE OnMessage(ws: WebSocket; handler: MessageProc; ctx: ADDRESS)`
  Register a callback for incoming messages.

- `PROCEDURE Close(ws: WebSocket; code: CARDINAL; reason: ARRAY OF CHAR): Future`
  Initiate a close handshake with the given status code and reason.

- `PROCEDURE Destroy(ws: WebSocket)`
  Free all resources associated with the connection.

- `PROCEDURE GetState(ws: WebSocket): State`
  Current connection state.

### WsFrame (frame encoding/decoding)

#### Types

- **Opcode** -- `OpContinuation`, `OpText`, `OpBinary`, `OpClose`, `OpPing`, `OpPong` (plus reserved values).
- **FrameHeader** -- Record with `fin`, `opcode`, `masked`, `payloadLen`, `maskKey`, `headerLen`.
- **Status** -- `Ok`, `Incomplete`, `Invalid`.

#### Procedures

- `PROCEDURE DecodeHeader(buf: ADDRESS; bufLen: CARDINAL; VAR hdr: FrameHeader): Status`
  Decode a WebSocket frame header from a byte buffer.

- `PROCEDURE EncodeHeader(VAR hdr: FrameHeader; buf: ADDRESS; maxLen: CARDINAL): CARDINAL`
  Encode a frame header into buf. Returns bytes written.

- `PROCEDURE ApplyMask(data: ADDRESS; len: CARDINAL; VAR mask: ARRAY OF CHAR; offset: CARDINAL)`
  XOR-mask payload data in place.

- `PROCEDURE GenerateMask(VAR mask: ARRAY OF CHAR)`
  Generate a random 4-byte masking key.

- `PROCEDURE IntToOpcode(n: CARDINAL): Opcode`
- `PROCEDURE OpcodeToInt(op: Opcode): CARDINAL`

## Example

```modula2
MODULE WsDemo;

FROM SYSTEM IMPORT ADDRESS;
FROM InOut IMPORT WriteString, WriteLn;
FROM WebSocket IMPORT WebSocket, Connect, SendText, OnMessage,
                       Close, Destroy, MessageProc;
FROM WsFrame IMPORT Opcode, OpText;
FROM EventLoop IMPORT Loop, LoopInit, LoopRun;
FROM Futures IMPORT Scheduler, Future;

VAR
  lp: Loop;
  sched: Scheduler;
  ws: WebSocket;
  f: Future;

  PROCEDURE OnMsg(w: ADDRESS; op: Opcode;
                  data: ADDRESS; len: CARDINAL; ctx: ADDRESS);
  BEGIN
    IF op = OpText THEN
      WriteString("received message"); WriteLn
    END
  END OnMsg;

BEGIN
  LoopInit(lp);
  f := Connect(lp, sched, "ws://echo.websocket.org", ws);
  OnMessage(ws, OnMsg, NIL);
  SendText(ws, "hello");
  LoopRun(lp)
END WsDemo.
```
