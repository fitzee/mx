# Http2Conn

HTTP/2 connection management. Handles connection preface, settings negotiation, flow control, stream allocation, and frame dispatch.

## Why Http2Conn?

Ties together framing, stream state machines, HPACK tables, and flow control into a single connection-level abstraction. Processes incoming frames and generates response frames automatically (SETTINGS_ACK, PING_ACK).

## Types

### H2Conn

```modula2
CONST MaxStreams = 64;

TYPE H2Conn = RECORD
  localSettings:  Settings;
  remoteSettings: Settings;
  connSendWindow: INTEGER;
  connRecvWindow: INTEGER;
  nextStreamId:   CARDINAL;
  lastPeerStream: CARDINAL;
  goawayCode:     CARDINAL;
  goawaySent:     BOOLEAN;
  goawayRecv:     BOOLEAN;
  streams:    ARRAY [0..63] OF H2Stream;
  streamUsed: ARRAY [0..63] OF BOOLEAN;
  numActive:  CARDINAL;
  streamTable: StreamTransTable;
  dynTableEnc: DynTable;
  dynTableDec: DynTable;
  outBuf:     Buf;
END;
```

## Procedures

### Lifecycle

```modula2
PROCEDURE InitConn(VAR c: H2Conn);
PROCEDURE FreeConn(VAR c: H2Conn);
```

### Connection Preface

```modula2
PROCEDURE SendPreface(VAR c: H2Conn);
```

Writes the 24-byte client magic + initial SETTINGS frame to `outBuf`.

### Frame Processing

```modula2
PROCEDURE ProcessFrame(VAR c: H2Conn; hdr: FrameHeader;
                       payload: BytesView; VAR ok: BOOLEAN);
```

Dispatches by frame type. Handles SETTINGS (updates remote, sends ACK), PING (echoes with ACK), GOAWAY (marks connection), WINDOW_UPDATE (adjusts windows), RST_STREAM (closes stream), DATA (consumes recv window), HEADERS (drives stream FSM).

### Stream Management

```modula2
PROCEDURE OpenStream(VAR c: H2Conn): CARDINAL;
PROCEDURE FindStream(VAR c: H2Conn; streamId: CARDINAL): CARDINAL;
```

Client streams use odd IDs starting from 1.

### Flow Control

```modula2
PROCEDURE UpdateConnSendWindow(VAR c: H2Conn; increment: CARDINAL);
PROCEDURE ConsumeConnSendWindow(VAR c: H2Conn; n: CARDINAL): BOOLEAN;
```

### Output

```modula2
PROCEDURE GetOutput(VAR c: H2Conn): BytesView;
PROCEDURE ClearOutput(VAR c: H2Conn);
```

## Usage

```modula2
VAR c: H2Conn;
InitConn(c);
SendPreface(c);
(* flush GetOutput(c) to network *)
ClearOutput(c);
(* receive frames, call ProcessFrame *)
(* flush GetOutput(c) after each ProcessFrame *)
FreeConn(c);
```
