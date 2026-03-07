# m2http2

## Why

Low-level HTTP/2 protocol library implementing RFC 7540/9113 framing, HPACK header compression (RFC 7541), and connection/stream state machines. No heap allocation -- all state lives in caller-provided records and fixed-size arrays.

## Modules

| Module | Purpose |
|--------|---------|
| Http2Types | Wire constants, frame/settings/header types, stream and connection FSM states/events |
| Http2Frame | Frame encoding and decoding (9-byte header, SETTINGS, PING, GOAWAY, WINDOW_UPDATE, RST_STREAM, DATA, HEADERS) |
| Http2Hpack | HPACK integer codec, static/dynamic table, header block encode/decode, Huffman codec |
| Http2Stream | Per-stream FSM (RFC 7540 Section 5.1) and per-stream flow control |
| Http2Conn | Connection-level FSM, settings negotiation, stream management, frame dispatch |
| Http2TestUtil | ByteBuf-based frame builders and readers for deterministic testing without network I/O |

## Types

### Http2Types

- **FrameHeader** -- Decoded 9-byte frame header: length (24-bit), ftype, flags, streamId (31-bit).
- **Settings** -- The six standard HTTP/2 settings: headerTableSize, enablePush, maxConcurrentStreams, initialWindowSize, maxFrameSize, maxHeaderListSize.
- **HeaderName** -- `ARRAY [0..127] OF CHAR` for header field names.
- **HeaderValue** -- `ARRAY [0..4095] OF CHAR` for header field values.
- **HeaderEntry** -- Name/value pair with explicit lengths (nameLen, valLen).

### Http2Hpack

- **DynTable** -- Ring-buffer dynamic table with FIFO eviction. 128 entry slots, tracks byteSize and maxSize.

### Http2Stream

- **H2Stream** -- Per-stream record: id, FSM handle, send/recv windows, RST error code.
- **StreamTransTable** -- Shared transition table (7 states x 9 events = 63 entries).

### Http2Conn

- **H2Conn** -- Full connection state: local/remote settings, connection-level flow control, stream ID allocation, up to 64 concurrent streams, HPACK encoder/decoder dynamic tables, output buffer.

## Constants

### Frame Types (Section 6)

| Constant | Value | Description |
|----------|-------|-------------|
| FrameData | 0 | DATA frame |
| FrameHeaders | 1 | HEADERS frame |
| FramePriority | 2 | PRIORITY frame |
| FrameRstStream | 3 | RST_STREAM frame |
| FrameSettings | 4 | SETTINGS frame |
| FramePushPromise | 5 | PUSH_PROMISE frame |
| FramePing | 6 | PING frame |
| FrameGoaway | 7 | GOAWAY frame |
| FrameWindowUpdate | 8 | WINDOW_UPDATE frame |
| FrameContinuation | 9 | CONTINUATION frame |

### Frame Flags

| Constant | Value | Description |
|----------|-------|-------------|
| FlagEndStream | 1 | END_STREAM (0x01) |
| FlagAck | 1 | ACK for SETTINGS/PING (0x01) |
| FlagEndHeaders | 4 | END_HEADERS (0x04) |
| FlagPadded | 8 | PADDED (0x08) |
| FlagPriority | 32 | PRIORITY (0x20) |

### Error Codes (Section 7)

ErrNoError (0), ErrProtocol (1), ErrInternal (2), ErrFlowControl (3), ErrSettingsTimeout (4), ErrStreamClosed (5), ErrFrameSize (6), ErrRefusedStream (7), ErrCancel (8), ErrCompression (9), ErrConnect (10), ErrEnhanceYourCalm (11), ErrInadequateSecurity (12), ErrHttp11Required (13).

### Settings Defaults

| Setting | Default |
|---------|---------|
| HeaderTableSize | 4096 |
| EnablePush | 1 |
| MaxConcurrentStreams | 4294967295 (unlimited) |
| InitialWindowSize | 65535 |
| MaxFrameSize | 16384 |
| MaxHeaderListSize | 4294967295 (unlimited) |

### Stream FSM States

StIdle (0), StReservedLocal (1), StReservedRemote (2), StOpen (3), StHalfClosedLocal (4), StHalfClosedRemote (5), StClosed (6).

### Connection FSM States

ConnIdle (0), ConnWaitPreface (1), ConnWaitSettings (2), ConnOpen (3), ConnGoingAway (4), ConnClosed (5).

## Procedures

### Http2Types

- `InitDefaultSettings(VAR s: Settings)` -- Fill a Settings record with RFC defaults.

### Http2Frame

- `DecodeHeader(v: BytesView; VAR hdr: FrameHeader; VAR ok: BOOLEAN)` -- Decode a 9-byte frame header from a BytesView.
- `EncodeHeader(VAR b: Buf; hdr: FrameHeader)` -- Encode a 9-byte frame header into a Buf.
- `WritePreface(VAR b: Buf)` -- Write the 24-byte HTTP/2 client connection preface.
- `CheckPreface(v: BytesView): BOOLEAN` -- Check if the first 24 bytes match the connection preface.
- `DecodeSettings(payload: BytesView; VAR s: Settings; VAR ok: BOOLEAN)` -- Decode a SETTINGS frame payload (must be a multiple of 6 bytes).
- `EncodeSettings(VAR b: Buf; s: Settings)` -- Encode a full SETTINGS frame (9 + 36 = 45 bytes).
- `EncodeSettingsAck(VAR b: Buf)` -- Encode a SETTINGS ACK frame (header only, 0 payload).
- `EncodePing(VAR b: Buf; data: BytesView; isAck: BOOLEAN)` -- Encode a PING frame with 8 bytes of opaque data.
- `EncodeGoaway(VAR b: Buf; lastStreamId, errorCode: CARDINAL)` -- Encode a GOAWAY frame.
- `DecodeGoaway(payload: BytesView; VAR lastStreamId, errorCode: CARDINAL; VAR ok: BOOLEAN)` -- Decode a GOAWAY frame payload.
- `EncodeWindowUpdate(VAR b: Buf; streamId, increment: CARDINAL)` -- Encode a WINDOW_UPDATE frame.
- `DecodeWindowUpdate(payload: BytesView; VAR increment: CARDINAL; VAR ok: BOOLEAN)` -- Decode a WINDOW_UPDATE payload (4 bytes).
- `EncodeRstStream(VAR b: Buf; streamId, errorCode: CARDINAL)` -- Encode a RST_STREAM frame.
- `DecodeRstStream(payload: BytesView; VAR errorCode: CARDINAL; VAR ok: BOOLEAN)` -- Decode a RST_STREAM payload (4 bytes).
- `EncodeDataHeader(VAR b: Buf; streamId, payloadLen: CARDINAL; endStream: BOOLEAN)` -- Encode a DATA frame header; caller appends payload bytes.
- `EncodeHeadersHeader(VAR b: Buf; streamId, payloadLen: CARDINAL; endStream, endHeaders: BOOLEAN)` -- Encode a HEADERS frame header; caller appends header block.

### Http2Hpack

- `EncodeInt(VAR b: Buf; value, prefixBits, mask: CARDINAL)` -- Encode an HPACK integer with the given prefix bit width and upper-bit mask.
- `DecodeInt(firstByte, prefixBits: CARDINAL; v: BytesView; VAR pos: CARDINAL; VAR ok: BOOLEAN): CARDINAL` -- Decode an HPACK integer from a BytesView.
- `StaticLookup(index: CARDINAL; VAR entry: HeaderEntry; VAR ok: BOOLEAN)` -- Look up a static table entry by 1-based index.
- `StaticFind(name: ARRAY OF CHAR; nameLen: CARDINAL; value: ARRAY OF CHAR; valLen: CARDINAL; nameOnly: BOOLEAN): CARDINAL` -- Find a static table entry matching name (and optionally value). Returns 0 if not found.
- `DynInit(VAR dt: DynTable; maxSize: CARDINAL)` -- Initialise a dynamic table with the given max size in bytes.
- `DynInsert(VAR dt: DynTable; name: ARRAY OF CHAR; nameLen: CARDINAL; value: ARRAY OF CHAR; valLen: CARDINAL)` -- Insert a header at the front; evicts oldest entries as needed. Entry size = nameLen + valLen + 32.
- `DynLookup(VAR dt: DynTable; index: CARDINAL; VAR entry: HeaderEntry; VAR ok: BOOLEAN)` -- Look up by 0-based dynamic table index.
- `DynResize(VAR dt: DynTable; newMaxSize: CARDINAL)` -- Resize the dynamic table, evicting as needed.
- `DynCount(VAR dt: DynTable): CARDINAL` -- Number of entries in the dynamic table.
- `DecodeHeaderBlock(v: BytesView; VAR dt: DynTable; VAR headers: ARRAY OF HeaderEntry; maxOut: CARDINAL; VAR numHeaders: CARDINAL; VAR ok: BOOLEAN)` -- Decode a complete header block into an array of HeaderEntry.
- `EncodeHeaderBlock(VAR b: Buf; VAR dt: DynTable; VAR headers: ARRAY OF HeaderEntry; numHeaders: CARDINAL)` -- Encode headers into a header block using incremental indexing.
- `HuffmanDecode(src: ADDRESS; srcLen: CARDINAL; dst: ADDRESS; dstMax: CARDINAL; VAR dstLen: CARDINAL): BOOLEAN` -- Decode Huffman-encoded bytes.
- `HuffmanEncode(src: ADDRESS; srcLen: CARDINAL; dst: ADDRESS; dstMax: CARDINAL; VAR dstLen: CARDINAL): BOOLEAN` -- Encode bytes using Huffman coding.
- `HuffmanDecodedLength(src: ADDRESS; srcLen: CARDINAL): CARDINAL` -- Return the decoded length of a Huffman-encoded buffer.

### Http2Stream

- `InitStreamTable(VAR table: StreamTransTable)` -- Initialise the shared stream transition table per RFC 7540.
- `InitStream(VAR s: H2Stream; streamId, initWindowSize: CARDINAL; table: ADDRESS)` -- Initialise a stream with its ID, initial window, and shared transition table.
- `StreamStep(VAR s: H2Stream; ev: CARDINAL; VAR status: StepStatus)` -- Process a stream FSM event. Returns step status.
- `ConsumeSendWindow(VAR s: H2Stream; n: CARDINAL): BOOLEAN` -- Consume n bytes from the send window. Returns FALSE if insufficient.
- `UpdateSendWindow(VAR s: H2Stream; increment: CARDINAL)` -- Add WINDOW_UPDATE increment to the send window.
- `ConsumeRecvWindow(VAR s: H2Stream; n: CARDINAL): BOOLEAN` -- Consume n bytes from the receive window.
- `UpdateRecvWindow(VAR s: H2Stream; increment: CARDINAL)` -- Add increment to the receive window.
- `StreamState(VAR s: H2Stream): CARDINAL` -- Current FSM state.
- `IsClosed(VAR s: H2Stream): BOOLEAN` -- Check if the stream is closed.

### Http2Conn

- `InitConn(VAR c: H2Conn)` -- Initialise a connection with default settings. Allocates the output buffer.
- `FreeConn(VAR c: H2Conn)` -- Free connection resources.
- `SendPreface(VAR c: H2Conn)` -- Write the client connection preface and initial SETTINGS into outBuf.
- `ProcessFrame(VAR c: H2Conn; hdr: FrameHeader; payload: BytesView; VAR ok: BOOLEAN)` -- Process a received frame. Updates state, may append response frames to outBuf. Returns FALSE on connection error.
- `OpenStream(VAR c: H2Conn): CARDINAL` -- Allocate a new client stream. Returns stream ID, or 0 if unavailable.
- `FindStream(VAR c: H2Conn; streamId: CARDINAL): CARDINAL` -- Find stream slot index by ID. Returns MaxStreams if not found.
- `UpdateConnSendWindow(VAR c: H2Conn; increment: CARDINAL)` -- Update connection-level send window.
- `ConsumeConnSendWindow(VAR c: H2Conn; n: CARDINAL): BOOLEAN` -- Consume from connection-level send window.
- `ApplyRemoteSettings(VAR c: H2Conn; s: Settings)` -- Apply received peer settings.
- `GetOutput(VAR c: H2Conn): BytesView` -- Get a view of pending output frames.
- `ClearOutput(VAR c: H2Conn)` -- Clear the output buffer after flushing to the wire.

### Http2TestUtil

- `BuildFrame(VAR b: Buf; ftype, flags, streamId: CARDINAL; payload: BytesView)` -- Build a raw frame (header + payload).
- `BuildSettingsFrame(VAR b: Buf; s: Settings)` -- Build a SETTINGS frame.
- `BuildSettingsAckFrame(VAR b: Buf)` -- Build a SETTINGS ACK frame.
- `BuildPingFrame(VAR b: Buf; data: BytesView; isAck: BOOLEAN)` -- Build a PING frame.
- `BuildGoawayFrame(VAR b: Buf; lastStreamId, errorCode: CARDINAL)` -- Build a GOAWAY frame.
- `BuildWindowUpdateFrame(VAR b: Buf; streamId, increment: CARDINAL)` -- Build a WINDOW_UPDATE frame.
- `BuildRstStreamFrame(VAR b: Buf; streamId, errorCode: CARDINAL)` -- Build a RST_STREAM frame.
- `ReadFrameHeader(VAR v: BytesView; VAR hdr: FrameHeader; VAR ok: BOOLEAN)` -- Read and advance past a 9-byte frame header.
- `ReadFramePayload(VAR v: BytesView; hdr: FrameHeader; VAR payload: BytesView; VAR ok: BOOLEAN)` -- Extract payload bytes from a view.

## Example

```modula2
MODULE H2Example;

FROM ByteBuf IMPORT Buf, BytesView, BufInit, BufFree, BufView;
FROM Http2Types IMPORT Settings, FrameHeader;
FROM Http2Frame IMPORT DecodeHeader, EncodeSettings, EncodeSettingsAck;
FROM Http2Hpack IMPORT DynTable, DynInit, EncodeHeaderBlock, DecodeHeaderBlock;
FROM Http2Conn IMPORT H2Conn, InitConn, FreeConn, SendPreface,
                       ProcessFrame, OpenStream, GetOutput, ClearOutput;

VAR
  conn: H2Conn;
  sid:  CARDINAL;
  out:  BytesView;

BEGIN
  (* Set up client connection *)
  InitConn(conn);
  SendPreface(conn);

  (* Flush conn.outBuf to the wire (preface + SETTINGS) *)
  out := GetOutput(conn);
  (* ... send out bytes over TCP ... *)
  ClearOutput(conn);

  (* Open a new stream for a request *)
  sid := OpenStream(conn);
  IF sid # 0 THEN
    (* Encode HEADERS frame with HPACK, send DATA, etc. *)
  END;

  FreeConn(conn);
END H2Example.
```
