# Http2ServerConn

Per-connection HTTP/2 driver. Manages the TLS handshake, client preface validation, HTTP/2 frame parsing, HPACK decoding, settings negotiation, stream dispatch, and connection-level flow control.

## Why a Separate Connection Module?

Each accepted TCP connection runs an independent HTTP/2 session with its own HPACK dynamic tables, settings, flow control windows, and stream slots. Isolating this state in `ConnRec` means the top-level server only tracks a pool of connection pointers, and each connection drives itself through an event loop watcher callback (`ConnOnEvent`).

## Connection Phase Diagram

```
CpPreface ──preface OK──▸ CpSettings ──SETTINGS ACK──▸ CpOpen
                                                          │
                                          Drain/error ────┤
                                                          ▼
                                                      CpGoaway ──all streams done──▸ CpClosed
```

| Phase | Value | Description |
|-------|-------|-------------|
| `CpPreface` | 0 | Waiting for the 24-byte client connection preface |
| `CpSettings` | 1 | Preface received; waiting for client SETTINGS frame |
| `CpOpen` | 2 | Operational -- processing HEADERS, DATA, and control frames |
| `CpGoaway` | 3 | GOAWAY sent, draining in-flight streams |
| `CpClosed` | 4 | Connection closed, resources pending cleanup |

### Phase Transitions

1. **Accept → CpPreface**: `ConnCreate` allocates the connection, starts TLS handshake via m2Stream.
2. **CpPreface → CpSettings**: After TLS completes and ALPN confirms `h2`, the server sends its own SETTINGS and waits for the client preface (24 magic bytes).
3. **CpSettings → CpOpen**: Client sends a SETTINGS frame; server sends SETTINGS ACK. Normal frame processing begins.
4. **CpOpen → CpGoaway**: `ConnDrain` sends GOAWAY with `NO_ERROR` and the last processed stream ID. No new streams are accepted.
5. **CpGoaway → CpClosed**: All active streams finish (or drain timeout expires). Connection is closed.

## Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `CpPreface` | 0 | Phase: waiting for client preface |
| `CpSettings` | 1 | Phase: waiting for client SETTINGS |
| `CpOpen` | 2 | Phase: operational |
| `CpGoaway` | 3 | Phase: GOAWAY sent, draining |
| `CpClosed` | 4 | Phase: done |
| `ArenaSize` | 32768 | 32 KB per-connection scratch arena |

## Types

### ConnRec

```modula2
TYPE ConnRec = RECORD
  id:              CARDINAL;       (* connection ID *)
  server:          ADDRESS;        (* back-pointer to ServerRec *)
  fd:              INTEGER;        (* socket file descriptor *)
  phase:           CARDINAL;       (* CpPreface..CpClosed *)
  tlsSess:         ADDRESS;        (* TLS session handle *)
  tlsCtx:          ADDRESS;        (* per-conn TLS context *)
  strm:            ADDRESS;        (* m2Stream handle *)
  localSettings:   Settings;       (* our SETTINGS *)
  remoteSettings:  Settings;       (* peer's SETTINGS *)
  connSendWindow:  INTEGER;        (* connection-level send window *)
  connRecvWindow:  INTEGER;        (* connection-level receive window *)
  lastPeerStream:  CARDINAL;       (* highest stream ID from peer *)
  goawaySent:      BOOLEAN;
  goawayRecvd:     BOOLEAN;
  prefaceRecvd:    BOOLEAN;
  settingsAckRecvd: BOOLEAN;
  numActive:       CARDINAL;       (* active stream count *)
  dynEnc:          DynTable;       (* HPACK encoder table *)
  dynDec:          DynTable;       (* HPACK decoder table *)
  streamTable:     StreamTransTable; (* shared stream FSM table *)
  slots:           ARRAY [0..MaxStreamSlots-1] OF StreamSlot;
  readBuf:         Buf;            (* incoming data buffer *)
  writeBuf:        Buf;            (* outgoing data buffer *)
  writeOff:        CARDINAL;       (* flush offset into writeBuf *)
  haveHeader:      BOOLEAN;        (* frame header parsed? *)
  curHeader:       FrameHeader;    (* current frame header *)
  arenaBase:       ADDRESS;        (* arena backing memory *)
  arena:           Arena;          (* per-connection scratch *)
  idleTimerId:     INTEGER;        (* idle timeout timer *)
  hsTimerId:       INTEGER;        (* handshake timeout timer *)
  remoteAddr:      ARRAY [0..63] OF CHAR;
  startTick:       INTEGER;
  lastActive:      INTEGER;
  watching:        BOOLEAN;        (* registered with event loop? *)
END;

ConnPtr = POINTER TO ConnRec;
```

### Key ConnRec Fields

- `dynEnc` / `dynDec` -- HPACK dynamic tables (per-connection, not per-stream)
- `streamTable` -- shared FSM transition table for all streams on this connection
- `slots` -- fixed-size array of stream slots; `AllocSlot`/`FindSlot` manage them
- `readBuf` / `writeBuf` -- I/O buffers; raw TLS bytes flow through m2Stream
- `arena` -- bump allocator for per-request scratch (reset between requests)
- `idleTimerId` / `hsTimerId` -- event loop timers for timeout enforcement

## Frame Processing Pipeline

When `ConnOnEvent` fires (I/O readiness from the event loop):

1. Read bytes from m2Stream into `readBuf`
2. If `phase = CpPreface`: validate 24-byte client preface, advance to `CpSettings`
3. Parse 9-byte frame headers from `readBuf`
4. Dispatch by frame type:
   - **SETTINGS**: apply peer settings, send ACK, advance phase if needed
   - **PING**: echo with ACK flag
   - **GOAWAY**: record, begin closing
   - **WINDOW_UPDATE**: adjust connection or stream send window, flush pending DATA
   - **RST_STREAM**: reset the target stream slot
   - **HEADERS**: HPACK decode, allocate stream slot, assemble pseudo-headers
   - **CONTINUATION**: append to in-progress HEADERS block
   - **DATA**: accumulate into request body
5. On `END_STREAM`: dispatch assembled request through middleware chain and router
6. Encode response as HEADERS + DATA frames into `writeBuf`
7. Flush `writeBuf` through m2Stream

## Procedures

### ConnCreate

```modula2
PROCEDURE ConnCreate(serverPtr: ADDRESS; connId: CARDINAL;
                     clientFd: INTEGER; peer: SockAddr;
                     VAR cp: ConnPtr): BOOLEAN;
```

Allocate and initialise a connection from an accepted socket. Initiates the TLS handshake asynchronously via m2Stream.

- `serverPtr` -- back-pointer to the `ServerRec` (for accessing router, middleware, metrics, logger)
- `connId` -- unique connection ID
- `clientFd` -- accepted socket file descriptor
- `peer` -- remote address
- `cp` -- on success, receives a pointer to the allocated `ConnRec`

Returns `TRUE` on success, `FALSE` on allocation failure.

### ConnOnEvent

```modula2
PROCEDURE ConnOnEvent(fd, events: INTEGER; user: ADDRESS);
```

Event loop watcher callback. Called when the connection's file descriptor has I/O readiness. `user` is the `ConnPtr` cast to `ADDRESS`.

Drives the frame processing pipeline described above. Not called directly by application code.

### ConnDrain

```modula2
PROCEDURE ConnDrain(cp: ConnPtr);
```

Initiate graceful shutdown for this connection. Sends a GOAWAY frame with `NO_ERROR` and the last processed stream ID. Moves phase to `CpGoaway`. Active streams continue to completion; no new streams are accepted.

### ConnClose

```modula2
PROCEDURE ConnClose(cp: ConnPtr);
```

Close the connection and free all resources: TLS session, m2Stream handle, stream slots, arena, I/O buffers, and event loop watcher. Moves phase to `CpClosed`.

### ConnFlush

```modula2
PROCEDURE ConnFlush(cp: ConnPtr);
```

Flush pending data in `writeBuf` through m2Stream. Called after encoding response frames or when WINDOW_UPDATE restores flow control budget.

### ConnFeedBytes

```modula2
PROCEDURE ConnFeedBytes(cp: ConnPtr; data: ADDRESS; len: CARDINAL);
```

Append raw bytes directly to the connection's `readBuf`, bypassing m2Stream and TLS. Used for deterministic testing with `ConnCreateTest`.

### ConnCreateTest

```modula2
PROCEDURE ConnCreateTest(serverPtr: ADDRESS; connId: CARDINAL;
                         VAR cp: ConnPtr): BOOLEAN;
```

Create an in-memory test connection with no TLS or m2Stream. Use with `ConnFeedBytes` and `Http2ServerTestUtil` frame builders for unit testing the H2 protocol logic.
