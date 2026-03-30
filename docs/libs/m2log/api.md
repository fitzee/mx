# m2log API Reference

## Module Log

Core logging module. Provides types, logger operations, formatting, and a
built-in console sink.

### Constants

```
MaxMsg      = 255    max message length
MaxKey      = 31     max field key length
MaxStrVal   = 127    max field string value length
MaxFields   = 16     max fields per log call
MaxSinks    = 8      max sinks per logger
MaxCategory = 63     max category string length
MaxLine     = 2048   max formatted output line length
```

### Types

**Level** = (TRACE, DEBUG, INFO, WARN, ERROR, FATAL)

Log severity levels in ascending order. Compared via ORD.

**FieldKind** = (FkStr, FkInt, FkBool)

Discriminator for field value type.

**FormatOptions**

```
RECORD
  showTimestamp: BOOLEAN;
  showLevel:    BOOLEAN;
  showCategory: BOOLEAN;
  showThread:   BOOLEAN;
END
```

Controls log line format. Defaults: timestamp=TRUE, level=TRUE, category=TRUE,
thread=FALSE. Set globally via SetFormatOptions.

**Field**

```
RECORD
  key:     ARRAY [0..MaxKey] OF CHAR;
  kind:    FieldKind;
  intVal:  INTEGER;
  boolVal: BOOLEAN;
  strVal:  ARRAY [0..MaxStrVal] OF CHAR;
END
```

A single key/value field. Fixed-size, stack-allocatable. Use KVStr, KVInt,
KVBool, or KVCard to construct.

**LineBuf** = ARRAY [0..MaxLine-1] OF CHAR

Buffer type used by Format.

**Record**

```
RECORD
  level:    Level;
  msg:      ARRAY [0..MaxMsg] OF CHAR;
  category: ARRAY [0..MaxCategory] OF CHAR;
  fields:   ARRAY [0..MaxFields-1] OF Field;
  nFields:  INTEGER;
END
```

A log record passed to sinks. Fully self-contained.

**SinkProc** = PROCEDURE(ADDRESS, VAR Record)

Sink callback type. First argument is user context. Second is the log record.

**Sink**

```
RECORD
  proc:     SinkProc;
  ctx:      ADDRESS;
  minLevel: Level;
END
```

A registered sink with optional level override.

**Logger**

```
RECORD
  minLevel:  Level;
  sinks:     ARRAY [0..MaxSinks-1] OF Sink;
  sinkCount: INTEGER;
  category:  ARRAY [0..MaxCategory] OF CHAR;
  inSink:    BOOLEAN;
  dropCount: INTEGER;
END
```

Logger state. Declare as a module-level or local variable. Always call Init
before use.

### Initialization

**Init(VAR l: Logger)**

Initialize a logger with level INFO and no sinks. Call AddSink to attach one
or more sinks before logging.

**InitDefault**

Initialize the module-level default logger with a console sink (stdout) at
level INFO. Safe to call multiple times. Called automatically by the *D
convenience procedures.

### Configuration

**SetLevel(VAR l: Logger; level: Level)**

Set the minimum level. Messages below this are discarded without formatting.

**SetDefaultLevel(level: Level)**

Set the minimum level on the default logger. Calls InitDefault if needed.

**InitFormatOptions(VAR opts: FormatOptions)**

Initialize format options to defaults (timestamp=TRUE, level=TRUE,
category=TRUE, thread=FALSE).

**SetFormatOptions(opts: FormatOptions)**

Set the global format options used by all Format calls.

**AddSink(VAR l: Logger; s: Sink): BOOLEAN**

Add a sink. Returns FALSE if the sink list is full (MaxSinks).

**SetCategory(VAR l: Logger; cat: ARRAY OF CHAR)**

Set the category string. Included in all subsequent records.

**WithCategory(VAR src: Logger; cat: ARRAY OF CHAR; VAR out: Logger)**

Create a copy of src with a different category. Sinks are copied, not shared.

### Logging

**LogMsg(VAR l: Logger; level: Level; msg: ARRAY OF CHAR)**

Log a plain message at the given level. If level < l.minLevel, returns
immediately.

**LogKV(VAR l: Logger; level: Level; msg: ARRAY OF CHAR; fields: ARRAY OF Field; nFields: INTEGER)**

Log a structured message with key/value fields. nFields is the number of valid
entries (clamped to HIGH(fields)+1 and MaxFields).

### Convenience (explicit logger)

**Trace(VAR l: Logger; msg: ARRAY OF CHAR)**
**Debug(VAR l: Logger; msg: ARRAY OF CHAR)**
**Info(VAR l: Logger; msg: ARRAY OF CHAR)**
**Warn(VAR l: Logger; msg: ARRAY OF CHAR)**
**Error(VAR l: Logger; msg: ARRAY OF CHAR)**
**Fatal(VAR l: Logger; msg: ARRAY OF CHAR)**

Shorthand for LogMsg(l, LEVEL, msg).

### Convenience (default logger)

**LogKVD(level: Level; msg: ARRAY OF CHAR; fields: ARRAY OF Field; nFields: INTEGER)**

Log a structured message with fields using the default logger.

**TraceD(msg: ARRAY OF CHAR)**
**DebugD(msg: ARRAY OF CHAR)**
**InfoD(msg: ARRAY OF CHAR)**
**WarnD(msg: ARRAY OF CHAR)**
**ErrorD(msg: ARRAY OF CHAR)**
**FatalD(msg: ARRAY OF CHAR)**

Log using the module-level default logger. Calls InitDefault on first use.

### Field constructors

**KVStr(key: ARRAY OF CHAR; val: ARRAY OF CHAR; VAR out: Field)**

Build a string field. key and val are truncated if too long.

**KVInt(key: ARRAY OF CHAR; val: INTEGER; VAR out: Field)**

Build an integer field.

**KVBool(key: ARRAY OF CHAR; val: BOOLEAN; VAR out: Field)**

Build a boolean field.

**KVCard(key: ARRAY OF CHAR; val: LONGCARD; VAR dst: Field)**

Build a cardinal field. Formats the LONGCARD as a decimal string internally.

### Diagnostics

**GetDropCount(VAR l: Logger): INTEGER**

Return the number of log calls suppressed by the recursion guard.

### Formatting

**Format(VAR rec: Record; VAR buf: LineBuf; VAR len: INTEGER)**

Format a log record into buf. len is set to the number of characters written
(excluding NUL terminator). Truncates at MaxLine.

Sink implementations call this to produce output.

### Sink helpers

**MakeConsoleSink(VAR out: Sink)**

Create a sink that writes formatted lines to stdout via InOut. Level override
set to TRACE so the logger's own minLevel controls filtering.

---

## Module LogSinkMemory

In-memory ring buffer sink for testing.

### Constants

```
MaxLines = 64    max lines stored
LineLen  = 512   max chars per stored line
```

### Types

**StoredLine** = ARRAY [0..LineLen-1] OF CHAR

**MemorySink**

```
RECORD
  lines:     ARRAY [0..MaxLines-1] OF StoredLine;
  count:     INTEGER;
  nextSlot:  INTEGER;
  totalSeen: INTEGER;
END
```

Declare as a module-level variable so it outlives the Sink handle.

### Procedures

**Create(VAR mem: MemorySink; VAR out: Sink)**

Initialize a MemorySink and return a Sink handle. mem must outlive the Sink.

**GetCount(VAR mem: MemorySink): INTEGER**

Number of lines currently stored (up to MaxLines).

**GetTotal(VAR mem: MemorySink): INTEGER**

Total number of log calls seen (may exceed MaxLines).

**GetLine(VAR mem: MemorySink; index: INTEGER; VAR buf: ARRAY OF CHAR): BOOLEAN**

Copy stored line at index into buf. Index 0 is the oldest stored line.
Returns FALSE if index is out of range.

**Clear(VAR mem: MemorySink)**

Clear all stored lines and reset counters.

**Contains(VAR mem: MemorySink; sub: ARRAY OF CHAR): BOOLEAN**

Check if any stored line contains the substring sub.

---

## Module LogSinkFile

File sink. Appends formatted log lines to a file.

### Procedures

**Create(path: ARRAY OF CHAR; VAR out: Sink): BOOLEAN**

Open a file for append and return a configured Sink. Returns FALSE if the file
cannot be opened. For stderr output on Unix, pass "/dev/stderr".

**Close(VAR s: Sink)**

Close the file handle. The sink becomes a no-op after this call.

### Dependencies

Requires m2sys (link libs/m2sys/m2sys.c).

---

## Module LogSinkStream

Network stream sink. Writes formatted log lines to an m2stream handle.

### Procedures

**Create(streamHandle: ADDRESS; VAR out: Sink)**

Create a sink that writes to an open stream. The stream must outlive the sink.
Uses non-blocking TryWrite; lines are silently dropped on WouldBlock or error.

### Dependencies

Requires m2stream library.
