IMPLEMENTATION MODULE URI;

(* ── Helpers ───────────────────────────────────────────────────── *)

PROCEDURE IsAlpha(ch: CHAR): BOOLEAN;
BEGIN
  RETURN ((ch >= 'a') AND (ch <= 'z')) OR
         ((ch >= 'A') AND (ch <= 'Z'))
END IsAlpha;

PROCEDURE IsDigit(ch: CHAR): BOOLEAN;
BEGIN
  RETURN (ch >= '0') AND (ch <= '9')
END IsDigit;

PROCEDURE ToLower(ch: CHAR): CHAR;
BEGIN
  IF (ch >= 'A') AND (ch <= 'Z') THEN
    RETURN CHR(ORD(ch) + 32)
  END;
  RETURN ch
END ToLower;

PROCEDURE HexVal(ch: CHAR; VAR val: INTEGER): BOOLEAN;
BEGIN
  IF (ch >= '0') AND (ch <= '9') THEN
    val := ORD(ch) - ORD('0'); RETURN TRUE
  ELSIF (ch >= 'a') AND (ch <= 'f') THEN
    val := ORD(ch) - ORD('a') + 10; RETURN TRUE
  ELSIF (ch >= 'A') AND (ch <= 'F') THEN
    val := ORD(ch) - ORD('A') + 10; RETURN TRUE
  END;
  RETURN FALSE
END HexVal;

PROCEDURE StrLen(VAR s: ARRAY OF CHAR): INTEGER;
VAR i: INTEGER;
BEGIN
  i := 0;
  WHILE (i <= HIGH(s)) AND (s[i] # 0C) DO INC(i) END;
  RETURN i
END StrLen;

PROCEDURE StrEqN(VAR a, b: ARRAY OF CHAR; n: INTEGER): BOOLEAN;
VAR i: INTEGER;
BEGIN
  FOR i := 0 TO n - 1 DO
    IF ToLower(a[i]) # ToLower(b[i]) THEN RETURN FALSE END
  END;
  RETURN TRUE
END StrEqN;

(* ── Parse ─────────────────────────────────────────────────────── *)

PROCEDURE Parse(VAR s: ARRAY OF CHAR; VAR uri: URIRec): Status;
VAR
  i, slen, start, portVal, digit: INTEGER;
  ch: CHAR;
BEGIN
  slen := StrLen(s);
  uri.schemeLen := 0;
  uri.hostLen := 0;
  uri.port := 0;
  uri.pathLen := 0;
  uri.queryLen := 0;
  uri.fragmentLen := 0;
  uri.scheme[0] := 0C;
  uri.host[0] := 0C;
  uri.path[0] := 0C;
  uri.query[0] := 0C;
  uri.fragment[0] := 0C;

  IF slen = 0 THEN RETURN Invalid END;
  i := 0;

  (* ── Scheme ── *)
  start := 0;
  IF IsAlpha(s[i]) THEN
    WHILE (i < slen) AND (s[i] # ':') DO INC(i) END;
    IF (i + 2 < slen) AND (s[i] = ':') AND
       (s[i+1] = '/') AND (s[i+2] = '/') THEN
      (* Valid scheme *)
      IF i - start >= MaxScheme THEN RETURN TooLong END;
      uri.schemeLen := i - start;
      FOR digit := 0 TO uri.schemeLen - 1 DO
        uri.scheme[digit] := ToLower(s[start + digit])
      END;
      uri.scheme[uri.schemeLen] := 0C;
      i := i + 3   (* skip :// *)
    ELSE
      RETURN BadScheme
    END
  ELSE
    RETURN BadScheme
  END;

  (* ── Host ── *)
  start := i;
  WHILE (i < slen) AND (s[i] # ':') AND (s[i] # '/') AND
        (s[i] # '?') AND (s[i] # '#') DO
    INC(i)
  END;
  uri.hostLen := i - start;
  IF uri.hostLen = 0 THEN RETURN BadHost END;
  IF uri.hostLen >= MaxHost THEN RETURN TooLong END;
  FOR digit := 0 TO uri.hostLen - 1 DO
    uri.host[digit] := s[start + digit]
  END;
  uri.host[uri.hostLen] := 0C;

  (* ── Port (optional) ── *)
  IF (i < slen) AND (s[i] = ':') THEN
    INC(i);
    portVal := 0;
    IF (i >= slen) OR NOT IsDigit(s[i]) THEN RETURN BadPort END;
    WHILE (i < slen) AND IsDigit(s[i]) DO
      portVal := portVal * 10 + (ORD(s[i]) - ORD('0'));
      IF portVal > 65535 THEN RETURN BadPort END;
      INC(i)
    END;
    uri.port := portVal
  ELSE
    uri.port := DefaultPort(uri.scheme, uri.schemeLen)
  END;

  (* ── Path ── *)
  IF (i < slen) AND (s[i] = '/') THEN
    start := i;
    WHILE (i < slen) AND (s[i] # '?') AND (s[i] # '#') DO
      INC(i)
    END;
    uri.pathLen := i - start;
    IF uri.pathLen >= MaxPath THEN RETURN TooLong END;
    FOR digit := 0 TO uri.pathLen - 1 DO
      uri.path[digit] := s[start + digit]
    END;
    uri.path[uri.pathLen] := 0C
  END;

  (* ── Query ── *)
  IF (i < slen) AND (s[i] = '?') THEN
    INC(i);
    start := i;
    WHILE (i < slen) AND (s[i] # '#') DO INC(i) END;
    uri.queryLen := i - start;
    IF uri.queryLen >= MaxQuery THEN RETURN TooLong END;
    FOR digit := 0 TO uri.queryLen - 1 DO
      uri.query[digit] := s[start + digit]
    END;
    uri.query[uri.queryLen] := 0C
  END;

  (* ── Fragment ── *)
  IF (i < slen) AND (s[i] = '#') THEN
    INC(i);
    start := i;
    WHILE i < slen DO INC(i) END;
    uri.fragmentLen := i - start;
    IF uri.fragmentLen >= MaxFragment THEN RETURN TooLong END;
    FOR digit := 0 TO uri.fragmentLen - 1 DO
      uri.fragment[digit] := s[start + digit]
    END;
    uri.fragment[uri.fragmentLen] := 0C
  END;

  RETURN OK
END Parse;

(* ── PercentDecode ─────────────────────────────────────────────── *)

PROCEDURE PercentDecode(VAR src: ARRAY OF CHAR; srcLen: INTEGER;
                        VAR dst: ARRAY OF CHAR;
                        VAR dstLen: INTEGER): Status;
VAR i, j, hi, lo, maxDst: INTEGER;
BEGIN
  i := 0;
  j := 0;
  maxDst := HIGH(dst);
  WHILE i < srcLen DO
    IF j > maxDst THEN RETURN TooLong END;
    IF (src[i] = '%') AND (i + 2 < srcLen) AND
       HexVal(src[i+1], hi) AND HexVal(src[i+2], lo) THEN
      dst[j] := CHR(hi * 16 + lo);
      i := i + 3
    ELSE
      dst[j] := src[i];
      INC(i)
    END;
    INC(j)
  END;
  dstLen := j;
  IF j <= maxDst THEN dst[j] := 0C END;
  RETURN OK
END PercentDecode;

(* ── DefaultPort ───────────────────────────────────────────────── *)

PROCEDURE DefaultPort(VAR scheme: ARRAY OF CHAR;
                      schemeLen: INTEGER): INTEGER;
VAR http, https: ARRAY [0..5] OF CHAR;
BEGIN
  http[0] := 'h'; http[1] := 't'; http[2] := 't';
  http[3] := 'p'; http[4] := 0C;
  https[0] := 'h'; https[1] := 't'; https[2] := 't';
  https[3] := 'p'; https[4] := 's'; https[5] := 0C;
  IF (schemeLen = 4) AND StrEqN(scheme, http, 4) THEN RETURN 80 END;
  IF (schemeLen = 5) AND StrEqN(scheme, https, 5) THEN RETURN 443 END;
  RETURN 0
END DefaultPort;

(* ── RequestPath ───────────────────────────────────────────────── *)

PROCEDURE RequestPath(VAR uri: URIRec;
                      VAR out: ARRAY OF CHAR;
                      VAR outLen: INTEGER): Status;
VAR i, j, maxOut: INTEGER;
BEGIN
  j := 0;
  maxOut := HIGH(out);

  IF uri.pathLen = 0 THEN
    IF j > maxOut THEN RETURN TooLong END;
    out[j] := '/'; INC(j)
  ELSE
    FOR i := 0 TO uri.pathLen - 1 DO
      IF j > maxOut THEN RETURN TooLong END;
      out[j] := uri.path[i]; INC(j)
    END
  END;

  IF uri.queryLen > 0 THEN
    IF j > maxOut THEN RETURN TooLong END;
    out[j] := '?'; INC(j);
    FOR i := 0 TO uri.queryLen - 1 DO
      IF j > maxOut THEN RETURN TooLong END;
      out[j] := uri.query[i]; INC(j)
    END
  END;

  outLen := j;
  IF j <= maxOut THEN out[j] := 0C END;
  RETURN OK
END RequestPath;

END URI.
