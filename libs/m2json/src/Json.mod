IMPLEMENTATION MODULE Json;

FROM SYSTEM IMPORT ADDRESS, ADR, TSIZE;

(* ── Internal helpers ────────────────────────────────── *)

PROCEDURE CharAt(VAR p: Parser; idx: CARDINAL): CHAR;
BEGIN
  IF idx >= p.srcLen THEN RETURN 0C END;
  RETURN p.src^[idx]
END CharAt;

PROCEDURE IsWhitespace(ch: CHAR): BOOLEAN;
BEGIN
  RETURN (ch = ' ') OR (ch = CHR(9)) OR (ch = CHR(10)) OR (ch = CHR(13))
END IsWhitespace;

PROCEDURE IsDigit(ch: CHAR): BOOLEAN;
BEGIN
  RETURN (ch >= '0') AND (ch <= '9')
END IsDigit;

PROCEDURE SkipWS(VAR p: Parser);
BEGIN
  WHILE (p.pos < p.srcLen) AND IsWhitespace(CharAt(p, p.pos)) DO
    INC(p.pos)
  END
END SkipWS;

PROCEDURE SetError(VAR p: Parser; msg: ARRAY OF CHAR);
VAR i, lim: CARDINAL;
BEGIN
  p.hasError := TRUE;
  lim := HIGH(msg);
  IF lim > HIGH(p.err) THEN lim := HIGH(p.err) END;
  i := 0;
  WHILE (i <= lim) AND (msg[i] # 0C) DO
    p.err[i] := msg[i];
    INC(i)
  END;
  IF i <= HIGH(p.err) THEN p.err[i] := 0C END
END SetError;

PROCEDURE CopyStr(src: ARRAY OF CHAR; VAR dst: ARRAY OF CHAR);
VAR i, lim: CARDINAL;
BEGIN
  lim := HIGH(src);
  IF lim > HIGH(dst) THEN lim := HIGH(dst) END;
  i := 0;
  WHILE (i <= lim) AND (src[i] # 0C) DO
    dst[i] := src[i];
    INC(i)
  END;
  IF i <= HIGH(dst) THEN dst[i] := 0C END
END CopyStr;

(* ── Hex digit helper ────────────────────────────────── *)

PROCEDURE HexVal(ch: CHAR; VAR val: CARDINAL): BOOLEAN;
BEGIN
  IF (ch >= '0') AND (ch <= '9') THEN
    val := ORD(ch) - ORD('0');
    RETURN TRUE
  ELSIF (ch >= 'a') AND (ch <= 'f') THEN
    val := ORD(ch) - ORD('a') + 10;
    RETURN TRUE
  ELSIF (ch >= 'A') AND (ch <= 'F') THEN
    val := ORD(ch) - ORD('A') + 10;
    RETURN TRUE
  END;
  RETURN FALSE
END HexVal;

(* ── Lifecycle ───────────────────────────────────────── *)

PROCEDURE Init(VAR p: Parser; src: ADDRESS; srcLen: CARDINAL);
BEGIN
  p.src := src;
  p.srcLen := srcLen;
  p.pos := 0;
  p.hasError := FALSE;
  p.err[0] := 0C
END Init;

(* ── Tokenisation ────────────────────────────────────── *)

PROCEDURE ScanString(VAR p: Parser; VAR tok: Token): BOOLEAN;
(* Called with p.pos pointing at the opening quote. *)
VAR ch: CHAR;
BEGIN
  INC(p.pos); (* skip opening " *)
  tok.start := p.pos;
  WHILE p.pos < p.srcLen DO
    ch := CharAt(p, p.pos);
    IF ch = '"' THEN
      tok.len := p.pos - tok.start;
      INC(p.pos); (* skip closing " *)
      tok.kind := JString;
      RETURN TRUE
    ELSIF ch = CHR(92) THEN (* backslash *)
      INC(p.pos);
      IF p.pos >= p.srcLen THEN
        SetError(p, "unexpected end in string escape");
        tok.kind := JError;
        tok.len := 0;
        RETURN FALSE
      END;
      INC(p.pos) (* skip escaped char *)
    ELSE
      INC(p.pos)
    END
  END;
  SetError(p, "unterminated string");
  tok.kind := JError;
  tok.len := 0;
  RETURN FALSE
END ScanString;

PROCEDURE ScanNumber(VAR p: Parser; VAR tok: Token): BOOLEAN;
VAR start: CARDINAL;
BEGIN
  start := p.pos;
  tok.start := start;

  (* optional leading minus *)
  IF (p.pos < p.srcLen) AND (CharAt(p, p.pos) = '-') THEN
    INC(p.pos)
  END;

  (* integer part *)
  IF (p.pos >= p.srcLen) OR NOT IsDigit(CharAt(p, p.pos)) THEN
    SetError(p, "expected digit in number");
    tok.kind := JError;
    tok.len := p.pos - start;
    RETURN FALSE
  END;
  IF CharAt(p, p.pos) = '0' THEN
    INC(p.pos)
  ELSE
    WHILE (p.pos < p.srcLen) AND IsDigit(CharAt(p, p.pos)) DO
      INC(p.pos)
    END
  END;

  (* fractional part *)
  IF (p.pos < p.srcLen) AND (CharAt(p, p.pos) = '.') THEN
    INC(p.pos);
    IF (p.pos >= p.srcLen) OR NOT IsDigit(CharAt(p, p.pos)) THEN
      SetError(p, "expected digit after decimal point");
      tok.kind := JError;
      tok.len := p.pos - start;
      RETURN FALSE
    END;
    WHILE (p.pos < p.srcLen) AND IsDigit(CharAt(p, p.pos)) DO
      INC(p.pos)
    END
  END;

  (* exponent part *)
  IF (p.pos < p.srcLen) AND
     ((CharAt(p, p.pos) = 'e') OR (CharAt(p, p.pos) = 'E')) THEN
    INC(p.pos);
    IF (p.pos < p.srcLen) AND
       ((CharAt(p, p.pos) = '+') OR (CharAt(p, p.pos) = '-')) THEN
      INC(p.pos)
    END;
    IF (p.pos >= p.srcLen) OR NOT IsDigit(CharAt(p, p.pos)) THEN
      SetError(p, "expected digit in exponent");
      tok.kind := JError;
      tok.len := p.pos - start;
      RETURN FALSE
    END;
    WHILE (p.pos < p.srcLen) AND IsDigit(CharAt(p, p.pos)) DO
      INC(p.pos)
    END
  END;

  tok.kind := JNumber;
  tok.len := p.pos - start;
  RETURN TRUE
END ScanNumber;

PROCEDURE MatchKeyword(VAR p: Parser; kw: ARRAY OF CHAR): BOOLEAN;
VAR i, kwLen: CARDINAL;
BEGIN
  kwLen := 0;
  WHILE (kwLen <= HIGH(kw)) AND (kw[kwLen] # 0C) DO INC(kwLen) END;

  IF p.pos + kwLen > p.srcLen THEN RETURN FALSE END;
  i := 0;
  WHILE i < kwLen DO
    IF CharAt(p, p.pos + i) # kw[i] THEN RETURN FALSE END;
    INC(i)
  END;
  p.pos := p.pos + kwLen;
  RETURN TRUE
END MatchKeyword;

PROCEDURE Next(VAR p: Parser; VAR tok: Token): BOOLEAN;
VAR ch: CHAR;
BEGIN
  IF p.hasError THEN
    tok.kind := JError;
    tok.start := p.pos;
    tok.len := 0;
    RETURN FALSE
  END;

  SkipWS(p);

  IF p.pos >= p.srcLen THEN
    tok.kind := JEnd;
    tok.start := p.pos;
    tok.len := 0;
    RETURN FALSE
  END;

  ch := CharAt(p, p.pos);

  (* structural characters *)
  IF ch = '{' THEN
    tok.kind := JObjectStart; tok.start := p.pos; tok.len := 1;
    INC(p.pos); RETURN TRUE
  ELSIF ch = '}' THEN
    tok.kind := JObjectEnd; tok.start := p.pos; tok.len := 1;
    INC(p.pos); RETURN TRUE
  ELSIF ch = '[' THEN
    tok.kind := JArrayStart; tok.start := p.pos; tok.len := 1;
    INC(p.pos); RETURN TRUE
  ELSIF ch = ']' THEN
    tok.kind := JArrayEnd; tok.start := p.pos; tok.len := 1;
    INC(p.pos); RETURN TRUE
  ELSIF ch = ':' THEN
    tok.kind := JColon; tok.start := p.pos; tok.len := 1;
    INC(p.pos); RETURN TRUE
  ELSIF ch = ',' THEN
    tok.kind := JComma; tok.start := p.pos; tok.len := 1;
    INC(p.pos); RETURN TRUE

  (* string *)
  ELSIF ch = '"' THEN
    RETURN ScanString(p, tok)

  (* number *)
  ELSIF IsDigit(ch) OR (ch = '-') THEN
    RETURN ScanNumber(p, tok)

  (* keywords *)
  ELSIF ch = 't' THEN
    tok.start := p.pos;
    IF MatchKeyword(p, "true") THEN
      tok.kind := JTrue; tok.len := 4; RETURN TRUE
    ELSE
      SetError(p, "invalid token");
      tok.kind := JError; tok.len := 0; RETURN FALSE
    END
  ELSIF ch = 'f' THEN
    tok.start := p.pos;
    IF MatchKeyword(p, "false") THEN
      tok.kind := JFalse; tok.len := 5; RETURN TRUE
    ELSE
      SetError(p, "invalid token");
      tok.kind := JError; tok.len := 0; RETURN FALSE
    END
  ELSIF ch = 'n' THEN
    tok.start := p.pos;
    IF MatchKeyword(p, "null") THEN
      tok.kind := JNull; tok.len := 4; RETURN TRUE
    ELSE
      SetError(p, "invalid token");
      tok.kind := JError; tok.len := 0; RETURN FALSE
    END
  ELSE
    tok.start := p.pos;
    tok.len := 1;
    tok.kind := JError;
    SetError(p, "unexpected character");
    RETURN FALSE
  END
END Next;

(* ── Value extraction ────────────────────────────────── *)

PROCEDURE GetString(VAR p: Parser; VAR tok: Token;
                    VAR buf: ARRAY OF CHAR): BOOLEAN;
VAR
  i, out, limit: CARDINAL;
  ch, esc: CHAR;
  h, hv, cp: CARDINAL;
BEGIN
  IF tok.kind # JString THEN
    IF 0 <= HIGH(buf) THEN buf[0] := 0C END;
    RETURN FALSE
  END;

  out := 0;
  limit := HIGH(buf);
  i := tok.start;

  WHILE i < tok.start + tok.len DO
    ch := p.src^[i];
    IF ch = CHR(92) THEN (* backslash *)
      INC(i);
      IF i >= tok.start + tok.len THEN
        (* truncated escape -- should not happen if ScanString succeeded *)
        IF out <= limit THEN buf[out] := 0C END;
        RETURN FALSE
      END;
      esc := p.src^[i];
      IF esc = 'n' THEN
        IF out <= limit THEN buf[out] := CHR(10); INC(out) END
      ELSIF esc = 't' THEN
        IF out <= limit THEN buf[out] := CHR(9); INC(out) END
      ELSIF esc = 'r' THEN
        IF out <= limit THEN buf[out] := CHR(13); INC(out) END
      ELSIF esc = 'b' THEN
        IF out <= limit THEN buf[out] := CHR(8); INC(out) END
      ELSIF esc = 'f' THEN
        IF out <= limit THEN buf[out] := CHR(12); INC(out) END
      ELSIF esc = CHR(92) THEN
        IF out <= limit THEN buf[out] := CHR(92); INC(out) END
      ELSIF esc = '"' THEN
        IF out <= limit THEN buf[out] := '"'; INC(out) END
      ELSIF esc = '/' THEN
        IF out <= limit THEN buf[out] := '/'; INC(out) END
      ELSIF esc = 'u' THEN
        (* \uXXXX: decode 4 hex digits into codepoint *)
        cp := 0;
        h := 0;
        WHILE (h < 4) AND (i + 1 + h < tok.start + tok.len) DO
          IF HexVal(p.src^[i + 1 + h], hv) THEN
            cp := cp * 16 + hv
          ELSE
            IF out <= limit THEN buf[out] := 0C END;
            RETURN FALSE
          END;
          INC(h)
        END;
        IF h < 4 THEN
          IF out <= limit THEN buf[out] := 0C END;
          RETURN FALSE
        END;
        i := i + 4; (* skip the 4 hex digits *)
        (* Emit as single byte for ASCII, else UTF-8 *)
        IF cp < 128 THEN
          IF out <= limit THEN buf[out] := CHR(cp); INC(out) END
        ELSIF cp < 2048 THEN
          IF out <= limit THEN
            buf[out] := CHR(192 + cp DIV 64); INC(out)
          END;
          IF out <= limit THEN
            buf[out] := CHR(128 + cp MOD 64); INC(out)
          END
        ELSE
          IF out <= limit THEN
            buf[out] := CHR(224 + cp DIV 4096); INC(out)
          END;
          IF out <= limit THEN
            buf[out] := CHR(128 + (cp DIV 64) MOD 64); INC(out)
          END;
          IF out <= limit THEN
            buf[out] := CHR(128 + cp MOD 64); INC(out)
          END
        END
      ELSE
        (* unknown escape: emit literal *)
        IF out <= limit THEN buf[out] := esc; INC(out) END
      END;
      INC(i)
    ELSE
      IF out <= limit THEN buf[out] := ch; INC(out) END;
      INC(i)
    END
  END;

  IF out <= limit THEN buf[out] := 0C END;
  RETURN TRUE
END GetString;

PROCEDURE GetInteger(VAR p: Parser; VAR tok: Token;
                     VAR val: INTEGER): BOOLEAN;
VAR
  i, j, endPos: CARDINAL;
  neg: BOOLEAN;
  ch: CHAR;
  result: INTEGER;
BEGIN
  IF tok.kind # JNumber THEN RETURN FALSE END;

  i := tok.start;
  endPos := tok.start + tok.len;
  neg := FALSE;
  result := 0;

  IF (i < endPos) AND (p.src^[i] = '-') THEN
    neg := TRUE;
    INC(i)
  END;

  (* reject if it contains a decimal point or exponent *)
  j := i;
  WHILE j < endPos DO
    ch := p.src^[j];
    IF (ch = '.') OR (ch = 'e') OR (ch = 'E') THEN
      RETURN FALSE
    END;
    INC(j)
  END;

  WHILE i < endPos DO
    ch := p.src^[i];
    IF NOT IsDigit(ch) THEN RETURN FALSE END;
    result := result * 10 + VAL(INTEGER, ORD(ch) - ORD('0'));
    INC(i)
  END;

  IF neg THEN val := -result ELSE val := result END;
  RETURN TRUE
END GetInteger;

PROCEDURE GetLong(VAR p: Parser; VAR tok: Token;
                  VAR val: LONGINT): BOOLEAN;
VAR
  i, j, endPos: CARDINAL;
  neg: BOOLEAN;
  ch: CHAR;
  result: LONGINT;
BEGIN
  IF tok.kind # JNumber THEN RETURN FALSE END;

  i := tok.start;
  endPos := tok.start + tok.len;
  neg := FALSE;
  result := 0;

  IF (i < endPos) AND (p.src^[i] = '-') THEN
    neg := TRUE;
    INC(i)
  END;

  (* reject if it contains a decimal point or exponent *)
  j := i;
  WHILE j < endPos DO
    ch := p.src^[j];
    IF (ch = '.') OR (ch = 'e') OR (ch = 'E') THEN
      RETURN FALSE
    END;
    INC(j)
  END;

  WHILE i < endPos DO
    ch := p.src^[i];
    IF NOT IsDigit(ch) THEN RETURN FALSE END;
    result := result * 10 + VAL(LONGINT, ORD(ch) - ORD('0'));
    INC(i)
  END;

  IF neg THEN val := -result ELSE val := result END;
  RETURN TRUE
END GetLong;

PROCEDURE GetReal(VAR p: Parser; VAR tok: Token;
                  VAR val: REAL): BOOLEAN;
VAR
  i, endPos: CARDINAL;
  neg, negExp: BOOLEAN;
  ch: CHAR;
  result, frac, divisor: REAL;
  exp: INTEGER;
BEGIN
  IF tok.kind # JNumber THEN RETURN FALSE END;

  i := tok.start;
  endPos := tok.start + tok.len;
  neg := FALSE;
  result := 0.0;

  IF (i < endPos) AND (p.src^[i] = '-') THEN
    neg := TRUE;
    INC(i)
  END;

  (* integer part *)
  WHILE (i < endPos) AND IsDigit(p.src^[i]) DO
    result := result * 10.0 + FLOAT(ORD(p.src^[i]) - ORD('0'));
    INC(i)
  END;

  (* fractional part *)
  IF (i < endPos) AND (p.src^[i] = '.') THEN
    INC(i);
    divisor := 10.0;
    WHILE (i < endPos) AND IsDigit(p.src^[i]) DO
      frac := FLOAT(ORD(p.src^[i]) - ORD('0'));
      result := result + frac / divisor;
      divisor := divisor * 10.0;
      INC(i)
    END
  END;

  (* exponent part *)
  IF (i < endPos) AND ((p.src^[i] = 'e') OR (p.src^[i] = 'E')) THEN
    INC(i);
    negExp := FALSE;
    IF (i < endPos) AND (p.src^[i] = '-') THEN
      negExp := TRUE; INC(i)
    ELSIF (i < endPos) AND (p.src^[i] = '+') THEN
      INC(i)
    END;
    exp := 0;
    WHILE (i < endPos) AND IsDigit(p.src^[i]) DO
      exp := exp * 10 + VAL(INTEGER, ORD(p.src^[i]) - ORD('0'));
      INC(i)
    END;
    (* apply exponent *)
    WHILE exp > 0 DO
      IF negExp THEN result := result / 10.0
      ELSE result := result * 10.0
      END;
      DEC(exp)
    END
  END;

  IF neg THEN val := -result ELSE val := result END;
  RETURN TRUE
END GetReal;

(* ── Navigation ──────────────────────────────────────── *)

PROCEDURE Skip(VAR p: Parser);
VAR tok: Token; depth: INTEGER;
BEGIN
  IF NOT Next(p, tok) THEN RETURN END;

  IF tok.kind = JObjectStart THEN
    depth := 1;
    WHILE (depth > 0) AND Next(p, tok) DO
      IF tok.kind = JObjectStart THEN INC(depth)
      ELSIF tok.kind = JObjectEnd THEN DEC(depth)
      END
    END
  ELSIF tok.kind = JArrayStart THEN
    depth := 1;
    WHILE (depth > 0) AND Next(p, tok) DO
      IF tok.kind = JArrayStart THEN INC(depth)
      ELSIF tok.kind = JArrayEnd THEN DEC(depth)
      END
    END
  END
  (* scalar tokens: already consumed by Next *)
END Skip;

(* ── Error reporting ─────────────────────────────────── *)

PROCEDURE GetError(VAR p: Parser; VAR buf: ARRAY OF CHAR);
BEGIN
  IF p.hasError THEN
    CopyStr(p.err, buf)
  ELSE
    IF 0 <= HIGH(buf) THEN buf[0] := 0C END
  END
END GetError;

END Json.
