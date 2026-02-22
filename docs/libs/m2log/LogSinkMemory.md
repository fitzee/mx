# LogSinkMemory

In-memory ring buffer sink for testing. Captures formatted log lines so tests can assert on log output without any I/O. When the buffer fills, the oldest entry is overwritten.

## Why use a memory sink?

Unit tests that verify logging behavior need a way to capture output and inspect it. Writing to a file and reading it back is fragile and slow. The memory sink stores up to 64 formatted lines in a fixed-size ring buffer that you can query directly -- check how many lines were captured, retrieve specific lines by index, or search for a substring across all stored lines.

## Types

### MemorySink

```modula2
TYPE MemorySink = RECORD
  lines:     ARRAY [0..MaxLines-1] OF StoredLine;
  count:     INTEGER;
  nextSlot:  INTEGER;
  totalSeen: INTEGER;
END;
```

Declare this as a module-level variable so it outlives the Sink handle. The ring buffer holds up to 64 lines of up to 512 characters each.

| Constant | Value | Purpose |
|----------|-------|---------|
| `MaxLines` | 64 | Maximum lines stored before wrapping. |
| `LineLen` | 512 | Maximum characters per stored line. |

## Procedures

### Create

```modula2
PROCEDURE Create(VAR mem: MemorySink; VAR out: Sink);
```

Initialize a MemorySink and return a Sink handle suitable for passing to `Log.AddSink`. The `mem` variable must outlive the sink -- typically a module-level variable or a variable in an outer scope that persists for the test's duration.

```modula2
VAR mem: MemorySink; ms: Sink; lg: Logger;
Create(mem, ms);
Init(lg);
IF AddSink(lg, ms) THEN END;
```

### GetCount

```modula2
PROCEDURE GetCount(VAR mem: MemorySink): INTEGER;
```

Number of lines currently stored (0 to MaxLines). After the ring buffer wraps, this stays at MaxLines -- older entries are silently overwritten.

### GetTotal

```modula2
PROCEDURE GetTotal(VAR mem: MemorySink): INTEGER;
```

Total number of log calls received since creation or the last `Clear`. May exceed MaxLines if the buffer has wrapped. Useful for asserting that the expected number of log calls occurred, even if the buffer can't hold them all.

### GetLine

```modula2
PROCEDURE GetLine(VAR mem: MemorySink; index: INTEGER;
                  VAR buf: ARRAY OF CHAR): BOOLEAN;
```

Copy the stored line at `index` into `buf`. Index 0 is the oldest stored line, index `GetCount(mem) - 1` is the newest. Returns FALSE if `index` is out of range.

### Clear

```modula2
PROCEDURE Clear(VAR mem: MemorySink);
```

Reset all counters and discard stored lines. Useful between test cases when reusing the same sink.

### Contains

```modula2
PROCEDURE Contains(VAR mem: MemorySink; sub: ARRAY OF CHAR): BOOLEAN;
```

Search all stored lines for the substring `sub`. Returns TRUE if any line contains it. This is the workhorse for test assertions -- rather than extracting a full line and doing string comparison, you can just check that the expected content appeared somewhere in the log output.

```modula2
Info(lg, "connection established");
IF NOT Contains(mem, "established") THEN
  WriteString("FAIL: expected log message"); WriteLn
END;
```

## Example

```modula2
MODULE LogTest;

FROM InOut IMPORT WriteString, WriteLn;
FROM Log IMPORT Logger, Init, AddSink, Sink, Info, Warn;
FROM LogSinkMemory IMPORT MemorySink, Create, GetCount, GetLine, Contains, Clear;

VAR
  lg: Logger;
  ms: Sink;
  mem: MemorySink;
  line: ARRAY [0..511] OF CHAR;

BEGIN
  Init(lg);
  Create(mem, ms);
  IF AddSink(lg, ms) THEN END;

  Info(lg, "first");
  Warn(lg, "second");

  IF GetCount(mem) = 2 THEN
    WriteString("PASS: 2 lines captured"); WriteLn
  END;

  IF Contains(mem, "first") THEN
    WriteString("PASS: found 'first'"); WriteLn
  END;

  GetLine(mem, 1, line);
  (* line contains: WARN msg="second" *)

  Clear(mem);
  (* GetCount(mem) = 0, ready for next test *)
END LogTest.
```
