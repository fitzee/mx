MODULE StructuredLogging;
(* Structured logging with key/value fields and categories.

   Demonstrates:
     - Custom logger with explicit level
     - Category tagging
     - KVStr, KVInt, KVBool field constructors
     - LogKV for structured messages
     - WithCategory for child loggers

   Build:
     m2c examples/structured.mod -I src -o structured
     ./structured

   Expected output (fields sorted alphabetically):
     INFO msg="server listening" category="net" host="0.0.0.0" port=8080 tls=true
     INFO msg="client connected" category="net.conn" fd=12 peer="192.168.1.5"
     WARN msg="slow query" category="db" ms=342 table="users" *)

FROM Log IMPORT Logger, Field, Level,
                Init, SetLevel, SetCategory, WithCategory,
                AddSink, MakeConsoleSink, Sink,
                Info, Warn, LogKV,
                KVStr, KVInt, KVBool;

VAR
  lg, connLog: Logger;
  cs: Sink;
  fs: ARRAY [0..2] OF Field;

BEGIN
  Init(lg);
  MakeConsoleSink(cs);
  IF AddSink(lg, cs) THEN END;
  SetCategory(lg, "net");

  (* Structured info message with 3 fields *)
  KVStr("host", "0.0.0.0", fs[0]);
  KVInt("port", 8080, fs[1]);
  KVBool("tls", TRUE, fs[2]);
  LogKV(lg, INFO, "server listening", fs, 3);

  (* Child logger with sub-category *)
  WithCategory(lg, "net.conn", connLog);
  KVStr("peer", "192.168.1.5", fs[0]);
  KVInt("fd", 12, fs[1]);
  LogKV(connLog, INFO, "client connected", fs, 2);

  (* Different category *)
  SetCategory(lg, "db");
  KVStr("table", "users", fs[0]);
  KVInt("ms", 342, fs[1]);
  LogKV(lg, WARN, "slow query", fs, 2)
END StructuredLogging.
