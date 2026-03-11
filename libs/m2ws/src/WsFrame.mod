IMPLEMENTATION MODULE WsFrame;

FROM SYSTEM IMPORT ADDRESS, ADR, LONGCARD, TSIZE;
FROM WsBridge IMPORT m2_ws_apply_mask, m2_ws_random_mask;

(* ── Internal types ────────────────────────────────────── *)

TYPE
  CharPtr = POINTER TO CHAR;

(* ── Helpers ───────────────────────────────────────────── *)

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

PROCEDURE GetByte(base: ADDRESS; idx: CARDINAL): CARDINAL;
BEGIN
  RETURN ORD(PeekChar(base, idx)) MOD 256
END GetByte;

PROCEDURE SetByte(base: ADDRESS; idx: CARDINAL; val: CARDINAL);
BEGIN
  PokeChar(base, idx, CHR(val MOD 256))
END SetByte;

(* ── IntToOpcode / OpcodeToInt ─────────────────────────── *)

PROCEDURE IntToOpcode(n: CARDINAL): Opcode;
BEGIN
  CASE n OF
    0:  RETURN OpContinuation |
    1:  RETURN OpText |
    2:  RETURN OpBinary |
    3:  RETURN OpReserved3 |
    4:  RETURN OpReserved4 |
    5:  RETURN OpReserved5 |
    6:  RETURN OpReserved6 |
    7:  RETURN OpReserved7 |
    8:  RETURN OpClose |
    9:  RETURN OpPing |
    10: RETURN OpPong
  ELSE
    RETURN OpContinuation
  END
END IntToOpcode;

PROCEDURE OpcodeToInt(op: Opcode): CARDINAL;
BEGIN
  CASE op OF
    OpContinuation: RETURN 0 |
    OpText:         RETURN 1 |
    OpBinary:       RETURN 2 |
    OpReserved3:    RETURN 3 |
    OpReserved4:    RETURN 4 |
    OpReserved5:    RETURN 5 |
    OpReserved6:    RETURN 6 |
    OpReserved7:    RETURN 7 |
    OpClose:        RETURN 8 |
    OpPing:         RETURN 9 |
    OpPong:         RETURN 10
  ELSE
    RETURN 0
  END
END OpcodeToInt;

(* ── DecodeHeader ──────────────────────────────────────── *)

PROCEDURE DecodeHeader(buf: ADDRESS; bufLen: CARDINAL;
                       VAR hdr: FrameHeader): Status;
VAR
  b0, b1, opcodeVal: CARDINAL;
  pos: CARDINAL;
  len7: CARDINAL;
  len16, len64hi, len64lo: CARDINAL;
  i: CARDINAL;
BEGIN
  IF bufLen < 2 THEN RETURN Incomplete END;

  b0 := GetByte(buf, 0);
  b1 := GetByte(buf, 1);

  (* FIN bit *)
  hdr.fin := (b0 DIV 128) = 1;

  (* RSV bits must be 0 (no extensions) *)
  IF (b0 DIV 16) MOD 8 # 0 THEN RETURN Invalid END;

  (* Opcode *)
  opcodeVal := b0 MOD 16;
  IF opcodeVal > 10 THEN RETURN Invalid END;
  (* Opcodes 3-7 are reserved non-control *)
  IF (opcodeVal >= 3) AND (opcodeVal <= 7) THEN RETURN Invalid END;
  hdr.opcode := IntToOpcode(opcodeVal);

  (* Control frames (opcode >= 8) must have FIN=1 *)
  IF (opcodeVal >= 8) AND (NOT hdr.fin) THEN RETURN Invalid END;

  (* Mask bit *)
  hdr.masked := (b1 DIV 128) = 1;

  (* Payload length *)
  len7 := b1 MOD 128;
  pos := 2;

  IF len7 <= 125 THEN
    hdr.payloadLen := len7
  ELSIF len7 = 126 THEN
    (* 16-bit extended length *)
    IF bufLen < 4 THEN RETURN Incomplete END;
    len16 := GetByte(buf,2) * 256 + GetByte(buf,3);
    hdr.payloadLen := len16;
    pos := 4
  ELSIF len7 = 127 THEN
    (* 64-bit extended length *)
    IF bufLen < 10 THEN RETURN Incomplete END;
    (* Check high 4 bytes are zero -- we only support 32-bit lengths *)
    len64hi := GetByte(buf,2) * 16777216 + GetByte(buf,3) * 65536
             + GetByte(buf,4) * 256 + GetByte(buf,5);
    IF len64hi # 0 THEN RETURN Invalid END;
    len64lo := GetByte(buf,6) * 16777216 + GetByte(buf,7) * 65536
             + GetByte(buf,8) * 256 + GetByte(buf,9);
    hdr.payloadLen := len64lo;
    pos := 10
  END;

  (* Control frames must have payload <= 125 *)
  IF (opcodeVal >= 8) AND (hdr.payloadLen > 125) THEN
    RETURN Invalid
  END;

  (* Read mask key if masked *)
  IF hdr.masked THEN
    IF bufLen < pos + 4 THEN RETURN Incomplete END;
    FOR i := 0 TO 3 DO
      hdr.maskKey[i] := CHR(GetByte(buf,pos + i))
    END;
    pos := pos + 4
  ELSE
    hdr.maskKey[0] := CHR(0);
    hdr.maskKey[1] := CHR(0);
    hdr.maskKey[2] := CHR(0);
    hdr.maskKey[3] := CHR(0)
  END;

  hdr.headerLen := pos;
  RETURN Ok
END DecodeHeader;

(* ── ApplyMask ─────────────────────────────────────────── *)

PROCEDURE ApplyMask(data: ADDRESS; len: CARDINAL;
                    VAR mask: ARRAY OF CHAR; offset: CARDINAL);
BEGIN
  IF len = 0 THEN RETURN END;
  m2_ws_apply_mask(data, VAL(INTEGER, len), ADR(mask), VAL(INTEGER, offset))
END ApplyMask;

(* ── EncodeHeader ──────────────────────────────────────── *)

PROCEDURE EncodeHeader(VAR hdr: FrameHeader;
                       buf: ADDRESS; maxLen: CARDINAL): CARDINAL;
VAR
  b0, b1: CARDINAL;
  pos: CARDINAL;
  i: CARDINAL;
BEGIN
  pos := 0;

  (* First byte: FIN + opcode *)
  b0 := OpcodeToInt(hdr.opcode);
  IF hdr.fin THEN b0 := b0 + 128 END;

  (* Calculate needed header size *)
  IF hdr.payloadLen <= 125 THEN
    IF hdr.masked THEN
      IF maxLen < 6 THEN RETURN 0 END
    ELSE
      IF maxLen < 2 THEN RETURN 0 END
    END
  ELSIF hdr.payloadLen <= 65535 THEN
    IF hdr.masked THEN
      IF maxLen < 8 THEN RETURN 0 END
    ELSE
      IF maxLen < 4 THEN RETURN 0 END
    END
  ELSE
    IF hdr.masked THEN
      IF maxLen < 14 THEN RETURN 0 END
    ELSE
      IF maxLen < 10 THEN RETURN 0 END
    END
  END;

  SetByte(buf,0, b0);
  pos := 1;

  (* Second byte: MASK + payload length *)
  IF hdr.payloadLen <= 125 THEN
    b1 := hdr.payloadLen;
    IF hdr.masked THEN b1 := b1 + 128 END;
    SetByte(buf,1, b1);
    pos := 2
  ELSIF hdr.payloadLen <= 65535 THEN
    b1 := 126;
    IF hdr.masked THEN b1 := b1 + 128 END;
    SetByte(buf,1, b1);
    (* 16-bit big-endian length *)
    SetByte(buf,2, hdr.payloadLen DIV 256);
    SetByte(buf,3, hdr.payloadLen MOD 256);
    pos := 4
  ELSE
    b1 := 127;
    IF hdr.masked THEN b1 := b1 + 128 END;
    SetByte(buf,1, b1);
    (* 64-bit big-endian length; high 4 bytes = 0 *)
    SetByte(buf,2, 0);
    SetByte(buf,3, 0);
    SetByte(buf,4, 0);
    SetByte(buf,5, 0);
    SetByte(buf,6, (hdr.payloadLen DIV 16777216) MOD 256);
    SetByte(buf,7, (hdr.payloadLen DIV 65536) MOD 256);
    SetByte(buf,8, (hdr.payloadLen DIV 256) MOD 256);
    SetByte(buf,9, hdr.payloadLen MOD 256);
    pos := 10
  END;

  (* Mask key *)
  IF hdr.masked THEN
    FOR i := 0 TO 3 DO
      SetByte(buf,pos, ORD(hdr.maskKey[i]) MOD 256);
      INC(pos)
    END
  END;

  hdr.headerLen := pos;
  RETURN pos
END EncodeHeader;

(* ── GenerateMask ──────────────────────────────────────── *)

PROCEDURE GenerateMask(VAR mask: ARRAY OF CHAR);
BEGIN
  m2_ws_random_mask(ADR(mask))
END GenerateMask;

BEGIN
  (* no module init needed *)
END WsFrame.
