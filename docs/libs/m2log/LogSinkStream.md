# LogSinkStream

Network stream sink for the Log module. Writes formatted log lines to an open m2stream handle over TCP or TLS. Designed for forwarding structured logs to a remote log collector or aggregation service.

## Behavior

Uses `Stream.TryWrite` internally for synchronous, non-blocking output. If the write would block (TLS renegotiation) or fails (connection closed, I/O error), the log line is silently dropped. This is intentional -- logging must never block the event loop or crash the program. Each line is terminated with a newline character.

The stream handle is **not** owned by the sink. The caller is responsible for creating the stream, keeping it alive for the sink's lifetime, and destroying it afterward.

## Dependencies

Requires the m2stream library and its transitive dependencies (m2futures, m2evloop, m2sockets). Only import this module if your program already uses networking -- it pulls in the full async I/O stack.

## Procedures

### Create

```modula2
PROCEDURE Create(streamHandle: ADDRESS; VAR out: Sink);
```

Create a sink that writes formatted log lines to an open stream. `streamHandle` must be a valid Stream handle (from `Stream.CreateTCP` or `Stream.CreateTLS`) in the Open state. The returned sink is ready to pass to `Log.AddSink`.

Unlike LogSinkFile.Create, this procedure does not return a success/failure flag -- the stream is assumed to already be connected and valid. Errors during individual writes are handled silently per the non-blocking policy.

## Example

```modula2
MODULE StreamLogDemo;

FROM SYSTEM IMPORT ADDRESS;
FROM Log IMPORT Logger, Init, AddSink, Sink, Info;
FROM LogSinkStream IMPORT Create;
FROM Stream IMPORT Stream, CreateTCP, Destroy, OK;
FROM Sockets IMPORT Connect;

VAR
  lg: Logger;
  ss: Sink;
  s: Stream;
  fd: INTEGER;

BEGIN
  (* Assume fd is a connected TCP socket to a log server *)
  IF CreateTCP(loop, sched, fd, s) = OK THEN
    Init(lg);
    Create(s, ss);
    IF AddSink(lg, ss) THEN END;

    Info(lg, "connected to log server");
    Info(lg, "processing started");

    (* When done, destroy the stream (sink becomes a no-op) *)
    Destroy(s)
  END
END StreamLogDemo.
```
