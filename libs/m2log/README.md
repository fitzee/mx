# m2log

Structured logging library for Modula-2 PIM4.

Level-filtered, sink-dispatched logging with key/value fields.
No heap allocation in the log path. Fixed buffers throughout.

## Quick start

```modula2
FROM Log IMPORT InitDefault, InfoD, WarnD;

BEGIN
  InitDefault;
  InfoD("server started");
  WarnD("disk space low")
END
```

Output:

```
INFO msg="server started"
WARN msg="disk space low"
```

## Build

The test suite uses only the core module (no m2sys dependency):

```sh
mx tests/log_tests.mod -I src -o log_tests
./log_tests
```

Examples:

```sh
mx examples/basic.mod -I src -o basic
mx examples/structured.mod -I src -o structured
mx examples/file_sink.mod -I src ../m2sys/m2sys.c -o file_sink
```

## Modules

| Module | Purpose | Dependencies |
|---|---|---|
| `Log` | Core types, logger, formatting, console sink | InOut, Strings |
| `LogSinkMemory` | In-memory ring buffer sink for testing | Log, Strings |
| `LogSinkFile` | Append-to-file sink | Log, Sys (m2sys) |
| `LogSinkStream` | Network stream sink | Log, Stream (m2stream) |

## Levels

Six severity levels in ascending order:

| Level | Typical use |
|---|---|
| TRACE | Fine-grained internal tracing |
| DEBUG | Development diagnostics |
| INFO | Normal operational events (default threshold) |
| WARN | Degraded but recoverable conditions |
| ERROR | Failed operations |
| FATAL | Unrecoverable conditions |

## Output format

Single-line, deterministic:

```
LEVEL msg="message" category="cat" key1=val1 key2=val2
```

- Level name first, uppercase
- Message as `msg="..."` with escape sequences
- Category next if non-empty
- Fields sorted by key, lexicographically
- Strings quoted and escaped (`\"`, `\\`, `\n`, `\t`, `\r`)
- Integers unquoted
- Booleans as `true`/`false` unquoted

## Two usage modes

**Default logger** -- call `InitDefault`, then use `InfoD`/`WarnD`/`ErrorD` etc.
The default logger writes to stdout via a console sink at INFO level.

**Custom loggers** -- declare a `Logger` variable, call `Init`, add sinks with
`AddSink`, then use `LogMsg`/`LogKV` or the convenience procedures (`Info`,
`Warn`, etc.) with an explicit logger argument.

## Structured fields

```modula2
VAR fs: ARRAY [0..1] OF Field;

KVStr("peer", "10.0.0.1", fs[0]);
KVInt("port", 8080, fs[1]);
LogKV(lg, INFO, "connected", fs, 2);
```

Output: `INFO msg="connected" peer="10.0.0.1" port=8080`

## Sinks

A sink is a `SinkProc` callback plus a context pointer and optional level
override. Up to 8 sinks per logger.

Built-in sinks:

- **Console** (`MakeConsoleSink`) -- writes to stdout via InOut
- **Memory** (`LogSinkMemory.Create`) -- ring buffer for tests
- **File** (`LogSinkFile.Create`) -- appends to a file via m2sys
- **Stream** (`LogSinkStream.Create`) -- writes to m2stream handle

Custom sinks implement `SinkProc = PROCEDURE(ADDRESS, VAR Record)` and call
`Log.Format` to produce the formatted line.

## Safety

- **Fast path**: if a message's level is below the logger's minimum, no
  formatting or allocation occurs.
- **Recursion guard**: if a sink triggers a log call on the same logger, the
  inner call is suppressed and a drop counter is incremented.
- **No heap allocation**: all buffers are stack-allocated with fixed sizes.
  Long messages and field values are safely truncated.

## Documentation

- [API reference](docs/api.md)
- [Design notes](docs/design.md)

## Tests

47 checks across 11 test procedures covering level filtering, formatting,
escaping, memory sink capture, recursion guard, field constructors, structured
logging, categories, multiple sinks, sink-level overrides, and negative integer
formatting.
