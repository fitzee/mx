# RpcClient

Promise-based RPC client with concurrent in-flight requests. Issues requests over a transport, correlates responses by request ID, and settles Promises with response data or error codes.

## Why Promises?

RPC is inherently asynchronous -- you send a request and receive a response later (maybe milliseconds, maybe seconds). Blocking synchronous RPC ties up a thread for the entire round trip, which doesn't scale. Callback-based RPC quickly leads to deeply nested code.

Promises (from the m2futures library) provide a clean solution: each `Call()` returns a `Future`, which you can chain with `.Map()`, `.OnReject()`, or await in a scheduler loop. The client tracks multiple in-flight requests simultaneously (up to `MaxInflight = 64`) without blocking or spawning threads.

## Design

The client is **single-threaded** and integrates with the m2futures Scheduler. You provide transport read/write functions (same signatures as RpcFrame), an optional EventLoop for timeouts, and a Scheduler for promise settlement.

**Flow**:
1. `InitClient` with transport and optional EventLoop/Scheduler
2. `Call(method, body, timeoutMs)` returns a Future
3. `OnReadable()` when the transport has data -- dispatches responses to pending promises
4. Scheduler pump executes promise continuations
5. `FreeClient` when done

**Timeouts**: If you pass a non-NIL EventLoop to `InitClient`, the client registers a timer for each call. If the timer fires before the response arrives, the promise is rejected with `RpcErrors.Timeout` and the pending slot is freed.

**Promise settlement**:
- **Fulfilled**: `Value.tag = 0`, `Value.ptr` points to a `BytesView` of the response body. The view is valid until the next `OnReadable()` call.
- **Rejected**: `Error.code` is an RpcErrors code (Timeout, Closed, etc.), `Error.ptr = NIL`.

## Constants

| Name | Value | Purpose |
|------|-------|---------|
| `MaxInflight` | 64 | Maximum concurrent in-flight requests |

## Types

### Client

```modula2
TYPE Client = RECORD
  readFn:    ReadFn;
  readCtx:   ADDRESS;
  writeFn:   WriteFn;
  writeCtx:  ADDRESS;
  loop:      ADDRESS;      (* EventLoop.Loop or NIL *)
  sched:     Scheduler;
  nextId:    CARDINAL;
  pending:   ARRAY [0..MaxInflight-1] OF PendingCall;
  outBuf:    Buf;
  respBuf:   Buf;
  alive:     BOOLEAN;
END;
```

Client state. The `pending` array tracks in-flight requests. `outBuf` is reused for encoding each request, `respBuf` holds the last decoded response body (so the BytesView returned in `Value.ptr` remains valid).

## Procedures

### InitClient

```modula2
PROCEDURE InitClient(VAR c: Client;
                     readFn: ReadFn; readCtx: ADDRESS;
                     writeFn: WriteFn; writeCtx: ADDRESS;
                     sched: Scheduler;
                     loop: ADDRESS);
```

Initialize a client with the given transport functions and scheduler.

- `readFn`/`readCtx`: transport for reading responses (e.g., RpcTest.ReadA)
- `writeFn`/`writeCtx`: transport for writing requests (e.g., RpcTest.WriteA)
- `sched`: Scheduler from m2futures for promise settlement
- `loop`: EventLoop.Loop for timeouts (pass NIL to disable timeouts)

```modula2
InitClient(c, ReadA, pipe, WriteA, pipe, sched, NIL);
```

### Call

```modula2
PROCEDURE Call(VAR c: Client;
               method: ARRAY OF CHAR;
               methodLen: CARDINAL;
               body: BytesView;
               timeoutMs: CARDINAL;
               VAR out: Future;
               VAR ok: BOOLEAN);
```

Issue an RPC request and return a Future that settles when the response arrives.

- `method`: method name (open CHAR array)
- `methodLen`: number of characters in method
- `body`: request payload (may be empty with `len=0`)
- `timeoutMs`: timeout in milliseconds (0 = no timeout for this call)
- `out`: Future that will settle with the response or error
- `ok`: set to FALSE if the in-flight table is full or the write fails

**Fulfilled value**: `Value.ptr` points to a `BytesView` of the response body. This view is valid until the next `OnReadable()` call, so copy any data you need before the next read.

**Rejected error**: `Error.code` is an RpcErrors code. Common codes:
- `Timeout`: no response before deadline
- `Closed`: connection closed or `CancelAll()` called
- Error messages from server: `errCode` from the ERR message

```modula2
Call(c, "Sum", 3, argsView, 5000, fut, ok);
IF ok THEN
  Map(fut, sched, OnSumResponse, NIL);
END;
```

### OnReadable

```modula2
PROCEDURE OnReadable(VAR c: Client): BOOLEAN;
```

Process incoming data from the transport. Reads frames and dispatches responses to pending promises. Returns TRUE if the connection is still alive, FALSE if closed or a fatal error occurred.

Call this when the transport signals readability (e.g., from an EventLoop watcher or a test pump loop).

```modula2
WHILE OnReadable(c) DO
  Pump(sched);  (* execute promise continuations *)
END;
```

### CancelAll

```modula2
PROCEDURE CancelAll(VAR c: Client);
```

Cancel all pending calls by rejecting their promises with `RpcErrors.Closed`. Clears the in-flight table. Does **not** close the transport -- the caller is responsible for transport lifecycle.

Use this when you want to abort all pending calls, e.g., during shutdown or after a timeout.

### FreeClient

```modula2
PROCEDURE FreeClient(VAR c: Client);
```

Free the client's internal buffers (`outBuf` and `respBuf`). Does **not** close the transport. After this call, the client must be re-initialized with `InitClient` before reuse.

## Example

```modula2
MODULE ClientDemo;

FROM InOut IMPORT WriteString, WriteCard, WriteLn;
FROM ByteBuf IMPORT Buf, BytesView, Init, Free, AppendByte, AsView;
FROM RpcClient IMPORT Client, InitClient, Call, OnReadable, FreeClient;
FROM RpcServer IMPORT Server, InitServer, RegisterHandler, ServeOnce, FreeServer;
FROM RpcTest IMPORT Pipe, CreatePipe, DestroyPipe, ReadA, WriteA, ReadB, WriteB;
FROM Scheduler IMPORT Scheduler, InitScheduler, Pump;
FROM Promise IMPORT Future, Value, Error, GetFate, GetResultIfSettled, Settled;

(* Simple echo handler for the server *)
PROCEDURE EchoHandler(ctx: ADDRESS; reqId: CARDINAL;
                      method: ARRAY OF CHAR; body: BytesView;
                      VAR outBody: Buf; VAR errCode: CARDINAL;
                      VAR ok: BOOLEAN);
BEGIN
  (* Just echo the request body back *)
  AppendView(outBody, body);
  ok := TRUE
END EchoHandler;

VAR
  pipe: Pipe;
  client: Client;
  server: Server;
  sched: Scheduler;
  req, resp: Buf;
  reqView: BytesView;
  fut: Future;
  ok: BOOLEAN;
  val: Value;
  fate: CARDINAL;

BEGIN
  (* Set up in-memory pipe and scheduler *)
  CreatePipe(pipe, 0, 0);  (* no partial I/O *)
  InitScheduler(sched);

  (* Initialize client and server back-to-back on the pipe *)
  InitClient(client, ReadB, pipe, WriteB, pipe, sched, NIL);
  InitServer(server, ReadA, pipe, WriteA, pipe);
  RegisterHandler(server, "Echo", 4, EchoHandler, NIL);

  (* Issue a request *)
  Init(req, 64);
  AppendByte(req, 42);
  reqView := AsView(req);

  Call(client, "Echo", 4, reqView, 0, fut, ok);
  WriteString("call issued: "); WriteCard(ok, 0); WriteLn;

  (* Serve the request *)
  ServeOnce(server);

  (* Read the response *)
  WHILE OnReadable(client) DO
    Pump(sched);
  END;

  (* Check the result *)
  fate := GetFate(fut);
  IF fate = Settled THEN
    val := GetResultIfSettled(fut, ok);
    IF ok THEN
      WriteString("response received, tag=");
      WriteCard(val.tag, 0); WriteLn;
      (* val.ptr points to BytesView of response body *)
    END
  END;

  Free(req);
  FreeClient(client);
  FreeServer(server);
  DestroyPipe(pipe)
END ClientDemo.
```
