# Documentation

## User guides

- [Language support](language-support.md) — PIM4 and Modula-2+ feature matrix
- [Using the toolchain](toolchain.md) — compiler commands, package manager, project workflows
- [VS Code integration](vscode.md) — extension setup, settings, commands, troubleshooting
- [FAQ](faq.md) — common questions and answers

## Libraries

### Graphics

- [m2gfx](libs/m2gfx/) — SDL2-based graphics: windowing, 2D drawing, textures, fonts, pixel buffers, events

### Transport

- [m2sockets](libs/m2sockets/) — POSIX/BSD sockets: TCP/UDP networking with blocking and non-blocking I/O
- [m2tls](libs/m2tls/) — TLS transport layer wrapping OpenSSL/LibreSSL
- [m2stream](libs/m2stream/) — transport-agnostic byte streams: unifies TCP and TLS behind sync/async I/O

### HTTP

- [m2http](libs/m2http/) — HTTP client: URI parsing, request/response, async I/O via m2stream
- [m2http2](libs/m2http2/) — HTTP/2 framing and HPACK header compression, stream FSM, settings negotiation
- [m2http2server](libs/m2http2server/) — HTTP/2 server: routing, middleware, TLS, connection and stream management

### Services

- [m2rpc](libs/m2rpc/) — length-prefixed RPC framing over abstract byte transports
- [m2auth](libs/m2auth/) — authentication and authorization: JWT HS256, Ed25519 PASETO, policy engine, replay detection

### Async

- [m2futures](libs/m2futures/) — promises/futures for single-threaded async: chaining, combinators (All, Race)
- [m2evloop](libs/m2evloop/) — single-threaded event loop with I/O watchers and timers

### Helpers

- [m2log](libs/m2log/) — structured logging: multiple sinks (console, memory, file, stream), no heap alloc in log path
- [m2bytes](libs/m2bytes/) — byte buffers and binary codec: growable Buf, zero-copy views, LE/BE/varint readers/writers, hex
- [m2alloc](libs/m2alloc/) — memory allocation: Arena (bump), Pool (fixed-block), no OS/C dependencies
- [m2fsm](libs/m2fsm/) — table-driven finite state machine: O(1) transitions, guards, actions, entry/exit hooks, trace

## Reference

- [Language reference](lang/) — keywords, types, builtins, stdlib, Modula-2+ extensions
- [LSP capabilities](lsp.md) — supported features, configuration, known limitations
- [LSP invariants](lsp-invariants.md) — formal guarantees of the indexing model
- [m2pkg package manager](m2pkg.md) — commands, manifest format, lockfile
- [Build plan schema](m2pkg-build-plan.md) — JSON build plan for `m2c compile --plan`

## Contributor guides

- [Architecture](architecture.md) — compiler pipeline, LSP internals, testing strategy
