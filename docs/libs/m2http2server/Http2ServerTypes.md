# Http2ServerTypes

Shared types and constants for the HTTP/2 server library. All public-facing types live here to avoid circular imports between server, connection, stream, router, and middleware modules.

## Why a Shared Types Module?

The server library has six modules that all need `Request`, `Response`, `HandlerProc`, and `Status`. Putting these types in a single leaf module breaks the cycle: every other module imports `Http2ServerTypes` and nothing imports them back.

## Status

```modula2
TYPE Status = (OK, Invalid, SysError, TLSFailed, ALPNFailed,
               OutOfMemory, PoolExhausted, Stopped, ProtoError);
```

| Value | Meaning |
|-------|---------|
| `OK` | Operation completed successfully |
| `Invalid` | Invalid argument or configuration (e.g. missing cert path) |
| `SysError` | OS-level error (socket bind, accept, etc.) |
| `TLSFailed` | TLS context creation or handshake failed |
| `ALPNFailed` | Client did not negotiate h2 via ALPN |
| `OutOfMemory` | Allocation failed |
| `PoolExhausted` | Connection or stream pool is full |
| `Stopped` | Operation rejected because the server is stopped |
| `ProtoError` | HTTP/2 protocol violation (bad preface, invalid frame) |

## Constants

### Capacity Limits

| Constant | Value | Purpose |
|----------|-------|---------|
| `MaxConns` | 16 | Maximum concurrent connections |
| `MaxStreamSlots` | 32 | Maximum concurrent streams per connection |
| `MaxRoutes` | 64 | Maximum registered routes |
| `MaxMiddleware` | 8 | Maximum middleware in the pre-handler chain |
| `MaxReqHeaders` | 32 | Maximum headers per request |
| `MaxRespHeaders` | 32 | Maximum headers per response |
| `ConnBufSize` | 16384 | 16 KB read/write buffer size per connection |

### String Length Limits

| Constant | Value | Purpose |
|----------|-------|---------|
| `MaxMethodLen` | 15 | HTTP method (e.g. `GET`, `POST`) |
| `MaxPathLen` | 511 | Request path |
| `MaxSchemeLen` | 7 | Scheme (e.g. `https`) |
| `MaxAuthorityLen` | 127 | Authority / Host header |
| `MaxRemoteLen` | 63 | Remote address string |
| `MaxReqNameLen` | 63 | Request header name |
| `MaxReqValueLen` | 255 | Request header value |

### Server Lifecycle FSM

| Constant | Value | State |
|----------|-------|-------|
| `SrvInit` | 0 | Created but not started |
| `SrvRunning` | 1 | Accepting connections, processing requests |
| `SrvDraining` | 2 | GOAWAY sent, waiting for connections to close |
| `SrvStopped` | 3 | Fully stopped |
| `NumSrvStates` | 4 | Total states |

| Constant | Value | Event |
|----------|-------|-------|
| `SrvEvStart` | 0 | Start called |
| `SrvEvDrain` | 1 | Drain called |
| `SrvEvDrained` | 2 | All connections closed |
| `SrvEvStop` | 3 | Stop called |
| `NumSrvEvents` | 4 | Total events |

## Types

### ReqHeader

```modula2
TYPE ReqHeader = RECORD
  name:    ARRAY [0..MaxReqNameLen] OF CHAR;
  nameLen: CARDINAL;
  value:   ARRAY [0..MaxReqValueLen] OF CHAR;
  valLen:  CARDINAL;
END;
```

A compact header entry sized for server use. Smaller than `Http2Types.HeaderEntry` (64+256 bytes vs 128+4096 bytes) to reduce per-request memory.

- `name` / `nameLen` -- header name and its length in characters
- `value` / `valLen` -- header value and its length in characters

### ServerOpts

```modula2
TYPE ServerOpts = RECORD
  port:           CARDINAL;
  certPath:       ARRAY [0..255] OF CHAR;
  keyPath:        ARRAY [0..255] OF CHAR;
  maxConns:       CARDINAL;
  maxStreams:      CARDINAL;
  idleTimeoutMs:  INTEGER;
  hsTimeoutMs:    INTEGER;
  drainTimeoutMs: INTEGER;
END;
```

Server configuration. Call `InitDefaultOpts` to fill with sensible defaults, then override individual fields.

| Field | Default | Description |
|-------|---------|-------------|
| `port` | 8443 | TCP port to listen on |
| `certPath` | `""` | Path to PEM certificate file |
| `keyPath` | `""` | Path to PEM private key file |
| `maxConns` | 16 | Maximum concurrent connections (capped at `MaxConns`) |
| `maxStreams` | 32 | Maximum concurrent streams per connection (capped at `MaxStreamSlots`) |
| `idleTimeoutMs` | 30000 | Idle connection timeout (30 s) |
| `hsTimeoutMs` | 5000 | TLS handshake timeout (5 s) |
| `drainTimeoutMs` | 10000 | Grace period during drain (10 s) |

### Request

```modula2
TYPE Request = RECORD
  method:     ARRAY [0..MaxMethodLen] OF CHAR;
  path:       ARRAY [0..MaxPathLen] OF CHAR;
  scheme:     ARRAY [0..MaxSchemeLen] OF CHAR;
  authority:  ARRAY [0..MaxAuthorityLen] OF CHAR;
  headers:    ARRAY [0..MaxReqHeaders-1] OF ReqHeader;
  numHeaders: CARDINAL;
  body:       Buf;
  bodyLen:    CARDINAL;
  streamId:   CARDINAL;
  connId:     CARDINAL;
  startTick:  INTEGER;
  remoteAddr: ARRAY [0..MaxRemoteLen] OF CHAR;
END;
```

Incoming request populated from decoded HEADERS and DATA frames.

- `method`, `path`, `scheme`, `authority` -- extracted from HTTP/2 pseudo-headers
- `headers` / `numHeaders` -- regular headers (up to `MaxReqHeaders`)
- `body` / `bodyLen` -- accumulated DATA payload (growable `Buf`)
- `streamId` / `connId` -- protocol identifiers for logging and correlation
- `startTick` -- monotonic timestamp when headers arrived
- `remoteAddr` -- client IP:port string

### Response

```modula2
TYPE Response = RECORD
  status:     CARDINAL;
  headers:    ARRAY [0..MaxRespHeaders-1] OF ReqHeader;
  numHeaders: CARDINAL;
  body:       Buf;
  bodyLen:    CARDINAL;
END;
```

Outgoing response. Handlers fill this in; the server encodes it as HEADERS + DATA frames.

- `status` -- HTTP status code (e.g. 200, 404, 500)
- `headers` / `numHeaders` -- response headers
- `body` / `bodyLen` -- response body (growable `Buf`)

### HandlerProc

```modula2
TYPE HandlerProc = PROCEDURE(VAR Request, VAR Response, ADDRESS);
```

Request handler callback. Parameters:

1. `VAR req: Request` -- the incoming request (read fields, may mutate)
2. `VAR resp: Response` -- the outgoing response (fill in status, headers, body)
3. `ctx: ADDRESS` -- opaque context pointer registered with the route

### MiddlewareProc

```modula2
TYPE MiddlewareProc = PROCEDURE(VAR Request, VAR Response, ADDRESS): BOOLEAN;
```

Middleware callback. Returns `TRUE` to continue the chain, `FALSE` to short-circuit (the middleware must have written the response). Parameters are the same as `HandlerProc` plus the boolean return.

## Procedures

### InitDefaultOpts

```modula2
PROCEDURE InitDefaultOpts(VAR opts: ServerOpts);
```

Fill `opts` with default values (see ServerOpts table above). Always call this before overriding individual fields.

### InitRequest

```modula2
PROCEDURE InitRequest(VAR req: Request);
```

Zero-initialise a request. Initialises the body `Buf` to empty.

### InitResponse

```modula2
PROCEDURE InitResponse(VAR resp: Response);
```

Zero-initialise a response. Initialises the body `Buf` to empty.

### FreeRequest

```modula2
PROCEDURE FreeRequest(VAR req: Request);
```

Free the request's body buffer. Call after the handler has finished and the response has been sent.

### FreeResponse

```modula2
PROCEDURE FreeResponse(VAR resp: Response);
```

Free the response's body buffer. Call after the response has been encoded and flushed.
