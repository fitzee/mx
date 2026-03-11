MODULE BytesTests;
(* Deterministic test suite for m2bytes.

   Tests:
     1. ByteBuf init/append/get
     2. ByteBuf growth (force multiple expansions)
     3. ByteBuf clear/truncate
     4. BytesView access
     5. Reader/Writer U8 roundtrip
     6. Reader/Writer U16LE/BE roundtrip
     7. Reader/Writer U32LE/BE roundtrip
     8. Reader/Writer I32LE/BE roundtrip (including negative)
     9. Reader failure does not advance cursor
    10. Reader skip and slice
    11. Varint U32 known vectors
    12. Varint malformed rejection
    13. ZigZag I32 known vectors
    14. Hex encode known vectors
    15. Hex decode known vectors
    16. Hex decode invalid rejection
    17. Large append stress test *)

FROM InOut IMPORT WriteString, WriteLn, WriteInt;
FROM ByteBuf IMPORT Buf, BytesView, Init, Free, Clear, Truncate,
                     AppendByte, AppendChars, GetByte, SetByte,
                     AsView, ViewGetByte, Reserve;
FROM Codec IMPORT Reader, Writer,
                  InitReader, InitWriter, Remaining,
                  ReadU8, WriteU8,
                  ReadU16LE, ReadU16BE, WriteU16LE, WriteU16BE,
                  ReadU32LE, ReadU32BE, WriteU32LE, WriteU32BE,
                  ReadI32LE, ReadI32BE, WriteI32LE, WriteI32BE,
                  Skip, ReadSlice,
                  WriteVarU32, ReadVarU32,
                  WriteVarI32, ReadVarI32;
FROM Hex IMPORT Encode, Decode, ByteToHex, HexToByte;

VAR
  passed, failed, total: INTEGER;

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

(* ── Test 1: ByteBuf basic ────────────────────────── *)

PROCEDURE TestBufBasic;
VAR b: Buf;
BEGIN
  Init(b, 16);
  Check("buf: init len=0", b.len = 0);
  Check("buf: init cap>=16", b.cap >= 16);

  AppendByte(b, 0);
  AppendByte(b, 127);
  AppendByte(b, 128);
  AppendByte(b, 255);
  Check("buf: len=4", b.len = 4);
  Check("buf: get 0", GetByte(b, 0) = 0);
  Check("buf: get 127", GetByte(b, 1) = 127);
  Check("buf: get 128", GetByte(b, 2) = 128);
  Check("buf: get 255", GetByte(b, 3) = 255);
  Check("buf: get oob", GetByte(b, 99) = 0);

  SetByte(b, 1, 42);
  Check("buf: set+get", GetByte(b, 1) = 42);
  Free(b)
END TestBufBasic;

(* ── Test 2: ByteBuf growth ───────────────────────── *)

PROCEDURE TestBufGrowth;
VAR b: Buf; i: CARDINAL;
BEGIN
  Init(b, 4);
  Check("grow: init cap=4", b.cap >= 4);

  i := 0;
  WHILE i < 500 DO
    AppendByte(b, i MOD 256);
    INC(i)
  END;
  Check("grow: len=500", b.len = 500);
  Check("grow: cap>=500", b.cap >= 500);

  (* verify data integrity *)
  Check("grow: byte[0]", GetByte(b, 0) = 0);
  Check("grow: byte[255]", GetByte(b, 255) = 255);
  Check("grow: byte[256]", GetByte(b, 256) = 0);
  Check("grow: byte[499]", GetByte(b, 499) = 499 MOD 256);
  Free(b)
END TestBufGrowth;

(* ── Test 3: Clear/Truncate ───────────────────────── *)

PROCEDURE TestClearTruncate;
VAR b: Buf; oldCap: CARDINAL;
BEGIN
  Init(b, 64);
  AppendByte(b, 1);
  AppendByte(b, 2);
  AppendByte(b, 3);

  oldCap := b.cap;
  Clear(b);
  Check("clear: len=0", b.len = 0);
  Check("clear: cap kept", b.cap = oldCap);

  AppendByte(b, 10);
  AppendByte(b, 20);
  AppendByte(b, 30);
  AppendByte(b, 40);
  Truncate(b, 2);
  Check("trunc: len=2", b.len = 2);
  Check("trunc: data ok", GetByte(b, 0) = 10);
  Check("trunc: data ok2", GetByte(b, 1) = 20);

  Truncate(b, 99);
  Check("trunc: no-op on bigger", b.len = 2);
  Free(b)
END TestClearTruncate;

(* ── Test 4: BytesView ────────────────────────────── *)

PROCEDURE TestView;
VAR b: Buf; v: BytesView;
BEGIN
  Init(b, 16);
  AppendByte(b, 10);
  AppendByte(b, 20);
  AppendByte(b, 30);

  v := AsView(b);
  Check("view: len=3", v.len = 3);
  Check("view: get 0", ViewGetByte(v, 0) = 10);
  Check("view: get 2", ViewGetByte(v, 2) = 30);
  Check("view: get oob", ViewGetByte(v, 5) = 0);
  Free(b)
END TestView;

(* ── Test 5: U8 roundtrip ────────────────────────── *)

PROCEDURE TestU8Roundtrip;
VAR b: Buf; w: Writer; r: Reader; v: BytesView;
    ok: BOOLEAN; val: CARDINAL;
BEGIN
  Init(b, 16);
  InitWriter(w, b);
  WriteU8(w, 0);
  WriteU8(w, 127);
  WriteU8(w, 255);

  v := AsView(b);
  InitReader(r, v);
  Check("u8: remaining=3", Remaining(r) = 3);
  val := ReadU8(r, ok);
  Check("u8: ok", ok);
  Check("u8: val=0", val = 0);
  val := ReadU8(r, ok);
  Check("u8: val=127", val = 127);
  val := ReadU8(r, ok);
  Check("u8: val=255", val = 255);
  Check("u8: remaining=0", Remaining(r) = 0);
  Free(b)
END TestU8Roundtrip;

(* ── Test 6: U16 roundtrip ───────────────────────── *)

PROCEDURE TestU16Roundtrip;
VAR b: Buf; w: Writer; r: Reader; v: BytesView;
    ok: BOOLEAN; val: CARDINAL;
BEGIN
  Init(b, 32);
  InitWriter(w, b);
  WriteU16LE(w, 0);
  WriteU16LE(w, 256);
  WriteU16LE(w, 65535);
  WriteU16BE(w, 0);
  WriteU16BE(w, 256);
  WriteU16BE(w, 65535);

  v := AsView(b);
  InitReader(r, v);
  val := ReadU16LE(r, ok); Check("u16le: 0", ok AND (val = 0));
  val := ReadU16LE(r, ok); Check("u16le: 256", ok AND (val = 256));
  val := ReadU16LE(r, ok); Check("u16le: 65535", ok AND (val = 65535));
  val := ReadU16BE(r, ok); Check("u16be: 0", ok AND (val = 0));
  val := ReadU16BE(r, ok); Check("u16be: 256", ok AND (val = 256));
  val := ReadU16BE(r, ok); Check("u16be: 65535", ok AND (val = 65535));
  Free(b)
END TestU16Roundtrip;

(* ── Test 7: U32 roundtrip ───────────────────────── *)

PROCEDURE TestU32Roundtrip;
VAR b: Buf; w: Writer; r: Reader; v: BytesView;
    ok: BOOLEAN; val: CARDINAL;
BEGIN
  Init(b, 64);
  InitWriter(w, b);
  WriteU32LE(w, 0);
  WriteU32LE(w, 1);
  WriteU32LE(w, 16777216);  (* 0x01000000 *)
  WriteU32BE(w, 0);
  WriteU32BE(w, 1);
  WriteU32BE(w, 16777216);

  v := AsView(b);
  InitReader(r, v);
  val := ReadU32LE(r, ok); Check("u32le: 0", ok AND (val = 0));
  val := ReadU32LE(r, ok); Check("u32le: 1", ok AND (val = 1));
  val := ReadU32LE(r, ok); Check("u32le: 16M", ok AND (val = 16777216));
  val := ReadU32BE(r, ok); Check("u32be: 0", ok AND (val = 0));
  val := ReadU32BE(r, ok); Check("u32be: 1", ok AND (val = 1));
  val := ReadU32BE(r, ok); Check("u32be: 16M", ok AND (val = 16777216));
  Free(b)
END TestU32Roundtrip;

(* ── Test 8: I32 roundtrip ───────────────────────── *)

PROCEDURE TestI32Roundtrip;
VAR b: Buf; w: Writer; r: Reader; v: BytesView;
    ok: BOOLEAN; ival: INTEGER;
BEGIN
  Init(b, 64);
  InitWriter(w, b);
  WriteI32LE(w, 0);
  WriteI32LE(w, 1);
  WriteI32LE(w, -1);
  WriteI32LE(w, -2147483647);
  WriteI32BE(w, 0);
  WriteI32BE(w, 42);
  WriteI32BE(w, -42);

  v := AsView(b);
  InitReader(r, v);
  ival := ReadI32LE(r, ok); Check("i32le: 0", ok AND (ival = 0));
  ival := ReadI32LE(r, ok); Check("i32le: 1", ok AND (ival = 1));
  ival := ReadI32LE(r, ok); Check("i32le: -1", ok AND (ival = -1));
  ival := ReadI32LE(r, ok); Check("i32le: minint+1", ok AND (ival = -2147483647));
  ival := ReadI32BE(r, ok); Check("i32be: 0", ok AND (ival = 0));
  ival := ReadI32BE(r, ok); Check("i32be: 42", ok AND (ival = 42));
  ival := ReadI32BE(r, ok); Check("i32be: -42", ok AND (ival = -42));
  Free(b)
END TestI32Roundtrip;

(* ── Test 9: Reader fail no advance ──────────────── *)

PROCEDURE TestReaderNoAdvance;
VAR b: Buf; r: Reader; v: BytesView;
    ok: BOOLEAN; val: CARDINAL; savePos: CARDINAL;
BEGIN
  Init(b, 16);
  AppendByte(b, 1);
  AppendByte(b, 2);

  v := AsView(b);
  InitReader(r, v);
  val := ReadU8(r, ok);  (* consume 1 byte *)
  savePos := r.pos;

  val := ReadU32LE(r, ok);  (* only 1 byte left, needs 4 *)
  Check("noadv: u32 fail", NOT ok);
  Check("noadv: pos unchanged", r.pos = savePos);

  val := ReadU16LE(r, ok);  (* only 1 byte left, needs 2 *)
  Check("noadv: u16 fail", NOT ok);
  Check("noadv: pos still same", r.pos = savePos);

  (* can still read the last byte *)
  val := ReadU8(r, ok);
  Check("noadv: u8 ok", ok AND (val = 2));
  Free(b)
END TestReaderNoAdvance;

(* ── Test 10: Skip and Slice ─────────────────────── *)

PROCEDURE TestSkipSlice;
VAR b: Buf; r: Reader; v, sv: BytesView;
    ok: BOOLEAN; val: CARDINAL;
BEGIN
  Init(b, 16);
  AppendByte(b, 10);
  AppendByte(b, 20);
  AppendByte(b, 30);
  AppendByte(b, 40);
  AppendByte(b, 50);

  v := AsView(b);
  InitReader(r, v);

  Skip(r, 2, ok);
  Check("skip: ok", ok);
  Check("skip: pos=2", r.pos = 2);

  ReadSlice(r, 2, sv, ok);
  Check("slice: ok", ok);
  Check("slice: len=2", sv.len = 2);
  Check("slice: byte0", ViewGetByte(sv, 0) = 30);
  Check("slice: byte1", ViewGetByte(sv, 1) = 40);

  (* try to skip too far *)
  Skip(r, 99, ok);
  Check("skip: fail oob", NOT ok);
  Free(b)
END TestSkipSlice;

(* ── Test 11: Varint U32 known vectors ───────────── *)

PROCEDURE TestVarU32;
VAR b: Buf; w: Writer; r: Reader; v: BytesView;
    ok: BOOLEAN; val: CARDINAL;
BEGIN
  Init(b, 64);
  InitWriter(w, b);
  WriteVarU32(w, 0);
  WriteVarU32(w, 1);
  WriteVarU32(w, 127);
  WriteVarU32(w, 128);
  WriteVarU32(w, 300);
  WriteVarU32(w, 16384);

  v := AsView(b);
  InitReader(r, v);
  val := ReadVarU32(r, ok); Check("var: 0", ok AND (val = 0));
  val := ReadVarU32(r, ok); Check("var: 1", ok AND (val = 1));
  val := ReadVarU32(r, ok); Check("var: 127", ok AND (val = 127));
  val := ReadVarU32(r, ok); Check("var: 128", ok AND (val = 128));
  val := ReadVarU32(r, ok); Check("var: 300", ok AND (val = 300));
  val := ReadVarU32(r, ok); Check("var: 16384", ok AND (val = 16384));
  Free(b)
END TestVarU32;

(* ── Test 12: Varint malformed ───────────────────── *)

PROCEDURE TestVarMalformed;
VAR b: Buf; r: Reader; v: BytesView;
    ok: BOOLEAN; val: CARDINAL; savePos: CARDINAL;
BEGIN
  Init(b, 16);
  (* write 6 continuation bytes -- too long for u32 *)
  AppendByte(b, 128);
  AppendByte(b, 128);
  AppendByte(b, 128);
  AppendByte(b, 128);
  AppendByte(b, 128);
  AppendByte(b, 1);

  v := AsView(b);
  InitReader(r, v);
  savePos := r.pos;
  val := ReadVarU32(r, ok);
  Check("malvar: fail", NOT ok);
  Check("malvar: pos reset", r.pos = savePos);

  (* truncated varint: only continuation byte, no terminator *)
  Clear(b);
  AppendByte(b, 128);
  v := AsView(b);
  InitReader(r, v);
  val := ReadVarU32(r, ok);
  Check("malvar: truncated", NOT ok);
  Free(b)
END TestVarMalformed;

(* ── Test 13: ZigZag I32 ────────────────────────── *)

PROCEDURE TestZigZag;
VAR b: Buf; w: Writer; r: Reader; v: BytesView;
    ok: BOOLEAN; ival: INTEGER;
BEGIN
  Init(b, 64);
  InitWriter(w, b);
  WriteVarI32(w, 0);
  WriteVarI32(w, -1);
  WriteVarI32(w, 1);
  WriteVarI32(w, -2);
  WriteVarI32(w, 2);
  WriteVarI32(w, -100);
  WriteVarI32(w, 100);

  v := AsView(b);
  InitReader(r, v);
  ival := ReadVarI32(r, ok); Check("zz: 0", ok AND (ival = 0));
  ival := ReadVarI32(r, ok); Check("zz: -1", ok AND (ival = -1));
  ival := ReadVarI32(r, ok); Check("zz: 1", ok AND (ival = 1));
  ival := ReadVarI32(r, ok); Check("zz: -2", ok AND (ival = -2));
  ival := ReadVarI32(r, ok); Check("zz: 2", ok AND (ival = 2));
  ival := ReadVarI32(r, ok); Check("zz: -100", ok AND (ival = -100));
  ival := ReadVarI32(r, ok); Check("zz: 100", ok AND (ival = 100));
  Free(b)
END TestZigZag;

(* ── Test 14: Hex encode ─────────────────────────── *)

PROCEDURE TestHexEncode;
VAR b: Buf; out: ARRAY [0..127] OF CHAR;
    outLen: CARDINAL; ok: BOOLEAN;
    hi, lo: CHAR;
BEGIN
  (* single byte *)
  ByteToHex(0, hi, lo);
  Check("hexenc: 0 hi", hi = '0');
  Check("hexenc: 0 lo", lo = '0');

  ByteToHex(255, hi, lo);
  Check("hexenc: ff hi", hi = 'f');
  Check("hexenc: ff lo", lo = 'f');

  ByteToHex(171, hi, lo);  (* 0xAB *)
  Check("hexenc: ab hi", hi = 'a');
  Check("hexenc: ab lo", lo = 'b');

  (* buffer encode *)
  Init(b, 16);
  AppendByte(b, 222);  (* 0xDE *)
  AppendByte(b, 173);  (* 0xAD *)
  AppendByte(b, 190);  (* 0xBE *)
  AppendByte(b, 239);  (* 0xEF *)

  Encode(b, 4, out, outLen, ok);
  Check("hexenc: ok", ok);
  Check("hexenc: len=8", outLen = 8);
  Check("hexenc: d", out[0] = 'd');
  Check("hexenc: e", out[1] = 'e');
  Check("hexenc: a", out[2] = 'a');
  Check("hexenc: d2", out[3] = 'd');
  Check("hexenc: b", out[4] = 'b');
  Check("hexenc: e2", out[5] = 'e');
  Check("hexenc: e3", out[6] = 'e');
  Check("hexenc: f", out[7] = 'f');
  Free(b)
END TestHexEncode;

(* ── Test 15: Hex decode ─────────────────────────── *)

PROCEDURE TestHexDecode;
VAR b: Buf; ok: BOOLEAN;
    inp: ARRAY [0..8] OF CHAR;
BEGIN
  Init(b, 16);
  inp := "deadbeef";
  Decode(inp, 8, b, ok);
  Check("hexdec: ok", ok);
  Check("hexdec: len=4", b.len = 4);
  Check("hexdec: byte0", GetByte(b, 0) = 222);
  Check("hexdec: byte1", GetByte(b, 1) = 173);
  Check("hexdec: byte2", GetByte(b, 2) = 190);
  Check("hexdec: byte3", GetByte(b, 3) = 239);

  (* uppercase input *)
  Clear(b);
  inp := "DEADBEEF";

  Decode(inp, 8, b, ok);
  Check("hexdec: upper ok", ok);
  Check("hexdec: upper val", GetByte(b, 0) = 222);
  Free(b)
END TestHexDecode;

(* ── Test 16: Hex decode invalid ─────────────────── *)

PROCEDURE TestHexDecodeInvalid;
VAR b: Buf; ok: BOOLEAN;
    inp: ARRAY [0..8] OF CHAR;
BEGIN
  Init(b, 16);

  (* odd length *)
  inp[0] := 'a';
  Decode(inp, 1, b, ok);
  Check("hexinv: odd len", NOT ok);

  (* invalid char *)
  inp := "zz000000";
  Decode(inp, 2, b, ok);
  Check("hexinv: bad char", NOT ok);

  inp := "0g000000";
  Decode(inp, 2, b, ok);
  Check("hexinv: bad char2", NOT ok);
  Free(b)
END TestHexDecodeInvalid;

(* ── Test 17: Large append stress ────────────────── *)

PROCEDURE TestStress;
VAR b: Buf; i: CARDINAL;
BEGIN
  Init(b, 8);
  i := 0;
  WHILE i < 4000 DO
    AppendByte(b, i MOD 256);
    INC(i)
  END;
  Check("stress: len=4000", b.len = 4000);
  Check("stress: first", GetByte(b, 0) = 0);
  Check("stress: last", GetByte(b, 3999) = 3999 MOD 256);
  Check("stress: mid", GetByte(b, 1000) = 1000 MOD 256);
  Free(b)
END TestStress;

BEGIN
  passed := 0;
  failed := 0;
  total := 0;

  WriteString("m2bytes test suite"); WriteLn;
  WriteString("=================="); WriteLn;

  TestBufBasic;
  TestBufGrowth;
  TestClearTruncate;
  TestView;
  TestU8Roundtrip;
  TestU16Roundtrip;
  TestU32Roundtrip;
  TestI32Roundtrip;
  TestReaderNoAdvance;
  TestSkipSlice;
  TestVarU32;
  TestVarMalformed;
  TestZigZag;
  TestHexEncode;
  TestHexDecode;
  TestHexDecodeInvalid;
  TestStress;

  WriteLn;
  WriteInt(total, 0); WriteString(" tests, ");
  WriteInt(passed, 0); WriteString(" passed, ");
  WriteInt(failed, 0); WriteString(" failed"); WriteLn;

  IF failed > 0 THEN
    WriteString("*** FAILURES ***"); WriteLn
  ELSE
    WriteString("*** ALL TESTS PASSED ***"); WriteLn
  END
END BytesTests.
