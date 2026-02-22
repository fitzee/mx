# RpcTest

In-memory duplex byte stream for deterministic RPC testing. Provides a back-to-back pipe connecting two endpoints (A and B) without real networking or syscalls.

## Why RpcTest?

Testing networked RPC code with real sockets is slow, flaky, and hard to debug. You need to bind ports, handle asynchronous I/O, and deal with timing-dependent failures. RpcTest provides a deterministic alternative: bytes written by endpoint A are readable by endpoint B, and vice versa, with no kernel involvement.

The pipe supports **partial I/O simulation** via `readLimit` and `writeLimit` parameters. This exercises the incremental state machines in FrameReader and RpcClient without the nondeterminism of real network buffering.

## Design

A Pipe is an opaque handle (ADDRESS) to two internal byte queues:
- **A→B**: bytes written by A, readable by B
- **B→A**: bytes written by B, readable by A

The `ReadA`/`WriteA` and `ReadB`/`WriteB` procedures have the **exact same signature** as RpcFrame's `ReadFn`/`WriteFn`, so you can pass them directly as transport callbacks:

```modula2
InitClient(client, ReadA, pipe, WriteA, pipe, sched, NIL);
InitServer(server, ReadB, pipe, WriteB, pipe);
```

**Partial I/O**: Set `readLimit` or `writeLimit` to a small value (e.g., 1) to simulate slow or split reads/writes. Each call transfers at most that many bytes. This exercises code paths that handle partial reads, like FrameReader's incremental state machine.

**Close semantics**: `CloseA()`/`CloseB()` close the write direction. Subsequent reads on the other end return `TsClosed` after draining any buffered data.

## Types

### Pipe

```modula2
TYPE Pipe = ADDRESS;
```

Opaque handle to a pipe pair. Create with `CreatePipe`, destroy with `DestroyPipe`.

## Procedures

### CreatePipe

```modula2
PROCEDURE CreatePipe(VAR p: Pipe;
                     readLimit: CARDINAL;
                     writeLimit: CARDINAL);
```

Create a pipe pair. The `readLimit` and `writeLimit` parameters control the maximum bytes transferred per read/write call. Use 0 for unlimited (full-speed I/O).

```modula2
CreatePipe(pipe, 1, 1);  (* 1 byte at a time, exercises partial I/O *)
CreatePipe(pipe, 0, 0);  (* no limit, full-speed *)
```

### DestroyPipe

```modula2
PROCEDURE DestroyPipe(VAR p: Pipe);
```

Destroy a pipe and free its internal buffers. Sets `p` to NIL. Safe to call multiple times on the same pipe.

### CloseA

```modula2
PROCEDURE CloseA(p: Pipe);
```

Close endpoint A's write direction. Subsequent `WriteA` calls fail with `TsClosed`. `ReadB` calls see `TsClosed` after draining any buffered data from the A→B queue.

### CloseB

```modula2
PROCEDURE CloseB(p: Pipe);
```

Close endpoint B's write direction. Subsequent `WriteB` calls fail with `TsClosed`. `ReadA` calls see `TsClosed` after draining the B→A queue.

## Endpoint A Transport Functions

### ReadA

```modula2
PROCEDURE ReadA(ctx: ADDRESS; buf: ADDRESS; max: CARDINAL;
                VAR got: CARDINAL): CARDINAL;
```

Read from the B→A direction. Signature matches `ReadFn` from RpcFrame.

- `ctx`: must be a valid Pipe handle (passed as ADDRESS)
- `buf`: destination buffer (ADDRESS to CHAR array)
- `max`: maximum bytes to read
- `got`: set to actual byte count read
- Returns: `TsOk`, `TsWouldBlock` (no data), `TsClosed`, or `TsError`

### WriteA

```modula2
PROCEDURE WriteA(ctx: ADDRESS; buf: ADDRESS; len: CARDINAL;
                 VAR sent: CARDINAL): CARDINAL;
```

Write to the A→B direction. Signature matches `WriteFn` from RpcFrame.

## Endpoint B Transport Functions

### ReadB

```modula2
PROCEDURE ReadB(ctx: ADDRESS; buf: ADDRESS; max: CARDINAL;
                VAR got: CARDINAL): CARDINAL;
```

Read from the A→B direction. Same semantics as `ReadA`, but for endpoint B.

### WriteB

```modula2
PROCEDURE WriteB(ctx: ADDRESS; buf: ADDRESS; len: CARDINAL;
                 VAR sent: CARDINAL): CARDINAL;
```

Write to the B→A direction. Same semantics as `WriteA`, but for endpoint B.

## Query Functions

### PendingAtoB

```modula2
PROCEDURE PendingAtoB(p: Pipe): CARDINAL;
```

Number of unread bytes pending in the A→B direction (written by A, not yet read by B).

### PendingBtoA

```modula2
PROCEDURE PendingBtoA(p: Pipe): CARDINAL;
```

Number of unread bytes pending in the B→A direction (written by B, not yet read by A).

## Example

```modula2
MODULE PipeDemo;

FROM InOut IMPORT WriteString, WriteCard, WriteLn;
FROM RpcTest IMPORT Pipe, CreatePipe, DestroyPipe,
                     ReadA, WriteA, ReadB, WriteB,
                     PendingAtoB, PendingBtoA;
FROM RpcFrame IMPORT TsOk;

VAR
  pipe: Pipe;
  buf: ARRAY [0..15] OF CHAR;
  sent, got: CARDINAL;
  status: CARDINAL;

BEGIN
  (* Create pipe with 1-byte partial I/O *)
  CreatePipe(pipe, 1, 1);

  (* A writes 4 bytes to B *)
  buf[0] := 'H';
  buf[1] := 'e';
  buf[2] := 'l';
  buf[3] := 'l';

  (* Write one byte at a time (writeLimit=1) *)
  status := WriteA(pipe, buf, 4, sent);
  WriteString("first write sent: ");
  WriteCard(sent, 0); WriteLn;  (* 1 *)

  status := WriteA(pipe, buf[1], 3, sent);
  WriteString("second write sent: ");
  WriteCard(sent, 0); WriteLn;  (* 1 *)

  status := WriteA(pipe, buf[2], 2, sent);
  WriteString("third write sent: ");
  WriteCard(sent, 0); WriteLn;  (* 1 *)

  status := WriteA(pipe, buf[3], 1, sent);
  WriteString("fourth write sent: ");
  WriteCard(sent, 0); WriteLn;  (* 1 *)

  WriteString("pending A->B: ");
  WriteCard(PendingAtoB(pipe), 0); WriteLn;  (* 4 *)

  (* B reads 4 bytes from A *)
  status := ReadB(pipe, buf, 4, got);
  WriteString("first read got: ");
  WriteCard(got, 0); WriteLn;  (* 1, readLimit=1 *)

  status := ReadB(pipe, buf[1], 3, got);
  WriteString("second read got: ");
  WriteCard(got, 0); WriteLn;  (* 1 *)

  status := ReadB(pipe, buf[2], 2, got);
  WriteString("third read got: ");
  WriteCard(got, 0); WriteLn;  (* 1 *)

  status := ReadB(pipe, buf[3], 1, got);
  WriteString("fourth read got: ");
  WriteCard(got, 0); WriteLn;  (* 1 *)

  WriteString("pending A->B: ");
  WriteCard(PendingAtoB(pipe), 0); WriteLn;  (* 0 *)

  DestroyPipe(pipe)
END PipeDemo.
```
