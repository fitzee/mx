# Modula-2 Language Reference

Reference documentation for PIM4 Modula-2 as implemented by m2c, including
Modula-2+ extensions.

## Categories

### [keywords/](keywords/)

Reserved words and keyword-level constructs: MODULE, PROCEDURE, IF, WHILE,
REPEAT, FOR, LOOP, CASE, WITH, RETURN, EXIT, IMPORT, FROM, EXPORT, VAR, CONST,
TYPE, BEGIN, END, DEFINITION, IMPLEMENTATION, QUALIFIED, ARRAY, RECORD, SET,
POINTER, AND, OR, NOT, DIV, MOD, IN.

### [types/](types/)

Built-in types: INTEGER, CARDINAL, REAL, LONGREAL, BOOLEAN, CHAR, BITSET,
WORD, BYTE, ADDRESS, LONGINT, LONGCARD, PROC.

### [builtins/](builtins/)

Built-in procedures and functions: NEW, DISPOSE, INC, DEC, INCL, EXCL, HALT,
ABS, ODD, CAP, ORD, CHR, VAL, HIGH, SIZE, TSIZE, ADR, MAX, MIN, FLOAT, TRUNC.
Bitwise operations: SHL, SHR, BAND, BOR, BXOR, BNOT, SHIFT, ROTATE.
Constants: TRUE, FALSE, NIL.

### [stdlib/](stdlib/)

Standard library modules: InOut, RealInOut, MathLib0, Strings, Terminal,
Storage, SYSTEM, Conversions, Args, STextIO, SWholeIO, SRealIO.

### [m2plus/](m2plus/)

Modula-2+ extensions (enabled with `--m2plus`): TRY, EXCEPT, FINALLY, RAISE,
EXCEPTION, RETRY, REF, REFANY, BRANDED.

### [constructs/](constructs/)

Composite language constructs and patterns (planned).

## Libraries

Bundled libraries that ship with m2c and can be added as dependencies
via `m2.toml`.

### [../libs/m2futures/](../libs/m2futures/) -- Async

Composable Promises/Futures for single-threaded asynchronous programming.
Scheduler (microtask queue), Promise (resolve/reject/chain/combine).

### [../libs/m2gfx/](../libs/m2gfx/) -- Graphics

2D graphics library backed by SDL2. Window management, canvas drawing,
pixel buffers, fonts, textures, events, and color utilities.

### [../libs/m2evloop/](../libs/m2evloop/) -- Event Loop

Cross-platform event loop for I/O polling and timer-based async workloads.
Poller (kqueue/epoll-based fd readiness), Timers (min-heap timer queue),
EventLoop (orchestrator integrating Poller + Timers + Scheduler).

### [../libs/m2tls/](../libs/m2tls/) -- TLS

TLS transport layer wrapping OpenSSL/LibreSSL. Context and session management,
sync (try-once) and async (Future-returning) operations, certificate
verification (ON by default), SNI, system root store loading.

### [../libs/m2http/](../libs/m2http/) -- HTTP Client

HTTP/1.1 networking stack built on the async runtime. Buffers (binary I/O),
URI (URL parsing), DNS (hostname resolution), HTTPClient (non-blocking
HTTP/HTTPS GET/HEAD with chunked transfer and TLS support).

### [../libs/m2sockets/](../libs/m2sockets/) -- Networking

TCP and UDP socket networking over POSIX/BSD system calls.

## See Also

- [grammar.md](grammar.md) -- Concise EBNF grammar reference
