# Stream Architecture

Internal design of the m2stream library: layering, data structures, dual API rationale, watcher ownership, and resource budget.

## Layer Diagram

```
┌──────────────────────────────────────────────────────────┐
│  Application Code                                        │
│  (echo_client.mod, https_get.mod, echo_server.mod, custom clients/servers, etc.) │
├──────────────────────────────────────────────────────────┤
│  HTTPClient                   (m2http)                   │
│  Drives Stream sync API from its own watcher callback    │
├──────────────────────────────────────────────────────────┤
│  Stream                       (m2stream)                 │
│  Unified handle: TCP or TLS, sync or async               │
├─────────────────────┬────────────────────────────────────┤
│  Sockets            │  TLS                               │
│  (m2sockets)        │  (m2tls)                           │
│  m2_recv / m2_send  │  TLS.Read / TLS.Write              │
├─────────────────────┴────────────────────────────────────┤
│  EventLoop / Poller / Timers           (m2evloop)        │
│  fd readiness polling, watcher registration              │
├──────────────────────────────────────────────────────────┤
│  Promise / Future / Scheduler          (m2futures)       │
│  Composable async values, microtask queue                │
├──────────────────────────────────────────────────────────┤
│  C Bridges (FFI)                                         │
│  sockets_bridge.c  tls_bridge.c  poller_bridge.c         │
├──────────────────────────────────────────────────────────┤
│  OS Kernel + OpenSSL/LibreSSL                            │
│  BSD sockets, kqueue/epoll/poll, libssl                  │
└──────────────────────────────────────────────────────────┘
```

Stream occupies a thin layer between consumers (HTTPClient, application code) and the transport modules (Sockets, TLS). It adds no buffering and no protocol logic -- it is purely a handle unification and watcher management layer.

## Library Dependencies

```
m2stream
├── m2tls       (TLS.Read, TLS.Write, TLS.Shutdown, TLS.SessionDestroy, TLS.ContextDestroy)
├── m2sockets   (m2_recv, m2_send, Sockets.CloseSocket, Sockets.Shutdown)
├── m2evloop    (EventLoop.WatchFd, ModifyFd, UnwatchFd)
└── m2futures   (PromiseCreate, Resolve, Reject)
```

## Server Path

Stream has no server-specific code. The server path uses the same `CreateTCP` and `CreateTLS` constructors as the client path:

```
                    Server                          Client
                      │                               │
          Sockets.Accept(fd)              Sockets.Connect(fd)
                      │                               │
             SetNonBlocking(fd)              SetNonBlocking(fd)
                      │                               │
         ┌────────────┴────────────┐     ┌────────────┴────────────┐
         │    TLS (optional)       │     │    TLS (optional)       │
         │ SessionCreateServer     │     │ SessionCreate           │
         │ Handshake               │     │ SetSNI + Handshake      │
         └────────────┬────────────┘     └────────────┬────────────┘
                      │                               │
         CreateTCP or CreateTLS          CreateTCP or CreateTLS
                      │                               │
                      └───────────┬───────────────────┘
                                  │
                      ┌───────────▼───────────┐
                      │       Stream          │
                      │  TryRead / TryWrite   │
                      │  ReadAsync / WriteAsync│
                      └───────────────────────┘
```

The `StreamRec` fields are identical for client and server streams. The `kind` field only distinguishes TCP from TLS, not client from server.

## Internal Data Structure: StreamRec

Each Stream wraps a heap-allocated `StreamRec`:

```modula2
TYPE StreamRec = RECORD
  kind     : StreamKind;      (* TCP or TLSStream *)
  state    : StreamState;     (* Open / ShutdownWr / Closed / Error *)
  fd       : INTEGER;         (* underlying socket file descriptor *)
  loop     : EventLoop.Loop;  (* event loop for watcher management *)
  sched    : Scheduler;       (* scheduler for Promise operations *)
  watching : BOOLEAN;         (* TRUE if a watcher is registered on fd *)
  (* TLS handles — NIL for TCP streams *)
  tlsCtx   : TLS.TLSContext;
  tlsSess  : TLS.TLSSession;
  (* Pending async operation *)
  op       : INTEGER;         (* OpNone / OpRead / OpWrite / OpWriteAll / OpClose *)
  promise  : Promise;         (* promise for the current async operation *)
  opBuf    : ADDRESS;         (* buffer for async read/write *)
  opLen    : INTEGER;         (* requested byte count *)
  opSent   : INTEGER;         (* bytes sent so far, used by WriteAllAsync *)
END;
```

The `op` field tracks which async operation is active. Only one async operation can be pending per stream. The `watching` field tracks whether Stream has registered its own EventLoop watcher on the fd.

### Operation Constants

| Constant     | Value | Description                         |
|--------------|-------|-------------------------------------|
| `OpNone`     | 0     | No async operation pending.         |
| `OpRead`     | 1     | ReadAsync in progress.              |
| `OpWrite`    | 2     | WriteAsync in progress.             |
| `OpWriteAll` | 3     | WriteAllAsync in progress.          |
| `OpClose`    | 4     | CloseAsync in progress.             |

## Dual API Design

Stream provides two APIs to accommodate different watcher ownership models.

### Sync API (for HTTPClient)

HTTPClient already owns the fd watcher for its connection state machine. It calls Stream sync operations (`TryRead`, `TryWrite`) from within its `OnSocketEvent` callback:

```
OnSocketEvent(fd, events, user)
  │
  ├── StSending: Stream.TryWrite(strm, buf, len, sent)
  │   ├── OK → advance send offset
  │   ├── WouldBlock → watcher mask already adjusted, stay in StSending
  │   └── SysError/TLSError → fail connection
  │
  └── StRecvBody: Stream.TryRead(strm, buf, max, got)
      ├── OK → process data
      ├── StreamClosed → done
      ├── WouldBlock → watcher mask already adjusted, stay in state
      └── SysError/TLSError → fail connection
```

The key insight: HTTPClient manages the watcher. Stream only does the I/O and, for TLS streams, adjusts the watcher mask when OpenSSL needs a different I/O direction. Stream does NOT register or unregister the watcher in sync mode.

### Async API (for standalone use)

For direct Stream usage without HTTPClient, the async API registers its own EventLoop watcher:

```
ReadAsync(strm, buf, max, future)
  │
  ├── PromiseCreate → promise + future
  ├── Set op = OpRead, store buf/max
  └── EnsureWatcher(sp, EvRead)
      │
      └── OnStreamEvent(fd, events, user)
          │
          ├── TryRead → OK → ResolveOp(sp, bytesRead)
          ├── TryRead → WouldBlock → (watcher already adjusted, wait)
          ├── TryRead → StreamClosed → RejectOp(sp, 2)
          └── TryRead → error → RejectOp(sp, 1)
```

### Why Two APIs?

A single fd can only have one watcher in the EventLoop (the watcher pool is keyed by fd). If both HTTPClient and ReadAsync tried to register watchers on the same fd, one would fail or overwrite the other. The sync API avoids this conflict by never touching the watcher.

| Aspect              | Sync API                     | Async API                    |
|---------------------|------------------------------|------------------------------|
| Watcher owner       | Caller (e.g. HTTPClient)     | Stream                       |
| Return mechanism    | Status code + VAR out param  | Status code + Future         |
| Retry handling      | Caller loops                 | Internal (OnStreamEvent)     |
| Use case            | Embedded in state machine    | Standalone I/O               |
| Buffer management   | Caller provides              | Caller provides              |

## Watcher Ownership Model

### Sync Mode: Caller Manages

When using `TryRead` and `TryWrite`, Stream never calls `WatchFd` or `UnwatchFd`. However, for TLS streams, it does call `ModifyFd` to adjust the watcher mask when OpenSSL returns WANT_READ or WANT_WRITE. This is safe because the caller's watcher is already registered.

```
Caller: WatchFd(fd, EvRead, OnMyEvent, ctx)
  │
  └── OnMyEvent:
        Stream.TryRead(strm, ...)
          ├── OK → process
          └── WouldBlock → Stream called ModifyFd(fd, EvWrite)
                           Caller's next event fires for write-ready
```

### Async Mode: Stream Manages

When using `ReadAsync`, `WriteAsync`, `WriteAllAsync`, or `CloseAsync`, Stream registers its own watcher via `EnsureWatcher`:

```
EnsureWatcher(sp, events):
  IF NOT sp^.watching THEN
    WatchFd(sp^.loop, sp^.fd, events, OnStreamEvent, sp)
    sp^.watching := TRUE
  ELSE
    ModifyFd(sp^.loop, sp^.fd, events)
  END
```

The watcher persists across the lifetime of the async operation. It is not removed between retries -- only the mask is changed via `ModifyFd`. The watcher is removed by:

- `CloseAsync` completion: `UnwatchFd` before closing the socket.
- `Destroy`: `UnwatchFd` if `watching` is TRUE.

### Cleanup: Destroy Behavior

`Destroy` adapts to the ownership model:

| Mode  | Watcher | Socket  | TLS handles |
|-------|---------|---------|-------------|
| Sync  | Not registered by Stream; not touched | Left open (caller closes) | Shutdown + Destroy |
| Async | Unwatched by Stream | Closed by Stream | Shutdown + Destroy |

The discriminator is the `watching` field: if TRUE, Stream owns the watcher and the fd.

## Async Callback: OnStreamEvent

All async operations share a single watcher callback, `OnStreamEvent`, which dispatches based on the `op` field:

```
OnStreamEvent(fd, events, user)
  │
  ├── OpRead: TryRead → resolve/reject/wait
  ├── OpWrite: TryWrite → resolve/reject/wait
  ├── OpWriteAll: TryWrite → advance offset → resolve/reject/wait
  ├── OpClose: TLS.Shutdown → close socket → resolve
  └── OpNone: ignore (spurious event)
```

### WriteAllAsync Internal Loop

`WriteAllAsync` uses `opSent` to track progress through the buffer:

```
OnStreamEvent for OpWriteAll:
  1. TryWrite(buf + opSent, opLen - opSent, n)
  2. IF OK: opSent += n
     IF opSent >= opLen: ResolveOp(sp, opSent)  -- done
     ELSE: ModifyFd(fd, EvWrite)                -- more to send
  3. IF WouldBlock: wait (watcher adjusted by TryWrite)
  4. IF error: RejectOp(sp, 1)
```

The `OffsetPtr` helper advances the buffer pointer by `opSent` bytes for each retry.

### CloseAsync Internal Steps

```
OnStreamEvent for OpClose:
  1. IF TLSStream:
       TLS.Shutdown(sess)
       IF WantRead: ModifyFd(fd, EvRead), RETURN
       IF WantWrite: ModifyFd(fd, EvWrite), RETURN
       (* OK or error — proceed *)
  2. UnwatchFd(fd), watching := FALSE
  3. CloseSocket(fd), fd := InvalidSocket
  4. state := Closed
  5. ResolveOp(sp, 0)
```

## Future Resolution

### ReadAsync

```
Resolves: Value { tag: bytesRead (1..max), ptr: NIL }
Rejects:  Error { code: 1 (I/O error) | 2 (peer closed), ptr: NIL }
```

### WriteAsync

```
Resolves: Value { tag: bytesWritten (1..len), ptr: NIL }
Rejects:  Error { code: 1 (I/O error), ptr: NIL }
```

### WriteAllAsync

```
Resolves: Value { tag: len (all bytes written), ptr: NIL }
Rejects:  Error { code: 1 (I/O error), ptr: NIL }
```

### CloseAsync

```
Resolves: Value { tag: 0, ptr: NIL }
```

CloseAsync does not reject -- even if TLS shutdown encounters an error, it proceeds to close the socket and resolves.

## Resource Budget

| Resource            | Per Stream   | Source              |
|---------------------|-------------|---------------------|
| StreamRec           | ~80 bytes   | ALLOCATE            |
| EventLoop watcher   | 1 of 64     | WatchFd (async only)|
| Promise slot        | 1 of 256    | PromiseCreate       |
| Socket fd           | 1           | Provided by caller  |
| TLS session         | ~variable   | Provided by caller  |
| TLS context         | ~variable   | Provided by caller  |
| **Total (Stream)**  | **~80 B**   | (excludes TLS/fd)   |

Stream itself is lightweight. The dominant resource costs come from the TLS session (OpenSSL structures) and the socket fd, both of which are provided by the caller at creation time.

## Error Propagation

```
OS error (TCP)
  → m2_recv/m2_send returns -1
    → Stream maps to SysError
      → Sync: caller handles directly
      → Async: RejectOp → Promise.Reject → Future settles with Error { code: 1 }

TLS error
  → TLS.Read/TLS.Write returns SysError
    → Stream maps to TLSError
      → Sync: caller handles directly
      → Async: RejectOp → Promise.Reject → Future settles with Error { code: 1 }

Peer close (TCP)
  → m2_recv returns 0
    → Stream maps to StreamClosed
      → Async: RejectOp → Promise.Reject → Future settles with Error { code: 2 }

Peer close (TLS)
  → TLS.Read returns Closed
    → Stream maps to StreamClosed
      → Async: RejectOp → Promise.Reject → Future settles with Error { code: 2 }
```

Fatal errors (`SysError`, `TLSError`) transition the stream state to `Error`, preventing further I/O operations.

## See Also

- [Stream](Stream.md) -- API reference
- [stream_usage_example](stream_usage_example.md) -- Usage examples
- [../m2tls/TLS-Architecture](../m2tls/TLS-Architecture.md) -- TLS internal design
- [../m2http/Net-Architecture](../m2http/Net-Architecture.md) -- Overall networking stack
- [../m2evloop/Async-Architecture](../m2evloop/Async-Architecture.md) -- Event loop internals
