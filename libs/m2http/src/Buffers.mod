IMPLEMENTATION MODULE Buffers;

FROM SYSTEM IMPORT ADDRESS, ADR, TSIZE;
FROM Storage IMPORT ALLOCATE, DEALLOCATE;

(* ── Internal types ────────────────────────────────────────────── *)

TYPE
  BufRec = RECORD
    data: ARRAY [0..MaxCap-1] OF CHAR;
    cap:  INTEGER;    (* logical capacity, <= MaxCap *)
    rpos: INTEGER;    (* read position *)
    wpos: INTEGER;    (* write position *)
    mode: GrowMode;
  END;

  BufPtr = POINTER TO BufRec;

(* ── Growth ────────────────────────────────────────────────────── *)

PROCEDURE EnsureSpace(bp: BufPtr; needed: INTEGER): BOOLEAN;
VAR newCap: INTEGER;
BEGIN
  IF bp^.wpos + needed <= bp^.cap THEN RETURN TRUE END;
  IF bp^.mode = Fixed THEN RETURN FALSE END;
  newCap := bp^.cap;
  WHILE (newCap < MaxCap) AND (bp^.wpos + needed > newCap) DO
    newCap := newCap * 2;
    IF newCap > MaxCap THEN newCap := MaxCap END
  END;
  IF bp^.wpos + needed > newCap THEN RETURN FALSE END;
  bp^.cap := newCap;
  RETURN TRUE
END EnsureSpace;

(* ── Lifecycle ─────────────────────────────────────────────────── *)

PROCEDURE Create(initialCap: INTEGER; mode: GrowMode;
                 VAR out: Buffer): Status;
VAR bp: BufPtr;
BEGIN
  IF (initialCap <= 0) OR (initialCap > MaxCap) THEN
    out := NIL;
    RETURN Invalid
  END;
  ALLOCATE(bp, TSIZE(BufRec));
  IF bp = NIL THEN
    out := NIL;
    RETURN OutOfMemory
  END;
  bp^.cap := initialCap;
  bp^.rpos := 0;
  bp^.wpos := 0;
  bp^.mode := mode;
  out := bp;
  RETURN OK
END Create;

PROCEDURE Destroy(VAR b: Buffer): Status;
VAR bp: BufPtr;
BEGIN
  IF b = NIL THEN RETURN Invalid END;
  bp := b;
  DEALLOCATE(bp, TSIZE(BufRec));
  b := NIL;
  RETURN OK
END Destroy;

(* ── Writing ───────────────────────────────────────────────────── *)

PROCEDURE AppendByte(b: Buffer; ch: CHAR): Status;
VAR bp: BufPtr;
BEGIN
  IF b = NIL THEN RETURN Invalid END;
  bp := b;
  IF NOT EnsureSpace(bp, 1) THEN RETURN Full END;
  bp^.data[bp^.wpos] := ch;
  INC(bp^.wpos);
  RETURN OK
END AppendByte;

PROCEDURE AppendBytes(b: Buffer; VAR data: ARRAY OF CHAR;
                      len: INTEGER): Status;
VAR bp: BufPtr; i: INTEGER;
BEGIN
  IF b = NIL THEN RETURN Invalid END;
  IF len <= 0 THEN RETURN OK END;
  bp := b;
  IF NOT EnsureSpace(bp, len) THEN RETURN Full END;
  FOR i := 0 TO len - 1 DO
    bp^.data[bp^.wpos + i] := data[i]
  END;
  bp^.wpos := bp^.wpos + len;
  RETURN OK
END AppendBytes;

PROCEDURE AppendString(b: Buffer; VAR s: ARRAY OF CHAR): Status;
VAR bp: BufPtr; i, len: INTEGER;
BEGIN
  IF b = NIL THEN RETURN Invalid END;
  bp := b;
  len := 0;
  WHILE (len <= HIGH(s)) AND (s[len] # 0C) DO INC(len) END;
  IF len = 0 THEN RETURN OK END;
  IF NOT EnsureSpace(bp, len) THEN RETURN Full END;
  FOR i := 0 TO len - 1 DO
    bp^.data[bp^.wpos + i] := s[i]
  END;
  bp^.wpos := bp^.wpos + len;
  RETURN OK
END AppendString;

(* ── Reading ───────────────────────────────────────────────────── *)

PROCEDURE PeekByte(b: Buffer; offset: INTEGER;
                   VAR ch: CHAR): Status;
VAR bp: BufPtr;
BEGIN
  IF b = NIL THEN RETURN Invalid END;
  bp := b;
  IF (offset < 0) OR (bp^.rpos + offset >= bp^.wpos) THEN
    RETURN Empty
  END;
  ch := bp^.data[bp^.rpos + offset];
  RETURN OK
END PeekByte;

PROCEDURE Consume(b: Buffer; n: INTEGER): Status;
VAR bp: BufPtr;
BEGIN
  IF b = NIL THEN RETURN Invalid END;
  bp := b;
  IF n <= 0 THEN RETURN OK END;
  IF n > bp^.wpos - bp^.rpos THEN RETURN Empty END;
  bp^.rpos := bp^.rpos + n;
  IF bp^.rpos = bp^.wpos THEN
    bp^.rpos := 0;
    bp^.wpos := 0
  END;
  RETURN OK
END Consume;

PROCEDURE CopyOut(b: Buffer; offset, len: INTEGER;
                  VAR dst: ARRAY OF CHAR): Status;
VAR bp: BufPtr; i, src, maxLen: INTEGER;
BEGIN
  IF b = NIL THEN RETURN Invalid END;
  bp := b;
  src := bp^.rpos + offset;
  IF (offset < 0) OR (src + len > bp^.wpos) THEN RETURN Empty END;
  maxLen := HIGH(dst) + 1;
  IF len > maxLen THEN len := maxLen END;
  FOR i := 0 TO len - 1 DO
    dst[i] := bp^.data[src + i]
  END;
  IF len < maxLen THEN dst[len] := 0C END;
  RETURN OK
END CopyOut;

(* ── State ─────────────────────────────────────────────────────── *)

PROCEDURE Length(b: Buffer): INTEGER;
VAR bp: BufPtr;
BEGIN
  IF b = NIL THEN RETURN 0 END;
  bp := b;
  RETURN bp^.wpos - bp^.rpos
END Length;

PROCEDURE Capacity(b: Buffer): INTEGER;
VAR bp: BufPtr;
BEGIN
  IF b = NIL THEN RETURN 0 END;
  bp := b;
  RETURN bp^.cap
END Capacity;

PROCEDURE Remaining(b: Buffer): INTEGER;
VAR bp: BufPtr;
BEGIN
  IF b = NIL THEN RETURN 0 END;
  bp := b;
  RETURN bp^.cap - bp^.wpos
END Remaining;

PROCEDURE Clear(b: Buffer): Status;
VAR bp: BufPtr;
BEGIN
  IF b = NIL THEN RETURN Invalid END;
  bp := b;
  bp^.rpos := 0;
  bp^.wpos := 0;
  RETURN OK
END Clear;

PROCEDURE Compact(b: Buffer): Status;
VAR bp: BufPtr; i, readable: INTEGER;
BEGIN
  IF b = NIL THEN RETURN Invalid END;
  bp := b;
  IF bp^.rpos = 0 THEN RETURN OK END;
  readable := bp^.wpos - bp^.rpos;
  IF readable > 0 THEN
    FOR i := 0 TO readable - 1 DO
      bp^.data[i] := bp^.data[bp^.rpos + i]
    END
  END;
  bp^.wpos := readable;
  bp^.rpos := 0;
  RETURN OK
END Compact;

(* ── Zero-copy ─────────────────────────────────────────────────── *)

PROCEDURE SlicePtr(b: Buffer): ADDRESS;
VAR bp: BufPtr;
BEGIN
  IF b = NIL THEN RETURN NIL END;
  bp := b;
  RETURN ADR(bp^.data[bp^.rpos])
END SlicePtr;

PROCEDURE SliceLen(b: Buffer): INTEGER;
BEGIN
  RETURN Length(b)
END SliceLen;

PROCEDURE WritePtr(b: Buffer): ADDRESS;
VAR bp: BufPtr;
BEGIN
  IF b = NIL THEN RETURN NIL END;
  bp := b;
  RETURN ADR(bp^.data[bp^.wpos])
END WritePtr;

PROCEDURE AdvanceWrite(b: Buffer; n: INTEGER): Status;
VAR bp: BufPtr;
BEGIN
  IF b = NIL THEN RETURN Invalid END;
  bp := b;
  IF n <= 0 THEN RETURN OK END;
  IF bp^.wpos + n > bp^.cap THEN RETURN Full END;
  bp^.wpos := bp^.wpos + n;
  RETURN OK
END AdvanceWrite;

(* ── Search ────────────────────────────────────────────────────── *)

PROCEDURE FindByte(b: Buffer; ch: CHAR;
                   VAR pos: INTEGER): BOOLEAN;
VAR bp: BufPtr; i: INTEGER;
BEGIN
  IF b = NIL THEN RETURN FALSE END;
  bp := b;
  FOR i := bp^.rpos TO bp^.wpos - 1 DO
    IF bp^.data[i] = ch THEN
      pos := i - bp^.rpos;
      RETURN TRUE
    END
  END;
  RETURN FALSE
END FindByte;

PROCEDURE FindCRLF(b: Buffer; VAR pos: INTEGER): BOOLEAN;
VAR bp: BufPtr; i: INTEGER;
BEGIN
  IF b = NIL THEN RETURN FALSE END;
  bp := b;
  IF bp^.wpos - bp^.rpos < 2 THEN RETURN FALSE END;
  FOR i := bp^.rpos TO bp^.wpos - 2 DO
    IF (bp^.data[i] = CHR(13)) AND (bp^.data[i+1] = CHR(10)) THEN
      pos := i - bp^.rpos;
      RETURN TRUE
    END
  END;
  RETURN FALSE
END FindCRLF;

END Buffers.
