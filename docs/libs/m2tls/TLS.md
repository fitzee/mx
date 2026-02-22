# TLS

TLS transport layer for Modula-2+ async networking. Wraps OpenSSL/LibreSSL behind a stable Modula-2+ interface, designed for integration with EventLoop, Scheduler, and Futures.

## Overview

The TLS module provides encrypted transport for TCP connections. It sits between the application (or HTTPClient) and the raw socket layer, encrypting all data with TLS 1.2 or 1.3.

Two usage modes:

1. **Sync (try-once)**: `Handshake`, `Read`, `Write` return immediately with a status indicating completion or retry direction. The caller drives retries from its own EventLoop watcher. Used by HTTPClient for HTTPS support.

2. **Async (Future-returning)**: `HandshakeAsync`, `ReadAsync`, `WriteAsync`, `WriteAllAsync` register their own EventLoop watcher and return a Future. For standalone TLS use without HTTPClient.

## Design Goals

- **Secure by default**: Peer certificate verification ON. TLS 1.2 minimum. System root store loaded automatically by HTTPClient.
- **Event-loop integrated**: Non-blocking operations with WANT_READ/WANT_WRITE retry semantics.
- **No hidden globals**: All state lives in TLSContext and TLSSession handles.
- **No threads**: Single-threaded model. All callbacks run on the event loop thread.
- **Deterministic cleanup**: `SessionDestroy` cancels pending async operations and frees resources.

## Threat Model

The TLS module protects against:

- **Eavesdropping**: All data encrypted with the negotiated TLS cipher suite.
- **Tampering**: TLS record MAC detects modification.
- **Server impersonation**: Peer certificate verification (default: ON) validates the server's identity against trusted CA roots.
- **Protocol downgrade**: Minimum version enforcement (default: TLS 1.2) prevents fallback to weak protocols.

The TLS module does NOT protect against:

- **Compromised CA roots**: If a trusted CA is compromised, verification succeeds for fraudulent certificates.
- **Application-level attacks**: XSS, injection, etc. are above the TLS layer.
- **Local key compromise**: If the server's private key is stolen, past traffic may be decryptable (unless using forward-secrecy cipher suites, which OpenSSL negotiates by default).

## Error Model

### WANT_READ / WANT_WRITE

Non-blocking TLS operations may need to perform I/O that the socket isn't ready for. The status values `WantRead` and `WantWrite` indicate which direction to wait for:

| Return       | Meaning                                      | Action                    |
|--------------|----------------------------------------------|---------------------------|
| `WantRead`   | TLS engine needs to read from the socket.    | Wait for fd readable.     |
| `WantWrite`  | TLS engine needs to write to the socket.     | Wait for fd writable.     |

This can happen during any operation. For example, `Write` may return `WantRead` during TLS renegotiation. The caller must adjust its watcher mask accordingly.

For sync operations, the caller manages the retry loop. For async operations, the TLS module handles this internally.

### Status Enum

| Value          | Meaning                                          |
|----------------|--------------------------------------------------|
| `OK`           | Operation succeeded.                             |
| `WantRead`     | Retry after fd becomes readable.                 |
| `WantWrite`    | Retry after fd becomes writable.                 |
| `Closed`       | Peer performed clean TLS shutdown.               |
| `SysError`     | TLS engine error. Use `GetLastError` for details.|
| `Invalid`      | Bad argument (NIL handle, op already pending).   |
| `OutOfMemory`  | Heap allocation or Promise pool exhausted.       |
| `VerifyFailed` | Peer certificate failed verification.            |

### Future Error Codes (async operations)

| Code | Meaning                   |
|------|---------------------------|
| 1    | SysError (TLS engine)     |
| 2    | VerifyFailed              |
| 3    | Closed (peer shutdown)    |

## Certificate Verification

### Default Behavior

Verification is **ON by default**. When `ContextCreate` returns, the context is configured with `VerifyPeer` mode and TLS 1.2 minimum. The caller must still load a trust store:

```modula2
tst := ContextCreate(ctx);
tst := LoadSystemRoots(ctx);   (* REQUIRED for verification to work *)
```

Without `LoadSystemRoots` (or `LoadCAFile`), handshake will fail with `VerifyFailed` because no CA certificates are available to validate the server.

### Disabling Verification (UNSAFE)

```modula2
tst := SetVerifyMode(ctx, NoVerify);
```

**WARNING**: `NoVerify` disables ALL certificate verification. The connection is still encrypted but provides no authentication -- any server can impersonate any other. Use ONLY for local development or debugging. NEVER use in production.

### Verification Diagnostics

After a handshake, use `GetVerifyResult` to check the X509 verification status:

```modula2
vr := GetVerifyResult(sess);
(* 0 = X509_V_OK, other values are OpenSSL X509_V_ERR_* codes *)
```

Use `GetPeerSummary` to inspect the server certificate:

```modula2
tst := GetPeerSummary(sess, summary);
(* summary = "/CN=example.com/O=Example Inc/..." *)
```

## SNI (Server Name Indication)

SNI tells the server which hostname the client is connecting to. This is essential for servers hosting multiple TLS domains on one IP address.

SNI is per-session, not per-context. Different hosts require different sessions:

```modula2
tst := SetSNI(sess, host);   (* Must be called BEFORE Handshake *)
```

HTTPClient sets SNI automatically from the URI host.

## Root Store Loading

### macOS

With Homebrew OpenSSL, `LoadSystemRoots` uses the bundled CA certificates at the Homebrew prefix (e.g., `/opt/homebrew/etc/openssl@3/cert.pem`). The system Keychain is NOT used directly -- OpenSSL has its own root store.

### Linux

`LoadSystemRoots` calls `SSL_CTX_set_default_verify_paths()`, which searches:
- `/etc/ssl/certs/` (Debian/Ubuntu)
- The `SSL_CERT_DIR` / `SSL_CERT_FILE` environment variables
- Distribution-specific paths (RHEL, Alpine, etc.)

### Custom CA Bundle

Use `LoadCAFile` for custom or self-signed CA certificates:

```modula2
tst := LoadCAFile(ctx, "/path/to/ca-bundle.pem");
```

## Types

**`TLSContext`** -- Opaque handle wrapping an OpenSSL `SSL_CTX*`:

```modula2
TYPE TLSContext = ADDRESS;
```

**`TLSSession`** -- Opaque handle wrapping an internal `SessRec`:

```modula2
TYPE TLSSession = ADDRESS;
```

**`VerifyMode`** -- Certificate verification mode:

| Value        | Meaning                                    |
|--------------|--------------------------------------------|
| `VerifyPeer` | Validate server certificate (default).     |
| `NoVerify`   | Skip ALL verification. UNSAFE.             |

**`TLSVersion`** -- Minimum TLS protocol version:

| Value   | Protocol | Notes                    |
|---------|----------|--------------------------|
| `TLS10` | TLS 1.0  | Deprecated. Avoid.       |
| `TLS11` | TLS 1.1  | Deprecated. Avoid.       |
| `TLS12` | TLS 1.2  | Default minimum.         |
| `TLS13` | TLS 1.3  | Recommended where available. |

## Procedures

### ContextCreate

```modula2
PROCEDURE ContextCreate(VAR out: TLSContext): Status;
```

Create a TLS client context. Defaults: `VerifyPeer`, TLS 1.2 minimum. Returns `OutOfMemory` if OpenSSL allocation fails.

### ContextDestroy

```modula2
PROCEDURE ContextDestroy(VAR ctx: TLSContext): Status;
```

Destroy a context. Sets `ctx` to `NIL`.

### SetVerifyMode

```modula2
PROCEDURE SetVerifyMode(ctx: TLSContext; mode: VerifyMode): Status;
```

Set peer certificate verification mode. Default is `VerifyPeer`. See "Disabling Verification" above.

### SetMinVersion

```modula2
PROCEDURE SetMinVersion(ctx: TLSContext; v: TLSVersion): Status;
```

Set minimum acceptable TLS version. Default is `TLS12`.

### LoadSystemRoots

```modula2
PROCEDURE LoadSystemRoots(ctx: TLSContext): Status;
```

Load the system default CA root store. Returns `SysError` if the root store cannot be found or loaded.

### LoadCAFile

```modula2
PROCEDURE LoadCAFile(ctx: TLSContext; VAR path: ARRAY OF CHAR): Status;
```

Load a CA bundle from a PEM file.

### SetClientCert

```modula2
PROCEDURE SetClientCert(ctx: TLSContext;
                        VAR certPath, keyPath: ARRAY OF CHAR): Status;
```

Load client certificate and private key from PEM files. Returns `SysError` on cert or key error. API present; not commonly needed for v1 use cases.

### ContextCreateServer

```modula2
PROCEDURE ContextCreateServer(VAR out: TLSContext): Status;
```

Create a TLS server context. Defaults: `NoVerify` (servers don't verify clients by default), TLS 1.2 minimum. Returns `OutOfMemory` if OpenSSL allocation fails.

### SetServerCert

```modula2
PROCEDURE SetServerCert(ctx: TLSContext;
                        VAR certPath, keyPath: ARRAY OF CHAR): Status;
```

Load server certificate and private key from PEM files. Both are **required** for server operation. Returns `SysError` on cert or key error.

### SetALPN

```modula2
PROCEDURE SetALPN(ctx: TLSContext;
                  protos: ADDRESS; protosLen: INTEGER): Status;
```

Set client-side ALPN protocol list. `protos` is wire format (length-prefixed strings, e.g. `\002h2`). `protosLen` must be <= `MaxALPNLen` (64).

### SetALPNServer

```modula2
PROCEDURE SetALPNServer(ctx: TLSContext;
                        protos: ADDRESS; protosLen: INTEGER): Status;
```

Set server-side ALPN preferred protocol list. `protos` is wire format. Installs a select callback that matches client-offered protocols against the server's list. `protosLen` must be <= `MaxALPNLen` (64). One server context per process.

### SessionCreate

```modula2
PROCEDURE SessionCreate(lp: Loop; sched: Scheduler;
                        ctx: TLSContext; fd: INTEGER;
                        VAR out: TLSSession): Status;
```

Create a TLS session over a connected, non-blocking socket. The `fd` must already have completed its TCP handshake. `lp` and `sched` are stored for async operations.

### SessionCreateServer

```modula2
PROCEDURE SessionCreateServer(lp: Loop; sched: Scheduler;
                              ctx: TLSContext; fd: INTEGER;
                              VAR out: TLSSession): Status;
```

Create a TLS server session over an accepted, non-blocking socket. The `fd` must come from `Sockets.Accept` with a completed TCP handshake. `ctx` must be a configured server context with cert loaded. `lp` and `sched` are stored for async operations.

### SessionDestroy

```modula2
PROCEDURE SessionDestroy(VAR s: TLSSession): Status;
```

Destroy a session. Cancels any pending async operation (rejecting its Future with `ErrSys`). Does NOT close the underlying fd. Sets `s` to `NIL`.

### SetSNI

```modula2
PROCEDURE SetSNI(s: TLSSession; VAR host: ARRAY OF CHAR): Status;
```

Set SNI hostname. Must be called before `Handshake`.

### Handshake

```modula2
PROCEDURE Handshake(s: TLSSession): Status;
```

Attempt one step of the TLS handshake. Returns `OK` when complete, `WantRead`/`WantWrite` to retry, `VerifyFailed` on cert error, `SysError` on TLS error.

### Read

```modula2
PROCEDURE Read(s: TLSSession; buf: ADDRESS; max: INTEGER;
               VAR got: INTEGER): Status;
```

Attempt to read up to `max` bytes into `buf`. On `OK`, `got` contains bytes read (1..max).

### Write

```modula2
PROCEDURE Write(s: TLSSession; buf: ADDRESS; len: INTEGER;
                VAR sent: INTEGER): Status;
```

Attempt to write up to `len` bytes from `buf`. On `OK`, `sent` contains bytes written (1..len).

### Shutdown

```modula2
PROCEDURE Shutdown(s: TLSSession): Status;
```

Initiate TLS shutdown (send `close_notify`). For non-blocking cleanup, calling once and ignoring the result is acceptable.

### HandshakeAsync

```modula2
PROCEDURE HandshakeAsync(s: TLSSession; VAR out: Future): Status;
```

Complete the TLS handshake asynchronously. Registers an EventLoop watcher on the session fd. Future resolves with `Value.tag=0`, rejects with `Error.code=1` (SysError) or `2` (VerifyFailed).

### ReadAsync

```modula2
PROCEDURE ReadAsync(s: TLSSession; buf: ADDRESS; max: INTEGER;
                    VAR out: Future): Status;
```

Read up to `max` bytes asynchronously. `buf` must remain valid until the Future settles. Resolves with `Value.tag = bytes read`, rejects with `Error.code=1` (SysError) or `3` (Closed).

### WriteAsync

```modula2
PROCEDURE WriteAsync(s: TLSSession; buf: ADDRESS; len: INTEGER;
                     VAR out: Future): Status;
```

Write up to `len` bytes asynchronously. Resolves with `Value.tag = bytes written`.

### WriteAllAsync

```modula2
PROCEDURE WriteAllAsync(s: TLSSession; buf: ADDRESS; len: INTEGER;
                        VAR out: Future): Status;
```

Write all `len` bytes asynchronously (loops until complete). Resolves with `Value.tag = total bytes written`.

### GetPeerSummary

```modula2
PROCEDURE GetPeerSummary(s: TLSSession; VAR out: ARRAY OF CHAR): Status;
```

Copy the peer certificate subject into `out` (one-line format). Must be called after a successful handshake.

### GetALPN

```modula2
PROCEDURE GetALPN(s: TLSSession;
                  VAR out: ARRAY OF CHAR;
                  VAR got: INTEGER): Status;
```

Query the negotiated ALPN protocol after handshake. Copies the protocol string (e.g. "h2") into `out`. `got` receives the string length, or 0 if no ALPN was negotiated.

### GetVerifyResult

```modula2
PROCEDURE GetVerifyResult(s: TLSSession): INTEGER;
```

Return the X509 verification result code. `0` = `X509_V_OK` (success).

### GetLastError

```modula2
PROCEDURE GetLastError(VAR out: ARRAY OF CHAR);
```

Copy the last TLS engine error string into `out`.

## Limitations

- **No session resumption**: Each connection performs a full handshake.
- **No client certificates**: `SetClientCert` API present but not tested.
- **No custom crypto**: Uses whatever cipher suites OpenSSL negotiates.
- **No full PEM parser**: CA loading delegates to OpenSSL's PEM reader.
- **No OCSP stapling**: Certificate revocation checking not implemented.
- **Single pending operation**: Only one async operation per session at a time.
- **OpenSSL required**: No pure-M2 TLS implementation.

## See Also

- [TLS-Architecture](TLS-Architecture.md) -- Internal design, state machines, and layering
- [https_get_example](https_get_example.md) -- HTTPS GET example walkthrough
- [https_server_example](https_server_example.md) -- HTTPS server example walkthrough
- [../m2http/HTTPClient](../m2http/HTTPClient.md) -- HTTP client with HTTPS support
- [../m2evloop/EventLoop](../m2evloop/EventLoop.md) -- Event loop integration
- [../m2futures/Promise](../m2futures/Promise.md) -- Future/Promise types
