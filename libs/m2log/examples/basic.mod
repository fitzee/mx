MODULE BasicLogging;
(* Basic logging example.

   Uses the default logger (stdout console sink at INFO level).
   Messages below INFO are silently discarded.

   Build:
     m2c examples/basic.mod -I src -o basic
     ./basic

   Expected output:
     INFO msg="application started"
     WARN msg="disk space low"
     ERROR msg="write failed" *)

FROM Log IMPORT InitDefault, SetLevel,
                InfoD, WarnD, ErrorD, DebugD, TraceD;

BEGIN
  InitDefault;

  (* These are below the default INFO threshold -- no output *)
  TraceD("tracing enabled");
  DebugD("cache miss");

  (* These pass the filter *)
  InfoD("application started");
  WarnD("disk space low");
  ErrorD("write failed")
END BasicLogging.
