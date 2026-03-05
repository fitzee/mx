IMPLEMENTATION MODULE ByteBuf;

FROM SYSTEM IMPORT ADDRESS, ADR, TSIZE;
FROM Storage IMPORT ALLOCATE, DEALLOCATE;

(* ── Internal helpers ────────────────────────────────── *)

PROCEDURE MinCap(a, b: CARDINAL): CARDINAL;
BEGIN
  IF a < b THEN RETURN a ELSE RETURN b END
END MinCap;

PROCEDURE CopyBytes(src, dst: BBufPtr; srcOff, dstOff, n: CARDINAL);
VAR i: CARDINAL;
BEGIN
  i := 0;
  WHILE i < n DO
    dst^[dstOff + i] := src^[srcOff + i];
    INC(i)
  END
END CopyBytes;

(* ── Buffer lifecycle ────────────────────────────────── *)

PROCEDURE Init(VAR b: Buf; initialCap: CARDINAL);
VAR c: CARDINAL; p: ADDRESS;
BEGIN
  c := initialCap;
  IF c > MaxBufCap THEN c := MaxBufCap END;
  IF c = 0 THEN c := 64 END;
  ALLOCATE(p, c);
  b.data := p;
  b.len := 0;
  b.cap := c
END Init;

PROCEDURE Free(VAR b: Buf);
VAR p: ADDRESS;
BEGIN
  IF b.data # NIL THEN
    p := b.data;
    DEALLOCATE(p, b.cap);
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
  p: ADDRESS;
  newData: BBufPtr;
BEGIN
  needed := b.len + extra;
  IF needed <= b.cap THEN RETURN TRUE END;
  IF needed > MaxBufCap THEN RETURN FALSE END;

  (* geometric growth: double, or needed, whichever is larger *)
  newCap := b.cap * 2;
  IF newCap < needed THEN newCap := needed END;
  IF newCap > MaxBufCap THEN newCap := MaxBufCap END;

  ALLOCATE(p, newCap);
  newData := p;
  IF newData = NIL THEN RETURN FALSE END;

  (* copy existing data *)
  IF b.len > 0 THEN
    CopyBytes(b.data, newData, 0, 0, b.len)
  END;

  (* free old buffer *)
  p := b.data;
  DEALLOCATE(p, b.cap);

  b.data := newData;
  b.cap := newCap;
  RETURN TRUE
END Reserve;

(* ── Append operations ──────────────────────────────── *)

PROCEDURE AppendByte(VAR b: Buf; x: CARDINAL);
BEGIN
  IF Reserve(b, 1) THEN
    b.data^[b.len] := CHR(x MOD 256);
    INC(b.len)
  END
END AppendByte;

PROCEDURE AppendChars(VAR b: Buf; a: ARRAY OF CHAR; n: CARDINAL);
VAR count, i: CARDINAL;
BEGIN
  count := n;
  IF count > HIGH(a) + 1 THEN count := HIGH(a) + 1 END;
  IF count = 0 THEN RETURN END;
  IF Reserve(b, count) THEN
    i := 0;
    WHILE i < count DO
      b.data^[b.len + i] := a[i];
      INC(i)
    END;
    b.len := b.len + count
  END
END AppendChars;

PROCEDURE AppendView(VAR b: Buf; v: BytesView);
VAR i: CARDINAL; vp: BBufPtr;
BEGIN
  IF v.len = 0 THEN RETURN END;
  IF Reserve(b, v.len) THEN
    vp := v.base;
    i := 0;
    WHILE i < v.len DO
      b.data^[b.len + i] := vp^[i];
      INC(i)
    END;
    b.len := b.len + v.len
  END
END AppendView;

(* ── Access ─────────────────────────────────────────── *)

PROCEDURE GetByte(VAR b: Buf; idx: CARDINAL): CARDINAL;
BEGIN
  IF idx >= b.len THEN RETURN 0 END;
  RETURN ORD(b.data^[idx]) MOD 256
END GetByte;

PROCEDURE SetByte(VAR b: Buf; idx: CARDINAL; val: CARDINAL);
BEGIN
  IF idx >= b.len THEN RETURN END;
  b.data^[idx] := CHR(val MOD 256)
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
VAR vp: BBufPtr;
BEGIN
  IF idx >= v.len THEN RETURN 0 END;
  vp := v.base;
  RETURN ORD(vp^[idx]) MOD 256
END ViewGetByte;

END ByteBuf.
