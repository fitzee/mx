# TLS

## Why
Provides TLS transport for Modula-2+ networking, wrapping OpenSSL/LibreSSL behind a stable interface with both synchronous (try-once) and asynchronous (Future-returning) operation modes.

## Types

- **TLSContext** -- Opaque handle wrapping an `SSL_CTX*`. One per trust configuration.
- **TLSSession** -- Opaque handle for a single TLS connection over a socket fd.
- **Status** -- Result code: `OK`, `WantRead`, `WantWrite`, `Closed`, `SysError`, `Invalid`, `OutOfMemory`, `VerifyFailed`.
- **VerifyMode** -- `VerifyPeer` (default) or `NoVerify` (debugging only).
- **TLSVersion** -- `TLS10`, `TLS11`, `TLS12`, `TLS13`. Default minimum is TLS12.

## Constants

- `MaxSummaryLen = 512` -- Maximum length for peer certificate summary strings.
- `MaxALPNLen = 64` -- Maximum length for ALPN wire-format protocol lists.

## Procedures

### Context Lifecycle

- `PROCEDURE ContextCreate(VAR out: TLSContext): Status`
  Create a TLS client context. Defaults to VerifyPeer, TLS 1.2 minimum.

- `PROCEDURE ContextCreateServer(VAR out: TLSContext): Status`
  Create a TLS server context. Defaults to NoVerify (clients not verified), TLS 1.2 minimum.

- `PROCEDURE ContextDestroy(VAR ctx: TLSContext): Status`
  Destroy a context and set the handle to NIL.

### Context Configuration

- `PROCEDURE SetVerifyMode(ctx: TLSContext; mode: VerifyMode): Status`
  Set peer certificate verification mode. Never use NoVerify in production.

- `PROCEDURE SetMinVersion(ctx: TLSContext; v: TLSVersion): Status`
  Set the minimum acceptable TLS protocol version.

- `PROCEDURE LoadSystemRoots(ctx: TLSContext): Status`
  Load the system default CA root store.

- `PROCEDURE LoadCAFile(ctx: TLSContext; VAR path: ARRAY OF CHAR): Status`
  Load a CA bundle from a PEM file.

- `PROCEDURE SetClientCert(ctx: TLSContext; VAR certPath, keyPath: ARRAY OF CHAR): Status`
  Load a client certificate and private key from PEM files.

- `PROCEDURE SetServerCert(ctx: TLSContext; VAR certPath, keyPath: ARRAY OF CHAR): Status`
  Load a server certificate and private key from PEM files.

### Mutual TLS

- `PROCEDURE SetClientCA(ctx: TLSContext; VAR caPath: ARRAY OF CHAR): Status`
  Load a CA file for verifying client certificates (typically on a server context).

- `PROCEDURE RequireClientCert(ctx: TLSContext; require: BOOLEAN): Status`
  Require or stop requiring that clients present a certificate. Must be called before creating sessions.

- `PROCEDURE GetPeerCertDN(s: TLSSession; VAR buf: ARRAY OF CHAR): BOOLEAN`
  Copy the peer certificate's distinguished name into buf. Returns TRUE if a peer certificate is present.

### ALPN

- `PROCEDURE SetALPN(ctx: TLSContext; protos: ADDRESS; protosLen: INTEGER): Status`
  Set client-side ALPN protocol list in wire format (length-prefixed strings).

- `PROCEDURE SetALPNServer(ctx: TLSContext; protos: ADDRESS; protosLen: INTEGER): Status`
  Set server-side ALPN preferred protocol list in wire format.

### Session Lifecycle

- `PROCEDURE SessionCreate(lp: Loop; sched: Scheduler; ctx: TLSContext; fd: INTEGER; VAR out: TLSSession): Status`
  Create a client session over a connected non-blocking socket.

- `PROCEDURE SessionCreateServer(lp: Loop; sched: Scheduler; ctx: TLSContext; fd: INTEGER; VAR out: TLSSession): Status`
  Create a server session over an accepted non-blocking socket.

- `PROCEDURE SessionDestroy(VAR s: TLSSession): Status`
  Destroy a session. Does NOT close the underlying fd.

- `PROCEDURE SetSNI(s: TLSSession; VAR host: ARRAY OF CHAR): Status`
  Set SNI hostname. Must be called before Handshake.

### Sync Operations (try-once, non-blocking)

- `PROCEDURE Handshake(s: TLSSession): Status`
  Attempt one step of the TLS handshake. Returns OK when complete, WantRead/WantWrite to retry later.

- `PROCEDURE Read(s: TLSSession; buf: ADDRESS; max: INTEGER; VAR got: INTEGER): Status`
  Attempt to read up to max bytes. On OK, got contains bytes read.

- `PROCEDURE Write(s: TLSSession; buf: ADDRESS; len: INTEGER; VAR sent: INTEGER): Status`
  Attempt to write up to len bytes. On OK, sent contains bytes written.

- `PROCEDURE Shutdown(s: TLSSession): Status`
  Initiate TLS shutdown (send close_notify).

### Async Operations (EventLoop + Futures)

- `PROCEDURE HandshakeAsync(s: TLSSession; VAR out: Future): Status`
  Complete the TLS handshake asynchronously. Future resolves on success or rejects with error code.

- `PROCEDURE ReadAsync(s: TLSSession; buf: ADDRESS; max: INTEGER; VAR out: Future): Status`
  Read up to max bytes asynchronously. Future resolves with Value.tag = bytes read.

- `PROCEDURE WriteAsync(s: TLSSession; buf: ADDRESS; len: INTEGER; VAR out: Future): Status`
  Write up to len bytes asynchronously. Future resolves with Value.tag = bytes written.

- `PROCEDURE WriteAllAsync(s: TLSSession; buf: ADDRESS; len: INTEGER; VAR out: Future): Status`
  Write all len bytes asynchronously (loops until complete).

### Diagnostics

- `PROCEDURE GetPeerSummary(s: TLSSession; VAR out: ARRAY OF CHAR): Status`
  Copy the peer certificate subject (one-line format) into out.

- `PROCEDURE GetALPN(s: TLSSession; VAR out: ARRAY OF CHAR; VAR got: INTEGER): Status`
  Query the negotiated ALPN protocol string after handshake.

- `PROCEDURE GetVerifyResult(s: TLSSession): INTEGER`
  Return the X509 verification result code (0 = success).

- `PROCEDURE GetLastError(VAR out: ARRAY OF CHAR)`
  Copy the last TLS engine error string into out.

## Example

### Simple HTTPS Client

```modula2
MODULE TLSExample;

FROM TLS IMPORT TLSContext, TLSSession, Status,
                ContextCreate, ContextDestroy, LoadSystemRoots,
                SessionCreate, SessionDestroy, SetSNI,
                Handshake, Read, Write, Shutdown;
FROM SYSTEM IMPORT ADR;

VAR
  ctx: TLSContext;
  sess: TLSSession;
  st: Status;
  buf: ARRAY [0..4095] OF CHAR;
  got, sent: INTEGER;

BEGIN
  st := ContextCreate(ctx);
  st := LoadSystemRoots(ctx);

  (* assume fd is a connected TCP socket *)
  st := SessionCreate(loop, sched, ctx, fd, sess);
  st := SetSNI(sess, "example.com");

  (* drive handshake to completion *)
  REPEAT
    st := Handshake(sess);
  UNTIL st = OK;

  (* send a request *)
  st := Write(sess, ADR(request), reqLen, sent);

  (* read response *)
  st := Read(sess, ADR(buf), 4096, got);

  st := Shutdown(sess);
  st := SessionDestroy(sess);
  st := ContextDestroy(ctx);
END TLSExample.
```

### Mutual TLS Server

```modula2
(* Server that requires client certificates *)
st := ContextCreateServer(ctx);
st := SetServerCert(ctx, "server.pem", "server-key.pem");
st := SetClientCA(ctx, "client-ca.pem");
st := RequireClientCert(ctx, TRUE);

(* After handshake, inspect client identity *)
IF GetPeerCertDN(sess, dnBuf) THEN
  (* dnBuf contains the client certificate subject *)
END;
```
