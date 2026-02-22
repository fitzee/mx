IMPLEMENTATION MODULE Hex;

FROM ByteBuf IMPORT Buf, GetByte, AppendByte;

(* ── Hex digit tables ───────────────────────────────── *)

CONST
  hexChars = "0123456789abcdef";

PROCEDURE HexDigit(val: CARDINAL): CHAR;
BEGIN
  RETURN hexChars[val MOD 16]
END HexDigit;

PROCEDURE DigitVal(ch: CHAR; VAR val: CARDINAL;
                   VAR ok: BOOLEAN);
BEGIN
  IF (ch >= '0') AND (ch <= '9') THEN
    val := ORD(ch) - ORD('0');
    ok := TRUE
  ELSIF (ch >= 'a') AND (ch <= 'f') THEN
    val := ORD(ch) - ORD('a') + 10;
    ok := TRUE
  ELSIF (ch >= 'A') AND (ch <= 'F') THEN
    val := ORD(ch) - ORD('A') + 10;
    ok := TRUE
  ELSE
    val := 0;
    ok := FALSE
  END
END DigitVal;

(* ── Public API ─────────────────────────────────────── *)

PROCEDURE ByteToHex(val: CARDINAL; VAR hi, lo: CHAR);
BEGIN
  hi := HexDigit(val DIV 16);
  lo := HexDigit(val MOD 16)
END ByteToHex;

PROCEDURE HexToByte(hi, lo: CHAR; VAR val: CARDINAL;
                    VAR ok: BOOLEAN);
VAR h, l: CARDINAL;
BEGIN
  DigitVal(hi, h, ok);
  IF NOT ok THEN val := 0; RETURN END;
  DigitVal(lo, l, ok);
  IF NOT ok THEN val := 0; RETURN END;
  val := h * 16 + l
END HexToByte;

PROCEDURE Encode(VAR src: Buf; nBytes: CARDINAL;
                 VAR out: ARRAY OF CHAR;
                 VAR outLen: CARDINAL;
                 VAR ok: BOOLEAN);
VAR i, pos, n, b: CARDINAL; hi, lo: CHAR;
BEGIN
  n := nBytes;
  outLen := 0;

  (* need 2 * n chars in out *)
  IF n * 2 > HIGH(out) + 1 THEN
    ok := FALSE;
    RETURN
  END;

  pos := 0;
  i := 0;
  WHILE i < n DO
    b := GetByte(src, i);
    ByteToHex(b, hi, lo);
    out[pos] := hi;
    out[pos + 1] := lo;
    pos := pos + 2;
    INC(i)
  END;
  outLen := pos;
  ok := TRUE
END Encode;

PROCEDURE Decode(s: ARRAY OF CHAR; sLen: CARDINAL;
                 VAR dst: Buf;
                 VAR ok: BOOLEAN);
VAR i, val: CARDINAL;
BEGIN
  (* must be even length *)
  IF sLen MOD 2 # 0 THEN ok := FALSE; RETURN END;
  IF sLen > HIGH(s) + 1 THEN ok := FALSE; RETURN END;

  i := 0;
  WHILE i < sLen DO
    HexToByte(s[i], s[i + 1], val, ok);
    IF NOT ok THEN RETURN END;
    AppendByte(dst, val);
    i := i + 2
  END;
  ok := TRUE
END Decode;

END Hex.
