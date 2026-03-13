MODULE LongcardDivMod;
FROM InOut IMPORT WriteString, WriteCard, WriteInt, WriteLn;

TYPE
  Timestamp = LONGCARD;
  Bytes = ARRAY [0..7] OF CARDINAL;

VAR
  ts: Timestamp;
  val, rebuilt: LONGCARD;
  buf: Bytes;
  i: CARDINAL;
  ok: BOOLEAN;

PROCEDURE PackBE(v: LONGCARD; VAR b: Bytes);
(* Pack a LONGCARD into 8 big-endian bytes using DIV/MOD.
   We iterate forward with j=0..7 and store from the low byte up,
   then the array is naturally big-endian: b[0] is the MSB. *)
VAR
  tmp: LONGCARD;
  j: CARDINAL;
BEGIN
  tmp := v;
  FOR j := 0 TO 7 DO
    b[7 - j] := VAL(CARDINAL, tmp MOD 256);
    tmp := tmp DIV 256
  END
END PackBE;

PROCEDURE UnpackBE(VAR b: Bytes): LONGCARD;
(* Unpack 8 big-endian bytes back to LONGCARD *)
VAR
  result: LONGCARD;
  j: CARDINAL;
BEGIN
  result := 0;
  FOR j := 0 TO 7 DO
    result := result * 256 + LONGCARD(b[j])
  END;
  RETURN result
END UnpackBE;

PROCEDURE DivModViaProc(dividend, divisor: LONGCARD;
                        VAR q, r: LONGCARD);
(* Exercise DIV/MOD through procedure parameters *)
BEGIN
  q := dividend DIV divisor;
  r := dividend MOD divisor
END DivModViaProc;

PROCEDURE DivModAlias(t: Timestamp; divisor: LONGCARD;
                      VAR q, r: LONGCARD);
(* Exercise DIV/MOD through a type-alias parameter *)
BEGIN
  q := t DIV divisor;
  r := t MOD divisor
END DivModAlias;

VAR
  q, r: LONGCARD;

BEGIN
  (* --- Test 1: Pack/unpack roundtrip with 1741827600000 --- *)
  ts := 1741827600000;  (* Unix epoch ms: 2025-03-13 *)
  PackBE(ts, buf);

  WriteString("bytes:");
  FOR i := 0 TO 7 DO
    WriteString(" ");
    WriteCard(buf[i], 1)
  END;
  WriteLn;

  rebuilt := UnpackBE(buf);
  IF rebuilt = ts THEN
    WriteString("roundtrip: PASS")
  ELSE
    WriteString("roundtrip: FAIL")
  END;
  WriteLn;

  (* --- Test 2: DIV/MOD via procedure parameters --- *)
  DivModViaProc(1741827600000, 1000, q, r);
  (* q should be 1741827600, r should be 0 *)
  IF (q = 1741827600) AND (r = 0) THEN
    WriteString("proc params: PASS")
  ELSE
    WriteString("proc params: FAIL")
  END;
  WriteLn;

  (* --- Test 3: DIV/MOD via type-alias parameter --- *)
  DivModAlias(ts, 86400000, q, r);
  (* 1741827600000 DIV 86400000 = 20160, MOD = 3600000 *)
  IF (q = 20160) AND (r = 3600000) THEN
    WriteString("alias params: PASS")
  ELSE
    WriteString("alias params: FAIL")
  END;
  WriteLn;

  (* --- Test 4: Large value exceeding 32-bit range --- *)
  val := 4294967296;  (* 2^32, exceeds CARDINAL range *)
  q := val DIV 65536;
  r := val MOD 65536;
  IF (q = 65536) AND (r = 0) THEN
    WriteString("large div mod: PASS")
  ELSE
    WriteString("large div mod: FAIL")
  END;
  WriteLn;

  (* --- Test 5: VAL(CARDINAL, expr) pattern --- *)
  val := 1741827600000;
  i := VAL(CARDINAL, val MOD 256);
  IF i = 128 THEN
    WriteString("val cardinal: PASS")
  ELSE
    WriteString("val cardinal: FAIL")
  END;
  WriteLn;

  WriteString("all longcard div mod OK"); WriteLn
END LongcardDivMod.
