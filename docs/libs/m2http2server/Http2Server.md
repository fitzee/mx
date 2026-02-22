# Http2Server

Top-level HTTP/2 server. Binds a socket, accepts TLS connections with ALPN h2 negotiation, and dispatches HTTP/2 requests through a middleware chain and router.

## Why Http2Server?

Building an HTTP/2 server from raw frames requires coordinating TLS, socket I/O, an event loop, connection tracking, HPACK codec state, flow control, and request routing. `Http2Server` composes all of these into a single `Create / AddRoute / Start / Destroy` workflow, so application code only writes handlers.

Internally it assembles:
- **m2tls** -- server-side TLS context with ALPN `h2`
- **m2sockets** -- listening socket and accept
- **m2evloop** -- event loop with I/O watchers and timers
- **m2futures** -- scheduler for async operations
- **m2fsm** -- lifecycle state machine (see below)
- **m2log** -- structured logging
- **Http2ServerConn** -- per-connection H2 driver
- **Http2Router** -- method+path dispatch
- **Http2Middleware** -- pre-handler chain

## Lifecycle State Machine

```
SrvInit ──Start──▸ SrvRunning ──Drain──▸ SrvDraining ──Drained──▸ SrvStopped
                       │                                              ▲
                       └──────────Stop────────────────────────────────┘
```

| State | Description |
|-------|-------------|
| `SrvInit` | Server created, routes and middleware registered, not yet listening |
| `SrvRunning` | Accepting connections, processing requests in the event loop |
| `SrvDraining` | GOAWAY sent to all connections, waiting up to `drainTimeoutMs` for graceful close |
| `SrvStopped` | All connections closed, event loop exited |

Transitions:
- `Start` moves `SrvInit` → `SrvRunning` (enters the event loop, blocks)
- `Drain` moves `SrvRunning` → `SrvDraining` (sends GOAWAY, stops accepting)
- When all connections close (or drain timeout expires): `SrvDraining` → `SrvStopped`
- `Stop` moves `SrvRunning` → `SrvStopped` immediately (force-closes everything)

## Types

```modula2
TYPE Server = ADDRESS;   (* opaque handle *)
```

Opaque server handle. Created by `Create`, freed by `Destroy`.

## Procedures

### Create

```modula2
PROCEDURE Create(VAR opts: ServerOpts; VAR out: Server): Status;
```

Create a server with the given configuration.

- `opts` -- server options (call `InitDefaultOpts` first, then set `port`, `certPath`, `keyPath`)
- `out` -- on `OK`, receives the opaque server handle

Allocates TLS context from the cert/key paths, binds a listening socket on `opts.port`, initialises the router, middleware chain, event loop, and connection pool.

**Pre:** `opts.certPath` and `opts.keyPath` must point to valid PEM files.
**Post:** On `OK`, `out` is a valid server handle. On error, `out` is `NIL`.

Returns: `OK`, `Invalid` (bad opts), `TLSFailed`, `SysError` (bind failed).

### AddRoute

```modula2
PROCEDURE AddRoute(s: Server; method, path: ARRAY OF CHAR;
                   handler: HandlerProc; ctx: ADDRESS): BOOLEAN;
```

Register a handler for an exact (method, path) match. Must be called before `Start`.

- `s` -- server handle
- `method` -- HTTP method (e.g. `"GET"`, `"POST"`)
- `path` -- request path (e.g. `"/api/users"`)
- `handler` -- procedure to call when this route matches
- `ctx` -- opaque context pointer passed to the handler on every request

Returns `TRUE` on success, `FALSE` if the route table is full (`MaxRoutes`).

### AddMiddleware

```modula2
PROCEDURE AddMiddleware(s: Server; mw: MiddlewareProc;
                        ctx: ADDRESS): BOOLEAN;
```

Add middleware to the pre-handler chain. Middleware runs in insertion order before the matched handler. Must be called before `Start`.

- `s` -- server handle
- `mw` -- middleware procedure
- `ctx` -- opaque context pointer passed to the middleware

Returns `TRUE` on success, `FALSE` if the chain is full (`MaxMiddleware`).

### Start

```modula2
PROCEDURE Start(s: Server): Status;
```

Start the server and enter the event loop. **Blocks** until `Drain` completes or `Stop` is called (typically from a signal handler or timer).

The listen socket is registered with the event loop. On accept readiness, the server performs TLS handshake, validates ALPN, and creates a `Http2ServerConn` for each client.

Returns: `OK` (normal shutdown), `SysError`, `Stopped` (already stopped).

### Drain

```modula2
PROCEDURE Drain(s: Server): Status;
```

Initiate graceful shutdown. Sends GOAWAY to all active connections, stops accepting new connections, and waits up to `drainTimeoutMs` for in-flight requests to complete. After timeout, remaining connections are force-closed and the event loop exits.

Call from a signal handler or shutdown hook.

Returns: `OK`, `Stopped` (already stopped), `Invalid` (not running).

### Stop

```modula2
PROCEDURE Stop(s: Server): Status;
```

Force-stop the server immediately. Closes all connections without GOAWAY and exits the event loop.

Returns: `OK`, `Stopped` (already stopped).

### Destroy

```modula2
PROCEDURE Destroy(VAR s: Server): Status;
```

Free all server resources: TLS context, socket, event loop, connection pool, router, and middleware chain. Sets `s` to `NIL`.

Must be called after `Start` returns (i.e. after the server has stopped).

Returns: `OK`, `Invalid` (nil handle).

## Example

```modula2
MODULE HelloServer;

FROM SYSTEM IMPORT ADDRESS;
FROM InOut IMPORT WriteString, WriteLn;
FROM ByteBuf IMPORT AppendStr;
FROM Http2ServerTypes IMPORT ServerOpts, InitDefaultOpts,
                              Request, Response, Status, OK;
FROM Http2Server IMPORT Server, Create, AddRoute, Start, Destroy;

PROCEDURE HelloHandler(VAR req: Request;
                       VAR resp: Response;
                       ctx: ADDRESS);
BEGIN
  resp.status := 200;
  AppendStr(resp.body, '{"message":"hello"}');
  resp.bodyLen := 19
END HelloHandler;

VAR
  opts: ServerOpts;
  srv: Server;
  st: Status;

BEGIN
  InitDefaultOpts(opts);
  opts.port := 8443;
  opts.certPath := "server.crt";
  opts.keyPath := "server.key";

  st := Create(opts, srv);
  IF st # OK THEN
    WriteString("create failed"); WriteLn;
    HALT
  END;

  AddRoute(srv, "GET", "/hello", HelloHandler, NIL);

  WriteString("listening on :8443"); WriteLn;
  st := Start(srv);   (* blocks until shutdown *)

  st := Destroy(srv)
END HelloServer.
```
