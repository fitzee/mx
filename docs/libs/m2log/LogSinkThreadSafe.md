# LogSinkThreadSafe

Thread-safe log sink for the Log module. Wraps the entire Format + write sequence inside a mutex lock so each log line is emitted atomically. Uses `m2sys_fwrite_str` (C `fwrite`) instead of InOut, avoiding the non-thread-safe `InOut.WriteString` path.

## Behavior

Each log call acquires a mutex, formats the record into a line buffer, writes the line and a trailing newline to the file handle, then releases the mutex. This guarantees that concurrent log calls from multiple threads never interleave partial lines or corrupt the FILE buffer.

Two factory functions are provided:

- **CreateFile** — opens a file for append, mutex-protected writes. Use this for log files.
- **CreateStderr** — writes to stderr (fd 2), mutex-protected. Use this as a drop-in replacement for the default console sink in threaded programs.

If a write fails, the error is silently ignored -- logging must never crash your program.

## Dependencies

Requires m2sys and m2pthreads. When building, link `libs/m2sys/m2sys.c` and `libs/m2pthreads/threads_shim.c` as extra C source files, and pass `-lpthread` to the linker.

## Procedures

### CreateFile

```modula2
PROCEDURE CreateFile(path: ARRAY OF CHAR; VAR out: Sink): BOOLEAN;
```

Open a file for append and return a mutex-protected Sink. Creates the file if it does not exist. Returns FALSE if the file cannot be opened (bad path, permissions, etc.). On success, `out` is ready to pass to `Log.AddSink`.

```modula2
VAR fs: Sink; ok: BOOLEAN;
ok := CreateFile("/var/log/myapp.log", fs);
IF ok THEN
  IF AddSink(lg, fs) THEN END
END;
```

### CreateStderr

```modula2
PROCEDURE CreateStderr(VAR out: Sink);
```

Create a sink that writes to stderr (fd 2), mutex-protected. Always succeeds. This is the recommended sink for threaded server applications that want structured logging on stderr.

```modula2
VAR ss: Sink;
CreateStderr(ss);
IF AddSink(lg, ss) THEN END;
```

### Close

```modula2
PROCEDURE Close(VAR s: Sink);
```

Destroy the mutex and, if the sink was created with `CreateFile`, close the underlying file handle. After this call the sink becomes a no-op. Safe to call multiple times; the second call is a no-op. Sinks created with `CreateStderr` never close fd 2.

Always call `Close` before program exit. In M2+ programs with exception handling, put it in a FINALLY block:

```modula2
TRY
  (* main application logic *)
FINALLY
  Close(ss)
END;
```

## Example

```modula2
MODULE ThreadSafeLogDemo;

FROM Log IMPORT Logger, Init, AddSink, Sink, Info, Warn;
FROM LogSinkThreadSafe IMPORT CreateStderr, Close;

VAR
  lg: Logger;
  ss: Sink;

BEGIN
  Init(lg);
  CreateStderr(ss);
  IF AddSink(lg, ss) THEN END;

  Info(lg, "server started");
  (* safe to call Info/Warn/Error from any thread *)
  Warn(lg, "connection pool exhausted");

  Close(ss)
END ThreadSafeLogDemo.
```
