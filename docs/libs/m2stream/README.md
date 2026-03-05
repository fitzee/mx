# m2stream

## Why

Transport-agnostic byte stream that wraps TCP sockets and TLS sessions behind a single interface. Supports both synchronous try-once I/O (caller drives retries via EventLoop) and asynchronous Future-returning operations for standalone use.

## Types

- **Stream** -- Opaque handle (ADDRESS) for a stream instance.
- **StreamKind** -- `(TCP, TLSStream)` -- the underlying transport.
- **StreamState** -- `(Open, ShutdownWr, Closed, Error)` -- current stream lifecycle state.
- **Status** -- `(OK, Invalid, WouldBlock, StreamClosed, SysError, TLSError, OutOfMemory)`.
  - OK -- operation succeeded.
  - Invalid -- bad argument or stream in wrong state.
  - WouldBlock -- TLS renegotiation in progress; EventLoop watcher adjusted, retry later.
  - StreamClosed -- peer closed the connection.
  - SysError -- fatal socket error.
  - TLSError -- fatal TLS error.
  - OutOfMemory -- allocation failure.

## Procedures

### Creation

- `CreateTCP(lp: ADDRESS; sched: Scheduler; fd: INTEGER; VAR out: Stream): Status` -- Create a stream over a connected non-blocking TCP socket. The socket must already be connected. lp is the EventLoop handle; sched is the m2futures Scheduler.
- `CreateTLS(lp: ADDRESS; sched: Scheduler; fd: INTEGER; ctx: ADDRESS; sess: ADDRESS; VAR out: Stream): Status` -- Create a stream over a completed TLS session. Stream takes ownership of ctx (TLS.TLSContext) and sess (TLS.TLSSession), destroying them on Destroy.

### Sync (Try-Once) Operations

Caller manages the EventLoop watcher on the fd. These return immediately.

- `TryRead(s: Stream; buf: ADDRESS; max: INTEGER; VAR got: INTEGER): Status` -- Attempt to read up to max bytes into buf. Returns OK with got > 0 on success, StreamClosed on peer close, WouldBlock on TLS renegotiation (TCP never returns WouldBlock).
- `TryWrite(s: Stream; buf: ADDRESS; len: INTEGER; VAR sent: INTEGER): Status` -- Attempt to write up to len bytes from buf. Returns OK with sent > 0 on success.

### Async Operations

Stream registers its own EventLoop watcher. Only ONE async operation may be pending at a time. buf must remain valid until the Future settles.

- `ReadAsync(s: Stream; buf: ADDRESS; max: INTEGER; VAR out: Future): Status` -- Read up to max bytes asynchronously. Future resolves with Value.tag = bytes read. Rejects with Error.code = 1 (I/O error) or 2 (closed).
- `WriteAsync(s: Stream; buf: ADDRESS; len: INTEGER; VAR out: Future): Status` -- Write up to len bytes asynchronously. Future resolves with Value.tag = bytes written (may be < len). Rejects with Error.code = 1 (I/O error).
- `WriteAllAsync(s: Stream; buf: ADDRESS; len: INTEGER; VAR out: Future): Status` -- Write all len bytes asynchronously (loops until complete). Future resolves with Value.tag = len.
- `CloseAsync(s: Stream; VAR out: Future): Status` -- Initiate graceful close. For TLS: sends close_notify then closes socket. For TCP: closes socket immediately. Stream is unusable after close completes.

### Sync Helpers

- `ShutdownWrite(s: Stream): Status` -- Half-close the write side. Sends TCP FIN (and TLS close_notify, best-effort). Reads remain possible.
- `GetState(s: Stream): StreamState` -- Query the current stream lifecycle state.
- `GetFd(s: Stream): INTEGER` -- Return the underlying file descriptor.
- `GetKind(s: Stream): StreamKind` -- Return the transport kind (TCP or TLSStream).
- `Destroy(VAR s: Stream): Status` -- Destroy the stream and release resources. TLS context and session are destroyed. In async mode, the watcher is removed and the socket is closed. In sync mode, the socket is left open for the caller. Sets s to NIL.

## Ownership Model

- Stream takes ownership of TLS context and session (destroyed on Destroy).
- Socket fd ownership depends on mode:
  - **Sync mode**: Caller manages fd lifecycle and EventLoop watcher.
  - **Async mode**: Stream manages the watcher; socket is closed on Destroy.

## Example

### Sync Mode (with EventLoop)

```modula2
MODULE StreamSyncExample;

FROM Stream IMPORT Stream, CreateTCP, TryRead, TryWrite,
                    ShutdownWrite, Destroy, Status;

VAR
  s:    Stream;
  st:   Status;
  buf:  ARRAY [0..4095] OF CHAR;
  got, sent: INTEGER;

BEGIN
  (* fd is a connected non-blocking TCP socket *)
  st := CreateTCP(loop, sched, fd, s);
  IF st = Status.OK THEN
    (* Caller's EventLoop watcher triggers when fd is readable *)
    st := TryRead(s, ADR(buf), 4096, got);
    IF (st = Status.OK) AND (got > 0) THEN
      st := TryWrite(s, ADR(buf), got, sent);
    END;
    st := ShutdownWrite(s);
    st := Destroy(s);
  END;
END StreamSyncExample.
```

### Async Mode (Future-Based)

```modula2
MODULE StreamAsyncExample;

FROM Stream IMPORT Stream, CreateTLS, ReadAsync, WriteAllAsync,
                    CloseAsync, Destroy, Status;
FROM Promise IMPORT Future;

VAR
  s:   Stream;
  st:  Status;
  buf: ARRAY [0..4095] OF CHAR;
  rf, wf, cf: Future;

BEGIN
  (* fd + ctx + sess from a completed TLS handshake *)
  st := CreateTLS(loop, sched, fd, ctx, sess, s);
  IF st = Status.OK THEN
    st := ReadAsync(s, ADR(buf), 4096, rf);
    (* When rf settles, rf.value.tag = bytes read *)

    st := WriteAllAsync(s, ADR(buf), 1024, wf);
    (* wf settles when all 1024 bytes are sent *)

    st := CloseAsync(s, cf);
    (* cf settles after close_notify + socket close *)

    st := Destroy(s);
  END;
END StreamAsyncExample.
```
