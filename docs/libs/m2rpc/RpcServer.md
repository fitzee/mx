# RpcServer

Single-threaded RPC server with handler dispatch. Accepts requests via a transport, dispatches to registered handler procedures, and writes responses back through the same transport.

## Design

The server is **synchronous** and **single-threaded**. Handlers are pure procedure values that run inline -- no threads, no promises, no async I/O. This keeps the server simple and predictable: each call to `ServeOnce()` reads frames until the transport reports `NeedMore` or an error, dispatching each complete request to its handler immediately.

For async handlers (e.g., handlers that need to query a database or call another service), wrap the server in an event-loop watcher and integrate with m2futures at the application level. The server itself only knows about synchronous handlers.

**Handler dispatch** is a linear scan over the registry -- O(N) for N methods. This is fast for the typical small N of an RPC service (< 32 methods). If you need hundreds of methods, consider a hash map or trie.

**Flow per request**:
1. `TryReadFrame` reads a complete frame
2. `DecodeRequest` parses the method name and body
3. Lookup handler by method name
4. Call `handler(ctx, reqId, method, body, outBody, errCode, ok)`
5. If `ok = TRUE`: encode and send `RESP(reqId, outBody)`
6. If `ok = FALSE`: encode and send `ERR(reqId, errCode, message)`
7. Unknown method: send `ERR(reqId, UnknownMethod, "unknown method")`

Malformed frames send `ERR(BadRequest)` if a request ID is recoverable, otherwise the connection should close.

## Constants

| Name | Value | Purpose |
|------|-------|---------|
| `MaxHandlers` | 32 | Maximum number of registered methods |
| `MaxMethodLen` | 64 | Maximum method name length in characters |

## Types

### Handler

```modula2
TYPE Handler = PROCEDURE(ADDRESS, CARDINAL,
                         ARRAY OF CHAR, BytesView,
                         VAR Buf, VAR CARDINAL, VAR BOOLEAN);
```

Handler callback signature. Parameters:
1. `ctx: ADDRESS` -- handler-specific context pointer (registered with the handler)
2. `requestId: CARDINAL` -- correlation ID from the request
3. `method: ARRAY OF CHAR` -- method name
4. `body: BytesView` -- request payload
5. `outBody: VAR Buf` -- buffer to fill with response payload
6. `errCode: VAR CARDINAL` -- error code to return if `ok = FALSE`
7. `ok: VAR BOOLEAN` -- set TRUE for success (RESP), FALSE for error (ERR)

**Success**: Set `ok := TRUE` and fill `outBody` with the response payload.

**Error**: Set `ok := FALSE` and `errCode` to an RpcErrors code (or application code >= 100). The server automatically generates an ERR message with a default error string.

```modula2
PROCEDURE MyHandler(ctx: ADDRESS; reqId: CARDINAL;
                    method: ARRAY OF CHAR; body: BytesView;
                    VAR outBody: Buf; VAR errCode: CARDINAL;
                    VAR ok: BOOLEAN);
BEGIN
  (* ... process request ... *)
  AppendByte(outBody, 42);
  ok := TRUE
END MyHandler;
```

### Server

```modula2
TYPE Server = RECORD
  frameReader: FrameReader;
  writeFn: WriteFn;
  writeCtx: ADDRESS;
  handlers: ARRAY [0..MaxHandlers-1] OF HandlerEntry;
  handlerCount: CARDINAL;
  outBuf: Buf;
  respBuf: Buf;
END;
```

Server state. The `handlers` array holds registered method handlers. `outBuf` is reused for encoding each response, `respBuf` is passed to handlers as the output buffer.

## Procedures

### InitServer

```modula2
PROCEDURE InitServer(VAR s: Server;
                     readFn: ReadFn; readCtx: ADDRESS;
                     writeFn: WriteFn; writeCtx: ADDRESS);
```

Initialize a server with the given transport functions.

- `readFn`/`readCtx`: transport for reading incoming requests
- `writeFn`/`writeCtx`: transport for writing outgoing responses

```modula2
InitServer(s, ReadA, pipe, WriteA, pipe);
```

### RegisterHandler

```modula2
PROCEDURE RegisterHandler(VAR s: Server;
                          method: ARRAY OF CHAR;
                          methodLen: CARDINAL;
                          handler: Handler;
                          ctx: ADDRESS): BOOLEAN;
```

Register a handler procedure for the given method name. Returns TRUE on success, FALSE if the registry is full (MaxHandlers reached).

- `method`: method name (open CHAR array)
- `methodLen`: number of characters in method
- `handler`: procedure value to call when this method is requested
- `ctx`: opaque context pointer passed to the handler on every call

```modula2
ok := RegisterHandler(s, "Sum", 3, SumHandler, NIL);
ok := RegisterHandler(s, "Product", 7, ProductHandler, NIL);
```

### ServeOnce

```modula2
PROCEDURE ServeOnce(VAR s: Server): BOOLEAN;
```

Process incoming data. Reads frames until `NeedMore` or an error. For each complete request:
1. Dispatch to registered handler
2. Send response (RESP or ERR)

Returns TRUE if the connection is still alive, FALSE if closed or a fatal error occurred.

Call this repeatedly from an event loop or a manual pump:

```modula2
WHILE ServeOnce(s) DO
  (* connection alive *)
END;
(* connection closed or error *)
```

**Automatic error handling**:
- Unknown method: sends `ERR(UnknownMethod, "unknown method")`
- Malformed frame: sends `ERR(BadRequest, "malformed request")` if possible

### FreeServer

```modula2
PROCEDURE FreeServer(VAR s: Server);
```

Free the server's internal buffers (`outBuf`, `respBuf`, and the frame reader's payload buffer). Does **not** close the transport. After this call, the server must be re-initialized with `InitServer` before reuse.

## Example

```modula2
MODULE ServerDemo;

FROM InOut IMPORT WriteString, WriteCard, WriteLn;
FROM SYSTEM IMPORT ADDRESS;
FROM ByteBuf IMPORT Buf, BytesView, AppendByte, AppendView;
FROM RpcServer IMPORT Server, InitServer, RegisterHandler, ServeOnce, FreeServer;
FROM RpcTest IMPORT Pipe, CreatePipe, DestroyPipe, ReadA, WriteA;
FROM RpcFrame IMPORT WriteFrame;
FROM RpcCodec IMPORT EncodeRequest;
FROM Codec IMPORT Reader, InitReader, ReadU32BE;

(* Handler: decode two u32 args, return their sum *)
PROCEDURE SumHandler(ctx: ADDRESS; reqId: CARDINAL;
                     method: ARRAY OF CHAR; body: BytesView;
                     VAR outBody: Buf; VAR errCode: CARDINAL;
                     VAR ok: BOOLEAN);
VAR
  r: Reader;
  a, b, sum: CARDINAL;
  readOk: BOOLEAN;
  w: Writer;
BEGIN
  InitReader(r, body);
  a := ReadU32BE(r, readOk);
  b := ReadU32BE(r, readOk);

  IF NOT readOk THEN
    errCode := BadRequest;
    ok := FALSE;
    RETURN
  END;

  sum := a + b;

  (* Encode sum as u32be into outBody *)
  InitWriter(w, outBody);
  WriteU32BE(w, sum);
  ok := TRUE
END SumHandler;

VAR
  pipe: Pipe;
  server: Server;
  reqBuf, bodyBuf: Buf;
  bodyView, reqView: BytesView;
  writeOk: BOOLEAN;
  w: Writer;

BEGIN
  CreatePipe(pipe, 0, 0);

  (* Initialize server and register handler *)
  InitServer(server, ReadA, pipe, WriteA, pipe);
  RegisterHandler(server, "Sum", 3, SumHandler, NIL);

  (* Client: encode a Sum(10, 32) request *)
  Init(bodyBuf, 16);
  InitWriter(w, bodyBuf);
  WriteU32BE(w, 10);
  WriteU32BE(w, 32);
  bodyView := AsView(bodyBuf);

  Init(reqBuf, 256);
  EncodeRequest(reqBuf, 1, "Sum", 3, bodyView);
  reqView := AsView(reqBuf);

  (* Write frame to pipe *)
  WriteFrame(WriteA, pipe, reqView, writeOk);
  WriteString("request sent: "); WriteCard(writeOk, 0); WriteLn;

  (* Serve the request *)
  ServeOnce(server);

  WriteString("request handled"); WriteLn;

  Free(reqBuf);
  Free(bodyBuf);
  FreeServer(server);
  DestroyPipe(pipe)
END ServerDemo.
```
