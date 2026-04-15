IMPLEMENTATION MODULE Tokenizer;

FROM SYSTEM IMPORT ADDRESS, LONGCARD;

(* ── Byte access helpers (same pattern as Text.mod) ─── *)

TYPE
  CharPtr = POINTER TO CHAR;

PROCEDURE PtrAt(base: ADDRESS; idx: CARDINAL): CharPtr;
BEGIN
  RETURN CharPtr(LONGCARD(base) + LONGCARD(idx))
END PtrAt;

PROCEDURE GetCh(base: ADDRESS; i: CARDINAL): CHAR;
VAR p: CharPtr;
BEGIN
  p := PtrAt(base, i);
  RETURN p^
END GetCh;

(* ── Character classification ────────────────────────── *)

PROCEDURE IsLetter(ch: CHAR): BOOLEAN;
BEGIN
  RETURN ((ch >= 'a') AND (ch <= 'z')) OR
         ((ch >= 'A') AND (ch <= 'Z')) OR
         (ch = '_')
END IsLetter;

PROCEDURE IsDigit(ch: CHAR): BOOLEAN;
BEGIN
  RETURN (ch >= '0') AND (ch <= '9')
END IsDigit;

PROCEDURE IsWhitespace(ch: CHAR): BOOLEAN;
BEGIN
  RETURN (ch = ' ') OR (ch = CHR(9)) OR (ch = CHR(10)) OR
         (ch = CHR(13)) OR (ch = CHR(12))
END IsWhitespace;

(* ── Skip helpers ────────────────────────────────────── *)

PROCEDURE SkipWhitespace(VAR s: State);
BEGIN
  WHILE (s.pos < s.blen) AND IsWhitespace(GetCh(s.buf, s.pos)) DO
    INC(s.pos)
  END
END SkipWhitespace;

(* Skip a "..." or '...' string, handling backslash escapes.
   Assumes s.pos is on the opening quote. Advances past closing quote. *)
PROCEDURE SkipString(VAR s: State);
VAR q: CHAR;
BEGIN
  q := GetCh(s.buf, s.pos);
  INC(s.pos);  (* skip opening quote *)
  WHILE s.pos < s.blen DO
    IF GetCh(s.buf, s.pos) = CHR(92) THEN
      (* backslash: skip next char *)
      INC(s.pos, 2)
    ELSIF GetCh(s.buf, s.pos) = q THEN
      INC(s.pos);  (* skip closing quote *)
      RETURN
    ELSE
      INC(s.pos)
    END
  END
END SkipString;

(* Skip // or # line comment. Advances past newline or to end. *)
PROCEDURE SkipLineComment(VAR s: State);
BEGIN
  WHILE (s.pos < s.blen) AND (GetCh(s.buf, s.pos) # CHR(10)) DO
    INC(s.pos)
  END;
  IF s.pos < s.blen THEN
    INC(s.pos)  (* skip the newline *)
  END
END SkipLineComment;

(* Skip /* ... */ block comment. Handles nesting. *)
PROCEDURE SkipBlockComment(VAR s: State);
VAR depth: CARDINAL;
BEGIN
  INC(s.pos, 2);  (* skip opening /* *)
  depth := 1;
  WHILE (s.pos < s.blen) AND (depth > 0) DO
    IF (GetCh(s.buf, s.pos) = '/') AND
       (s.pos + 1 < s.blen) AND
       (GetCh(s.buf, s.pos + 1) = '*') THEN
      INC(depth);
      INC(s.pos, 2)
    ELSIF (GetCh(s.buf, s.pos) = '*') AND
          (s.pos + 1 < s.blen) AND
          (GetCh(s.buf, s.pos + 1) = '/') THEN
      DEC(depth);
      INC(s.pos, 2)
    ELSE
      INC(s.pos)
    END
  END
END SkipBlockComment;

(* ── Public API ──────────────────────────────────────── *)

PROCEDURE Init(VAR s: State; buf: ADDRESS; len: CARDINAL);
BEGIN
  s.buf := buf;
  s.blen := len;
  s.pos := 0;
  s.keepStrings := FALSE
END Init;

PROCEDURE SetKeepStrings(VAR s: State; keep: BOOLEAN);
BEGIN
  s.keepStrings := keep
END SetKeepStrings;

PROCEDURE Next(VAR s: State; VAR t: Token): BOOLEAN;
VAR ch, ch2: CHAR;
BEGIN
  LOOP
    SkipWhitespace(s);
    IF s.pos >= s.blen THEN
      RETURN FALSE
    END;

    ch := GetCh(s.buf, s.pos);

    (* Shebang: only at position 0 *)
    IF (s.pos = 0) AND (ch = '#') AND
       (s.pos + 1 < s.blen) AND (GetCh(s.buf, s.pos + 1) = '!') THEN
      t.start := s.pos;
      t.kind := Shebang;
      (* advance to end of line *)
      WHILE (s.pos < s.blen) AND (GetCh(s.buf, s.pos) # CHR(10)) DO
        INC(s.pos)
      END;
      t.len := s.pos - t.start;
      IF s.pos < s.blen THEN
        INC(s.pos)  (* skip newline *)
      END;
      RETURN TRUE
    END;

    (* String literals: skip or yield *)
    IF (ch = '"') OR (ch = "'") THEN
      IF s.keepStrings THEN
        t.start := s.pos + 1;  (* skip opening quote *)
        t.kind := StringLit;
        SkipString(s);
        (* t.len = content between quotes *)
        t.len := s.pos - t.start - 1;
        RETURN TRUE
      ELSE
        SkipString(s)
      END

    (* Block comment: /* ... */ — skip *)
    ELSIF (ch = '/') AND (s.pos + 1 < s.blen) AND
          (GetCh(s.buf, s.pos + 1) = '*') THEN
      SkipBlockComment(s)

    (* Line comment: // — skip *)
    ELSIF (ch = '/') AND (s.pos + 1 < s.blen) AND
          (GetCh(s.buf, s.pos + 1) = '/') THEN
      SkipLineComment(s)

    (* Line comment: # — skip (but not shebang, handled above) *)
    ELSIF ch = '#' THEN
      SkipLineComment(s)

    (* Identifier: letter/digit/underscore run *)
    ELSIF IsLetter(ch) OR IsDigit(ch) THEN
      t.start := s.pos;
      t.kind := Ident;
      WHILE (s.pos < s.blen) AND
            (IsLetter(GetCh(s.buf, s.pos)) OR
             IsDigit(GetCh(s.buf, s.pos))) DO
        INC(s.pos)
      END;
      t.len := s.pos - t.start;
      RETURN TRUE

    (* Operator: single punctuation character *)
    ELSE
      t.start := s.pos;
      t.len := 1;
      t.kind := Operator;
      INC(s.pos);
      RETURN TRUE
    END
  END  (* LOOP *)
END Next;

PROCEDURE CopyToken(VAR s: State; VAR t: Token; VAR out: ARRAY OF CHAR);
VAR i, n: CARDINAL;
BEGIN
  n := t.len;
  IF n > HIGH(out) THEN
    n := HIGH(out)
  END;
  i := 0;
  WHILE i < n DO
    out[i] := GetCh(s.buf, t.start + i);
    INC(i)
  END;
  IF i <= HIGH(out) THEN
    out[i] := 0C
  END
END CopyToken;

END Tokenizer.
