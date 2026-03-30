# m2log Design Notes

## Goals

1. Structured logging with key/value fields, not printf-style interpolation
2. Pluggable sinks via a simple procedure-type callback
3. Deterministic, machine-parseable single-line output format
4. Zero heap allocation in the hot path
5. Fast-path level check before any work
6. Recursion guard to prevent infinite loops when sinks log
7. Testable via in-memory sink with substring search

## Architecture

```
caller -> LogMsg/LogKV
            |
            v
         level check (fast path: return if below threshold)
            |
            v
         FillRecord (copy msg + category into Record)
            |
            v
         Dispatch -> for each sink:
                       level check (per-sink override)
                       call SinkProc(ctx, rec)
                         |
                         v
                       sink calls Format(rec, buf, len)
                       sink writes buf to output
```

Formatting is performed by the sink, not the logger. This means the logger
never allocates a line buffer. Sinks that do not need text output (e.g., a
hypothetical binary protocol sink) can skip Format entirely and read the
Record fields directly.

## Output format

```
LEVEL msg="message" category="cat" key1=val1 key2=val2
```

Design choices:

- **Level first** so log aggregators can parse severity without scanning.
- **msg= always present** even if empty, for consistent parsing.
- **Fields sorted by key** so output is deterministic regardless of insertion
  order. This makes tests reliable and diff output stable.
- **String values escaped** using backslash sequences: `\"`, `\\`, `\n`, `\t`,
  `\r`. Other control characters below space are dropped.
- **Timestamps** included by default via `m2sys_format_time` (ISO 8601 with
  millisecond precision). Can be disabled via `FormatOptions.showTimestamp`.
  Thread ID is also available via `FormatOptions.showThread`.

## Memory model

All buffers are fixed-size arrays on the stack:

| Buffer | Size | Location |
|---|---|---|
| Record.msg | 256 bytes | caller stack (LogMsg/LogKV) |
| Record.fields | 16 * ~168 bytes | caller stack |
| LineBuf | 2048 bytes | sink stack |
| MemorySink.lines | 64 * 512 bytes | module-level var |

No malloc, no free, no garbage collection in the log path.

The Field record is intentionally wide (contains both strVal and intVal
regardless of kind) to avoid pointer indirection and heap allocation. The
trade-off is ~168 bytes per field on the stack, but MaxFields=16 keeps the
total under 3KB.

## Recursion guard

The Logger has an `inSink` boolean flag. When Dispatch enters, it sets
`inSink := TRUE`. If a sink calls back into the same logger, Dispatch
detects the flag, increments `dropCount`, and returns immediately. The flag
is cleared when the original Dispatch returns.

This prevents infinite loops when (for example) an error handler logs to
the same logger. The drop count is available via GetDropCount for diagnostics.

The guard is per-logger, not global. Two independent loggers can call each
other's sinks without triggering the guard.

## Sink interface

```
TYPE SinkProc = PROCEDURE(ADDRESS, VAR Record);
TYPE Sink = RECORD proc: SinkProc; ctx: ADDRESS; minLevel: Level END;
```

The ADDRESS context parameter allows sinks to carry state (file handles,
buffers, network connections) without module-level variables.

Each sink has its own minLevel override. The logger's minLevel is checked
first (fast path); then each sink's minLevel is checked before dispatch.
This allows a single logger to send all messages to a file but only warnings
to the console.

## Module structure

The library was originally designed with a separate LogFmt module for
formatting. This caused a circular dependency: Log imported LogFmt for
Format, and LogFmt imported Log for Record/Field types. The topo sort
in the compiler cannot resolve cycles.

The fix was to merge all formatting code into Log.mod. This eliminates the
cycle and keeps the dependency graph acyclic:

```
Log (core + formatting + timestamps via Sys)
  ^           ^
  |           |
LogSinkMemory LogSinkFile
              |
              v
             Sys (m2sys FFI)
```

LogSinkStream depends on the m2stream library and is optional.

## Codegen workarounds

Three issues were encountered during development:

1. **VAR open array parameters**: The C backend generates incorrect code
   (`(*buf)_high`) when a procedure takes `VAR buf: ARRAY OF CHAR`. The
   workaround is to use `VAR buf: LineBuf` (a fixed-size named type) for
   internal format helpers instead of open arrays.

2. **Procedure variable VAR parameters**: Calling through a procedure-typed
   record field (`l.sinks[i].proc(ctx, rec)`) does not emit `&` for VAR
   parameters. The workaround is to extract the proc var to a local variable
   first: `sp := l.sinks[i].proc; sp(ctx, rec)`.

3. **C name collision**: A Modula-2 variable named `log` collides with the
   C standard library `log()` function from math.h. Use `lg` or `logger`
   instead.

## Field sorting

Fields are sorted by key using insertion sort. With MaxFields=16, insertion
sort is optimal: no recursion, no auxiliary allocation, and fewer comparisons
than quicksort for small n. The sort operates on a local copy of the fields
array so the caller's data is unchanged.

## Testing strategy

The test suite uses LogSinkMemory to capture formatted output and verify it
with substring searches (Contains) and positional checks (Pos). Tests cover:

- Level filtering fast path
- Deterministic formatting with field ordering
- String escaping (quotes, backslash, control characters)
- Ring buffer capture (GetCount, GetTotal, GetLine, Clear)
- Recursion guard (sink triggers log call)
- Field constructor correctness (KVStr, KVInt, KVBool, KVCard)
- Structured logging via LogKV
- Category propagation and WithCategory
- Multiple sinks receiving the same message
- Per-sink level override
- Negative integer formatting

No external test framework. The test module uses a Check(name, condition)
procedure and reports pass/fail counts.
