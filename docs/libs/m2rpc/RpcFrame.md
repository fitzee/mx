# RpcFrame

Length-prefixed framing over an abstract byte transport. Transforms a raw stream of bytes (TCP, TLS, in-memory pipe) into a sequence of discrete message frames, handling partial reads and blocking writes transparently.

## Why Framing?

Byte streams like TCP deliver an undifferentiated sequence of bytes with no inherent message boundaries. Applications need to know where one message ends and the next begins. Length-prefixed framing is the simplest solution: prefix each payload with a fixed-size big-endian length field, then read exactly that many bytes.

RpcFrame's wire format:
```
4 bytes  payload length (big-endian, 0..MaxFrame)
N bytes  payload
```

This format is self-synchronizing (after reading the 4-byte header, you know exactly how many more bytes to read) and efficient (no byte-scanning or escape sequences).

## Transport Abstraction

RpcFrame works over any byte transport via two procedure types:

- **ReadFn**: `PROCEDURE(ctx, buf, max, VAR got): status`
- **WriteFn**: `PROCEDURE(ctx, buf, len, VAR sent): status`

The `ctx` parameter is an opaque ADDRESS -- it could point to a socket descriptor, a TLS session, or a test pipe. This design keeps the framing logic decoupled from the transport layer.

Transport status codes:
- `TsOk` (0): success
- `TsWouldBlock` (1): no data available (non-blocking I/O)
- `TsClosed` (2): peer closed connection
- `TsError` (3): fatal error

## Constants

| Name | Value | Purpose |
|------|-------|---------|
| `MaxFrame` | 65531 | Maximum payload size, constrained by ByteBuf.MaxBufCap |
| `TsOk` | 0 | Transport status: operation succeeded |
| `TsWouldBlock` | 1 | Transport status: no data available |
| `TsClosed` | 2 | Transport status: connection closed |
| `TsError` | 3 | Transport status: fatal I/O error |

## Types

### ReadFn

```modula2
TYPE ReadFn = PROCEDURE(ADDRESS, ADDRESS, CARDINAL, VAR CARDINAL): CARDINAL;
```

Transport read callback. Reads up to `max` bytes into `buf`, sets `got` to the actual byte count, and returns a TsXxx status code. The first ADDRESS parameter is the transport context (socket, session, etc.).

### WriteFn

```modula2
TYPE WriteFn = PROCEDURE(ADDRESS, ADDRESS, CARDINAL, VAR CARDINAL): CARDINAL;
```

Transport write callback. Writes `len` bytes from `buf`, sets `sent` to the actual byte count, and returns a TsXxx status code.

### FrameStatus

```modula2
TYPE FrameStatus = (FrmOk, FrmNeedMore, FrmClosed, FrmTooLarge, FrmError);
```

Result codes for frame reading:
- `FrmOk`: complete frame read successfully
- `FrmNeedMore`: partial read, call again when more data is available
- `FrmClosed`: transport closed cleanly (after draining any buffered data)
- `FrmTooLarge`: frame's declared length exceeds MaxFrame (protocol violation)
- `FrmError`: transport error, connection should close

### FrameReader

```modula2
TYPE FrameReader = RECORD
  state:      CARDINAL;
  lenBuf:     ARRAY [0..3] OF CHAR;
  lenPos:     CARDINAL;
  payloadBuf: Buf;
  payloadLen: CARDINAL;
  payloadPos: CARDINAL;
  maxFrame:   CARDINAL;
  readFn:     ReadFn;
  readCtx:    ADDRESS;
END;
```

Incremental state machine for reading frames. Accumulates bytes across multiple `TryReadFrame` calls until a complete frame is assembled. The reader retains partial state between calls, so it correctly handles both blocking and non-blocking transports.

- `state`: 0 = reading length prefix, 1 = reading payload
- `lenBuf`/`lenPos`: partial length bytes
- `payloadBuf`: reusable buffer for payload (grows once, reused across frames)

## Procedures

### InitFrameReader

```modula2
PROCEDURE InitFrameReader(VAR fr: FrameReader;
                          maxFrame: CARDINAL;
                          fn: ReadFn; ctx: ADDRESS);
```

Initialize a frame reader. The `maxFrame` parameter sets the maximum allowed payload size (clamped to MaxFrame). Any frame declaring a larger payload is rejected with `FrmTooLarge`.

The `fn` and `ctx` parameters install the transport read function and its context pointer. The context is passed to every `fn` call.

```modula2
InitFrameReader(fr, 32768, MyReadFunc, socketPtr);
```

### TryReadFrame

```modula2
PROCEDURE TryReadFrame(VAR fr: FrameReader;
                       VAR out: BytesView;
                       VAR status: FrameStatus);
```

Attempt to read a complete frame. Call repeatedly until `status = FrmOk` or an error status.

- **FrmOk**: `out` is a view of the assembled payload. The view points into the reader's internal buffer, so it remains valid until the next `TryReadFrame` call.
- **FrmNeedMore**: partial read; call again when the transport has more data.
- **FrmClosed**: transport closed cleanly.
- **FrmTooLarge**: payload exceeds `maxFrame`; connection should close.
- **FrmError**: transport error; connection should close.

The reader's internal buffer is reused across frames -- it grows once to the largest frame size seen, then holds that capacity for subsequent frames (avoiding repeated allocations).

```modula2
LOOP
  TryReadFrame(fr, payload, status);
  IF status = FrmOk THEN
    ProcessPayload(payload);
  ELSIF status = FrmNeedMore THEN
    (* wait for readable event, then loop *)
  ELSE
    EXIT  (* closed or error *)
  END
END;
```

### ResetFrameReader

```modula2
PROCEDURE ResetFrameReader(VAR fr: FrameReader);
```

Reset the reader to initial state, discarding any partial frame. The internal buffer and transport callbacks are retained. Use this after an error to resynchronize without reallocating.

### FreeFrameReader

```modula2
PROCEDURE FreeFrameReader(VAR fr: FrameReader);
```

Free the reader's internal payload buffer. After this call, the reader must be re-initialized with `InitFrameReader` before reuse.

### WriteFrame

```modula2
PROCEDURE WriteFrame(fn: WriteFn; ctx: ADDRESS;
                     payload: BytesView;
                     VAR ok: BOOLEAN);
```

Write a complete frame (4-byte length prefix + payload) in a blocking loop. Calls the write function repeatedly until all bytes are sent or an error occurs. Sets `ok := TRUE` on success, `ok := FALSE` on write failure.

This is simpler than an incremental write state machine -- most RPC protocols issue complete frames atomically, so a blocking write loop is sufficient.

```modula2
WriteFrame(MyWriteFunc, socketPtr, payload, ok);
IF NOT ok THEN
  (* connection failed *)
END;
```

## Example

```modula2
MODULE FrameDemo;

FROM InOut IMPORT WriteString, WriteCard, WriteLn;
FROM ByteBuf IMPORT Buf, BytesView, Init, Free, AppendByte, AsView;
FROM RpcFrame IMPORT FrameReader, FrameStatus, ReadFn, WriteFn,
                     InitFrameReader, TryReadFrame, WriteFrame,
                     FrmOk, FrmNeedMore;
FROM RpcTest IMPORT Pipe, CreatePipe, DestroyPipe, ReadB, WriteA;

VAR
  pipe: Pipe;
  fr: FrameReader;
  payload, received: BytesView;
  status: FrameStatus;
  ok: BOOLEAN;
  b: Buf;
  i: CARDINAL;

BEGIN
  (* Create in-memory pipe with partial I/O (1 byte at a time) *)
  CreatePipe(pipe, 1, 1);

  (* Write a frame from side A *)
  Init(b, 16);
  AppendByte(b, 72);  (* 'H' *)
  AppendByte(b, 105); (* 'i' *)
  payload := AsView(b);
  WriteFrame(WriteA, pipe, payload, ok);
  WriteString("frame written: "); WriteCard(ok, 0); WriteLn;

  (* Read frame from side B with incremental reads *)
  InitFrameReader(fr, 1024, ReadB, pipe);

  LOOP
    TryReadFrame(fr, received, status);
    IF status = FrmOk THEN
      WriteString("frame received, len = ");
      WriteCard(received.len, 0); WriteLn;
      EXIT
    ELSIF status = FrmNeedMore THEN
      WriteString(".");  (* partial read *)
    ELSE
      WriteString("error or closed"); WriteLn;
      EXIT
    END
  END;

  Free(b);
  DestroyPipe(pipe)
END FrameDemo.
```
