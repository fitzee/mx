# Usage Patterns

## Protocol Stack: HTTP/2 Connection FSM

A typical HTTP/2 connection has these states and events:

```
CONST
  (* States *)
  StPreface    = 0;   (* waiting for connection preface *)
  StOpen       = 1;   (* connection active *)
  StGoingAway  = 2;   (* GOAWAY sent, draining *)
  StClosed     = 3;
  NumStates    = 4;

  (* Events *)
  EvPrefaceOk  = 0;   (* preface received and valid *)
  EvFrame      = 1;   (* normal frame received *)
  EvGoAway     = 2;   (* GOAWAY sent *)
  EvDrained    = 3;   (* all streams closed *)
  EvError      = 4;   (* protocol error *)
  NumEvents    = 5;
```

Transition table setup:
```modula2
VAR table: ARRAY [0..19] OF Transition;  (* 4 * 5 *)
Fsm.ClearTable(ADR(table), 20);
Fsm.SetTrans(table[StPreface*5 + EvPrefaceOk], StOpen, ActInit, NoGuard);
Fsm.SetTrans(table[StPreface*5 + EvError], StClosed, ActLogErr, NoGuard);
Fsm.SetTrans(table[StOpen*5 + EvFrame], StOpen, ActDispatch, NoGuard);
Fsm.SetTrans(table[StOpen*5 + EvGoAway], StGoingAway, ActSendGoAway, NoGuard);
Fsm.SetTrans(table[StGoingAway*5 + EvDrained], StClosed, ActClose, NoGuard);
(* ... etc ... *)
```

## Per-Stream FSM

Each HTTP/2 stream has its own FSM instance. Since Fsm records are stack-allocated and share the same transition table, you can have hundreds of streams cheaply:

```modula2
VAR streams: ARRAY [0..127] OF Fsm;
    i: CARDINAL;
i := 0;
WHILE i < 128 DO
  Fsm.Init(streams[i], StIdle, connCtx, NumStates, NumEvents, ADR(table));
  Fsm.SetActions(streams[i], ADR(acts), NumActions);
  INC(i)
END;
```

All streams share the same `table` and `acts` arrays.

## State/Event Numbering Conventions

Start at 0. Use named constants. Group related states:
```
(* Connection states: 0-9 *)
(* Stream states: 10-19 *)
(* Error states: 90-99 *)
```

Events similarly:
```
(* Control events: 0-9 *)
(* Data events: 10-19 *)
(* Error events: 90-99 *)
```

## NoAction Slot

Action index 0 is always `NIL`. Transitions that need no action use `NoAction = 0`. This avoids needing a separate flag or sentinel in the Transition record.

In practice, most FSMs have ~3-10 actions. The one wasted slot is negligible.

## Payload Passing

The `payload` ADDRESS is passed through to all callbacks unchanged. Use it to pass event-specific data:

```modula2
TYPE FrameInfo = RECORD
  streamId: CARDINAL;
  frameType: CARDINAL;
  length: CARDINAL;
END;

VAR info: FrameInfo;
info.streamId := 1;
info.frameType := 0;
info.length := 256;
Fsm.Step(f, EvFrame, ADR(info), status);
```

In the action:
```modula2
PROCEDURE HandleFrame(ctx: ADDRESS; ev: EventId;
                      payload: ADDRESS; VAR ok: BOOLEAN);
VAR fp: POINTER TO FrameInfo;
BEGIN
  fp := payload;
  (* use fp^.streamId, fp^.frameType, fp^.length *)
  ok := TRUE
END HandleFrame;
```

## Guards for Backpressure

Use guards to implement flow control:

```modula2
PROCEDURE CheckWindow(ctx: ADDRESS; ev: EventId;
                      payload: ADDRESS; VAR allow: BOOLEAN);
VAR conn: ConnPtr;
BEGIN
  conn := ctx;
  allow := conn^.sendWindow > 0
END CheckWindow;
```

When the send window is exhausted, the guard rejects `EvSendData` transitions without changing state, and the FSM counter tracks how often backpressure was applied.
