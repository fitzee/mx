# m2http2server — HTTP/2 Server Library

Pure Modula-2 HTTP/2 server built entirely on the m2 ecosystem libraries.

## Architecture

```
┌─────────────────────────────────────────────────┐
│                  Http2Server                     │
│  (lifecycle, accept loop, TLS context)          │
├─────────────────┬───────────────────────────────┤
│  Http2Router    │  Http2Middleware               │
│  (dispatch)     │  (chain)                       │
├─────────────────┴───────────────────────────────┤
│              Http2ServerConn                      │
│  (per-connection: preface, frames, HPACK, FSM)  │
├─────────────────────────────────────────────────┤
│              Http2ServerStream                    │
│  (per-stream: headers, data, response)          │
├─────────────────────────────────────────────────┤
│  Http2ServerTypes  │  Metrics  │  Log  │  Test  │
└────────────────────┴──────────┴───────┴────────┘
```

## Library Reuse

| Ecosystem library | Role in server |
|---|---|
| **m2Http2** | Frame encode/decode, HPACK, stream FSM |
| **m2Stream** | Transport abstraction (TCP + TLS) |
| **m2TLS** | Server-side TLS context, ALPN "h2" |
| **m2Sockets** | Socket create, bind, listen, accept |
| **m2EvLoop** | Event loop, fd watchers, timers |
| **m2Futures** | Scheduler for async microtasks |
| **m2Bytes** | ByteBuf for I/O buffers, BytesView |
| **m2Alloc** | Arena for per-connection scratch |
| **m2Fsm** | Stream lifecycle FSM |
| **m2Log** | Structured logging |

## Request Lifecycle

```
Accept → TLS handshake → ALPN check ("h2")
  → Client preface (24 bytes)
  → Client SETTINGS → Server SETTINGS + ACK
  → HEADERS frame → HPACK decode → pseudo-headers
  → [DATA frames → body accumulation]
  → Router dispatch → Middleware chain → Handler
  → Response HEADERS + DATA → HPACK encode → frames
```

## Memory Model

- **Per-connection Arena** (32KB): scratch space for HPACK decode
- **ByteBuf**: growable buffers for request bodies and I/O
- **Fixed arrays**: stream slots, route table, middleware chain
- **HPACK DynTable**: ~541KB per table (128 entries × 4232 bytes)

## Configuration

```modula2
ServerOpts = RECORD
  port:           CARDINAL;     (* listen port, default 8443 *)
  certPath:       ARRAY OF CHAR; (* PEM certificate *)
  keyPath:        ARRAY OF CHAR; (* PEM private key *)
  maxConns:       CARDINAL;     (* max connections, default 16 *)
  maxStreams:      CARDINAL;     (* max streams/conn, default 32 *)
  idleTimeoutMs:  INTEGER;      (* idle timeout, default 30s *)
  hsTimeoutMs:    INTEGER;      (* handshake timeout, default 5s *)
  drainTimeoutMs: INTEGER;      (* drain timeout, default 10s *)
END;
```

## Limitations

- No CONTINUATION frame support (requires END_HEADERS on HEADERS)
- No server push (PUSH_PROMISE)
- No HPACK Huffman encoding
- Max 16 concurrent connections (DynTable memory)
- Max 32 concurrent streams per connection
- Exact-match routing only (no patterns/wildcards)
- Single-threaded (event-loop model)

## Modules

| Module | Purpose |
|---|---|
| Http2ServerTypes | Shared types, constants, Status enum |
| Http2Server | Top-level server lifecycle |
| Http2ServerConn | Per-connection H2 driver |
| Http2ServerStream | Per-stream request/response |
| Http2Router | Method+path dispatch |
| Http2Middleware | Pre-handler chain |
| Http2ServerMetrics | Observability counters |
| Http2ServerLog | Structured logging adapter |
| Http2ServerTestUtil | Deterministic test harness |
