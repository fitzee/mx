MODULE WsTests;
(* Deterministic test suite for m2ws.

   Tests:
     1. WsFrame encode/decode roundtrip (small payload)
     2. WsFrame encode/decode with 16-bit extended length
     3. WsFrame encode/decode with 64-bit extended length
     4. Mask application and removal
     5. Frame header parsing for unmasked frames
     6. Frame header parsing for masked frames
     7. Control frame (ping) encode/decode
     8. Close frame encode/decode
     9. Incomplete header detection
    10. Invalid frame rejection (bad RSV bits)
    11. Invalid frame rejection (fragmented control)
    12. Opcode conversion roundtrip
    13. GenerateMask produces 4 bytes
    14. SHA-1 known vector
    15. Base64 known vector
    16. WebSocket accept key computation *)

FROM SYSTEM IMPORT ADDRESS, ADR, LONGCARD, TSIZE;
FROM InOut IMPORT WriteString, WriteLn, WriteInt;
FROM WsFrame IMPORT Opcode, FrameHeader, MaxFrameHeader,
                    DecodeHeader, EncodeHeader, ApplyMask,
                    GenerateMask, IntToOpcode, OpcodeToInt,
                    OpContinuation, OpText, OpBinary,
                    OpClose, OpPing, OpPong;
IMPORT WsFrame;
FROM WsBridge IMPORT m2_ws_sha1, m2_ws_base64_encode, m2_ws_apply_mask;

VAR
  passed, failed, total: INTEGER;

TYPE
  CharPtr = POINTER TO CHAR;

PROCEDURE Check(name: ARRAY OF CHAR; cond: BOOLEAN);
BEGIN
  INC(total);
  IF cond THEN
    INC(passed)
  ELSE
    INC(failed);
    WriteString("FAIL: "); WriteString(name); WriteLn
  END
END Check;

PROCEDURE GetByte(p: ADDRESS; idx: CARDINAL): CARDINAL;
VAR cp: CharPtr;
BEGIN
  cp := CharPtr(LONGCARD(p) + LONGCARD(idx));
  RETURN ORD(cp^) MOD 256
END GetByte;

PROCEDURE SetByte(p: ADDRESS; idx: CARDINAL; val: CARDINAL);
VAR cp: CharPtr;
BEGIN
  cp := CharPtr(LONGCARD(p) + LONGCARD(idx));
  cp^ := CHR(val MOD 256)
END SetByte;

(* ── Test 1: Small frame roundtrip ──────────────── *)

PROCEDURE TestSmallFrame;
VAR
  hdr, hdr2: FrameHeader;
  buf: ARRAY [0..31] OF CHAR;
  n: CARDINAL;
  st: WsFrame.Status;
BEGIN
  (* Encode a text frame, FIN=TRUE, unmasked, 5-byte payload *)
  hdr.fin := TRUE;
  hdr.opcode := OpText;
  hdr.masked := FALSE;
  hdr.payloadLen := 5;

  n := EncodeHeader(hdr, ADR(buf), 32);
  Check("small: encode ok", n > 0);
  Check("small: header 2 bytes", n = 2);

  (* Decode it back *)
  st := DecodeHeader(ADR(buf), n, hdr2);
  Check("small: decode ok", st = WsFrame.Ok);
  Check("small: fin", hdr2.fin = TRUE);
  Check("small: opcode text", hdr2.opcode = OpText);
  Check("small: unmasked", hdr2.masked = FALSE);
  Check("small: payloadLen=5", hdr2.payloadLen = 5);
  Check("small: headerLen=2", hdr2.headerLen = 2)
END TestSmallFrame;

(* ── Test 2: 16-bit extended length ─────────────── *)

PROCEDURE TestExtLen16;
VAR
  hdr, hdr2: FrameHeader;
  buf: ARRAY [0..31] OF CHAR;
  n: CARDINAL;
  st: WsFrame.Status;
BEGIN
  hdr.fin := TRUE;
  hdr.opcode := OpBinary;
  hdr.masked := FALSE;
  hdr.payloadLen := 300;  (* > 125, needs 16-bit ext *)

  n := EncodeHeader(hdr, ADR(buf), 32);
  Check("ext16: encode ok", n > 0);
  Check("ext16: header 4 bytes", n = 4);

  st := DecodeHeader(ADR(buf), n, hdr2);
  Check("ext16: decode ok", st = WsFrame.Ok);
  Check("ext16: payloadLen=300", hdr2.payloadLen = 300);
  Check("ext16: binary", hdr2.opcode = OpBinary);
  Check("ext16: headerLen=4", hdr2.headerLen = 4)
END TestExtLen16;

(* ── Test 3: 64-bit extended length ─────────────── *)

PROCEDURE TestExtLen64;
VAR
  hdr, hdr2: FrameHeader;
  buf: ARRAY [0..31] OF CHAR;
  n: CARDINAL;
  st: WsFrame.Status;
BEGIN
  hdr.fin := TRUE;
  hdr.opcode := OpBinary;
  hdr.masked := FALSE;
  hdr.payloadLen := 70000;  (* > 65535, needs 64-bit ext *)

  n := EncodeHeader(hdr, ADR(buf), 32);
  Check("ext64: encode ok", n > 0);
  Check("ext64: header 10 bytes", n = 10);

  st := DecodeHeader(ADR(buf), n, hdr2);
  Check("ext64: decode ok", st = WsFrame.Ok);
  Check("ext64: payloadLen=70000", hdr2.payloadLen = 70000);
  Check("ext64: headerLen=10", hdr2.headerLen = 10)
END TestExtLen64;

(* ── Test 4: Mask application ───────────────────── *)

PROCEDURE TestMask;
VAR
  data: ARRAY [0..7] OF CHAR;
  mask: ARRAY [0..3] OF CHAR;
  orig0, orig1, orig2, orig3: CARDINAL;
BEGIN
  (* Set up data *)
  data[0] := CHR(72);   (* 'H' *)
  data[1] := CHR(101);  (* 'e' *)
  data[2] := CHR(108);  (* 'l' *)
  data[3] := CHR(108);  (* 'l' *)
  data[4] := CHR(111);  (* 'o' *)

  orig0 := ORD(data[0]);
  orig1 := ORD(data[1]);
  orig2 := ORD(data[2]);
  orig3 := ORD(data[3]);

  mask[0] := CHR(37);
  mask[1] := CHR(82);
  mask[2] := CHR(191);
  mask[3] := CHR(64);

  (* Apply mask *)
  ApplyMask(ADR(data), 5, mask, 0);
  Check("mask: data changed", ORD(data[0]) # orig0);

  (* Apply mask again to unmask *)
  ApplyMask(ADR(data), 5, mask, 0);
  Check("mask: roundtrip 0", ORD(data[0]) = 72);
  Check("mask: roundtrip 1", ORD(data[1]) = 101);
  Check("mask: roundtrip 2", ORD(data[2]) = 108);
  Check("mask: roundtrip 3", ORD(data[3]) = 108);
  Check("mask: roundtrip 4", ORD(data[4]) = 111)
END TestMask;

(* ── Test 5: Unmasked server frame ──────────────── *)

PROCEDURE TestUnmaskedDecode;
VAR
  buf: ARRAY [0..7] OF CHAR;
  hdr: FrameHeader;
  st: WsFrame.Status;
BEGIN
  (* Construct: FIN=1, opcode=1 (text), mask=0, len=5 *)
  buf[0] := CHR(129);  (* 10000001: FIN + text *)
  buf[1] := CHR(5);    (* 00000101: len=5, no mask *)

  st := DecodeHeader(ADR(buf), 2, hdr);
  Check("unmasked: ok", st = WsFrame.Ok);
  Check("unmasked: fin", hdr.fin = TRUE);
  Check("unmasked: text", hdr.opcode = OpText);
  Check("unmasked: no mask", hdr.masked = FALSE);
  Check("unmasked: len=5", hdr.payloadLen = 5);
  Check("unmasked: hdrLen=2", hdr.headerLen = 2)
END TestUnmaskedDecode;

(* ── Test 6: Masked client frame ────────────────── *)

PROCEDURE TestMaskedDecode;
VAR
  buf: ARRAY [0..15] OF CHAR;
  hdr: FrameHeader;
  st: WsFrame.Status;
BEGIN
  (* Construct: FIN=1, opcode=1, mask=1, len=5, mask key=AA BB CC DD *)
  buf[0] := CHR(129);  (* FIN + text *)
  buf[1] := CHR(133);  (* 10000101: mask + len=5 *)
  buf[2] := CHR(170);  (* mask key byte 0 *)
  buf[3] := CHR(187);  (* mask key byte 1 *)
  buf[4] := CHR(204);  (* mask key byte 2 *)
  buf[5] := CHR(221);  (* mask key byte 3 *)

  st := DecodeHeader(ADR(buf), 6, hdr);
  Check("masked: ok", st = WsFrame.Ok);
  Check("masked: fin", hdr.fin = TRUE);
  Check("masked: text", hdr.opcode = OpText);
  Check("masked: is masked", hdr.masked = TRUE);
  Check("masked: len=5", hdr.payloadLen = 5);
  Check("masked: hdrLen=6", hdr.headerLen = 6);
  Check("masked: key[0]", ORD(hdr.maskKey[0]) = 170);
  Check("masked: key[1]", ORD(hdr.maskKey[1]) = 187);
  Check("masked: key[2]", ORD(hdr.maskKey[2]) = 204);
  Check("masked: key[3]", ORD(hdr.maskKey[3]) = 221)
END TestMaskedDecode;

(* ── Test 7: Ping frame ─────────────────────────── *)

PROCEDURE TestPingFrame;
VAR
  hdr, hdr2: FrameHeader;
  buf: ARRAY [0..15] OF CHAR;
  n: CARDINAL;
  st: WsFrame.Status;
BEGIN
  hdr.fin := TRUE;
  hdr.opcode := OpPing;
  hdr.masked := FALSE;
  hdr.payloadLen := 0;

  n := EncodeHeader(hdr, ADR(buf), 16);
  Check("ping: encode ok", n = 2);

  st := DecodeHeader(ADR(buf), n, hdr2);
  Check("ping: decode ok", st = WsFrame.Ok);
  Check("ping: fin", hdr2.fin = TRUE);
  Check("ping: opcode", hdr2.opcode = OpPing);
  Check("ping: len=0", hdr2.payloadLen = 0)
END TestPingFrame;

(* ── Test 8: Close frame ────────────────────────── *)

PROCEDURE TestCloseFrame;
VAR
  hdr, hdr2: FrameHeader;
  buf: ARRAY [0..15] OF CHAR;
  n: CARDINAL;
  st: WsFrame.Status;
BEGIN
  hdr.fin := TRUE;
  hdr.opcode := OpClose;
  hdr.masked := FALSE;
  hdr.payloadLen := 2;  (* status code only *)

  n := EncodeHeader(hdr, ADR(buf), 16);
  Check("close: encode ok", n = 2);

  st := DecodeHeader(ADR(buf), n, hdr2);
  Check("close: decode ok", st = WsFrame.Ok);
  Check("close: opcode", hdr2.opcode = OpClose);
  Check("close: len=2", hdr2.payloadLen = 2)
END TestCloseFrame;

(* ── Test 9: Incomplete header ──────────────────── *)

PROCEDURE TestIncomplete;
VAR
  buf: ARRAY [0..1] OF CHAR;
  hdr: FrameHeader;
  st: WsFrame.Status;
BEGIN
  (* Only 1 byte *)
  buf[0] := CHR(129);
  st := DecodeHeader(ADR(buf), 1, hdr);
  Check("incomplete: 1 byte", st = WsFrame.Incomplete);

  (* 0 bytes *)
  st := DecodeHeader(ADR(buf), 0, hdr);
  Check("incomplete: 0 bytes", st = WsFrame.Incomplete)
END TestIncomplete;

(* ── Test 10: Invalid RSV bits ──────────────────── *)

PROCEDURE TestInvalidRSV;
VAR
  buf: ARRAY [0..3] OF CHAR;
  hdr: FrameHeader;
  st: WsFrame.Status;
BEGIN
  (* RSV1 set: 0x90 = 10010000 *)
  buf[0] := CHR(144 + 1);  (* RSV1 + FIN + text *)
  buf[1] := CHR(0);
  st := DecodeHeader(ADR(buf), 2, hdr);
  Check("rsv: invalid", st = WsFrame.Invalid)
END TestInvalidRSV;

(* ── Test 11: Fragmented control frame ──────────── *)

PROCEDURE TestFragmentedControl;
VAR
  buf: ARRAY [0..3] OF CHAR;
  hdr: FrameHeader;
  st: WsFrame.Status;
BEGIN
  (* Ping with FIN=0: opcode=9, no FIN *)
  buf[0] := CHR(9);   (* 00001001: no FIN + ping *)
  buf[1] := CHR(0);
  st := DecodeHeader(ADR(buf), 2, hdr);
  Check("fragctl: invalid", st = WsFrame.Invalid)
END TestFragmentedControl;

(* ── Test 12: Opcode conversion ─────────────────── *)

PROCEDURE TestOpcodeConversion;
BEGIN
  Check("opcode: 0->cont", IntToOpcode(0) = OpContinuation);
  Check("opcode: 1->text", IntToOpcode(1) = OpText);
  Check("opcode: 2->bin",  IntToOpcode(2) = OpBinary);
  Check("opcode: 8->close", IntToOpcode(8) = OpClose);
  Check("opcode: 9->ping", IntToOpcode(9) = OpPing);
  Check("opcode: 10->pong", IntToOpcode(10) = OpPong);
  Check("opcode: cont->0", OpcodeToInt(OpContinuation) = 0);
  Check("opcode: text->1", OpcodeToInt(OpText) = 1);
  Check("opcode: bin->2",  OpcodeToInt(OpBinary) = 2);
  Check("opcode: close->8", OpcodeToInt(OpClose) = 8);
  Check("opcode: ping->9", OpcodeToInt(OpPing) = 9);
  Check("opcode: pong->10", OpcodeToInt(OpPong) = 10)
END TestOpcodeConversion;

(* ── Test 13: GenerateMask ──────────────────────── *)

PROCEDURE TestGenerateMask;
VAR
  mask1, mask2: ARRAY [0..3] OF CHAR;
BEGIN
  GenerateMask(mask1);
  GenerateMask(mask2);
  (* Two sequential masks should differ *)
  Check("genmask: not zero",
        (ORD(mask1[0]) + ORD(mask1[1]) + ORD(mask1[2]) + ORD(mask1[3])) > 0);
  Check("genmask: different",
        (mask1[0] # mask2[0]) OR (mask1[1] # mask2[1])
        OR (mask1[2] # mask2[2]) OR (mask1[3] # mask2[3]))
END TestGenerateMask;

(* ── Test 14: SHA-1 known vector ────────────────── *)

PROCEDURE TestSHA1;
VAR
  input: ARRAY [0..2] OF CHAR;
  output: ARRAY [0..19] OF CHAR;
  (* SHA1("abc") = a9993e36 4706816a ba3e2571 7850c26c 9cd0d89d *)
BEGIN
  input[0] := 'a';
  input[1] := 'b';
  input[2] := 'c';

  m2_ws_sha1(ADR(input), 3, ADR(output));

  Check("sha1: byte0",  GetByte(ADR(output), 0) = 169);  (* 0xa9 *)
  Check("sha1: byte1",  GetByte(ADR(output), 1) = 153);  (* 0x99 *)
  Check("sha1: byte2",  GetByte(ADR(output), 2) = 62);   (* 0x3e *)
  Check("sha1: byte3",  GetByte(ADR(output), 3) = 54);   (* 0x36 *)
  Check("sha1: byte4",  GetByte(ADR(output), 4) = 71);   (* 0x47 *)
  Check("sha1: byte19", GetByte(ADR(output), 19) = 157)  (* 0x9d *)
END TestSHA1;

(* ── Test 15: Base64 known vector ───────────────── *)

PROCEDURE TestBase64;
VAR
  input: ARRAY [0..5] OF CHAR;
  output: ARRAY [0..31] OF CHAR;
  outLen: INTEGER;
BEGIN
  (* Base64("Hello!") = "SGVsbG8h" *)
  input[0] := 'H'; input[1] := 'e'; input[2] := 'l';
  input[3] := 'l'; input[4] := 'o'; input[5] := '!';

  m2_ws_base64_encode(ADR(input), 6, ADR(output), 32, outLen);

  Check("b64: len=8", outLen = 8);
  Check("b64: char0", output[0] = 'S');
  Check("b64: char1", output[1] = 'G');
  Check("b64: char2", output[2] = 'V');
  Check("b64: char3", output[3] = 's');
  Check("b64: char4", output[4] = 'b');
  Check("b64: char5", output[5] = 'G');
  Check("b64: char6", output[6] = '8');
  Check("b64: char7", output[7] = 'h')
END TestBase64;

(* ── Test 16: WebSocket accept key ──────────────── *)

PROCEDURE TestAcceptKey;
VAR
  (* RFC 6455 Section 4.2.2 example:
     Key: "dGhlIHNhbXBsZSBub25jZQ=="
     Concatenated: "dGhlIHNhbXBsZSBub25jZQ==258EAFA5-E914-47DA-95CA-C5AB0DC85B11"
     SHA-1: 0xb3 0x7a 0x4f 0x2c 0xc0 0x62 0x4f 0x16 0x90 0xf6
            0x46 0x06 0xcf 0x38 0x59 0x45 0xb2 0xbe 0xc4 0xea
     Accept: "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=" *)
  key: ARRAY [0..63] OF CHAR;
  guid: ARRAY [0..35] OF CHAR;
  concat: ARRAY [0..79] OF CHAR;
  sha1Out: ARRAY [0..19] OF CHAR;
  b64Out: ARRAY [0..31] OF CHAR;
  b64Len: INTEGER;
  i, j: INTEGER;
  expected: ARRAY [0..27] OF CHAR;
BEGIN
  key := "dGhlIHNhbXBsZSBub25jZQ==";
  guid := "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

  (* Concatenate key + GUID *)
  i := 0;
  j := 0;
  WHILE (j <= HIGH(key)) AND (key[j] # 0C) DO
    concat[i] := key[j];
    INC(i); INC(j)
  END;
  j := 0;
  WHILE (j <= HIGH(guid)) AND (guid[j] # 0C) DO
    concat[i] := guid[j];
    INC(i); INC(j)
  END;

  m2_ws_sha1(ADR(concat), i, ADR(sha1Out));
  m2_ws_base64_encode(ADR(sha1Out), 20, ADR(b64Out), 32, b64Len);

  expected := "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=";

  Check("accept: len=28", b64Len = 28);
  Check("accept: char0", b64Out[0] = 's');
  Check("accept: char1", b64Out[1] = '3');
  Check("accept: char2", b64Out[2] = 'p');
  Check("accept: char3", b64Out[3] = 'P');

  (* Full comparison *)
  j := 0;
  WHILE (j < b64Len) AND (b64Out[j] = expected[j]) DO INC(j) END;
  Check("accept: full match", j = b64Len)
END TestAcceptKey;

(* ── Test 17: Masked frame roundtrip ────────────── *)

PROCEDURE TestMaskedRoundtrip;
VAR
  hdr, hdr2: FrameHeader;
  buf: ARRAY [0..31] OF CHAR;
  n: CARDINAL;
  st: WsFrame.Status;
BEGIN
  hdr.fin := TRUE;
  hdr.opcode := OpText;
  hdr.masked := TRUE;
  hdr.payloadLen := 10;
  hdr.maskKey[0] := CHR(1);
  hdr.maskKey[1] := CHR(2);
  hdr.maskKey[2] := CHR(3);
  hdr.maskKey[3] := CHR(4);

  n := EncodeHeader(hdr, ADR(buf), 32);
  Check("masked_rt: encode ok", n > 0);
  Check("masked_rt: header 6 bytes", n = 6);

  st := DecodeHeader(ADR(buf), n, hdr2);
  Check("masked_rt: decode ok", st = WsFrame.Ok);
  Check("masked_rt: fin", hdr2.fin = TRUE);
  Check("masked_rt: text", hdr2.opcode = OpText);
  Check("masked_rt: masked", hdr2.masked = TRUE);
  Check("masked_rt: len=10", hdr2.payloadLen = 10);
  Check("masked_rt: hdrLen=6", hdr2.headerLen = 6);
  Check("masked_rt: key[0]", ORD(hdr2.maskKey[0]) = 1);
  Check("masked_rt: key[3]", ORD(hdr2.maskKey[3]) = 4)
END TestMaskedRoundtrip;

(* ── Test 18: Continuation frame ────────────────── *)

PROCEDURE TestContinuationFrame;
VAR
  hdr, hdr2: FrameHeader;
  buf: ARRAY [0..15] OF CHAR;
  n: CARDINAL;
  st: WsFrame.Status;
BEGIN
  (* First fragment: FIN=FALSE, opcode=text *)
  hdr.fin := FALSE;
  hdr.opcode := OpText;
  hdr.masked := FALSE;
  hdr.payloadLen := 100;

  n := EncodeHeader(hdr, ADR(buf), 16);
  Check("cont: first encode ok", n = 2);

  st := DecodeHeader(ADR(buf), n, hdr2);
  Check("cont: first decode ok", st = WsFrame.Ok);
  Check("cont: first no fin", hdr2.fin = FALSE);
  Check("cont: first text", hdr2.opcode = OpText);

  (* Continuation: FIN=FALSE, opcode=continuation *)
  hdr.fin := FALSE;
  hdr.opcode := OpContinuation;
  hdr.payloadLen := 50;

  n := EncodeHeader(hdr, ADR(buf), 16);
  st := DecodeHeader(ADR(buf), n, hdr2);
  Check("cont: mid decode ok", st = WsFrame.Ok);
  Check("cont: mid no fin", hdr2.fin = FALSE);
  Check("cont: mid cont opcode", hdr2.opcode = OpContinuation);

  (* Final: FIN=TRUE, opcode=continuation *)
  hdr.fin := TRUE;
  hdr.opcode := OpContinuation;
  hdr.payloadLen := 25;

  n := EncodeHeader(hdr, ADR(buf), 16);
  st := DecodeHeader(ADR(buf), n, hdr2);
  Check("cont: final decode ok", st = WsFrame.Ok);
  Check("cont: final fin", hdr2.fin = TRUE);
  Check("cont: final cont opcode", hdr2.opcode = OpContinuation)
END TestContinuationFrame;

BEGIN
  passed := 0;
  failed := 0;
  total := 0;

  WriteString("m2ws test suite"); WriteLn;
  WriteString("==============="); WriteLn;

  TestSmallFrame;
  TestExtLen16;
  TestExtLen64;
  TestMask;
  TestUnmaskedDecode;
  TestMaskedDecode;
  TestPingFrame;
  TestCloseFrame;
  TestIncomplete;
  TestInvalidRSV;
  TestFragmentedControl;
  TestOpcodeConversion;
  TestGenerateMask;
  TestSHA1;
  TestBase64;
  TestAcceptKey;
  TestMaskedRoundtrip;
  TestContinuationFrame;

  WriteLn;
  WriteString("m2ws: ");
  WriteInt(passed, 0); WriteString(" passed, ");
  WriteInt(failed, 0); WriteString(" failed, ");
  WriteInt(total, 0); WriteString(" total"); WriteLn;

  IF failed > 0 THEN
    WriteString("*** FAILURES ***"); WriteLn
  ELSE
    WriteString("*** ALL TESTS PASSED ***"); WriteLn
  END
END WsTests.
