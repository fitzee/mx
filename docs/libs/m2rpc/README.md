# m2rpc

## Why
Length-prefixed, binary RPC framework for Modula-2 with transport-agnostic framing, a binary codec, a promise-based client, and a synchronous handler-dispatch server. Works over TCP, TLS, or in-memory pipes without code changes.

## Modules

### RpcErrors

Stable error codes for the RPC wire protocol. Codes 1-99 are reserved for the framework; application-defined codes should start at 100.

#### Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `Ok` | 0 | No error (sentinel, never sent on wire). |
| `BadRequest` | 1 | Malformed request. |
| `UnknownMethod` | 2 | No handler registered for the method. |
| `Timeout` | 3 | Request timed out. |
| `Internal` | 4 | Internal server error. |
| `TooLarge` | 5 | Message exceeds size limit. |
| `Closed` | 6 | Connection closed. |

#### Procedures

```modula2
PROCEDURE ToString(code: CARDINAL; VAR s: ARRAY OF CHAR);
```
Return a human-readable string for a framework error code. Unknown codes return `"Unknown"`. The string is a compile-time constant.

---

### RpcFrame

Length-prefixed framing over an abstract byte transport. No heap allocation in steady state.

#### Wire Format

```
4 bytes  big-endian payload length (0..MaxFrame)
N bytes  payload
```

#### Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `MaxFrame` | 65531 | Maximum payload size in bytes. |
| `TsOk` | 0 | Transport success. |
| `TsWouldBlock` | 1 | No data available (non-blocking). |
| `TsClosed` | 2 | Peer closed connection. |
| `TsError` | 3 | Fatal transport error. |

#### Types

| Type | Description |
|------|-------------|
| `ReadFn` | `PROCEDURE(ADDRESS, ADDRESS, CARDINAL, VAR CARDINAL): CARDINAL` -- transport read callback. |
| `WriteFn` | `PROCEDURE(ADDRESS, ADDRESS, CARDINAL, VAR CARDINAL): CARDINAL` -- transport write callback. |
| `FrameStatus` | `(FrmOk, FrmNeedMore, FrmClosed, FrmTooLarge, FrmError)` -- frame read result. |
| `FrameReader` | Incremental state machine that accumulates bytes across multiple `TryReadFrame` calls until a complete frame is assembled. |

#### Procedures

```modula2
PROCEDURE InitFrameReader(VAR fr: FrameReader;
                           maxFrame: CARDINAL;
                           fn: ReadFn; ctx: ADDRESS);
```
Initialize a frame reader with a maximum payload size and transport callbacks.

```modula2
PROCEDURE TryReadFrame(VAR fr: FrameReader;
                        VAR out: BytesView;
                        VAR status: FrameStatus);
```
Attempt to read a complete frame. Call repeatedly until `FrmOk` (complete frame available in `out`) or an error. `FrmNeedMore` means a partial read occurred; call again when more data is available.

```modula2
PROCEDURE ResetFrameReader(VAR fr: FrameReader);
```
Reset the reader to initial state, discarding any partial frame.

```modula2
PROCEDURE FreeFrameReader(VAR fr: FrameReader);
```
Free the reader's internal buffer.

```modula2
PROCEDURE WriteFrame(fn: WriteFn; ctx: ADDRESS;
                      payload: BytesView;
                      VAR ok: BOOLEAN);
```
Write a complete frame (length prefix + payload) in a blocking loop. Returns `TRUE` on success.

---

### RpcCodec

Binary message encoding and decoding for the RPC wire protocol.

#### Wire Format

All messages share a 6-byte header: `u8 version`, `u8 msg_type`, `u32 request_id` (big-endian).

| Message Type | Code | Additional Fields |
|-------------|------|-------------------|
| Request | 0 | `u16 method_len` + method bytes + `u32 body_len` + body bytes |
| Response | 1 | `u32 body_len` + body bytes |
| Error | 2 | `u16 err_code` + `u16 err_msg_len` + error message + `u32 body_len` + body bytes |

#### Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `Version` | 1 | Protocol version. |
| `MsgRequest` | 0 | Request message type. |
| `MsgResponse` | 1 | Response message type. |
| `MsgError` | 2 | Error message type. |

#### Procedures

**Encoding**

```modula2
PROCEDURE EncodeRequest(VAR buf: Buf;
                         requestId: CARDINAL;
                         method: ARRAY OF CHAR;
                         methodLen: CARDINAL;
                         body: BytesView);
```
Encode an RPC request into `buf`.

```modula2
PROCEDURE EncodeResponse(VAR buf: Buf;
                          requestId: CARDINAL;
                          body: BytesView);
```
Encode an RPC response into `buf`.

```modula2
PROCEDURE EncodeError(VAR buf: Buf;
                       requestId: CARDINAL;
                       errCode: CARDINAL;
                       errMsg: ARRAY OF CHAR;
                       errMsgLen: CARDINAL;
                       body: BytesView);
```
Encode an RPC error into `buf`.

**Decoding**

```modula2
PROCEDURE DecodeHeader(payload: BytesView;
                        VAR version: CARDINAL;
                        VAR msgType: CARDINAL;
                        VAR requestId: CARDINAL;
                        VAR ok: BOOLEAN);
```
Decode the 6-byte common header from a frame payload.

```modula2
PROCEDURE DecodeRequest(payload: BytesView;
                         VAR requestId: CARDINAL;
                         VAR method: BytesView;
                         VAR body: BytesView;
                         VAR ok: BOOLEAN);
```
Decode a request message. `method` and `body` are views into `payload`.

```modula2
PROCEDURE DecodeResponse(payload: BytesView;
                          VAR requestId: CARDINAL;
                          VAR body: BytesView;
                          VAR ok: BOOLEAN);
```
Decode a response message.

```modula2
PROCEDURE DecodeError(payload: BytesView;
                       VAR requestId: CARDINAL;
                       VAR errCode: CARDINAL;
                       VAR errMsg: BytesView;
                       VAR body: BytesView;
                       VAR ok: BOOLEAN);
```
Decode an error message.

---

### RpcClient

Promise-based RPC client with concurrent in-flight requests, optional timeouts via EventLoop, and automatic request-id correlation.

#### Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `MaxInflight` | 64 | Maximum concurrent in-flight requests. |

#### Types

| Type | Description |
|------|-------------|
| `Client` | Client state record holding transport callbacks, scheduler, pending call table, and encoding buffers. |
| `PendingCall` | Per-request tracking: active flag, request id, promise handle, optional timer. |

#### Promise Settlement

- **Fulfilled**: `Value.tag = 0`, `Value.ptr` = pointer to response body `BytesView` (valid until the next `OnReadable` call).
- **Rejected**: `Error.code` = `RpcErrors` error code, `Error.ptr = NIL`.

#### Procedures

```modula2
PROCEDURE InitClient(VAR c: Client;
                      readFn: ReadFn; readCtx: ADDRESS;
                      writeFn: WriteFn; writeCtx: ADDRESS;
                      sched: Scheduler;
                      loop: ADDRESS);
```
Initialize a client. Pass `NIL` for `loop` to disable timeouts.

```modula2
PROCEDURE Call(VAR c: Client;
               method: ARRAY OF CHAR;
               methodLen: CARDINAL;
               body: BytesView;
               timeoutMs: CARDINAL;
               VAR out: Future;
               VAR ok: BOOLEAN);
```
Issue an RPC request. Returns a `Future` that settles when the response arrives or the request times out. `timeoutMs = 0` disables the timeout. `ok` is `FALSE` if the in-flight table is full or the write fails.

```modula2
PROCEDURE OnReadable(VAR c: Client): BOOLEAN;
```
Process incoming data from the transport. Reads frames and dispatches responses to pending promises. Returns `TRUE` if the connection is still alive.

```modula2
PROCEDURE CancelAll(VAR c: Client);
```
Cancel all pending calls with `RpcErrors.Closed`.

```modula2
PROCEDURE FreeClient(VAR c: Client);
```
Free internal buffers. Does not close the transport.

---

### RpcServer

Single-threaded RPC server with synchronous handler dispatch. Handlers are registered per method name and run inline.

#### Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `MaxHandlers` | 32 | Maximum registered handlers. |
| `MaxMethodLen` | 64 | Maximum method name length in bytes. |

#### Types

| Type | Description |
|------|-------------|
| `Handler` | `PROCEDURE(ADDRESS, CARDINAL, ADDRESS, CARDINAL, BytesView, VAR Buf, VAR CARDINAL, VAR BOOLEAN)` -- handler callback. Set `ok=TRUE` and fill `outBody` for success, or set `ok=FALSE` and `errCode` for error. |
| `Server` | Server state record holding frame reader, transport, handler table, and encoding buffers. |

#### Procedures

```modula2
PROCEDURE InitServer(VAR s: Server;
                      readFn: ReadFn; readCtx: ADDRESS;
                      writeFn: WriteFn; writeCtx: ADDRESS);
```
Initialize a server with transport callbacks.

```modula2
PROCEDURE RegisterHandler(VAR s: Server;
                           method: ARRAY OF CHAR;
                           methodLen: CARDINAL;
                           handler: Handler;
                           ctx: ADDRESS): BOOLEAN;
```
Register a handler for a method name. Returns `FALSE` if the handler table is full.

```modula2
PROCEDURE ServeOnce(VAR s: Server): BOOLEAN;
```
Read and dispatch one request. Returns `TRUE` if the connection is still alive.

```modula2
PROCEDURE FreeServer(VAR s: Server);
```
Free internal buffers.

---

### RpcTest

In-memory duplex byte stream for deterministic RPC testing. No network, no syscalls.

#### Types

| Type | Description |
|------|-------------|
| `Pipe` | Opaque pipe handle (`ADDRESS`). Connects two endpoints (A and B) back-to-back. |

#### Procedures

```modula2
PROCEDURE CreatePipe(VAR p: Pipe;
                      readLimit: CARDINAL;
                      writeLimit: CARDINAL);
```
Create a pipe pair. `readLimit` and `writeLimit` control the maximum bytes per I/O call (`0` = unlimited), enabling simulation of partial I/O.

```modula2
PROCEDURE DestroyPipe(VAR p: Pipe);
```
Destroy a pipe and free buffers.

```modula2
PROCEDURE CloseA(p: Pipe);
PROCEDURE CloseB(p: Pipe);
```
Close one endpoint's write direction. The other endpoint will see `TsClosed` after draining.

```modula2
PROCEDURE ReadA(ctx, buf: ADDRESS; max: CARDINAL; VAR got: CARDINAL): CARDINAL;
PROCEDURE WriteA(ctx, buf: ADDRESS; len: CARDINAL; VAR sent: CARDINAL): CARDINAL;
PROCEDURE ReadB(ctx, buf: ADDRESS; max: CARDINAL; VAR got: CARDINAL): CARDINAL;
PROCEDURE WriteB(ctx, buf: ADDRESS; len: CARDINAL; VAR sent: CARDINAL): CARDINAL;
```
Transport functions matching `RpcFrame.ReadFn`/`WriteFn` signatures. Pass directly as transport callbacks with the pipe as the context pointer.

```modula2
PROCEDURE PendingAtoB(p: Pipe): CARDINAL;
PROCEDURE PendingBtoA(p: Pipe): CARDINAL;
```
Query the number of unread bytes in each direction.

## Example

```modula2
MODULE RpcDemo;

FROM SYSTEM IMPORT ADDRESS;
FROM ByteBuf IMPORT Buf, BytesView, BufInit, BufClear,
                     ViewFromBuf, ViewEmpty;
FROM RpcFrame IMPORT ReadFn, WriteFn;
FROM RpcClient IMPORT Client, InitClient, Call, OnReadable, FreeClient;
FROM RpcServer IMPORT Server, InitServer, RegisterHandler,
                      ServeOnce, FreeServer;
FROM RpcTest IMPORT Pipe, CreatePipe, DestroyPipe, ReadA, WriteA,
                    ReadB, WriteB;
FROM Scheduler IMPORT Scheduler, SchedulerCreate, SchedulerPump,
                      SchedulerDestroy;
FROM Promise IMPORT Future;

VAR
  pipe: Pipe;
  client: Client;
  server: Server;
  sched: Scheduler;
  f: Future;
  ok: BOOLEAN;
  didWork: BOOLEAN;
  body: BytesView;

PROCEDURE EchoHandler(ctx: ADDRESS; reqId: CARDINAL;
                      methodPtr: ADDRESS; methodLen: CARDINAL;
                      reqBody: BytesView;
                      VAR outBody: Buf; VAR errCode: CARDINAL;
                      VAR ok: BOOLEAN);
BEGIN
  (* Echo the request body back as the response *)
  BufClear(outBody);
  (* copy reqBody into outBody here *)
  ok := TRUE;
END EchoHandler;

BEGIN
  CreatePipe(pipe, 0, 0);
  SchedulerCreate(64, sched);

  (* Client writes to A, reads from A *)
  InitClient(client, ReadA, pipe, WriteA, pipe, sched, NIL);

  (* Server reads from B, writes to B *)
  InitServer(server, ReadB, pipe, WriteB, pipe);
  ok := RegisterHandler(server, "echo", 4, EchoHandler, NIL);

  (* Issue a request *)
  ViewEmpty(body);
  Call(client, "echo", 4, body, 0, f, ok);

  (* Server processes it *)
  ok := ServeOnce(server);

  (* Client receives the response *)
  ok := OnReadable(client);

  (* Pump scheduler to settle the future *)
  SchedulerPump(sched, 100, didWork);

  FreeClient(client);
  FreeServer(server);
  DestroyPipe(pipe);
  SchedulerDestroy(sched);
END RpcDemo.
```
