# LogSinkFile

File sink for the Log module. Appends formatted log lines to a file via m2sys. The file is opened on `Create` and held open until `Close`, so there is no open/close overhead per log call.

## Behavior

Each log call formats the record into a single line and writes it to the file. If the write fails (disk full, broken pipe, etc.), the error is silently ignored -- logging must never crash your program. Use `Log.GetDropCount` if you need to detect write failures.

For stderr output on Unix systems, pass `"/dev/stderr"` as the path. This is useful for programs that want structured logging on stderr while keeping stdout clean for data output.

## Dependencies

Requires m2sys. When building, link `libs/m2sys/m2sys.c` as an extra C source file.

## Procedures

### Create

```modula2
PROCEDURE Create(path: ARRAY OF CHAR; VAR out: Sink): BOOLEAN;
```

Open a file for append and return a configured Sink. Creates the file if it does not exist. Returns FALSE if the file cannot be opened (bad path, permissions, etc.). On success, `out` is ready to pass to `Log.AddSink`.

```modula2
VAR fs: Sink; ok: BOOLEAN;
ok := Create("/var/log/myapp.log", fs);
IF ok THEN
  IF AddSink(lg, fs) THEN END
END;
```

### Close

```modula2
PROCEDURE Close(VAR s: Sink);
```

Close the underlying file handle and release the internal handle record. After this call the sink becomes a no-op -- any log calls dispatched to it are silently ignored. Safe to call multiple times; the second call is a no-op.

Always call `Close` before program exit to flush the file. In M2+ programs with exception handling, put it in a FINALLY block:

```modula2
TRY
  (* main application logic *)
FINALLY
  IF ok THEN Close(fs) END
END;
```

## Example

```modula2
MODULE FileLogDemo;

FROM Log IMPORT Logger, Init, AddSink, Sink, Info, Warn;
FROM LogSinkFile IMPORT Create, Close;

VAR
  lg: Logger;
  fs: Sink;
  ok: BOOLEAN;

BEGIN
  Init(lg);
  ok := Create("app.log", fs);
  IF ok THEN
    IF AddSink(lg, fs) THEN END
  END;

  Info(lg, "application started");
  Warn(lg, "disk space low");

  IF ok THEN Close(fs) END
  (* app.log now contains two lines *)
END FileLogDemo.
```
