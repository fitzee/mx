IMPLEMENTATION MODULE RpcTest;

FROM SYSTEM IMPORT ADDRESS, ADR, TSIZE;
FROM Storage IMPORT ALLOCATE, DEALLOCATE;
FROM ByteBuf IMPORT Buf, BBufPtr, Init, Free, Clear, AppendByte,
                     GetByte;

CONST
  TsOk        = 0;
  TsWouldBlock = 1;
  TsClosed    = 2;
  TsError     = 3;

TYPE
  PipeRec = RECORD
    aToB:       Buf;
    aToBPos:    CARDINAL;
    aClosed:    BOOLEAN;
    bToA:       Buf;
    bToAPos:    CARDINAL;
    bClosed:    BOOLEAN;
    readLimit:  CARDINAL;
    writeLimit: CARDINAL;
  END;
  PipePtr = POINTER TO PipeRec;

(* ── Lifecycle ────────────────────────────────────────── *)

PROCEDURE CreatePipe(VAR p: Pipe;
                     readLimit: CARDINAL;
                     writeLimit: CARDINAL);
VAR pp: PipePtr;
BEGIN
  ALLOCATE(pp, TSIZE(PipeRec));
  Init(pp^.aToB, 256);
  pp^.aToBPos := 0;
  pp^.aClosed := FALSE;
  Init(pp^.bToA, 256);
  pp^.bToAPos := 0;
  pp^.bClosed := FALSE;
  pp^.readLimit := readLimit;
  pp^.writeLimit := writeLimit;
  p := pp
END CreatePipe;

PROCEDURE DestroyPipe(VAR p: Pipe);
VAR pp: PipePtr;
BEGIN
  IF p = NIL THEN RETURN END;
  pp := p;
  Free(pp^.aToB);
  Free(pp^.bToA);
  DEALLOCATE(pp, TSIZE(PipeRec));
  p := NIL
END DestroyPipe;

PROCEDURE CloseA(p: Pipe);
VAR pp: PipePtr;
BEGIN
  pp := p;
  pp^.aClosed := TRUE
END CloseA;

PROCEDURE CloseB(p: Pipe);
VAR pp: PipePtr;
BEGIN
  pp := p;
  pp^.bClosed := TRUE
END CloseB;

(* ── Internal: read from a Buf/pos pair ───────────────── *)

PROCEDURE DoRead(VAR src: Buf; VAR srcPos: CARDINAL;
                 closed: BOOLEAN;
                 limit: CARDINAL;
                 buf: ADDRESS; max: CARDINAL;
                 VAR got: CARDINAL): CARDINAL;
VAR
  avail, n, i: CARDINAL;
  dst: BBufPtr;
BEGIN
  got := 0;
  avail := src.len - srcPos;
  IF avail = 0 THEN
    IF closed THEN RETURN TsClosed END;
    RETURN TsWouldBlock
  END;
  n := max;
  IF n > avail THEN n := avail END;
  IF (limit > 0) AND (n > limit) THEN n := limit END;
  dst := buf;
  i := 0;
  WHILE i < n DO
    dst^[i] := CHR(GetByte(src, srcPos + i));
    INC(i)
  END;
  srcPos := srcPos + n;
  got := n;

  (* Compact: if we've consumed everything, reset *)
  IF srcPos = src.len THEN
    Clear(src);
    srcPos := 0
  END;

  RETURN TsOk
END DoRead;

(* ── Internal: write to a Buf ─────────────────────────── *)

PROCEDURE DoWrite(VAR dst: Buf; closed: BOOLEAN;
                  limit: CARDINAL;
                  buf: ADDRESS; len: CARDINAL;
                  VAR sent: CARDINAL): CARDINAL;
VAR
  n, i: CARDINAL;
  src: BBufPtr;
BEGIN
  sent := 0;
  IF closed THEN RETURN TsClosed END;
  n := len;
  IF (limit > 0) AND (n > limit) THEN n := limit END;
  src := buf;
  i := 0;
  WHILE i < n DO
    AppendByte(dst, ORD(src^[i]));
    INC(i)
  END;
  sent := n;
  RETURN TsOk
END DoWrite;

(* ── Endpoint A ───────────────────────────────────────── *)

PROCEDURE ReadA(ctx: ADDRESS; buf: ADDRESS; max: CARDINAL;
                VAR got: CARDINAL): CARDINAL;
VAR pp: PipePtr;
BEGIN
  pp := ctx;
  RETURN DoRead(pp^.bToA, pp^.bToAPos, pp^.bClosed,
                pp^.readLimit, buf, max, got)
END ReadA;

PROCEDURE WriteA(ctx: ADDRESS; buf: ADDRESS; len: CARDINAL;
                 VAR sent: CARDINAL): CARDINAL;
VAR pp: PipePtr;
BEGIN
  pp := ctx;
  RETURN DoWrite(pp^.aToB, pp^.aClosed, pp^.writeLimit,
                 buf, len, sent)
END WriteA;

(* ── Endpoint B ───────────────────────────────────────── *)

PROCEDURE ReadB(ctx: ADDRESS; buf: ADDRESS; max: CARDINAL;
                VAR got: CARDINAL): CARDINAL;
VAR pp: PipePtr;
BEGIN
  pp := ctx;
  RETURN DoRead(pp^.aToB, pp^.aToBPos, pp^.aClosed,
                pp^.readLimit, buf, max, got)
END ReadB;

PROCEDURE WriteB(ctx: ADDRESS; buf: ADDRESS; len: CARDINAL;
                 VAR sent: CARDINAL): CARDINAL;
VAR pp: PipePtr;
BEGIN
  pp := ctx;
  RETURN DoWrite(pp^.bToA, pp^.bClosed, pp^.writeLimit,
                 buf, len, sent)
END WriteB;

(* ── Query ────────────────────────────────────────────── *)

PROCEDURE PendingAtoB(p: Pipe): CARDINAL;
VAR pp: PipePtr;
BEGIN
  pp := p;
  RETURN pp^.aToB.len - pp^.aToBPos
END PendingAtoB;

PROCEDURE PendingBtoA(p: Pipe): CARDINAL;
VAR pp: PipePtr;
BEGIN
  pp := p;
  RETURN pp^.bToA.len - pp^.bToAPos
END PendingBtoA;

END RpcTest.
