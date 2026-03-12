IMPLEMENTATION MODULE ByteBuf;

FROM SYSTEM IMPORT ADDRESS, TSIZE;
FROM Storage IMPORT ALLOCATE, DEALLOCATE;

TYPE
  CharPtr = POINTER TO CHAR;

(* ── Internal helpers ────────────────────────────────── *)

PROCEDURE PeekChar(base: ADDRESS; idx: CARDINAL): CHAR;
VAR p: CharPtr;
BEGIN
  p := CharPtr(LONGCARD(base) + LONGCARD(idx));
  RETURN p^
END PeekChar;

PROCEDURE PokeChar(base: ADDRESS; idx: CARDINAL; ch: CHAR);
VAR p: CharPtr;
BEGIN
  p := CharPtr(LONGCARD(base) + LONGCARD(idx));
  p^ := ch
END PokeChar;

PROCEDURE CopyBytes(src, dst: ADDRESS; srcOff, dstOff, n: CARDINAL);
VAR i: CARDINAL;
BEGIN
  (* 1 byte stride — 64 chars per cache line *)
  i := 0;
  WHILE i < n DO
    PokeChar(dst, dstOff + i, PeekChar(src, srcOff + i));
    INC(i)
  END
END CopyBytes;

(* ── Buffer lifecycle ────────────────────────────────── *)

PROCEDURE Init(VAR b: Buf; initialCap: CARDINAL);
VAR c: CARDINAL;
BEGIN
  c := initialCap;
  IF c > MaxBufCap THEN c := MaxBufCap END;
  IF c = 0 THEN c := 64 END;
  ALLOCATE(b.data, c);
  b.len := 0;
  b.cap := c
END Init;

PROCEDURE Free(VAR b: Buf);
BEGIN
  IF b.data # NIL THEN
    DEALLOCATE(b.data, b.cap);
    b.data := NIL
  END;
  b.len := 0;
  b.cap := 0
END Free;

PROCEDURE Clear(VAR b: Buf);
BEGIN
  b.len := 0
END Clear;

(* ── Capacity management ────────────────────────────── *)

PROCEDURE Reserve(VAR b: Buf; extra: CARDINAL): BOOLEAN;
VAR
  needed, newCap: CARDINAL;
  newData: ADDRESS;
BEGIN
  needed := b.len + extra;
  IF needed <= b.cap THEN RETURN TRUE END;
  IF needed > MaxBufCap THEN RETURN FALSE END;

  (* geometric growth: double, or needed, whichever is larger *)
  newCap := b.cap * 2;
  IF newCap < needed THEN newCap := needed END;
  IF newCap > MaxBufCap THEN newCap := MaxBufCap END;

  ALLOCATE(newData, newCap);
  IF newData = NIL THEN RETURN FALSE END;

  (* copy existing data *)
  IF b.len > 0 THEN
    CopyBytes(b.data, newData, 0, 0, b.len)
  END;

  (* free old buffer *)
  DEALLOCATE(b.data, b.cap);

  b.data := newData;
  b.cap := newCap;
  RETURN TRUE
END Reserve;

(* ── Append operations ──────────────────────────────── *)

PROCEDURE AppendByte(VAR b: Buf; x: CARDINAL): BOOLEAN;
BEGIN
  IF NOT Reserve(b, 1) THEN RETURN FALSE END;
  PokeChar(b.data, b.len, CHR(x MOD 256));
  INC(b.len);
  RETURN TRUE
END AppendByte;

PROCEDURE AppendChars(VAR b: Buf; a: ARRAY OF CHAR;
                      n: CARDINAL): BOOLEAN;
VAR count, i: CARDINAL;
BEGIN
  count := n;
  IF count > HIGH(a) + 1 THEN count := HIGH(a) + 1 END;
  IF count = 0 THEN RETURN TRUE END;
  IF NOT Reserve(b, count) THEN RETURN FALSE END;
  i := 0;
  WHILE i < count DO
    PokeChar(b.data, b.len + i, a[i]);
    INC(i)
  END;
  b.len := b.len + count;
  RETURN TRUE
END AppendChars;

PROCEDURE AppendView(VAR b: Buf; v: BytesView): BOOLEAN;
BEGIN
  IF v.len = 0 THEN RETURN TRUE END;
  IF NOT Reserve(b, v.len) THEN RETURN FALSE END;
  CopyBytes(v.base, b.data, 0, b.len, v.len);
  b.len := b.len + v.len;
  RETURN TRUE
END AppendView;

(* ── Access ─────────────────────────────────────────── *)

PROCEDURE GetByte(VAR b: Buf; idx: CARDINAL): CARDINAL;
BEGIN
  IF idx >= b.len THEN RETURN 0 END;
  RETURN ORD(PeekChar(b.data, idx)) MOD 256
END GetByte;

PROCEDURE SetByte(VAR b: Buf; idx: CARDINAL; val: CARDINAL);
BEGIN
  IF idx >= b.len THEN RETURN END;
  PokeChar(b.data, idx, CHR(val MOD 256))
END SetByte;

PROCEDURE AsView(VAR b: Buf): BytesView;
VAR v: BytesView;
BEGIN
  v.base := b.data;
  v.len := b.len;
  RETURN v
END AsView;

PROCEDURE Truncate(VAR b: Buf; newLen: CARDINAL);
BEGIN
  IF newLen < b.len THEN b.len := newLen END
END Truncate;

PROCEDURE DataPtr(VAR b: Buf): ADDRESS;
BEGIN
  RETURN b.data
END DataPtr;

(* ── View helpers ───────────────────────────────────── *)

PROCEDURE ViewGetByte(v: BytesView; idx: CARDINAL): CARDINAL;
BEGIN
  IF idx >= v.len THEN RETURN 0 END;
  RETURN ORD(PeekChar(v.base, idx)) MOD 256
END ViewGetByte;

END ByteBuf.
