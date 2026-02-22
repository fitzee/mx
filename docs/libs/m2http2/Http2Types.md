# Http2Types

Constants, frame types, error codes, settings, and shared types for HTTP/2 (RFC 7540 / RFC 9113).

## Why Http2Types?

Centralises all wire protocol constants so other modules import numeric values by name rather than scattering magic numbers.

## Constants

### Frame Types

| Constant | Value | Description |
|----------|-------|-------------|
| `FrameData` | 0 | DATA frame |
| `FrameHeaders` | 1 | HEADERS frame |
| `FramePriority` | 2 | PRIORITY frame |
| `FrameRstStream` | 3 | RST_STREAM frame |
| `FrameSettings` | 4 | SETTINGS frame |
| `FramePushPromise` | 5 | PUSH_PROMISE frame |
| `FramePing` | 6 | PING frame |
| `FrameGoaway` | 7 | GOAWAY frame |
| `FrameWindowUpdate` | 8 | WINDOW_UPDATE frame |
| `FrameContinuation` | 9 | CONTINUATION frame |

### Frame Flags

| Constant | Value | Used By |
|----------|-------|---------|
| `FlagEndStream` | 1 | DATA, HEADERS |
| `FlagAck` | 1 | SETTINGS, PING |
| `FlagEndHeaders` | 4 | HEADERS, CONTINUATION |
| `FlagPadded` | 8 | DATA, HEADERS |
| `FlagPriority` | 32 | HEADERS |

### Error Codes

`ErrNoError` (0) through `ErrHttp11Required` (13) per RFC 7540 Section 7.

### Settings

`SetHeaderTableSize` (1) through `SetMaxHeaderListSize` (6) with default values matching RFC 7540 Section 6.5.2.

### Stream FSM

7 states (`StIdle`..`StClosed`) and 9 events (`EvSendH`..`EvRecvPP`) for the per-stream state machine.

### Connection FSM

6 states (`ConnIdle`..`ConnClosed`) and 9 events (`CEvSendPreface`..`CEvConnError`).

## Types

### FrameHeader

```modula2
TYPE FrameHeader = RECORD
  length:   CARDINAL;   (* 24-bit payload length *)
  ftype:    CARDINAL;   (* frame type 0..255 *)
  flags:    CARDINAL;   (* frame flags 0..255 *)
  streamId: CARDINAL;   (* 31-bit stream identifier *)
END;
```

### Settings

```modula2
TYPE Settings = RECORD
  headerTableSize:     CARDINAL;
  enablePush:          CARDINAL;
  maxConcurrentStreams: CARDINAL;
  initialWindowSize:   CARDINAL;
  maxFrameSize:        CARDINAL;
  maxHeaderListSize:   CARDINAL;
END;
```

### HeaderEntry

```modula2
TYPE HeaderEntry = RECORD
  name:    ARRAY [0..127] OF CHAR;
  nameLen: CARDINAL;
  value:   ARRAY [0..4095] OF CHAR;
  valLen:  CARDINAL;
END;
```

## Procedures

### InitDefaultSettings

```modula2
PROCEDURE InitDefaultSettings(VAR s: Settings);
```

Fill a Settings record with RFC 7540 default values.
