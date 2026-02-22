# Http2TestUtil

Test utilities for HTTP/2. ByteBuf-based frame builders and readers for constructing test vectors without network I/O.

## Why Http2TestUtil?

Deterministic testing requires constructing and parsing frames in-memory. This module provides convenience wrappers around Http2Frame for building complete frames and reading them back.

## Procedures

### Frame Builders

```modula2
PROCEDURE BuildFrame(VAR b: Buf; ftype, flags, streamId: CARDINAL;
                     payload: BytesView);
PROCEDURE BuildSettingsFrame(VAR b: Buf; s: Settings);
PROCEDURE BuildSettingsAckFrame(VAR b: Buf);
PROCEDURE BuildPingFrame(VAR b: Buf; data: BytesView; isAck: BOOLEAN);
PROCEDURE BuildGoawayFrame(VAR b: Buf; lastStreamId, errorCode: CARDINAL);
PROCEDURE BuildWindowUpdateFrame(VAR b: Buf; streamId, increment: CARDINAL);
PROCEDURE BuildRstStreamFrame(VAR b: Buf; streamId, errorCode: CARDINAL);
```

### Frame Readers

```modula2
PROCEDURE ReadFrameHeader(VAR v: BytesView; VAR hdr: FrameHeader; VAR ok: BOOLEAN);
PROCEDURE ReadFramePayload(VAR v: BytesView; hdr: FrameHeader;
                           VAR payload: BytesView; VAR ok: BOOLEAN);
```

`ReadFrameHeader` and `ReadFramePayload` advance the view, consuming bytes. This allows sequential parsing of multiple frames from a single buffer.
