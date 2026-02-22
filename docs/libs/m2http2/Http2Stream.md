# Http2Stream

Per-stream state machine for HTTP/2 (RFC 7540 Section 5.1).

## Why Http2Stream?

Each HTTP/2 stream has its own lifecycle with well-defined state transitions. This module wraps m2Fsm with the RFC 7540 stream state table, plus per-stream flow control windows.

## Types

### H2Stream

```modula2
TYPE H2Stream = RECORD
  id:         CARDINAL;
  fsm:        Fsm;
  sendWindow: INTEGER;
  recvWindow: INTEGER;
  rstCode:    CARDINAL;
END;
```

### StreamTransTable

```modula2
TYPE StreamTransTable = ARRAY [0..62] OF Transition;
```

Shared across all streams on a connection.

## Procedures

### Table Setup

```modula2
PROCEDURE InitStreamTable(VAR table: StreamTransTable);
```

Fills the 7x9 transition table per RFC 7540 Section 5.1:

| From | Event | To |
|------|-------|----|
| Idle | SendH | Open |
| Idle | SendHES | HalfClosedLocal |
| Idle | RecvH | Open |
| Idle | RecvHES | HalfClosedRemote |
| Open | SendES | HalfClosedLocal |
| Open | RecvES | HalfClosedRemote |
| HalfClosedLocal | RecvES | Closed |
| HalfClosedRemote | SendES | Closed |
| Any | SendRst/RecvRst | Closed |

### Stream Lifecycle

```modula2
PROCEDURE InitStream(VAR s: H2Stream; streamId: CARDINAL;
                     initWindowSize: CARDINAL; table: ADDRESS);
PROCEDURE StreamStep(VAR s: H2Stream; ev: CARDINAL; VAR status: StepStatus);
PROCEDURE StreamState(VAR s: H2Stream): CARDINAL;
PROCEDURE IsClosed(VAR s: H2Stream): BOOLEAN;
```

### Flow Control

```modula2
PROCEDURE ConsumeSendWindow(VAR s: H2Stream; n: CARDINAL): BOOLEAN;
PROCEDURE UpdateSendWindow(VAR s: H2Stream; increment: CARDINAL);
PROCEDURE ConsumeRecvWindow(VAR s: H2Stream; n: CARDINAL): BOOLEAN;
PROCEDURE UpdateRecvWindow(VAR s: H2Stream; increment: CARDINAL);
```

## Usage

```modula2
VAR table: StreamTransTable;
    s: H2Stream;
    status: StepStatus;
InitStreamTable(table);
InitStream(s, 1, 65535, ADR(table));
StreamStep(s, EvSendH, status);   (* Idle -> Open *)
StreamStep(s, EvSendES, status);  (* Open -> HalfClosedLocal *)
```
