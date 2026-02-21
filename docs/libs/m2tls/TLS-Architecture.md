# TLS Architecture

Internal design of the m2tls library: layering, state machines, watcher management, and Future resolution.

## Layer Diagram

```
┌──────────────────────────────────────────────────────────┐
│  Application Code                                        │
│  (https_get.mod, custom TLS clients, etc.)               │
├──────────────────────────────────────────────────────────┤
│  HTTPClient                   (m2http)                   │
│  HTTPS: TLS context + session per connection             │
│  Uses sync TLS ops inside its own watcher callback       │
├──────────────────────────────────────────────────────────┤
│  TLS                          (m2tls)                    │
│  M2 module: TLSContext, TLSSession, sync + async ops     │
├──────────────────────────────────────────────────────────┤
│  TlsBridge (FFI)              (m2tls)                    │
│  DEFINITION MODULE FOR "C" → tls_bridge.c                │
├──────────────────────────────────────────────────────────┤
│  tls_bridge.c                                            │
│  OpenSSL/LibreSSL wrapper: SSL_CTX, SSL, handshake,      │
│  read, write, shutdown, verify, diagnostics              │
├──────────────────────────────────────────────────────────┤
│  OpenSSL / LibreSSL                                      │
│  SSL_CTX_new, SSL_do_handshake, SSL_read, SSL_write      │
├──────────────────────────────────────────────────────────┤
│  OS Kernel (TCP sockets)                                 │
└──────────────────────────────────────────────────────────┘
```

## Dual API Design

The TLS module provides two APIs to avoid watcher conflicts:

### Sync API (for HTTPClient)

HTTPClient already owns the fd watcher for its state machine. It calls TLS sync operations (`Handshake`, `Read`, `Write`) from within its `OnSocketEvent` callback:

```
OnSocketEvent(fd, events, user)
  │
  ├── StHandshaking: TLS.Handshake(sess)
  │   ├── OK → transition to StSending
  │   ├── WantRead → ModifyFd(fd, EvRead), stay in StHandshaking
  │   └── WantWrite → ModifyFd(fd, EvWrite), stay in StHandshaking
  │
  ├── StSending: TLS.Write(sess, buf, len, sent)
  │   ├── OK → advance send offset
  │   ├── WantRead → ModifyFd(fd, EvRead), stay in StSending
  │   └── WantWrite → ModifyFd(fd, EvWrite), stay in StSending
  │
  └── StRecvBody: TLS.Read(sess, buf, max, got)
      ├── OK → process data
      ├── Closed → done
      ├── WantRead → ModifyFd(fd, EvRead), stay in state
      └── WantWrite → ModifyFd(fd, EvWrite), stay in state
```

The key insight: HTTPClient manages the watcher; TLS just does the I/O and reports what it needs next.

### Async API (for standalone use)

For direct TLS usage without HTTPClient, the async API registers its own EventLoop watcher:

```
HandshakeAsync(sess, future)
  │
  ├── Try sync first: m2_tls_handshake()
  │   ├── Complete → return settled future (already resolved)
  │   └── Error → return settled future (already rejected)
  │
  └── WANT_READ/WANT_WRITE → StartAsync + WatchDir
      │
      └── OnTLSEvent(fd, events, user)
          └── RetryHandshake(sp)
              ├── OK → ResolveSess(sp, 0)
              ├── WantRead → WatchDir(sp, EvRead)
              ├── WantWrite → WatchDir(sp, EvWrite)
              └── Error → RejectSess(sp, code)
```

### Why Two APIs?

A single fd can only have one watcher in the EventLoop (the watcher pool is keyed by fd). If both HTTPClient and TLS.HandshakeAsync tried to register watchers on the same fd, one would fail or overwrite the other. The sync API avoids this by not touching the watcher at all.

## Internal State: SessRec

Each TLSSession wraps a heap-allocated `SessRec`:

```modula2
TYPE SessRec = RECORD
  ssl:      ADDRESS;     (* SSL* from C bridge *)
  lp:       Loop;        (* EventLoop for watcher registration *)
  sched:    Scheduler;   (* Scheduler for Promise operations *)
  fd:       INTEGER;     (* underlying TCP socket *)
  op:       INTEGER;     (* OpNone..OpWriteAll *)
  promise:  Promise;     (* pending async operation *)
  rdBuf:    ADDRESS;     (* read target buffer *)
  rdMax:    INTEGER;     (* read max bytes *)
  wrBuf:    ADDRESS;     (* write source buffer *)
  wrLen:    INTEGER;     (* write total length *)
  wrSent:   INTEGER;     (* write bytes sent so far *)
  watching: BOOLEAN;     (* watcher currently registered *)
END;
```

The `op` field tracks which async operation is active. Only one async operation can be pending per session.

## Handshake State Machine

```
                  ┌─────────────────┐
                  │ SessionCreate    │
                  └────────┬────────┘
                           │
                  ┌────────▼────────┐
                  │ SetSNI           │ (optional but recommended)
                  └────────┬────────┘
                           │
            ┌──────────────▼──────────────┐
            │        Handshake()           │
            └──────┬──────┬──────┬────────┘
                   │      │      │
              OK   │  WANT│  WANT│  Error
                   │  READ│  WRITE│
                   │      │      │
            ┌──────▼┐ ┌──▼──┐ ┌─▼──────┐
            │Complete│ │Wait │ │Wait    │
            │       │ │read │ │write   │
            └───────┘ └──┬──┘ └──┬─────┘
                         │       │
                         └───┬───┘
                             │ retry
                  ┌──────────▼──────────────┐
                  │        Handshake()       │
                  └─────────────────────────┘
                  (repeat until OK or Error)
```

## Watcher Mask Changes

During async operations, the watcher mask changes dynamically based on what OpenSSL needs:

```
HandshakeAsync:
  1. try_handshake → WANT_WRITE → WatchFd(fd, EvWrite)
  2. OnTLSEvent → retry → WANT_READ → ModifyFd(fd, EvRead)
  3. OnTLSEvent → retry → OK → UnwatchFd(fd) + Resolve

ReadAsync (during renegotiation):
  1. try_read → WANT_WRITE → WatchFd(fd, EvWrite)
  2. OnTLSEvent → retry → OK → UnwatchFd(fd) + Resolve

WriteAllAsync:
  1. try_write → partial (500 of 1000) → WatchFd(fd, EvWrite)
  2. OnTLSEvent → retry → partial (300 more) → keep watching
  3. OnTLSEvent → retry → complete → UnwatchFd(fd) + Resolve
```

The `WatchDir` helper manages the transition:
- If not watching: calls `WatchFd` with the `OnTLSEvent` callback
- If already watching: calls `ModifyFd` to change the mask

## Future Resolution

### Handshake

```
Resolves: Value { tag: 0, ptr: NIL }
Rejects:  Error { code: 1 (SysError) | 2 (VerifyFailed), ptr: NIL }
```

### Read

```
Resolves: Value { tag: bytesRead, ptr: NIL }
Rejects:  Error { code: 1 (SysError) | 3 (Closed), ptr: NIL }
```

### Write / WriteAll

```
Resolves: Value { tag: bytesWritten, ptr: NIL }
Rejects:  Error { code: 1 (SysError), ptr: NIL }
```

### Immediate Settlement

Async operations try the sync path first. If the operation completes immediately (common for reads when data is buffered in OpenSSL), the Future is returned already settled. This avoids unnecessary watcher registration.

## Error Propagation

```
OpenSSL error
  → tls_bridge.c returns error code
    → TLS.mod maps to Status enum
      → Sync: caller handles directly
      → Async: RejectSess → Promise.Reject → Future settles with Error

OpenSSL verify failure
  → SSL_get_verify_result() ≠ X509_V_OK
    → tls_bridge.c returns -2
      → TLS.mod: VerifyFailed status
        → Async: RejectSess(sp, ErrVerify)
```

`GetLastError` provides the OpenSSL error string for diagnostics:

```modula2
IF tst # TLS.OK THEN
  GetLastError(errBuf);
  WriteString("TLS error: "); WriteString(errBuf); WriteLn
END;
```

## C Bridge Return Codes

### m2_tls_handshake

| Return | Meaning        |
|--------|----------------|
| 0      | Complete       |
| 1      | WANT_READ      |
| 2      | WANT_WRITE     |
| -1     | SysError       |
| -2     | VerifyFailed   |

### m2_tls_read

| Return | Meaning        |
|--------|----------------|
| > 0    | Bytes read     |
| 0      | Closed         |
| -1     | WANT_READ      |
| -2     | WANT_WRITE     |
| -3     | SysError       |

### m2_tls_write

| Return | Meaning        |
|--------|----------------|
| > 0    | Bytes written  |
| -1     | WANT_READ      |
| -2     | WANT_WRITE     |
| -3     | SysError       |

## Session Cleanup

`SessionDestroy` performs cleanup in order:

1. If an async operation is pending (`op ≠ OpNone`): reject its Future with `ErrSys`
2. Unwatch the fd (if watching)
3. Destroy the OpenSSL SSL object
4. Deallocate the SessRec
5. Set the handle to NIL

Note: `SessionDestroy` does NOT close the underlying fd. The caller (typically HTTPClient or the application) is responsible for closing the socket.

## OpenSSL Compatibility

The C bridge supports:

| Feature                  | OpenSSL Version | Notes                           |
|--------------------------|-----------------|---------------------------------|
| TLS_client_method()      | 1.1.0+          | Replaces SSLv23_client_method() |
| Auto-initialization      | 1.1.0+          | Compat shim for older versions  |
| SSL_get1_peer_certificate| 3.0+            | Macro alias for older versions  |
| TLS 1.3                  | 1.1.1+          | Conditional on TLS1_3_VERSION   |
| SSL_CTX_set_min_proto_version | 1.1.0+     | Used for version enforcement    |

## See Also

- [TLS](TLS.md) -- API reference
- [https_get_example](https_get_example.md) -- HTTPS GET example walkthrough
- [../m2http/Net-Architecture](../m2http/Net-Architecture.md) -- Overall networking stack
- [../m2evloop/Async-Architecture](../m2evloop/Async-Architecture.md) -- Event loop internals
