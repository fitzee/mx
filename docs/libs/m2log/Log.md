# Log

Structured logging library for Modula-2. Provides level-filtered, sink-dispatched logging with optional key/value fields. Designed for single-threaded programs -- no heap allocation in the log path, fixed buffers throughout, long messages truncated safely.

## Why use structured logging?

Traditional `WriteString`-based debugging is fine for small programs, but falls apart when you need to filter by severity, route output to files or network sinks, or parse logs programmatically. The Log module gives you:

- **Level filtering** -- set a minimum severity and messages below it are discarded immediately, with zero formatting cost.
- **Pluggable sinks** -- attach up to 8 output destinations (console, file, memory ring buffer, network stream) to a single logger.
- **Structured fields** -- attach typed key/value pairs (strings, integers, booleans) to any log call. Fields are sorted and formatted deterministically so logs are both human-readable and machine-parseable.
- **Categories** -- tag loggers with a category string (e.g. "http", "db") to identify the subsystem.
- **Recursion guard** -- if a sink itself triggers a log call, the inner call is silently suppressed and a drop counter is incremented. This prevents infinite loops without crashing.

## Output format

Every log line is deterministic and single-line:

```
LEVEL msg="message text" category="cat" key1=val1 key2="string val"
```

Rules: level name first (uppercase), message always as `msg="..."`, category next if non-empty, then fields sorted lexicographically by key. String values are quoted and escaped. Integers and booleans are unquoted.

## Types

### Level

```modula2
TYPE Level = (TRACE, DEBUG, INFO, WARN, ERROR, FATAL);
```

Severity levels in ascending order. Compared via `ORD` -- `ORD(INFO) > ORD(DEBUG)`, so setting minimum level to INFO discards TRACE and DEBUG messages.

### Field

```modula2
TYPE Field = RECORD
  key:     ARRAY [0..MaxKey] OF CHAR;
  kind:    FieldKind;
  intVal:  INTEGER;
  boolVal: BOOLEAN;
  strVal:  ARRAY [0..MaxStrVal] OF CHAR;
END;
```

A single key/value pair. Fixed-size and stack-allocatable -- no heap allocation. Construct fields using `KVStr`, `KVInt`, or `KVBool` rather than filling the record manually.

### Record

```modula2
TYPE Record = RECORD
  level:    Level;
  msg:      ARRAY [0..MaxMsg] OF CHAR;
  category: ARRAY [0..MaxCategory] OF CHAR;
  fields:   ARRAY [0..MaxFields-1] OF Field;
  nFields:  INTEGER;
END;
```

A fully self-contained log record passed to sinks. Sinks receive this by VAR reference and should treat it as read-only.

### Sink

```modula2
TYPE SinkProc = PROCEDURE(ADDRESS, VAR Record);

TYPE Sink = RECORD
  proc:     SinkProc;
  ctx:      ADDRESS;
  minLevel: Level;
END;
```

A registered output destination. `proc` is called with `ctx` as the first argument and the log record as the second. `minLevel` provides per-sink level filtering on top of the logger's own minimum.

### Logger

```modula2
TYPE Logger = RECORD
  minLevel:  Level;
  sinks:     ARRAY [0..MaxSinks-1] OF Sink;
  sinkCount: INTEGER;
  category:  ARRAY [0..MaxCategory] OF CHAR;
  inSink:    BOOLEAN;
  dropCount: INTEGER;
END;
```

Logger state. Declare as a module-level or local variable -- the Logger is a value type, not a heap object. Always call `Init` before use.

## Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `MaxMsg` | 255 | Maximum message length in characters. |
| `MaxKey` | 31 | Maximum field key length. |
| `MaxStrVal` | 127 | Maximum field string value length. |
| `MaxFields` | 16 | Maximum fields per log call. |
| `MaxSinks` | 8 | Maximum sinks per logger. |
| `MaxCategory` | 63 | Maximum category string length. |
| `MaxLine` | 2048 | Maximum formatted output line length. |

## Procedures

### Init

```modula2
PROCEDURE Init(VAR l: Logger);
```

Initialize a logger with level INFO and no sinks. You must call `AddSink` at least once before logging, otherwise log calls are silently discarded (which is fine during setup).

### InitDefault

```modula2
PROCEDURE InitDefault;
```

Initialize the module-level default logger with a console sink (stdout) at level INFO. Safe to call multiple times -- subsequent calls are no-ops. The `*D` convenience procedures (InfoD, WarnD, etc.) call this automatically on first use.

### SetLevel

```modula2
PROCEDURE SetLevel(VAR l: Logger; level: Level);
```

Set the minimum severity level. Messages below this level are discarded immediately without formatting. This is the primary performance knob -- set to WARN in production to eliminate the cost of DEBUG/INFO messages entirely.

### AddSink

```modula2
PROCEDURE AddSink(VAR l: Logger; s: Sink): BOOLEAN;
```

Attach an output sink. Returns FALSE if the logger already has MaxSinks (8) sinks attached. A logger with zero sinks silently discards all messages.

### SetCategory

```modula2
PROCEDURE SetCategory(VAR l: Logger; cat: ARRAY OF CHAR);
```

Set a category string that appears in all subsequent log records. Useful for tagging loggers by subsystem (e.g. "http", "db", "auth").

### WithCategory

```modula2
PROCEDURE WithCategory(VAR src: Logger; cat: ARRAY OF CHAR; VAR out: Logger);
```

Create a copy of `src` with a different category. The sink list is copied (not shared), so changes to one logger's sinks do not affect the other. This is the idiomatic way to create per-subsystem loggers that share the same output configuration.

```modula2
VAR appLog, dbLog: Logger;
Init(appLog);
MakeConsoleSink(cs);
IF AddSink(appLog, cs) THEN END;
WithCategory(appLog, "db", dbLog);
Info(dbLog, "connection pool ready");
(* Output: INFO msg="connection pool ready" category="db" *)
```

### LogMsg

```modula2
PROCEDURE LogMsg(VAR l: Logger; level: Level; msg: ARRAY OF CHAR);
```

Log a plain message. If `level < l.minLevel`, returns immediately -- the message string is never touched. This is the fast path that makes it safe to leave TRACE/DEBUG calls in production code.

### LogKV

```modula2
PROCEDURE LogKV(VAR l: Logger; level: Level; msg: ARRAY OF CHAR;
                fields: ARRAY OF Field; nFields: INTEGER);
```

Log a structured message with key/value fields. `nFields` is clamped to both `HIGH(fields)+1` and `MaxFields`. Fields are sorted by key before formatting, so the output is deterministic regardless of the order you build them.

```modula2
VAR fs: ARRAY [0..2] OF Field;
KVStr("method", "GET", fs[0]);
KVStr("path", "/api/users", fs[1]);
KVInt("status", 200, fs[2]);
LogKV(lg, INFO, "request handled", fs, 3);
(* Output: INFO msg="request handled" method="GET" path="/api/users" status=200 *)
```

### Trace, Debug, Info, Warn, Error, Fatal

```modula2
PROCEDURE Trace(VAR l: Logger; msg: ARRAY OF CHAR);
PROCEDURE Debug(VAR l: Logger; msg: ARRAY OF CHAR);
PROCEDURE Info(VAR l: Logger; msg: ARRAY OF CHAR);
PROCEDURE Warn(VAR l: Logger; msg: ARRAY OF CHAR);
PROCEDURE Error(VAR l: Logger; msg: ARRAY OF CHAR);
PROCEDURE Fatal(VAR l: Logger; msg: ARRAY OF CHAR);
```

Shorthand for `LogMsg(l, LEVEL, msg)`. Use these when you don't need structured fields.

### TraceD, DebugD, InfoD, WarnD, ErrorD, FatalD

```modula2
PROCEDURE InfoD(msg: ARRAY OF CHAR);
(* etc. *)
```

Log using the module-level default logger. Calls `InitDefault` on first use, so these work without any setup. Useful for quick debugging or simple programs that don't need multiple loggers.

### KVStr

```modula2
PROCEDURE KVStr(key: ARRAY OF CHAR; val: ARRAY OF CHAR; VAR out: Field);
```

Build a string field. Both `key` and `val` are truncated to MaxKey and MaxStrVal respectively if too long. The field is fully self-contained (no pointers to external data).

### KVInt

```modula2
PROCEDURE KVInt(key: ARRAY OF CHAR; val: INTEGER; VAR out: Field);
```

Build an integer field. Formatted as an unquoted decimal in the output.

### KVBool

```modula2
PROCEDURE KVBool(key: ARRAY OF CHAR; val: BOOLEAN; VAR out: Field);
```

Build a boolean field. Formatted as `true` or `false` (unquoted) in the output.

### Format

```modula2
PROCEDURE Format(VAR rec: Record; VAR buf: LineBuf; VAR len: INTEGER);
```

Format a log record into a character buffer. `len` is set to the number of characters written (excluding the NUL terminator). If the formatted output would exceed MaxLine, it is truncated. Sink implementations call this to produce their output string.

### MakeConsoleSink

```modula2
PROCEDURE MakeConsoleSink(VAR out: Sink);
```

Create a sink that writes formatted lines to stdout via `InOut.WriteString`. The sink's minLevel is set to TRACE so that the logger's own minLevel is the sole filter. This is the sink used by `InitDefault`.

### GetDropCount

```modula2
PROCEDURE GetDropCount(VAR l: Logger): INTEGER;
```

Return the number of log calls suppressed by the recursion guard. If this is non-zero, it means a sink triggered a log call during dispatch. Useful for diagnosing misbehaving sinks.

## Example

```modula2
MODULE LogDemo;

FROM Log IMPORT Logger, Level, Field, Sink,
                Init, SetLevel, AddSink, SetCategory,
                MakeConsoleSink, LogKV, KVStr, KVInt, Info;

VAR
  lg: Logger;
  cs: Sink;
  fs: ARRAY [0..1] OF Field;

BEGIN
  Init(lg);
  MakeConsoleSink(cs);
  IF AddSink(lg, cs) THEN END;
  SetLevel(lg, DEBUG);
  SetCategory(lg, "app");

  Info(lg, "started");
  (* Output: INFO msg="started" category="app" *)

  KVStr("user", "alice", fs[0]);
  KVInt("latency", 42, fs[1]);
  LogKV(lg, DEBUG, "login", fs, 2);
  (* Output: DEBUG msg="login" category="app" latency=42 user="alice" *)
END LogDemo.
```
