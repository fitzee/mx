IMPLEMENTATION MODULE Zlib;

FROM SYSTEM IMPORT ADDRESS, ADR, TSIZE;
FROM ZlibBridge IMPORT m2_deflate_init, m2_deflate, m2_deflate_end,
                        m2_inflate_init, m2_inflate, m2_inflate_end;

(* ── Enum mapping helpers ──────────────────────────── *)

PROCEDURE LevelToInt(level: Level): INTEGER;
BEGIN
  CASE level OF
    NoCompression:   RETURN 0  |
    BestSpeed:       RETURN 1  |
    Default:         RETURN 6  |
    BestCompression: RETURN 9
  END;
  RETURN 6
END LevelToInt;

PROCEDURE FormatToWindowBits(fmt: Format): INTEGER;
BEGIN
  CASE fmt OF
    Raw:     RETURN -15 |
    ZlibFmt: RETURN  15 |
    Gzip:    RETURN  31
  END;
  RETURN 15
END FormatToWindowBits;

PROCEDURE IntToStatus(rc: INTEGER): Status;
BEGIN
  IF rc = 0 THEN RETURN Ok
  ELSIF rc = 1 THEN RETURN StreamEnd
  ELSIF rc = 2 THEN RETURN NeedMore
  ELSE RETURN Error
  END
END IntToStatus;

(* ── Streaming deflate API ─────────────────────────── *)

PROCEDURE DeflateInit(VAR s: Stream; level: Level; fmt: Format): Status;
VAR wb, lv: INTEGER;
BEGIN
  lv := LevelToInt(level);
  wb := FormatToWindowBits(fmt);
  s := m2_deflate_init(lv, wb);
  IF s = NIL THEN
    RETURN Error
  END;
  RETURN Ok
END DeflateInit;

PROCEDURE Deflate(VAR s: Stream; src: ADDRESS; srcLen: CARDINAL;
                  dst: ADDRESS; dstMax: CARDINAL;
                  VAR produced: CARDINAL; flush: BOOLEAN): Status;
VAR rc, prod, fl: INTEGER;
BEGIN
  IF flush THEN fl := 1 ELSE fl := 0 END;
  prod := 0;
  rc := m2_deflate(s, src, INTEGER(srcLen), dst, INTEGER(dstMax), prod, fl);
  produced := CARDINAL(prod);
  RETURN IntToStatus(rc)
END Deflate;

PROCEDURE DeflateEnd(VAR s: Stream): Status;
VAR rc: INTEGER;
BEGIN
  rc := m2_deflate_end(s);
  s := NIL;
  RETURN IntToStatus(rc)
END DeflateEnd;

(* ── Streaming inflate API ─────────────────────────── *)

PROCEDURE InflateInit(VAR s: Stream; fmt: Format): Status;
VAR wb: INTEGER;
BEGIN
  wb := FormatToWindowBits(fmt);
  s := m2_inflate_init(wb);
  IF s = NIL THEN
    RETURN Error
  END;
  RETURN Ok
END InflateInit;

PROCEDURE Inflate(VAR s: Stream; src: ADDRESS; srcLen: CARDINAL;
                  dst: ADDRESS; dstMax: CARDINAL;
                  VAR produced: CARDINAL): Status;
VAR rc, prod: INTEGER;
BEGIN
  prod := 0;
  rc := m2_inflate(s, src, INTEGER(srcLen), dst, INTEGER(dstMax), prod);
  produced := CARDINAL(prod);
  RETURN IntToStatus(rc)
END Inflate;

PROCEDURE InflateEnd(VAR s: Stream): Status;
VAR rc: INTEGER;
BEGIN
  rc := m2_inflate_end(s);
  s := NIL;
  RETURN IntToStatus(rc)
END InflateEnd;

(* ── One-shot convenience ──────────────────────────── *)

PROCEDURE Compress(src: ADDRESS; srcLen: CARDINAL;
                   dst: ADDRESS; dstMax: CARDINAL;
                   VAR dstLen: CARDINAL; fmt: Format): Status;
VAR
  s: Stream;
  st: Status;
  produced: CARDINAL;
BEGIN
  dstLen := 0;
  st := DeflateInit(s, Default, fmt);
  IF st # Ok THEN RETURN st END;

  st := Deflate(s, src, srcLen, dst, dstMax, produced, TRUE);
  dstLen := produced;

  IF (st # StreamEnd) AND (st # Ok) THEN
    DeflateEnd(s);
    RETURN Error
  END;

  DeflateEnd(s);
  RETURN Ok
END Compress;

PROCEDURE Decompress(src: ADDRESS; srcLen: CARDINAL;
                     dst: ADDRESS; dstMax: CARDINAL;
                     VAR dstLen: CARDINAL; fmt: Format): Status;
VAR
  s: Stream;
  st: Status;
  produced: CARDINAL;
BEGIN
  dstLen := 0;
  st := InflateInit(s, fmt);
  IF st # Ok THEN RETURN st END;

  st := Inflate(s, src, srcLen, dst, dstMax, produced);
  dstLen := produced;

  IF (st # StreamEnd) AND (st # Ok) THEN
    InflateEnd(s);
    RETURN Error
  END;

  InflateEnd(s);
  RETURN Ok
END Decompress;

END Zlib.
