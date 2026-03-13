# mx Module Resolution and Dependencies

How the mx compiler finds modules and how projects declare dependencies.

---

## Hard Rules

1. **Never invent module names.** Every `FROM X IMPORT ...` must correspond to a real module. If you are unsure whether a module exists, check the standard library list and the library table below.
2. **Never create a .def file for a standard library module.** InOut, Strings, SYSTEM, Storage, MathLib, Args, BinaryIO, and the other stdlib modules are compiled into the compiler. No files are needed.
3. **Never mix up module names and library names.** The library is `m2bytes`. The modules inside it are `ByteBuf`, `Codec`, `Hex`. You import `FROM ByteBuf IMPORT ...`, not `FROM m2bytes IMPORT ...`.

---

## Three Categories of Modules

### 1. Builtin Procedures (no import needed)

Always available. Never require `FROM ... IMPORT`:

```
ABS  CAP  CHR  DEC  EXCL  FLOAT  HALT  HIGH  INC  INCL
MAX  MIN  ODD  ORD  SIZE  TRUNC  VAL  NEW  DISPOSE
```

From SYSTEM (requires `FROM SYSTEM IMPORT ...`):

```
ADR  TSIZE  ADDRESS  WORD  BYTE
```

### 2. Standard Library Modules (compiled into mx)

These modules exist inside the compiler. No `.def` or `.mod` files are needed. No dependency declarations are needed. Just write `FROM ModuleName IMPORT ...`.

| Module | Key Exports |
|--------|-------------|
| `InOut` | WriteString, WriteInt, WriteCard, WriteLn, ReadString, ReadInt, Read, Write, OpenInput, OpenOutput, CloseInput, CloseOutput, Done |
| `RealInOut` | WriteReal, WriteLongReal, ReadReal |
| `Strings` | Assign, Length, Concat, Pos, Copy, CompareStr, Delete, Insert |
| `Storage` | ALLOCATE, DEALLOCATE |
| `SYSTEM` | ADDRESS, ADR, TSIZE, WORD, BYTE |
| `MathLib` | sqrt, sin, cos, exp, ln, arctan |
| `MathLib0` | sqrt, sin, cos, exp, ln, arctan, entier |
| `Terminal` | Read, Write, WriteString, WriteLn |
| `FileSystem` | Lookup, Close, ReadWord, WriteWord, ReadChar, WriteChar |
| `Args` | ArgCount, GetArg |
| `BinaryIO` | OpenRead, OpenWrite, Close, ReadBytes, WriteBytes, FileSize, IsEOF |
| `STextIO` | ReadChar, WriteChar, WriteString, WriteLn, ReadString |
| `SWholeIO` | ReadInt, WriteInt, ReadCard, WriteCard |
| `SRealIO` | ReadReal, WriteReal |

**Modula-2+ only** (requires `--m2plus` or `m2plus=true` in m2.toml):

| Module | Key Exports |
|--------|-------------|
| `Thread` | Fork, Join, Self, Sleep, Yield |
| `Mutex` | T, Init, Destroy, Lock, Unlock |
| `Condition` | T, Init, Destroy, Wait, Signal, Broadcast |

### 3. Library Modules (require files + dependency)

These ship as source in `libs/m2<name>/`. They require:
- A `.def` file reachable via include paths
- A dependency declaration in `m2.toml` (or `-I` flag)

---

## Library Reference

Each row shows: library name (for m2.toml `[deps]`), the module names you actually import, and what you need.

| Library | Modules | Deps | Notes |
|---------|---------|------|-------|
| **m2alloc** | AllocUtil, Arena, Pool | (none) | Arena + pool allocators |
| **m2auth** | Auth, AuthBridge, AuthMiddleware | m2bytes, m2http2server | JWT HS256, keyring |
| **m2bytes** | ByteBuf, Codec, Hex | (none) | Growable byte buffer, varint codec |
| **m2cli** | CLI | (none) | Argument parsing |
| **m2conf** | Conf | (none) | Config file parsing |
| **m2evloop** | EventLoop, Poller, Timers | m2sockets | Event loop, kqueue/epoll |
| **m2fmt** | Fmt | (none) | String formatting |
| **m2fsm** | Fsm, FsmTrace | (none) | Table-driven FSM |
| **m2futures** | Future, Promise, Scheduler | m2alloc | Promises, single-threaded async |
| **m2gfx** | Canvas, Color, DrawAlgo, Events, Font, Gfx, PixBuf, Texture | (none) | SDL2 graphics; needs `-lSDL2 -lSDL2_ttf` |
| **m2glob** | Glob | m2sys | Gitignore-grade glob matching |
| **m2hash** | HashMap | (none) | Hash map |
| **m2http** | Buffers, DNS, H2Client, HTTPClient, URI | m2futures, m2evloop, m2sockets, m2tls, m2stream, m2http2, m2bytes | HTTP client |
| **m2http2** | Http2Conn, Http2Frame, Http2Hpack, Http2Stream, Http2Types | m2bytes | HTTP/2 framing, HPACK |
| **m2http2server** | Http2Server, Http2Router, Http2Middleware, Http2ServerConn, Http2ServerStream, Http2ServerLog, Http2ServerMetrics, Http2ServerTypes | m2http2, m2bytes, m2sockets, m2tls, m2stream, m2log | Full HTTP/2 server |
| **m2json** | Json | (none) | JSON parser/generator |
| **m2lmdb** | Lmdb, LmdbBridge | (none) | LMDB key/value store; needs `-llmdb` |
| **m2log** | Log, LogSinkFile, LogSinkMemory, LogSinkStream | (none) | Structured logging |
| **m2metrics** | Metrics, MetricsBridge | (none) | System metrics: load avg, memory, CPU, RSS |
| **m2oidc** | Oidc, Jwks, OidcBridge | m2auth, m2json, m2pthreads | OIDC/JWKS/RS256; needs `-lssl -lcrypto` |
| **m2path** | Path | (none) | Path manipulation |
| **m2pthreads** | Threads, ThreadsBridge | (none) | Pthreads wrapper (M2+) |
| **m2regex** | Regex, RegexBridge | (none) | POSIX regex via `regex.h` (no external lib) |
| **m2rpc** | RpcClient, RpcCodec, RpcErrors, RpcFrame, RpcServer | m2bytes, m2sockets, m2stream | RPC framework |
| **m2sockets** | Sockets, SocketsBridge | m2sys | BSD sockets |
| **m2sqlite** | SQLite, SQLiteBridge | (none) | SQLite3; needs `-lsqlite3` |
| **m2stream** | Stream | m2sockets, m2tls | Transport-agnostic streams |
| **m2sys** | Sys | (none) | C shim: file I/O, exec, SHA256, paths, tar, time |
| **m2text** | Text | (none) | Text processing |
| **m2tls** | TLS, TlsBridge | m2sockets | TLS via OpenSSL; needs `-lssl -lcrypto` |
| **m2tok** | Tokenizer | (none) | Lexical tokenizer |
| **m2ws** | WebSocket, WsBridge, WsFrame | m2sockets, m2stream, m2bytes | WebSocket protocol |
| **m2zlib** | Zlib, ZlibBridge | (none) | Zlib; needs `-lz` |

---

## Module Resolution Algorithm

When the compiler sees `FROM Foo IMPORT ...`, it searches for `Foo.def` in this order:

1. **Same directory** as the source file being compiled (3 case variants: `Foo.def`, `Foo.DEF`, `foo.def`)
2. **Include paths** from m2.toml `includes=` and transitive dependencies, in order (same 3 case variants each)
3. **Global install prefix**: `~/.mx/lib/*/src/Foo.def` (override with `MX_HOME` env var)

The first match wins. Standard library modules are resolved before any file search.

---

## Project Manifest (m2.toml)

### Minimal Example

```toml
name=hello
version=1.0.0
entry=src/Main.mod
includes=src
```

### Full Example

```toml
name=myapp
version=0.2.0
edition=pim4
entry=src/Main.mod
includes=src lib
m2plus=true

[deps]
m2bytes
m2cli
m2sys
locallib=path:../locallib

[cc]
cflags=-I/opt/homebrew/include
ldflags=-L/opt/homebrew/lib
libs=-lssl -lcrypto
extra-c=src/bridge.c
frameworks=CoreFoundation Security

[test]
entry=tests/Main.mod
includes=tests

[features]
use_tls
debug_logging
```

### Dependency Formats

```toml
[deps]
m2bytes                      # Bare name: resolved from ~/.mx/lib/m2bytes/
locallib=path:../locallib    # Local path: relative to m2.toml
remotelib=0.2.0              # Registry: fetched by mxpkg
```

### [cc] Section

All fields are optional. Values are transitive -- a library's `[cc]` settings propagate to anything that depends on it.

| Field | Purpose | Example |
|-------|---------|---------|
| `cflags` | Passed to cc during compilation | `-I/opt/homebrew/include` |
| `ldflags` | Passed to cc during linking | `-L/opt/homebrew/lib` |
| `libs` | Libraries to link | `-lssl -lcrypto` |
| `extra-c` | Additional C source files (paths relative to m2.toml) | `src/bridge.c` |
| `frameworks` | macOS frameworks | `CoreFoundation Security` |

---

## Common Mistakes

### Importing a library name instead of a module name

```modula-2
(* WRONG *)
FROM m2bytes IMPORT ByteBuf;

(* RIGHT *)
FROM ByteBuf IMPORT Buf, Init, Free;
```

### Forgetting to declare a dependency

If your code uses `FROM ByteBuf IMPORT ...`, your `m2.toml` needs:

```toml
[deps]
m2bytes
```

### Importing something that does not exist

The compiler will report "definition file not found" if you import a nonexistent module. Check the tables above before writing imports.

### Using SYSTEM exports without importing SYSTEM

```modula-2
(* WRONG -- ADR not in scope *)
ptr := ADR(buf);

(* RIGHT *)
FROM SYSTEM IMPORT ADR;
ptr := ADR(buf);
```

### Confusing ADDRESS and POINTER TO

`ADDRESS` (from SYSTEM) is a raw `void*`. `POINTER TO T` is a typed pointer. They are not interchangeable without explicit casting.
