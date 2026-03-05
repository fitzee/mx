# m2http2server

## Why

Full HTTP/2 server library built on top of m2http2. Accepts TLS connections with ALPN negotiation, dispatches requests through a middleware chain and method+path router, and manages per-connection protocol state including HPACK, flow control, and stream multiplexing.

## Modules

| Module | Purpose |
|--------|---------|
| Http2ServerTypes | Shared types: Request, Response, ServerOpts, HandlerProc, MiddlewareProc, Status, and server lifecycle FSM constants |
| Http2Server | Top-level server: create, start, drain, stop, destroy |
| Http2Router | Method + path routing with exact match and default 404 handler |
| Http2Middleware | Middleware chain with built-in logging, size-limit, and guard middleware |
| Http2ServerConn | Per-connection driver: TLS handshake, H2 preface, frame parsing, stream dispatch |
| Http2ServerStream | Per-stream request assembly (HEADERS + DATA) and response framing |
| Http2ServerMetrics | Server-wide counters for observability (connections, requests, bytes, errors) |
| Http2ServerLog | Structured logging adapter wrapping m2log with server-specific formatters |
| Http2ServerTestUtil | Deterministic test harness: frame builders, scripted peer, in-memory connections |

## Types

### Http2ServerTypes

- **ReqHeader** -- Compact header name/value pair (64-byte name, 256-byte value) for request headers.
- **ServerOpts** -- Server configuration: port, TLS cert/key paths, maxConns (runtime limit, clamped to MaxConns), maxStreams (runtime limit, clamped to MaxStreamSlots), idle/handshake/drain timeouts.
- **Request** -- Incoming request: method, path, scheme, authority, headers array, body buffer, stream/connection IDs, remote address, start timestamp.
- **Response** -- Outgoing response: status code, headers array, body buffer.
- **HandlerProc** -- `PROCEDURE(VAR Request, VAR Response, ADDRESS)` -- called for each matched request.
- **MiddlewareProc** -- `PROCEDURE(VAR Request, VAR Response, ADDRESS): BOOLEAN` -- returns TRUE to continue the chain, FALSE to short-circuit.
- **Status** -- `(OK, Invalid, SysError, TLSFailed, ALPNFailed, OutOfMemory, PoolExhausted, Stopped, ProtoError)`.

### Http2Server

- **Server** -- Opaque handle (ADDRESS) for the top-level server instance.

### Http2Router

- **Route** -- A registered route: method, path, handler procedure, context pointer.
- **Router** -- Route table holding up to MaxRoutes (64) entries.

### Http2Middleware

- **MwEntry** -- A middleware procedure paired with its context pointer.
- **Chain** -- Ordered list of up to MaxMiddleware (8) middleware entries.

### Http2ServerConn

- **ConnRec** -- Full per-connection state: TLS session, H2 protocol state (settings, flow control, HPACK tables, stream slots), I/O buffers, frame parse state, per-connection arena, timers, remote address.
- **ConnPtr** -- `POINTER TO ConnRec`.
- **DispatchProc** -- `PROCEDURE(ADDRESS, VAR Request, VAR Response)` -- server-level dispatch callback.
- **CleanupProc** -- `PROCEDURE(ADDRESS, ConnPtr)` -- connection cleanup callback.

### Http2ServerStream

- **StreamSlot** -- Per-stream slot: active flag, H2Stream FSM, assembled Request, phase (Idle/Headers/Data/Dispatched/Responding/Done), end-stream flags.

### Http2ServerMetrics

- **Metrics** -- Counters: connsAccepted, connsActive, connsClosed, connsRejected, tlsHandshakeFail, alpnReject, streamsOpened, reqTotal, resp2xx, resp4xx, resp5xx, protoErrors, bytesIn, bytesOut.

## Constants

### Limits

| Constant | Value | Description |
|----------|-------|-------------|
| MaxConns | 16 | Compile-time upper bound for concurrent connections |
| MaxStreamSlots | 32 | Compile-time upper bound for concurrent streams per connection |
| MaxRoutes | 64 | Max registered routes |
| MaxMiddleware | 8 | Max middleware in chain |
| MaxReqHeaders | 32 | Max headers per request |
| MaxRespHeaders | 32 | Max headers per response |
| ConnBufSize | 16384 | 16 KB read/write buffers per connection |
| ArenaSize | 32768 | 32 KB per-connection arena |

### Server Lifecycle FSM

States: SrvInit (0), SrvRunning (1), SrvDraining (2), SrvStopped (3).

Events: SrvEvStart (0), SrvEvDrain (1), SrvEvDrained (2), SrvEvStop (3).

### Connection Phases

CpPreface (0), CpSettings (1), CpOpen (2), CpGoaway (3), CpClosed (4).

### Stream Slot Phases

PhIdle (0), PhHeaders (1), PhData (2), PhDispatched (3), PhResponding (4), PhDone (5).

## Procedures

### Http2ServerTypes

- `InitDefaultOpts(VAR opts: ServerOpts)` -- Fill ServerOpts with default values.
- `InitRequest(VAR req: Request)` -- Initialise a Request record.
- `InitResponse(VAR resp: Response)` -- Initialise a Response record.
- `FreeRequest(VAR req: Request)` -- Free request resources (body buffer).
- `FreeResponse(VAR resp: Response)` -- Free response resources (body buffer).

### Http2Server

- `Create(VAR opts: ServerOpts; VAR out: Server): Status` -- Create a server. Allocates TLS context, binds socket, initialises router and middleware. Requires opts.certPath and opts.keyPath.
- `AddRoute(s: Server; method, path: ARRAY OF CHAR; handler: HandlerProc; ctx: ADDRESS): BOOLEAN` -- Register an exact-match route. Must be called before Start.
- `AddMiddleware(s: Server; mw: MiddlewareProc; ctx: ADDRESS): BOOLEAN` -- Add middleware to the pre-handler chain.
- `Start(s: Server): Status` -- Enter the event loop. Blocks until Stop or Drain completes.
- `Drain(s: Server): Status` -- Graceful shutdown: sends GOAWAY to all connections, waits drainTimeoutMs, then stops.
- `Stop(s: Server): Status` -- Force-stop: closes all connections and stops the event loop immediately.
- `Destroy(VAR s: Server): Status` -- Destroy the server and free all resources.

### Http2Router

- `RouterInit(VAR r: Router)` -- Initialise an empty router.
- `AddRoute(VAR r: Router; method, path: ARRAY OF CHAR; handler: HandlerProc; ctx: ADDRESS): BOOLEAN` -- Register a handler for (method, path). Returns FALSE if full.
- `Dispatch(VAR r: Router; VAR req: Request; VAR resp: Response)` -- Dispatch to the matching handler, or the default 404 handler.

### Http2Middleware

- `ChainInit(VAR c: Chain)` -- Initialise an empty middleware chain.
- `ChainAdd(VAR c: Chain; mw: MiddlewareProc; ctx: ADDRESS): BOOLEAN` -- Add middleware. Runs in insertion order. Returns FALSE if full.
- `ChainRun(VAR c: Chain; VAR req: Request; VAR resp: Response; handler: HandlerProc; handlerCtx: ADDRESS)` -- Run all middleware, then call handler if all returned TRUE.
- `LoggingMw(VAR req: Request; VAR resp: Response; ctx: ADDRESS): BOOLEAN` -- Built-in: logs method + path at INFO level. ctx must point to a Log.Logger.
- `SizeLimitMw(VAR req: Request; VAR resp: Response; ctx: ADDRESS): BOOLEAN` -- Built-in: rejects bodies exceeding the limit. ctx must point to a CARDINAL.
- `GuardMw(VAR req: Request; VAR resp: Response; ctx: ADDRESS): BOOLEAN` -- Built-in: catches handler errors (status 0) and returns 500.

### Http2ServerConn

- `ConnCreate(serverPtr: ADDRESS; connId: CARDINAL; clientFd: INTEGER; peer: SockAddr; VAR cp: ConnPtr): BOOLEAN` -- Create a connection from an accepted socket. Initiates TLS handshake.
- `ConnOnEvent(fd, events: INTEGER; user: ADDRESS)` -- EventLoop watcher callback for connection I/O.
- `ConnDrain(cp: ConnPtr)` -- Initiate graceful shutdown by sending GOAWAY.
- `ConnClose(cp: ConnPtr)` -- Close and destroy connection, freeing all resources.
- `ConnFlush(cp: ConnPtr)` -- Flush pending write data.
- `ConnFeedBytes(cp: ConnPtr; data: ADDRESS; len: CARDINAL)` -- Feed raw bytes for testing (bypasses TLS).
- `ConnCreateTest(serverPtr: ADDRESS; connId: CARDINAL; VAR cp: ConnPtr): BOOLEAN` -- Create a test connection with no TLS (in-memory).
- `SetServerDispatch(p: DispatchProc)` -- Set the server dispatch callback.
- `SetConnCleanup(p: CleanupProc)` -- Set the connection cleanup callback.

### Http2ServerStream

- `SlotInit(VAR slot: StreamSlot)` -- Initialise a stream slot.
- `AssembleHeaders(VAR slot: StreamSlot; VAR decoded: ARRAY OF HeaderEntry; numDecoded: CARDINAL; endStream: BOOLEAN): BOOLEAN` -- Extract pseudo-headers and regular headers into the request. Returns FALSE if required pseudo-headers are missing.
- `AccumulateData(VAR slot: StreamSlot; data: BytesView; endStream: BOOLEAN): BOOLEAN` -- Accumulate DATA frame payload into request body.
- `SendResponse(VAR slot: StreamSlot; VAR resp: Response; VAR dynEnc: DynTable; VAR outBuf: Buf; maxFrameSize: CARDINAL; VAR connWindow: INTEGER): CARDINAL` -- Encode response as HEADERS + DATA frames. Respects maxFrameSize and connection flow control.
- `FlushData(VAR slot: StreamSlot; VAR resp: Response; VAR outBuf: Buf; maxFrameSize: CARDINAL; VAR connWindow: INTEGER): CARDINAL` -- Flush remaining buffered response DATA after WINDOW_UPDATE.
- `AllocSlot(VAR slots: ARRAY OF StreamSlot; streamId, initWindowSize: CARDINAL; tablePtr: ADDRESS): CARDINAL` -- Find and initialise an unused slot. Returns MaxStreamSlots if full.
- `FindSlot(VAR slots: ARRAY OF StreamSlot; streamId: CARDINAL): CARDINAL` -- Find slot by stream ID. Returns MaxStreamSlots if not found.
- `SlotFree(VAR slot: StreamSlot)` -- Release a slot after response is complete.

### Http2ServerMetrics

- `MetricsInit(VAR m: Metrics)` -- Zero all counters.
- `IncConnsAccepted(VAR m: Metrics)` -- Increment accepted connections counter.
- `IncConnsActive(VAR m: Metrics)` / `DecConnsActive(VAR m: Metrics)` -- Track active connections.
- `IncConnsClosed(VAR m: Metrics)` -- Increment closed connections counter.
- `IncConnsRejected(VAR m: Metrics)` -- Increment rejected connections counter (at-capacity refusals).
- `IncTLSFail(VAR m: Metrics)` -- Increment TLS handshake failure counter.
- `IncALPNReject(VAR m: Metrics)` -- Increment ALPN rejection counter.
- `IncStreamsOpened(VAR m: Metrics)` -- Increment streams opened counter.
- `IncReqTotal(VAR m: Metrics)` -- Increment total request counter.
- `IncResp(VAR m: Metrics; statusCode: CARDINAL)` -- Increment 2xx/4xx/5xx response bucket.
- `IncProtoErrors(VAR m: Metrics)` -- Increment protocol error counter.
- `AddBytesIn(VAR m: Metrics; n: CARDINAL)` / `AddBytesOut(VAR m: Metrics; n: CARDINAL)` -- Track bytes transferred.
- `MetricsLog(VAR m: Metrics; VAR lg: Logger)` -- Log all counters at INFO level.

### Http2ServerLog

- `LogInit(VAR lg: Logger)` -- Initialise a logger with "h2server" category and console sink.
- `LogRequest(VAR lg: Logger; VAR req: Request; VAR resp: Response; durationTicks: INTEGER)` -- Log request completion with conn_id, stream_id, method, path, status, bodyLen, duration.
- `LogProtocol(VAR lg: Logger; connId: CARDINAL; event, detail: ARRAY OF CHAR)` -- Log protocol events (SETTINGS negotiated, GOAWAY sent/received).
- `LogConn(VAR lg: Logger; connId: CARDINAL; event: ARRAY OF CHAR)` -- Log connection events (accepted, closed, error).

## Example

```modula2
MODULE ServerExample;

FROM SYSTEM IMPORT ADR;
FROM Http2ServerTypes IMPORT ServerOpts, Request, Response,
                              InitDefaultOpts, Status;
FROM Http2Server IMPORT Server, Create, AddRoute, AddMiddleware,
                         Start, Destroy;
FROM Http2Middleware IMPORT LoggingMw, GuardMw;
FROM Log IMPORT Logger;

VAR
  opts: ServerOpts;
  srv:  Server;
  st:   Status;
  lg:   Logger;

PROCEDURE HelloHandler(VAR req: Request; VAR resp: Response;
                        ctx: ADDRESS);
BEGIN
  resp.status := 200;
  (* write body into resp.body *)
END HelloHandler;

BEGIN
  InitDefaultOpts(opts);
  opts.port := 8443;
  opts.maxConns := 8;    (* runtime limit, max MaxConns=16 *)
  opts.maxStreams := 16;  (* runtime limit, max MaxStreamSlots=32 *)
  (* set opts.certPath, opts.keyPath *)

  st := Create(opts, srv);
  IF st = Status.OK THEN
    AddMiddleware(srv, GuardMw, NIL);
    AddMiddleware(srv, LoggingMw, ADR(lg));
    AddRoute(srv, "GET", "/hello", HelloHandler, NIL);
    st := Start(srv);  (* blocks until shutdown *)
    Destroy(srv);
  END;
END ServerExample.
```
