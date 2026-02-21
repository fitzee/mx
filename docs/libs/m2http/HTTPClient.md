# HTTPClient

HTTP/1.1 client using non-blocking sockets and Futures. Sends `Connection: close`, reads the full response, and resolves a Future.

## Overview

`HTTPClient` is the main user-facing module of the m2http library. It issues HTTP GET and HEAD requests over non-blocking TCP sockets, integrated with the m2evloop event loop. Response data is delivered through a Future that resolves with a heap-allocated `Response` record containing status code, headers, and body.

## Design Goals

- **Event-loop integrated**: Uses `EventLoop.WatchFd` for non-blocking connect/send/recv.
- **Futures-based**: Results delivered via Promise/Future — composable with other async operations.
- **No hidden globals**: All state is in the per-connection `ConnRec`.
- **Robust parsing**: Handles Content-Length, chunked transfer encoding, and connection-close body termination.
- **Minimal surface**: Two request procedures (`Get`, `Head`) plus two response helpers (`FindHeader`, `FreeResponse`).

## Architecture

```
┌──────────────────────────────────────────────────────────┐
│  Application                                             │
│  Get(loop, sched, uri, future) → Run(loop) → check      │
├──────────────────────────────────────────────────────────┤
│  HTTPClient                                              │
│                                                          │
│  DoRequest:                                              │
│    1. DNS.ResolveA → AddrRec                             │
│    2. ALLOCATE ConnRec                                   │
│    3. PromiseCreate → promise + future                   │
│    4. ALLOCATE Response                                  │
│    5. Buffers.Create → recvBuf                           │
│    6. BuildRequest → "GET /path HTTP/1.1\r\n..."         │
│    7. SocketCreate + SetNonBlocking                      │
│    8. m2_connect_ipv4 (non-blocking)                     │
│    9. EventLoop.WatchFd → OnSocketEvent                  │
│                                                          │
│  OnSocketEvent (state machine):                          │
│    StConnecting → StSending → StRecvStatus →             │
│    StRecvHeaders → StRecvBody → SucceedConn              │
├──────────────────────────────────────────────────────────┤
│  EventLoop     Buffers     DNS      Sockets              │
│  (fd watch)    (I/O buf)   (resolve) (TCP)               │
└──────────────────────────────────────────────────────────┘
```

## Internal Data Structures

### ConnRec (per-connection state)

```modula2
TYPE ConnRec = RECORD
  state      : INTEGER;         (* StConnecting..StError *)
  sock       : Socket;          (* TCP socket fd *)
  promise    : Promise;         (* for resolving the Future *)
  loop       : EventLoop.Loop;  (* event loop handle *)
  sched      : Scheduler;       (* scheduler handle *)
  recvBuf    : Buffers.Buffer;  (* incoming data buffer *)
  resp       : ResponsePtr;     (* response being built *)
  request    : ARRAY [0..4095] OF CHAR;  (* outgoing request *)
  reqLen     : INTEGER;         (* request byte count *)
  reqSent    : INTEGER;         (* bytes sent so far *)
  contentLen : INTEGER;         (* from Content-Length, or -1 *)
  bodyRead   : INTEGER;         (* body bytes received *)
  chunked    : BOOLEAN;         (* chunked transfer? *)
  headOnly   : BOOLEAN;         (* HEAD request? *)
  chunkState : INTEGER;         (* ChSize/ChData/ChTrailer/ChDone *)
  chunkRem   : INTEGER;         (* bytes remaining in chunk *)
END;
```

### Connection States

| State          | Value | Description                             |
|----------------|-------|-----------------------------------------|
| `StConnecting` | 0     | Non-blocking connect in progress.       |
| `StSending`    | 1     | Sending HTTP request.                   |
| `StRecvStatus` | 2     | Receiving status line.                  |
| `StRecvHeaders`| 3     | Receiving headers.                      |
| `StRecvBody`   | 4     | Receiving body (Content-Length/chunked). |
| `StDone`       | 5     | Complete. Future resolved.              |
| `StError`      | 6     | Failed. Future rejected.                |

### Chunked Sub-States

| State       | Value | Description                        |
|-------------|-------|------------------------------------|
| `ChSize`    | 0     | Reading chunk size line.           |
| `ChData`    | 1     | Reading chunk data bytes.          |
| `ChTrailer` | 2     | Consuming CRLF after chunk data.   |
| `ChDone`    | 3     | Final zero-length chunk received.  |

### Response Record

```modula2
TYPE
  Header = RECORD
    name     : ARRAY [0..63] OF CHAR;
    value    : ARRAY [0..1023] OF CHAR;
    nameLen  : INTEGER;
    valueLen : INTEGER;
  END;

  Response = RECORD
    statusCode    : INTEGER;
    headers       : ARRAY [0..31] OF Header;
    headerCount   : INTEGER;
    body          : Buffer;
    contentLength : INTEGER;   (* -1 if not specified *)
  END;
```

## Memory Model

| Resource      | Allocation        | Freed by             |
|---------------|-------------------|----------------------|
| ConnRec       | DoRequest         | SucceedConn/FailConn |
| Response      | DoRequest         | FreeResponse (caller)|
| recvBuf       | DoRequest         | SucceedConn/FailConn |
| body Buffer   | OnSocketEvent     | FreeResponse (caller)|
| AddrRec (DNS) | DNS.ResolveA      | DoRequest            |

On success, the body buffer is transferred from `recvBuf` to `Response.body`. The caller is responsible for calling `FreeResponse` to free both the Response and its body buffer.

On failure, all resources are cleaned up by `FailConn` and the Future is rejected.

## Error Model

### Status (returned by Get/Head)

| Status          | Meaning                                       |
|-----------------|-----------------------------------------------|
| `OK`            | Request initiated. Future will settle later.  |
| `Invalid`       | NIL loop or scheduler.                        |
| `ConnectFailed` | Socket creation or connect failed.            |
| `DNSFailed`     | Hostname resolution failed.                   |
| `OutOfMemory`   | Heap or Promise pool exhausted.               |
| `ParseError`    | (not returned by Get/Head, used internally)   |
| `Timeout`       | (reserved for future use)                     |
| `TLSFailed`     | TLS context or session creation failed.       |

### Rejection Error Codes (in Future)

| Code | Meaning                           |
|------|-----------------------------------|
| 1    | Connect failed or socket error.   |
| 2    | Send failed.                      |
| 3    | Recv failed or connection closed. |
| 5    | HTTP parse error.                 |
| 6    | TLS error (handshake, verify).    |

## Performance Characteristics

- **Non-blocking I/O**: Connect, send, and recv are all non-blocking. The event loop polls for readiness.
- **Incremental parsing**: Status line and headers are parsed incrementally as data arrives.
- **Single allocation per request**: One `ConnRec` + one `Response` + two `Buffer`s (~130 KB total).
- **No connection pooling**: Each request creates and closes a TCP connection.

## Limitations

- **HTTPS**: Supported via m2tls. Peer verification ON by default.
- **No HTTP/2**: HTTP/1.1 only.
- **No keep-alive**: Sends `Connection: close`. No connection pooling.
- **No request body**: GET and HEAD only. No POST/PUT support.
- **64 KB body limit**: Body size limited by `Buffers.MaxCap`.
- **32 headers max**: `MaxHeaders = 32`.
- **4 KB request limit**: `MaxReqSize = 4096` for the outgoing request.
- **Blocking DNS**: DNS resolution blocks the event loop (see DNS module).
- **No redirects**: Does not follow 3xx redirects automatically.
- **No timeouts**: No per-request timeout (future extension).

## Future Extension Points

- ~~**TLS support**~~: Done. HTTPS via m2tls (OpenSSL/LibreSSL).
- **Connection pooling**: Reuse connections with keep-alive.
- **POST/PUT**: Add request body support with `DoRequest` accepting a body buffer.
- **Automatic redirects**: Follow 3xx responses up to a configurable limit.
- **Request timeouts**: Timer-based timeout that rejects the Future.
- **Streaming**: Callback-based body reception for large responses.
- **HTTP/2**: Frame-based protocol implementation.

## API Reference

### Constants

| Constant        | Value | Description               |
|-----------------|-------|---------------------------|
| `MaxHeaders`    | 32    | Max response headers.     |
| `MaxHeaderName` | 64    | Max header name length.   |
| `MaxHeaderVal`  | 1024  | Max header value length.  |

### Types

**`Header`** — Response header with name/value arrays and lengths.

**`Response`** — Complete HTTP response: status code, headers, body buffer, content length.

**`ResponsePtr`** — `POINTER TO Response`.

**`Status`** — `(OK, Invalid, ConnectFailed, DNSFailed, OutOfMemory, ParseError, Timeout, TLSFailed)`.

### Procedures

```modula2
PROCEDURE Get(lp: Loop; sched: Scheduler;
              VAR uri: URIRec;
              VAR outFuture: Future): Status;
```

Issue an HTTP GET request. On `OK`, the event loop will drive the request to completion and settle `outFuture`. The Future resolves with `Value.ptr` pointing to a heap-allocated `Response`. Caller must call `FreeResponse` when done.

```modula2
PROCEDURE Head(lp: Loop; sched: Scheduler;
               VAR uri: URIRec;
               VAR outFuture: Future): Status;
```

Issue an HTTP HEAD request. Same as `Get` but the server returns no body.

```modula2
PROCEDURE FindHeader(resp: ResponsePtr;
                     VAR name: ARRAY OF CHAR;
                     VAR out: ARRAY OF CHAR): BOOLEAN;
```

Case-insensitive header lookup. Returns `TRUE` if found, copies value into `out`.

```modula2
PROCEDURE FreeResponse(VAR resp: ResponsePtr);
```

Free a Response and its body buffer. Sets `resp` to `NIL`.

## Example

```modula2
FROM EventLoop IMPORT Loop, Create, GetScheduler, Run;
FROM URI IMPORT URIRec, Parse;
FROM HTTPClient IMPORT Get, FreeResponse, FindHeader;
FROM Promise IMPORT Future, GetResultIfSettled;

VAR loop: Loop; uri: URIRec; future: Future; resp: ResponsePtr;

Parse("http://example.com/api", uri);
Create(loop);
Get(loop, GetScheduler(loop), uri, future);
Run(loop);
(* ... check future, use resp, FreeResponse(resp) ... *)
```

## See Also

- [Buffers](Buffers.md) — Buffer used for recv and response body
- [URI](URI.md) — URI parsing for request targeting
- [DNS](DNS.md) — Hostname resolution
- [Net-Architecture](Net-Architecture.md) — Overall networking stack design
- [../m2evloop/EventLoop](../m2evloop/EventLoop.md) — Event loop integration
- [../m2futures/Promise](../m2futures/Promise.md) — Future/Promise types
- [../m2tls/TLS](../m2tls/TLS.md) — TLS transport layer (HTTPS support)
