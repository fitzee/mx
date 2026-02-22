# Testing Guide

## In-Memory Testing

The server library supports deterministic testing without TLS or
real sockets using `Http2ServerTestUtil` and `ConnCreateTest`.

### Test Connection Pattern

```modula2
VAR cp: ConnPtr;
ConnCreateTest(NIL, 1, cp);   (* no server, conn id=1 *)
(* ... send frames, check responses ... *)
ConnClose(cp);
```

### FeedAndCollect

```modula2
PROCEDURE FeedAndCollect(cp: ConnPtr; VAR input: Buf; VAR output: Buf);
```

Feeds raw bytes into the connection's read buffer, processes them,
and collects the server's output frames.

### Frame Building

Build client-to-server frames using the `Build*` procedures:

```modula2
Init(input, 1024);
BuildClientPreface(input);
BuildSettings(input, settings);
FeedAndCollect(cp, input, output);
```

### Response Parsing

Parse server-to-client frames using `ReadNextFrame`:

```modula2
v := AsView(output);
WHILE ReadNextFrame(v, hdr, payload) DO
  IF hdr.ftype = FrameSettings THEN (* ... *) END;
END;
```

## Test Scenarios

### 1. Handshake + SETTINGS
Send client preface + SETTINGS. Verify server responds with
SETTINGS frame and SETTINGS ACK.

### 2. HEADERS Parsing
After handshake, send GET HEADERS on stream 1.
Verify stream allocation (`lastPeerStream = 1`).

### 3. DATA Accumulation
Send POST headers + split DATA frames. Verify WINDOW_UPDATE
sent after each DATA frame.

### 4. Multiplexing
Open 3 streams (1, 3, 5) simultaneously. Verify all accepted.

### 5. Flow Control
Send 1000-byte DATA frame. Verify both connection-level and
stream-level WINDOW_UPDATE frames.

### 6. PING Echo
Send PING with 8 bytes. Verify PING ACK with same data.

### 7. Error Paths
- Invalid preface → GOAWAY
- HEADERS on stream 0 → GOAWAY
- HEADERS on even stream → GOAWAY
