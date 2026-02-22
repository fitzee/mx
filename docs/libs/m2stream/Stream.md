# Stream

Transport-agnostic byte stream for TCP and TLS connections. Wraps m2sockets and m2tls behind a single opaque handle with both synchronous (try-once) and asynchronous (Future-returning) I/O operations.

## Overview

The Stream module sits between higher-level consumers (HTTPClient, application code) and the raw transport layer (Sockets, TLS). It provides a uniform read/write interface regardless of whether the underlying connection is plain TCP or encrypted TLS.

Two usage modes:

1. **Sync (try-once)**: `TryRead`, `TryWrite` return immediately with a status. The caller drives retries from its own EventLoop watcher. Used by HTTPClient for HTTP/HTTPS support.

2. **Async (Future-returning)**: `ReadAsync`, `WriteAsync`, `WriteAllAsync`, `CloseAsync` register their own EventLoop watcher and return a Future. For standalone use without HTTPClient.

## Design Goals

- **Transport-agnostic**: TCP and TLS share the same API. Callers switch transports by choosing `CreateTCP` or `CreateTLS` at construction time; all subsequent I/O is identical.
- **Event-loop integrated**: Non-blocking operations with proper watcher management.
- **No hidden globals**: All state lives in the opaque Stream handle.
- **No threads**: Single-threaded model. All callbacks run on the event loop thread.
- **Deterministic cleanup**: `Destroy` releases TLS resources and unwatches the fd.
- **Single pending operation**: At most one async operation per stream at a time, enforced at the API level.

## Types

**`Stream`** -- Opaque handle (ADDRESS) wrapping a heap-allocated `StreamRec`. A NIL value indicates no valid stream.

**`StreamKind`** -- Transport variant:

| Value       | Description                                   |
|-------------|-----------------------------------------------|
| `TCP`       | Plain TCP socket. I/O via `m2_recv`/`m2_send`.|
| `TLSStream` | Encrypted TLS session over TCP socket.        |

**`StreamState`** -- Current lifecycle state:

| Value        | Description                                              |
|--------------|----------------------------------------------------------|
| `Open`       | Normal operating state. Read and write permitted.        |
| `ShutdownWr` | Write side shut down (`ShutdownWrite` called). Reads still possible. |
| `Closed`     | Stream fully closed. No operations permitted.            |
| `Error`      | Fatal error occurred. No operations permitted.           |

**`Status`** -- Return value from every procedure:

| Value          | Meaning                                                      |
|----------------|--------------------------------------------------------------|
| `OK`           | Operation succeeded.                                         |
| `Invalid`      | Bad argument: NIL handle, negative fd, op already pending, or invalid state. |
| `WouldBlock`   | TLS needs to retry in a different I/O direction. Sync API only. |
| `StreamClosed` | Peer closed the connection (recv returned 0 or TLS close_notify). |
| `SysError`     | OS-level socket error.                                       |
| `TLSError`     | TLS engine error (OpenSSL/LibreSSL failure).                 |
| `OutOfMemory`  | Heap allocation or Promise pool exhausted.                   |

## State Machine

```
                     CreateTCP / CreateTLS
                            │
                     ┌──────▼──────┐
                     │    Open     │
                     └──┬───┬───┬─┘
                        │   │   │
         ShutdownWrite  │   │   │  fatal error (SysError/TLSError)
                        │   │   │
                 ┌──────▼┐  │  ┌▼──────┐
                 │Shutdown│  │  │ Error │
                 │  Wr    │  │  └───────┘
                 └───┬────┘  │
                     │       │
                     │  CloseAsync / Destroy
                     │       │
                     └───┬───┘
                         │
                  ┌──────▼──────┐
                  │   Closed    │
                  └─────────────┘
```

Transitions:

- **Open -> ShutdownWr**: `ShutdownWrite` called. Sends TCP FIN (and TLS `close_notify`, best-effort). Reads remain possible.
- **Open -> Closed**: `CloseAsync` completes or `Destroy` called.
- **Open -> Error**: Fatal I/O or TLS error during `TryRead`, `TryWrite`, or an async operation.
- **ShutdownWr -> Closed**: `CloseAsync` completes or `Destroy` called.

## Procedures

### CreateTCP

```modula2
PROCEDURE CreateTCP(lp: Loop; sched: Scheduler;
                    fd: INTEGER;
                    VAR out: Stream): Status;
```

Create a Stream over a connected, non-blocking TCP socket. The socket must already have completed its TCP handshake.

**Pre**: `lp` and `sched` are non-NIL. `fd` is a non-negative connected socket in non-blocking mode.

**Post**: On `OK`, `out` holds a valid Stream in state `Open`.

**Returns**: `Invalid` if `lp`, `sched`, or `fd` is bad. `OutOfMemory` if heap allocation fails.

```modula2
st := Stream.CreateTCP(loop, sched, fd, strm);
IF st # Stream.OK THEN (* handle error *) END;
```

### CreateTLS

```modula2
PROCEDURE CreateTLS(lp: Loop; sched: Scheduler;
                    fd: INTEGER;
                    ctx: TLS.TLSContext;
                    sess: TLS.TLSSession;
                    VAR out: Stream): Status;
```

Create a Stream over a completed TLS session. The TLS handshake must already be finished. **Stream takes ownership** of `ctx` and `sess` -- they are destroyed when `Destroy` is called.

**Pre**: `lp`, `sched`, `ctx`, and `sess` are non-NIL. `fd` is a non-negative connected socket. TLS handshake is complete.

**Post**: On `OK`, `out` holds a valid Stream in state `Open` with kind `TLSStream`.

**Returns**: `Invalid` if any handle is NIL or `fd` is negative. `OutOfMemory` if heap allocation fails.

```modula2
st := Stream.CreateTLS(loop, sched, fd, tlsCtx, tlsSess, strm);
IF st # Stream.OK THEN (* handle error *) END;
```

### TryRead

```modula2
PROCEDURE TryRead(s: Stream; buf: ADDRESS; max: INTEGER;
                  VAR got: INTEGER): Status;
```

Attempt to read up to `max` bytes into `buf`. Returns immediately with whatever data is available.

**Pre**: Caller manages the EventLoop watcher on the underlying fd. Stream is in state `Open` or `ShutdownWr`.

**Post**: On `OK`, `got` contains the number of bytes read (1..max).

**Returns**:

| Status        | When                                               | Action                          |
|---------------|----------------------------------------------------|---------------------------------|
| `OK`          | Data read successfully.                            | Process `got` bytes.            |
| `WouldBlock`  | TLS renegotiation (TLS streams only).              | Wait for fd readiness, retry.   |
| `StreamClosed`| Peer closed the connection.                        | Handle EOF.                     |
| `SysError`    | OS socket error (TCP streams).                     | Stream transitions to Error.    |
| `TLSError`    | TLS engine error (TLS streams).                    | Stream transitions to Error.    |
| `Invalid`     | NIL stream handle.                                 | Programming error.              |

For TCP streams, `WouldBlock` is never returned -- the underlying `m2_recv` either succeeds or fails. `WouldBlock` only occurs on TLS streams when OpenSSL returns `WANT_READ` or `WANT_WRITE` during renegotiation. When `WouldBlock` is returned, Stream automatically calls `EventLoop.ModifyFd` to adjust the watcher mask for the correct I/O direction.

### TryWrite

```modula2
PROCEDURE TryWrite(s: Stream; buf: ADDRESS; len: INTEGER;
                   VAR sent: INTEGER): Status;
```

Attempt to write up to `len` bytes from `buf`. May perform a partial write -- the caller must check `sent` and loop if all bytes must be delivered.

**Pre**: Caller manages the EventLoop watcher. Stream is in state `Open` (not `ShutdownWr`).

**Post**: On `OK`, `sent` contains the number of bytes written (1..len).

**Returns**:

| Status        | When                                               | Action                          |
|---------------|----------------------------------------------------|---------------------------------|
| `OK`          | Data written successfully.                         | Advance by `sent` bytes.        |
| `WouldBlock`  | TLS renegotiation or send buffer full.             | Wait for fd readiness, retry.   |
| `StreamClosed`| Stream state is `Closed` or `Error`.               | Handle shutdown.                |
| `Invalid`     | NIL handle or stream in `ShutdownWr` state.        | Programming error.              |
| `SysError`    | OS socket error (TCP streams).                     | Stream transitions to Error.    |
| `TLSError`    | TLS engine error (TLS streams).                    | Stream transitions to Error.    |

### ReadAsync

```modula2
PROCEDURE ReadAsync(s: Stream; buf: ADDRESS; max: INTEGER;
                    VAR out: Future): Status;
```

Read up to `max` bytes asynchronously. Stream registers an EventLoop watcher on the fd and handles retries internally. `buf` must remain valid until the Future settles.

**Pre**: Stream is in state `Open`. No other async operation is pending. No other watcher is registered on this fd.

**Post**: On `OK`, `out` holds a Future that will settle when data arrives.

**Future resolves**: `Value.tag` = bytes read (1..max), `Value.ptr` = NIL.

**Future rejects**: `Error.code` = 1 (I/O error) or 2 (peer closed), `Error.ptr` = NIL.

**Returns**: `Invalid` if stream is NIL, not Open, or an operation is already pending. `OutOfMemory` if Promise pool exhausted.

### WriteAsync

```modula2
PROCEDURE WriteAsync(s: Stream; buf: ADDRESS; len: INTEGER;
                     VAR out: Future): Status;
```

Write up to `len` bytes asynchronously. May perform a partial write -- the Future resolves with the number of bytes actually written, which may be less than `len`. `buf` must remain valid until the Future settles.

**Pre**: Stream is in state `Open`. No other async operation pending.

**Post**: On `OK`, `out` holds a Future.

**Future resolves**: `Value.tag` = bytes written (may be < len), `Value.ptr` = NIL.

**Future rejects**: `Error.code` = 1 (I/O error), `Error.ptr` = NIL.

### WriteAllAsync

```modula2
PROCEDURE WriteAllAsync(s: Stream; buf: ADDRESS; len: INTEGER;
                        VAR out: Future): Status;
```

Write all `len` bytes asynchronously. Loops internally until every byte is sent or an error occurs. `buf` must remain valid until the Future settles.

**Pre**: Stream is in state `Open`. No other async operation pending.

**Post**: On `OK`, `out` holds a Future.

**Future resolves**: `Value.tag` = `len` (total bytes written), `Value.ptr` = NIL.

**Future rejects**: `Error.code` = 1 (I/O error), `Error.ptr` = NIL.

The internal loop uses `OffsetPtr` to advance through the buffer, calling `TryWrite` repeatedly and re-registering the watcher after each partial write.

### CloseAsync

```modula2
PROCEDURE CloseAsync(s: Stream; VAR out: Future): Status;
```

Initiate graceful close asynchronously. For TLS streams, sends `close_notify` then closes the socket. For TCP streams, closes the socket directly. The stream is unusable after the close completes.

**Pre**: No other async operation pending.

**Post**: On `OK`, `out` holds a Future. When the Future resolves, the stream is in state `Closed` with fd closed and watcher removed.

**Future resolves**: `Value.tag` = 0, `Value.ptr` = NIL.

The TLS shutdown may require multiple round-trips (WANT_READ/WANT_WRITE), which are handled internally by the watcher callback.

### ShutdownWrite

```modula2
PROCEDURE ShutdownWrite(s: Stream): Status;
```

Half-close the write side synchronously. Sends TCP FIN via `Sockets.Shutdown(fd, SHUT_WR)`. For TLS streams, also sends `close_notify` (best-effort -- WANT_READ/WANT_WRITE results are ignored). The stream transitions to `ShutdownWr` state; reads remain possible.

**Pre**: Stream is in state `Open`.

**Post**: Stream transitions to `ShutdownWr`.

**Returns**: `Invalid` if stream is NIL or not in `Open` state.

```modula2
st := ShutdownWrite(strm);
(* Can still TryRead; writes are no longer permitted *)
```

### GetState

```modula2
PROCEDURE GetState(s: Stream): StreamState;
```

Return the current stream state. Returns `Error` if `s` is NIL.

### GetFd

```modula2
PROCEDURE GetFd(s: Stream): INTEGER;
```

Return the underlying file descriptor. Returns `InvalidSocket` (-1) if `s` is NIL.

### GetKind

```modula2
PROCEDURE GetKind(s: Stream): StreamKind;
```

Return the stream transport kind (`TCP` or `TLSStream`). Returns `TCP` if `s` is NIL.

### Destroy

```modula2
PROCEDURE Destroy(VAR s: Stream): Status;
```

Destroy the stream and release all resources. Cleanup order:

1. For TLS streams: call `TLS.Shutdown`, then `TLS.SessionDestroy` and `TLS.ContextDestroy`.
2. If Stream owns a watcher (async mode): call `EventLoop.UnwatchFd` and `Sockets.CloseSocket`.
3. Deallocate the `StreamRec`.
4. Set `s` to NIL.

**Post**: `s` is NIL. If the stream was using async mode (watcher registered), the fd is closed. If the stream was used in sync mode (no watcher), the fd is left open for the caller to close.

**Returns**: `Invalid` if `s` is already NIL.

```modula2
st := Stream.Destroy(strm);
(* strm is now NIL *)
```

## Error Semantics

### Sync Error Flow

```
TryRead / TryWrite
  │
  ├── OK            → data transferred, continue
  ├── WouldBlock    → watcher mask adjusted, wait and retry
  ├── StreamClosed  → peer shut down, handle EOF
  ├── SysError      → state → Error, fatal
  └── TLSError      → state → Error, fatal
```

For sync operations, errors are immediate. The caller inspects the return status and acts accordingly. Fatal errors (`SysError`, `TLSError`) transition the stream to `Error` state, after which no further I/O is possible.

### Async Error Flow

```
ReadAsync / WriteAsync / WriteAllAsync / CloseAsync
  │
  ├── Returns OK    → Future will settle later
  │     │
  │     ├── Resolves → operation succeeded
  │     └── Rejects  → operation failed (Error.code)
  │
  ├── Returns Invalid     → programming error (no Future created)
  └── Returns OutOfMemory → pool exhausted (no Future created)
```

For async operations, the initial return status indicates whether the operation was successfully started. If `OK`, the caller must wait for the Future to settle. Errors during the asynchronous operation are delivered through Future rejection.

### Future Reject Codes

| Code | Meaning                   | When                                                |
|------|---------------------------|-----------------------------------------------------|
| 1    | I/O error                 | SysError or TLSError during async read/write/close. |
| 2    | Peer closed               | recv returned 0 or TLS close_notify during ReadAsync.|

## Buffering Model

Stream performs no internal buffering. Every `TryRead` and `TryWrite` call maps directly to a single underlying `m2_recv`/`m2_send` (TCP) or `TLS.Read`/`TLS.Write` (TLS) call. The caller is responsible for providing and managing buffers.

For `WriteAllAsync`, the module tracks the send offset (`opSent`) internally and advances through the caller's buffer until all bytes are written. The buffer is not copied -- the caller must keep it valid until the Future settles.

For `ReadAsync` and `WriteAsync`, the operation completes after a single successful underlying I/O call (partial transfer). The caller must issue additional operations to transfer more data.

## Server-Side Usage

Stream requires no special API for server-side connections. The `CreateTCP` and `CreateTLS` constructors accept any connected socket, including sockets returned by `Sockets.Accept`.

### TCP Server Pattern

```modula2
(* Accept a client connection *)
sst := Sockets.Accept(listenSock, clientSock, peer);
sst := Sockets.SetNonBlocking(clientSock, TRUE);

(* Wrap in a Stream *)
st := Stream.CreateTCP(loop, sched, INTEGER(clientSock), strm);
(* Now use TryRead/TryWrite or ReadAsync/WriteAsync as normal *)
```

### TLS Server Pattern

```modula2
(* Accept a client connection *)
sst := Sockets.Accept(listenSock, clientSock, peer);
sst := Sockets.SetNonBlocking(clientSock, TRUE);

(* Create server TLS session and handshake *)
tst := TLS.SessionCreateServer(loop, sched, tlsCtx,
                                INTEGER(clientSock), tlsSess);
tst := TLS.SetSNI(tlsSess, host);  (* not needed for servers *)
tst := TLS.HandshakeAsync(tlsSess, hsFut);
(* ... wait for handshake Future to resolve ... *)

(* Wrap in a TLS Stream *)
st := Stream.CreateTLS(loop, sched, INTEGER(clientSock),
                        tlsCtx, tlsSess, strm);
(* Now use TryRead/TryWrite or ReadAsync/WriteAsync as normal *)
```

The key insight: **Stream is transport-agnostic and direction-agnostic**. Whether the socket was created via `Connect` (client) or `Accept` (server), and whether TLS uses connect state or accept state, Stream handles I/O identically.

## Limitations

- **Single pending operation**: Only one async operation per stream at a time. Starting a second returns `Invalid`.
- **No internal buffering**: All buffering responsibility is on the caller.
- **No connect**: Stream requires an already-connected socket. Use Sockets.Connect or m2http for connection setup.
- **No handshake**: Stream requires a completed TLS handshake. Use TLS.Handshake or TLS.HandshakeAsync before creating a TLS stream.
- **No read after ShutdownWr via async**: The `ShutdownWrite` call is synchronous only; async reads after shutdown require the caller to manage the watcher.
- **No server-specific API**: Stream has no `AcceptTLS` or `ListenTLS`. Server setup (bind, listen, accept, TLS handshake) is done with Sockets and TLS modules directly; Stream wraps the resulting connected socket.
- **Best-effort TLS shutdown**: `ShutdownWrite` ignores WANT_READ/WANT_WRITE from TLS.Shutdown. Use `CloseAsync` for a proper TLS close.

## See Also

- [Stream-Architecture](Stream-Architecture.md) -- Internal design, layering, and watcher model
- [stream_usage_example](stream_usage_example.md) -- TCP echo client, HTTPS GET, and HTTPClient integration examples
- [../m2tls/TLS](../m2tls/TLS.md) -- TLS transport layer
- [../m2sockets/Sockets](../m2sockets/Sockets.md) -- Socket layer
- [../m2http/HTTPClient](../m2http/HTTPClient.md) -- HTTP client (uses Stream sync API)
- [../m2evloop/EventLoop](../m2evloop/EventLoop.md) -- Event loop integration
- [../m2futures/Promise](../m2futures/Promise.md) -- Future/Promise types
- [../m2tls/https_server_example](../m2tls/https_server_example.md) -- TLS server example
