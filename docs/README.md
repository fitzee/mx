# Documentation

## User guides

- [Language support](language-support.md) — PIM4 and Modula-2+ feature matrix
- [Using the toolchain](toolchain.md) — compiler flags, project builds, debugging, environment variables
- [VS Code integration](vscode.md) — extension setup, settings, commands, troubleshooting
- [Lint warnings](lint.md) — W01–W11 static analysis checks, suppression pragmas, architecture
- [FAQ](faq.md) — common questions and answers

## Libraries

34 libraries ship with the toolchain. The standard library is compiled into the compiler; extension libraries are added via `m2.toml` deps or `-I` paths.

### Standard Library

- [m2stdlib](libs/m2stdlib/) — PIM4 + ISO standard library: InOut, Strings, Storage, MathLib, FileSystem, BinaryIO, Args, Terminal, RealInOut, STextIO, SWholeIO, SRealIO, SLongIO, SYSTEM

### Graphics

- [m2gfx](libs/m2gfx/) — SDL2-based graphics: windowing, 2D drawing, textures, fonts, pixel buffers, events

### Networking

- [m2sockets](libs/m2sockets/) — POSIX/BSD sockets: TCP/UDP with blocking and non-blocking I/O
- [m2tls](libs/m2tls/) — TLS transport layer wrapping OpenSSL/LibreSSL
- [m2stream](libs/m2stream/) — transport-agnostic byte streams: unifies TCP and TLS behind sync/async I/O
- [m2ws](libs/m2ws/WebSocket.md) — WebSocket client (RFC 6455): text, binary, ping/pong frames over m2stream

### HTTP

- [m2http](libs/m2http/) — HTTP client: URI parsing, request/response, async I/O via m2stream
- [m2http2](libs/m2http2/) — HTTP/2 framing and HPACK header compression, stream FSM, settings negotiation
- [m2http2server](libs/m2http2server/) — HTTP/2 server: routing, middleware, TLS, connection and stream management

### Services & Security

- [m2rpc](libs/m2rpc/) — length-prefixed RPC framing over abstract byte transports
- [m2auth](libs/m2auth/) — authentication and authorization: JWT HS256, Ed25519 PASETO, policy engine, replay detection
- [m2oidc](libs/m2oidc/Oidc.md) — OpenID Connect: OIDC discovery, JWKS key sets, RS256 JWT verification

### Async & Concurrency

- [m2futures](libs/m2futures/) — promises/futures for single-threaded async: chaining, combinators (All, Race)
- [m2evloop](libs/m2evloop/) — single-threaded event loop with I/O watchers and timers
- [m2pthreads](libs/m2pthreads/Threads.md) — pthreads wrapper for M2+ concurrency: threads, mutexes, conditions

### Database

- [m2sqlite](libs/m2sqlite/SQLite.md) — SQLite3 interface: prepared statements, caller-provided buffers
- [m2lmdb](libs/m2lmdb/Lmdb.md) — LMDB key/value store: MVCC concurrency, zero-copy reads via memory-mapped B+ trees

### Data Formats

- [m2json](libs/m2json/Json.md) — SAX-style JSON tokenizer: zero-allocation streaming parser
- [m2fmt](libs/m2fmt/Fmt.md) — output formatting: mini JSON writer, CSV encoder, text table renderer
- [m2conf](libs/m2conf/Conf.md) — INI-style config file parser: `[section]` / `key=value` / `# comment`

### Core

- [m2bytes](libs/m2bytes/) — byte buffers and binary codec: growable Buf, zero-copy views, LE/BE/varint, hex
- [m2log](libs/m2log/) — structured logging: multiple sinks (console, memory, file, stream), no heap alloc in log path
- [m2alloc](libs/m2alloc/) — memory allocation: Arena (bump), Pool (fixed-block), no OS/C dependencies
- [m2hash](libs/m2hash/) — static hash table: open-addressing with FNV-1a hashing and linear probing
- [m2fsm](libs/m2fsm/) — table-driven finite state machine: O(1) transitions, guards, actions, entry/exit hooks, trace
- [m2sys](libs/m2sys/Sys.md) — C runtime shim: file I/O, process exec, SHA-256, tar, path utilities

### Utilities

- [m2cli](libs/m2cli/CLI.md) — CLI argument parser: flags, options, positional arguments
- [m2path](libs/m2path/Path.md) — path string manipulation: normalize, split, join, match, relative paths
- [m2glob](libs/m2glob/Glob.md) — gitignore-grade glob pattern matching: `*`, `?`, `[abc]`, `**`
- [m2regex](libs/m2regex/Regex.md) — POSIX regex matching via system `regex.h`
- [m2text](libs/m2text/Text.md) — text analysis: UTF-8 validation, encoding detection, line endings, text-vs-binary
- [m2tok](libs/m2tok/) — language-agnostic source tokenizer: strips strings/comments, yields identifiers
- [m2zlib](libs/m2zlib/Zlib.md) — zlib compression/decompression: raw, zlib, and gzip formats
- [m2metrics](libs/m2metrics/Metrics.md) — system metrics: load average, memory, CPU time, process RSS

## Tools

- [m2dap](toolchain.md#m2dap-debug-adapter) — Debug Adapter Protocol server for M2-idiomatic IDE debugging
- [mxpkg](mxpkg.md) — package manager: commands, manifest format, lockfile, dependency resolution

## Reference

- [Language reference](lang/) — keywords, types, builtins, stdlib, Modula-2+ extensions
- [LSP capabilities](lsp.md) — supported features, configuration, known limitations
- [Build plan schema](mxpkg-build-plan.md) — JSON build plan for `mx compile --plan`

## Project

- [Versioning policy](versioning.md) — semver rules, library graduation, release manifests

## Contributor guides

- [Architecture](architecture.md) — compiler pipeline, LSP internals, testing strategy
- [LSP invariants](lsp-invariants.md) — formal guarantees of the indexing model
