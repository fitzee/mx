# Http2ServerStream

Per-stream request assembly and response sending. Operates on `StreamSlot` records within a connection's fixed-size slot array.

## Why a Separate Stream Module?

Each HTTP/2 stream within a connection follows its own lifecycle: headers arrive, optional body data accumulates, the request is dispatched to a handler, and the response is encoded back as HEADERS + DATA frames. Separating this from the connection driver keeps `Http2ServerConn` focused on framing and connection-level concerns while `Http2ServerStream` handles request/response semantics.

## Stream Phase Diagram

```
PhIdle ──HEADERS──▸ PhHeaders ──END_STREAM──▸ PhDispatched
                        │                          │
                        │ (has body)               │
                        ▼                          ▼
                    PhData ──END_STREAM──▸ PhDispatched ──handler done──▸ PhResponding ──sent──▸ PhDone
```

| Phase | Value | Description |
|-------|-------|-------------|
| `PhIdle` | 0 | Slot allocated, no frames received yet |
| `PhHeaders` | 1 | HEADERS frame received; pseudo-headers extracted |
| `PhData` | 2 | Receiving DATA frames (request body accumulation) |
| `PhDispatched` | 3 | END_STREAM received; request dispatched to handler |
| `PhResponding` | 4 | Handler has filled the response; encoding in progress |
| `PhDone` | 5 | Response fully sent; slot ready for reuse |

### Phase Transitions

1. **PhIdle → PhHeaders**: `AssembleHeaders` extracts `:method`, `:path`, `:scheme`, `:authority` pseudo-headers and copies regular headers into the request.
2. **PhHeaders → PhData**: DATA frame arrives without END_STREAM; body accumulation begins.
3. **PhHeaders → PhDispatched**: HEADERS had END_STREAM (no body); request is complete.
4. **PhData → PhDispatched**: DATA frame with END_STREAM; body is complete.
5. **PhDispatched → PhResponding**: Connection driver runs the middleware chain and handler.
6. **PhResponding → PhDone**: `SendResponse` encodes the full response (or `FlushData` sends remaining DATA after WINDOW_UPDATE).

## Flow Control

HTTP/2 has two levels of flow control, both enforced during response sending:

- **Connection window** (`connSendWindow` in `ConnRec`): shared across all streams. `SendResponse` and `FlushData` take a `VAR connWindow` parameter and decrement it as DATA frames are sent.
- **Per-stream window** (`H2Stream` inside `StreamSlot`): tracked by the m2Http2 stream FSM. Each stream starts with `initialWindowSize` from SETTINGS.

If the window is exhausted mid-body, `SendResponse` returns early with unsent data remaining in `resp.body`. The connection driver calls `FlushData` after receiving a WINDOW_UPDATE frame that replenishes the budget.

DATA frames are split to respect `maxFrameSize` from the peer's SETTINGS (default 16384 bytes).

## Types

### StreamSlot

```modula2
TYPE StreamSlot = RECORD
  active:     BOOLEAN;     (* slot is in use *)
  stream:     H2Stream;    (* m2Http2 stream FSM handle *)
  req:        Request;     (* assembled request *)
  phase:      CARDINAL;    (* PhIdle..PhDone *)
  endRecvd:   BOOLEAN;     (* END_STREAM received from client *)
  endSent:    BOOLEAN;     (* END_STREAM sent in response *)
END;
```

- `active` -- `TRUE` when the slot is allocated to a live stream
- `stream` -- the m2Http2 per-stream FSM that tracks RFC 9113 stream states and flow control
- `req` -- the request being assembled from HEADERS and DATA frames
- `phase` -- current phase in the stream lifecycle (see diagram above)
- `endRecvd` -- set when the client sends END_STREAM (request is complete)
- `endSent` -- set when the server sends END_STREAM (response is complete)

## Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `PhIdle` | 0 | Slot idle |
| `PhHeaders` | 1 | Headers received |
| `PhData` | 2 | Receiving body data |
| `PhDispatched` | 3 | Request dispatched to handler |
| `PhResponding` | 4 | Encoding response |
| `PhDone` | 5 | Response sent, slot reusable |

## Procedures

### SlotInit

```modula2
PROCEDURE SlotInit(VAR slot: StreamSlot);
```

Initialise a slot to idle state. Sets `active := FALSE`, `phase := PhIdle`, clears flags.

### AssembleHeaders

```modula2
PROCEDURE AssembleHeaders(VAR slot: StreamSlot;
                          VAR decoded: ARRAY OF HeaderEntry;
                          numDecoded: CARDINAL;
                          endStream: BOOLEAN): BOOLEAN;
```

Extract pseudo-headers and regular headers from HPACK-decoded entries into the slot's request.

- `slot` -- target stream slot
- `decoded` / `numDecoded` -- array of decoded header entries from `Http2Hpack`
- `endStream` -- `TRUE` if the HEADERS frame had the END_STREAM flag

Pseudo-header extraction:
- `:method` → `req.method`
- `:path` → `req.path`
- `:scheme` → `req.scheme`
- `:authority` → `req.authority`

Regular headers are copied into `req.headers[0..numHeaders-1]`.

Returns `FALSE` if required pseudo-headers (`:method`, `:path`) are missing.

Advances phase from `PhIdle` to `PhHeaders`, or to `PhDispatched` if `endStream` is `TRUE`.

### AccumulateData

```modula2
PROCEDURE AccumulateData(VAR slot: StreamSlot;
                         data: BytesView;
                         endStream: BOOLEAN): BOOLEAN;
```

Append DATA frame payload to the request body.

- `slot` -- target stream slot (must be in `PhHeaders` or `PhData`)
- `data` -- payload bytes (zero-copy view)
- `endStream` -- `TRUE` if this DATA frame had END_STREAM

Appends `data` to `req.body` and increments `req.bodyLen`.

Returns `FALSE` if the slot is in the wrong phase or body exceeds limits.

Advances phase to `PhData` (if not already), or to `PhDispatched` if `endStream` is `TRUE`.

### SendResponse

```modula2
PROCEDURE SendResponse(VAR slot: StreamSlot;
                       VAR resp: Response;
                       VAR dynEnc: DynTable;
                       VAR outBuf: Buf;
                       maxFrameSize: CARDINAL;
                       VAR connWindow: INTEGER): CARDINAL;
```

Encode the response as HEADERS + DATA frames into the output buffer.

- `slot` -- stream slot (phase becomes `PhResponding`)
- `resp` -- filled-in response from the handler
- `dynEnc` -- connection's HPACK encoder dynamic table
- `outBuf` -- output buffer to append frames to
- `maxFrameSize` -- peer's maximum frame size from SETTINGS
- `connWindow` -- connection-level send window (decremented as DATA is written)

Returns the total number of bytes appended to `outBuf`.

The HEADERS frame encodes `:status` and any response headers via HPACK. DATA frames are split into chunks of at most `maxFrameSize` bytes. If the connection or stream window is exhausted, the function returns early; call `FlushData` after WINDOW_UPDATE.

### FlushData

```modula2
PROCEDURE FlushData(VAR slot: StreamSlot;
                    VAR resp: Response;
                    VAR outBuf: Buf;
                    maxFrameSize: CARDINAL;
                    VAR connWindow: INTEGER): CARDINAL;
```

Flush remaining buffered response DATA for this stream. Called after a WINDOW_UPDATE frame restores flow control budget.

Parameters and return value are the same as `SendResponse`. Picks up where `SendResponse` left off in `resp.body`.

### AllocSlot

```modula2
PROCEDURE AllocSlot(VAR slots: ARRAY OF StreamSlot;
                    streamId: CARDINAL;
                    initWindowSize: CARDINAL;
                    tablePtr: ADDRESS): CARDINAL;
```

Find an unused slot and initialise it for the given stream ID.

- `slots` -- the connection's slot array
- `streamId` -- HTTP/2 stream identifier (odd numbers for client-initiated)
- `initWindowSize` -- initial window size from SETTINGS
- `tablePtr` -- pointer to the shared `StreamTransTable` for the stream FSM

Returns the slot index (0..MaxStreamSlots-1), or `MaxStreamSlots` if all slots are in use (pool exhausted).

### FindSlot

```modula2
PROCEDURE FindSlot(VAR slots: ARRAY OF StreamSlot;
                   streamId: CARDINAL): CARDINAL;
```

Find the slot currently assigned to `streamId`.

Returns the slot index, or `MaxStreamSlots` if not found.

### SlotFree

```modula2
PROCEDURE SlotFree(VAR slot: StreamSlot);
```

Release a slot after the response is complete. Sets `active := FALSE`, frees the request body buffer, resets phase to `PhIdle`.
