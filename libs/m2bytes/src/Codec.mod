IMPLEMENTATION MODULE Codec;

FROM ByteBuf IMPORT BytesView, Buf,
                     GetByte, AppendByte, Reserve, ViewGetByte;
FROM SYSTEM IMPORT ADDRESS, ADR;

(* ── Internal: read byte from view at absolute index ── *)

PROCEDURE ViewByte(VAR v: BytesView; idx: CARDINAL): CARDINAL;
BEGIN
  RETURN ViewGetByte(v, idx)
END ViewByte;

(* ── Reader ─────────────────────────────────────────── *)

PROCEDURE InitReader(VAR r: Reader; v: BytesView);
BEGIN
  r.v := v;
  r.pos := 0
END InitReader;

PROCEDURE Remaining(VAR r: Reader): CARDINAL;
BEGIN
  IF r.pos >= r.v.len THEN RETURN 0 END;
  RETURN r.v.len - r.pos
END Remaining;

PROCEDURE ReadU8(VAR r: Reader; VAR ok: BOOLEAN): CARDINAL;
VAR val: CARDINAL;
BEGIN
  IF r.pos >= r.v.len THEN ok := FALSE; RETURN 0 END;
  val := ViewByte(r.v, r.pos);
  INC(r.pos);
  ok := TRUE;
  RETURN val
END ReadU8;

PROCEDURE ReadU16LE(VAR r: Reader; VAR ok: BOOLEAN): CARDINAL;
VAR lo, hi: CARDINAL;
BEGIN
  IF r.pos + 2 > r.v.len THEN ok := FALSE; RETURN 0 END;
  lo := ViewByte(r.v, r.pos);
  hi := ViewByte(r.v, r.pos + 1);
  r.pos := r.pos + 2;
  ok := TRUE;
  RETURN lo + hi * 256
END ReadU16LE;

PROCEDURE ReadU16BE(VAR r: Reader; VAR ok: BOOLEAN): CARDINAL;
VAR lo, hi: CARDINAL;
BEGIN
  IF r.pos + 2 > r.v.len THEN ok := FALSE; RETURN 0 END;
  hi := ViewByte(r.v, r.pos);
  lo := ViewByte(r.v, r.pos + 1);
  r.pos := r.pos + 2;
  ok := TRUE;
  RETURN lo + hi * 256
END ReadU16BE;

PROCEDURE ReadU32LE(VAR r: Reader; VAR ok: BOOLEAN): CARDINAL;
VAR b0, b1, b2, b3: CARDINAL;
BEGIN
  IF r.pos + 4 > r.v.len THEN ok := FALSE; RETURN 0 END;
  b0 := ViewByte(r.v, r.pos);
  b1 := ViewByte(r.v, r.pos + 1);
  b2 := ViewByte(r.v, r.pos + 2);
  b3 := ViewByte(r.v, r.pos + 3);
  r.pos := r.pos + 4;
  ok := TRUE;
  RETURN b0 + b1 * 256 + b2 * 65536 + b3 * 16777216
END ReadU32LE;

PROCEDURE ReadU32BE(VAR r: Reader; VAR ok: BOOLEAN): CARDINAL;
VAR b0, b1, b2, b3: CARDINAL;
BEGIN
  IF r.pos + 4 > r.v.len THEN ok := FALSE; RETURN 0 END;
  b3 := ViewByte(r.v, r.pos);
  b2 := ViewByte(r.v, r.pos + 1);
  b1 := ViewByte(r.v, r.pos + 2);
  b0 := ViewByte(r.v, r.pos + 3);
  r.pos := r.pos + 4;
  ok := TRUE;
  RETURN b0 + b1 * 256 + b2 * 65536 + b3 * 16777216
END ReadU32BE;

PROCEDURE ReadI32LE(VAR r: Reader; VAR ok: BOOLEAN): INTEGER;
VAR u: CARDINAL;
BEGIN
  u := ReadU32LE(r, ok);
  IF NOT ok THEN RETURN 0 END;
  RETURN INTEGER(u)
END ReadI32LE;

PROCEDURE ReadI32BE(VAR r: Reader; VAR ok: BOOLEAN): INTEGER;
VAR u: CARDINAL;
BEGIN
  u := ReadU32BE(r, ok);
  IF NOT ok THEN RETURN 0 END;
  RETURN INTEGER(u)
END ReadI32BE;

PROCEDURE Skip(VAR r: Reader; n: CARDINAL; VAR ok: BOOLEAN);
BEGIN
  IF r.pos + n > r.v.len THEN ok := FALSE; RETURN END;
  r.pos := r.pos + n;
  ok := TRUE
END Skip;

PROCEDURE ReadSlice(VAR r: Reader; n: CARDINAL;
                    VAR out: BytesView; VAR ok: BOOLEAN);
BEGIN
  IF r.pos + n > r.v.len THEN
    ok := FALSE;
    out.base := NIL;
    out.len := 0;
    RETURN
  END;
  out.base := ADDRESS(LONGCARD(r.v.base) + LONGCARD(r.pos));
  out.len := n;
  r.pos := r.pos + n;
  ok := TRUE
END ReadSlice;

(* ── Writer ─────────────────────────────────────────── *)

PROCEDURE InitWriter(VAR w: Writer; VAR b: Buf);
BEGIN
  w.buf := ADR(b)
END InitWriter;

PROCEDURE WriteU8(VAR w: Writer; val: CARDINAL);
BEGIN
  AppendByte(w.buf^, val MOD 256)
END WriteU8;

PROCEDURE WriteU16LE(VAR w: Writer; val: CARDINAL);
BEGIN
  AppendByte(w.buf^, val MOD 256);
  AppendByte(w.buf^, (val DIV 256) MOD 256)
END WriteU16LE;

PROCEDURE WriteU16BE(VAR w: Writer; val: CARDINAL);
BEGIN
  AppendByte(w.buf^, (val DIV 256) MOD 256);
  AppendByte(w.buf^, val MOD 256)
END WriteU16BE;

PROCEDURE WriteU32LE(VAR w: Writer; val: CARDINAL);
BEGIN
  AppendByte(w.buf^, val MOD 256);
  AppendByte(w.buf^, (val DIV 256) MOD 256);
  AppendByte(w.buf^, (val DIV 65536) MOD 256);
  AppendByte(w.buf^, (val DIV 16777216) MOD 256)
END WriteU32LE;

PROCEDURE WriteU32BE(VAR w: Writer; val: CARDINAL);
BEGIN
  AppendByte(w.buf^, (val DIV 16777216) MOD 256);
  AppendByte(w.buf^, (val DIV 65536) MOD 256);
  AppendByte(w.buf^, (val DIV 256) MOD 256);
  AppendByte(w.buf^, val MOD 256)
END WriteU32BE;

PROCEDURE WriteI32LE(VAR w: Writer; val: INTEGER);
BEGIN
  WriteU32LE(w, CARDINAL(val))
END WriteI32LE;

PROCEDURE WriteI32BE(VAR w: Writer; val: INTEGER);
BEGIN
  WriteU32BE(w, CARDINAL(val))
END WriteI32BE;

PROCEDURE WriteChars(VAR w: Writer; a: ARRAY OF CHAR; n: CARDINAL);
VAR count, i: CARDINAL;
BEGIN
  count := n;
  IF count > HIGH(a) + 1 THEN count := HIGH(a) + 1 END;
  i := 0;
  WHILE i < count DO
    AppendByte(w.buf^, ORD(a[i]) MOD 256);
    INC(i)
  END
END WriteChars;

(* ── Varint (LEB128) ────────────────────────────────── *)

PROCEDURE WriteVarU32(VAR w: Writer; val: CARDINAL);
VAR v: CARDINAL;
BEGIN
  v := val;
  WHILE v >= 128 DO
    AppendByte(w.buf^, (v MOD 128) + 128);
    v := v DIV 128
  END;
  AppendByte(w.buf^, v)
END WriteVarU32;

PROCEDURE ReadVarU32(VAR r: Reader; VAR ok: BOOLEAN): CARDINAL;
VAR
  result, shift, b: CARDINAL;
  count: CARDINAL;
  savePos: CARDINAL;
BEGIN
  savePos := r.pos;
  result := 0;
  shift := 1;  (* multiplicative shift: 1, 128, 16384, ... *)
  count := 0;
  LOOP
    IF r.pos >= r.v.len THEN
      r.pos := savePos;
      ok := FALSE;
      RETURN 0
    END;
    b := ViewByte(r.v, r.pos);
    INC(r.pos);
    INC(count);
    result := result + (b MOD 128) * shift;
    IF b < 128 THEN
      ok := TRUE;
      RETURN result
    END;
    IF count >= 5 THEN
      (* too many bytes for 32-bit varint *)
      r.pos := savePos;
      ok := FALSE;
      RETURN 0
    END;
    shift := shift * 128
  END
END ReadVarU32;

(* ── ZigZag encoding ────────────────────────────────── *)

PROCEDURE ZigZagEncode(val: INTEGER): CARDINAL;
BEGIN
  IF val >= 0 THEN
    RETURN CARDINAL(val) * 2
  ELSE
    RETURN CARDINAL(-(val + 1)) * 2 + 1
  END
END ZigZagEncode;

PROCEDURE ZigZagDecode(val: CARDINAL): INTEGER;
BEGIN
  IF val MOD 2 = 0 THEN
    RETURN INTEGER(val DIV 2)
  ELSE
    RETURN -(INTEGER(val DIV 2)) - 1
  END
END ZigZagDecode;

PROCEDURE WriteVarI32(VAR w: Writer; val: INTEGER);
BEGIN
  WriteVarU32(w, ZigZagEncode(val))
END WriteVarI32;

PROCEDURE ReadVarI32(VAR r: Reader; VAR ok: BOOLEAN): INTEGER;
VAR u: CARDINAL;
BEGIN
  u := ReadVarU32(r, ok);
  IF NOT ok THEN RETURN 0 END;
  RETURN ZigZagDecode(u)
END ReadVarI32;

END Codec.
