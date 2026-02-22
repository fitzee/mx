MODULE FileSinkDemo;
(* File sink example.

   Logs to both stdout and a file simultaneously.
   The file sink appends to the file; it creates the file if
   it does not exist.

   Requires m2sys (extra-c=../m2sys/m2sys.c in m2.toml).

   Build:
     m2c examples/file_sink.mod -I src \
         ../m2sys/m2sys.c -o file_sink
     ./file_sink
     cat /tmp/m2log_demo.log

   Expected: same 3 lines on stdout and in /tmp/m2log_demo.log *)

FROM Log IMPORT Logger, Init, AddSink, Sink, MakeConsoleSink,
                Info, Warn, Error;
FROM LogSinkFile IMPORT Create, Close;
FROM InOut IMPORT WriteString, WriteLn;

VAR
  lg: Logger;
  cs, fs: Sink;
  ok: BOOLEAN;

BEGIN
  Init(lg);

  (* Console sink *)
  MakeConsoleSink(cs);
  IF AddSink(lg, cs) THEN END;

  (* File sink *)
  ok := Create("/tmp/m2log_demo.log", fs);
  IF ok THEN
    IF AddSink(lg, fs) THEN END
  ELSE
    WriteString("warning: could not open log file"); WriteLn
  END;

  Info(lg, "file sink demo started");
  Warn(lg, "example warning");
  Error(lg, "example error");

  IF ok THEN Close(fs) END
END FileSinkDemo.
