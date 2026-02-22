# Http2Frame

HTTP/2 frame encoding and decoding. Handles the 9-byte frame header and payload serialisation for SETTINGS, PING, GOAWAY, WINDOW_UPDATE, RST_STREAM, DATA, and HEADERS frames.

## Why Http2Frame?

The framing layer is the foundation of HTTP/2. This module converts between in-memory records and wire bytes using m2Codec for binary I/O. No heap allocation.

## Procedures

### Frame Header

```modula2
PROCEDURE DecodeHeader(v: BytesView; VAR hdr: FrameHeader; VAR ok: BOOLEAN);
PROCEDURE EncodeHeader(VAR b: Buf; hdr: FrameHeader);
```

Decode/encode the 9-byte frame header. The 24-bit length field is composed from three `ReadU8` calls (no `ReadU24BE` in m2Codec). The stream ID has its reserved bit masked off.

### Connection Preface

```modula2
PROCEDURE WritePreface(VAR b: Buf);
PROCEDURE CheckPreface(v: BytesView): BOOLEAN;
```

The 24-byte client magic string `PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n`.

### SETTINGS

```modula2
PROCEDURE DecodeSettings(payload: BytesView; VAR s: Settings; VAR ok: BOOLEAN);
PROCEDURE EncodeSettings(VAR b: Buf; s: Settings);
PROCEDURE EncodeSettingsAck(VAR b: Buf);
```

`EncodeSettings` writes a full frame (9 + 36 bytes for 6 standard settings). `DecodeSettings` updates only settings present in the payload; unknown IDs are ignored per RFC.

### PING

```modula2
PROCEDURE EncodePing(VAR b: Buf; data: BytesView; isAck: BOOLEAN);
```

8 bytes of opaque data, echoed back with ACK flag set.

### GOAWAY

```modula2
PROCEDURE EncodeGoaway(VAR b: Buf; lastStreamId: CARDINAL; errorCode: CARDINAL);
PROCEDURE DecodeGoaway(payload: BytesView; VAR lastStreamId: CARDINAL;
                       VAR errorCode: CARDINAL; VAR ok: BOOLEAN);
```

### WINDOW_UPDATE

```modula2
PROCEDURE EncodeWindowUpdate(VAR b: Buf; streamId: CARDINAL; increment: CARDINAL);
PROCEDURE DecodeWindowUpdate(payload: BytesView; VAR increment: CARDINAL; VAR ok: BOOLEAN);
```

### RST_STREAM

```modula2
PROCEDURE EncodeRstStream(VAR b: Buf; streamId: CARDINAL; errorCode: CARDINAL);
PROCEDURE DecodeRstStream(payload: BytesView; VAR errorCode: CARDINAL; VAR ok: BOOLEAN);
```

### DATA / HEADERS Headers

```modula2
PROCEDURE EncodeDataHeader(VAR b: Buf; streamId: CARDINAL;
                           payloadLen: CARDINAL; endStream: BOOLEAN);
PROCEDURE EncodeHeadersHeader(VAR b: Buf; streamId: CARDINAL;
                              payloadLen: CARDINAL;
                              endStream: BOOLEAN; endHeaders: BOOLEAN);
```

Write only the 9-byte frame header; caller appends payload bytes.
