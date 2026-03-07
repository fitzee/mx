MODULE ZlibTests;
(* Deterministic test suite for m2zlib.

   Tests:
     1. Compress/decompress roundtrip (Zlib format)
     2. Gzip format roundtrip
     3. Raw format roundtrip
     4. Decompressed data matches original byte-for-byte *)

FROM SYSTEM IMPORT ADDRESS, ADR, TSIZE;
FROM InOut IMPORT WriteString, WriteLn, WriteInt;
FROM Zlib IMPORT Status, Level, Format, Stream,
                 Compress, Decompress,
                 DeflateInit, Deflate, DeflateEnd,
                 InflateInit, Inflate, InflateEnd;

CONST
  BufSize = 4096;

VAR
  passed, failed, total: INTEGER;

  (* Source data — a recognizable repeating pattern *)
  src: ARRAY [0..255] OF CHAR;
  comp: ARRAY [0..BufSize-1] OF CHAR;
  decomp: ARRAY [0..BufSize-1] OF CHAR;

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

(* ── Fill source buffer with test pattern ──────────── *)

PROCEDURE FillSource;
VAR i: CARDINAL;
BEGIN
  i := 0;
  WHILE i <= 255 DO
    src[i] := CHR(i MOD 128);
    INC(i)
  END
END FillSource;

(* ── Compare n bytes of src and dst ────────────────── *)

PROCEDURE CompareBytes(VAR a, b: ARRAY OF CHAR; n: CARDINAL): BOOLEAN;
VAR i: CARDINAL;
BEGIN
  i := 0;
  WHILE i < n DO
    IF a[i] # b[i] THEN RETURN FALSE END;
    INC(i)
  END;
  RETURN TRUE
END CompareBytes;

(* ── Test 1: Zlib format roundtrip ─────────────────── *)

PROCEDURE TestZlibRoundtrip;
VAR
  compLen, decompLen: CARDINAL;
  st: Status;
BEGIN
  st := Compress(ADR(src), 256, ADR(comp), BufSize, compLen, ZlibFmt);
  Check("zlib: compress ok", st = Ok);
  Check("zlib: compressed size > 0", compLen > 0);
  Check("zlib: compressed smaller", compLen < 256);

  st := Decompress(ADR(comp), compLen, ADR(decomp), BufSize, decompLen, ZlibFmt);
  Check("zlib: decompress ok", st = Ok);
  Check("zlib: decompressed size", decompLen = 256);
  Check("zlib: data matches", CompareBytes(src, decomp, 256))
END TestZlibRoundtrip;

(* ── Test 2: Gzip format roundtrip ─────────────────── *)

PROCEDURE TestGzipRoundtrip;
VAR
  compLen, decompLen: CARDINAL;
  st: Status;
BEGIN
  st := Compress(ADR(src), 256, ADR(comp), BufSize, compLen, Gzip);
  Check("gzip: compress ok", st = Ok);
  Check("gzip: compressed size > 0", compLen > 0);

  st := Decompress(ADR(comp), compLen, ADR(decomp), BufSize, decompLen, Gzip);
  Check("gzip: decompress ok", st = Ok);
  Check("gzip: decompressed size", decompLen = 256);
  Check("gzip: data matches", CompareBytes(src, decomp, 256))
END TestGzipRoundtrip;

(* ── Test 3: Raw format roundtrip ──────────────────── *)

PROCEDURE TestRawRoundtrip;
VAR
  compLen, decompLen: CARDINAL;
  st: Status;
BEGIN
  st := Compress(ADR(src), 256, ADR(comp), BufSize, compLen, Raw);
  Check("raw: compress ok", st = Ok);
  Check("raw: compressed size > 0", compLen > 0);

  st := Decompress(ADR(comp), compLen, ADR(decomp), BufSize, decompLen, Raw);
  Check("raw: decompress ok", st = Ok);
  Check("raw: decompressed size", decompLen = 256);
  Check("raw: data matches", CompareBytes(src, decomp, 256))
END TestRawRoundtrip;

(* ── Test 4: Streaming API roundtrip ───────────────── *)

PROCEDURE TestStreamingRoundtrip;
VAR
  s: Stream;
  st: Status;
  produced: CARDINAL;
  compLen, decompLen: CARDINAL;
BEGIN
  (* Deflate *)
  st := DeflateInit(s, Default, ZlibFmt);
  Check("stream: deflate init ok", st = Ok);

  st := Deflate(s, ADR(src), 256, ADR(comp), BufSize, produced, TRUE);
  compLen := produced;
  Check("stream: deflate ok", (st = Ok) OR (st = StreamEnd));
  Check("stream: deflate produced > 0", compLen > 0);

  st := DeflateEnd(s);
  Check("stream: deflate end ok", st = Ok);

  (* Inflate *)
  st := InflateInit(s, ZlibFmt);
  Check("stream: inflate init ok", st = Ok);

  st := Inflate(s, ADR(comp), compLen, ADR(decomp), BufSize, produced);
  decompLen := produced;
  Check("stream: inflate ok", (st = Ok) OR (st = StreamEnd));
  Check("stream: inflate size", decompLen = 256);
  Check("stream: data matches", CompareBytes(src, decomp, 256));

  st := InflateEnd(s);
  Check("stream: inflate end ok", st = Ok)
END TestStreamingRoundtrip;

BEGIN
  passed := 0;
  failed := 0;
  total := 0;

  WriteString("m2zlib test suite"); WriteLn;
  WriteString("================="); WriteLn;

  FillSource;

  TestZlibRoundtrip;
  TestGzipRoundtrip;
  TestRawRoundtrip;
  TestStreamingRoundtrip;

  WriteLn;
  WriteString("m2zlib: ");
  WriteInt(passed, 0); WriteString(" passed, ");
  WriteInt(failed, 0); WriteString(" failed, ");
  WriteInt(total, 0); WriteString(" total"); WriteLn;

  IF failed > 0 THEN
    WriteString("*** FAILURES ***"); WriteLn
  ELSE
    WriteString("*** ALL TESTS PASSED ***"); WriteLn
  END
END ZlibTests.
