IMPLEMENTATION MODULE Path;

FROM Strings IMPORT Assign, Length, CompareStr;

CONST
  MaxSegs = 64;
  MaxSeg  = 256;

TYPE
  SegArray = ARRAY [0..MaxSegs-1] OF ARRAY [0..MaxSeg-1] OF CHAR;

(* ── Internal helpers ──────────────────────────────────── *)

PROCEDURE StrLen(VAR s: ARRAY OF CHAR): CARDINAL;
BEGIN
  RETURN Length(s)
END StrLen;

PROCEDURE StrCopy(VAR src: ARRAY OF CHAR; from, len: CARDINAL;
                  VAR dst: ARRAY OF CHAR);
VAR i: CARDINAL;
BEGIN
  i := 0;
  WHILE (i < len) AND (from + i <= HIGH(src)) AND (i <= HIGH(dst)) DO
    dst[i] := src[from + i];
    INC(i)
  END;
  IF i <= HIGH(dst) THEN
    dst[i] := 0C
  END
END StrCopy;

PROCEDURE AppendChar(VAR s: ARRAY OF CHAR; VAR pos: CARDINAL; ch: CHAR);
BEGIN
  IF pos <= HIGH(s) THEN
    s[pos] := ch;
    INC(pos)
  END;
  IF pos <= HIGH(s) THEN
    s[pos] := 0C
  END
END AppendChar;

PROCEDURE AppendStr(VAR dst: ARRAY OF CHAR; VAR pos: CARDINAL;
                    VAR src: ARRAY OF CHAR);
VAR i, slen: CARDINAL;
BEGIN
  slen := StrLen(src);
  i := 0;
  WHILE (i < slen) AND (pos <= HIGH(dst)) DO
    dst[pos] := src[i];
    INC(pos);
    INC(i)
  END;
  IF pos <= HIGH(dst) THEN
    dst[pos] := 0C
  END
END AppendStr;

PROCEDURE IsDot(VAR s: ARRAY OF CHAR): BOOLEAN;
BEGIN
  RETURN (Length(s) = 1) AND (s[0] = '.')
END IsDot;

PROCEDURE IsDotDot(VAR s: ARRAY OF CHAR): BOOLEAN;
BEGIN
  RETURN (Length(s) = 2) AND (s[0] = '.') AND (s[1] = '.')
END IsDotDot;

(* ── ParseSegments: split path on '/' into fixed-size seg array ── *)

PROCEDURE ParseSegments(VAR path: ARRAY OF CHAR;
                        VAR segs: SegArray;
                        VAR nsegs: CARDINAL);
VAR
  plen, i, segStart: CARDINAL;
BEGIN
  plen := StrLen(path);
  nsegs := 0;
  i := 0;
  WHILE i < plen DO
    WHILE (i < plen) AND (path[i] = '/') DO
      INC(i)
    END;
    IF i < plen THEN
      segStart := i;
      WHILE (i < plen) AND (path[i] # '/') DO
        INC(i)
      END;
      IF nsegs < MaxSegs THEN
        StrCopy(path, segStart, i - segStart, segs[nsegs]);
        INC(nsegs)
      END
    END
  END
END ParseSegments;

(* ── Normalize ─────────────────────────────────────────── *)

PROCEDURE Normalize(path: ARRAY OF CHAR; VAR out: ARRAY OF CHAR);
VAR
  segs: SegArray;
  nsegs: CARDINAL;
  plen, i, segStart: CARDINAL;
  isAbs: BOOLEAN;
  pos: CARDINAL;
BEGIN
  plen := Length(path);

  IF plen = 0 THEN
    out[0] := '.';
    IF HIGH(out) >= 1 THEN out[1] := 0C END;
    RETURN
  END;

  isAbs := (path[0] = '/');

  nsegs := 0;
  i := 0;
  WHILE i < plen DO
    WHILE (i < plen) AND (path[i] = '/') DO
      INC(i)
    END;
    IF i < plen THEN
      segStart := i;
      WHILE (i < plen) AND (path[i] # '/') DO
        INC(i)
      END;
      IF nsegs < MaxSegs THEN
        StrCopy(path, segStart, i - segStart, segs[nsegs]);

        IF IsDot(segs[nsegs]) THEN
          (* skip "." segment *)
        ELSIF IsDotDot(segs[nsegs]) THEN
          IF isAbs THEN
            IF nsegs > 0 THEN
              DEC(nsegs)
            END
          ELSE
            IF (nsegs > 0) AND NOT IsDotDot(segs[nsegs - 1]) THEN
              DEC(nsegs)
            ELSE
              INC(nsegs)
            END
          END
        ELSE
          INC(nsegs)
        END
      END
    END
  END;

  pos := 0;
  IF isAbs THEN
    AppendChar(out, pos, '/');
    IF nsegs = 0 THEN
      RETURN
    END
  ELSE
    IF nsegs = 0 THEN
      out[0] := '.';
      IF HIGH(out) >= 1 THEN out[1] := 0C END;
      RETURN
    END
  END;

  i := 0;
  WHILE i < nsegs DO
    IF i > 0 THEN
      AppendChar(out, pos, '/')
    END;
    AppendStr(out, pos, segs[i]);
    INC(i)
  END;

  IF pos <= HIGH(out) THEN
    out[pos] := 0C
  END
END Normalize;

(* ── Extension ─────────────────────────────────────────── *)

PROCEDURE FindBasenameStart(VAR path: ARRAY OF CHAR; plen: CARDINAL): CARDINAL;
VAR i: CARDINAL;
BEGIN
  IF plen = 0 THEN RETURN 0 END;
  i := plen;
  WHILE i > 0 DO
    DEC(i);
    IF path[i] = '/' THEN
      RETURN i + 1
    END
  END;
  RETURN 0
END FindBasenameStart;

PROCEDURE Extension(path: ARRAY OF CHAR; VAR out: ARRAY OF CHAR);
VAR
  plen, baseStart, i, dotPos: CARDINAL;
  found: BOOLEAN;
BEGIN
  plen := Length(path);
  out[0] := 0C;

  IF plen = 0 THEN RETURN END;

  baseStart := FindBasenameStart(path, plen);

  found := FALSE;
  dotPos := 0;
  i := plen;
  WHILE i > baseStart DO
    DEC(i);
    IF path[i] = '.' THEN
      IF i = baseStart THEN
        RETURN
      END;
      dotPos := i;
      found := TRUE;
      i := baseStart
    END
  END;

  IF NOT found THEN RETURN END;

  StrCopy(path, dotPos, plen - dotPos, out)
END Extension;

(* ── StripExt ──────────────────────────────────────────── *)

PROCEDURE StripExt(path: ARRAY OF CHAR; VAR out: ARRAY OF CHAR);
VAR
  plen, baseStart, i, dotPos: CARDINAL;
  found: BOOLEAN;
BEGIN
  plen := Length(path);

  IF plen = 0 THEN
    out[0] := 0C;
    RETURN
  END;

  baseStart := FindBasenameStart(path, plen);

  found := FALSE;
  dotPos := 0;
  i := plen;
  WHILE i > baseStart DO
    DEC(i);
    IF path[i] = '.' THEN
      IF i = baseStart THEN
        found := FALSE
      ELSE
        dotPos := i;
        found := TRUE
      END;
      i := baseStart
    END
  END;

  IF found THEN
    StrCopy(path, 0, dotPos, out)
  ELSE
    Assign(path, out)
  END
END StripExt;

(* ── IsAbsolute ────────────────────────────────────────── *)

PROCEDURE IsAbsolute(path: ARRAY OF CHAR): BOOLEAN;
BEGIN
  IF Length(path) = 0 THEN RETURN FALSE END;
  RETURN path[0] = '/'
END IsAbsolute;

(* ── Split ─────────────────────────────────────────────── *)

PROCEDURE Split(path: ARRAY OF CHAR;
                VAR dir: ARRAY OF CHAR; VAR base: ARRAY OF CHAR);
VAR
  plen, i, lastSlash: CARDINAL;
  found: BOOLEAN;
BEGIN
  plen := Length(path);

  IF plen = 0 THEN
    dir[0] := 0C;
    base[0] := 0C;
    RETURN
  END;

  found := FALSE;
  lastSlash := 0;
  i := plen;
  WHILE i > 0 DO
    DEC(i);
    IF path[i] = '/' THEN
      lastSlash := i;
      found := TRUE;
      i := 0
    END
  END;

  IF NOT found THEN
    dir[0] := 0C;
    Assign(path, base);
    RETURN
  END;

  IF lastSlash = 0 THEN
    dir[0] := '/';
    IF HIGH(dir) >= 1 THEN dir[1] := 0C END;
    IF lastSlash + 1 < plen THEN
      StrCopy(path, lastSlash + 1, plen - lastSlash - 1, base)
    ELSE
      base[0] := 0C
    END
  ELSE
    StrCopy(path, 0, lastSlash, dir);
    IF lastSlash + 1 < plen THEN
      StrCopy(path, lastSlash + 1, plen - lastSlash - 1, base)
    ELSE
      base[0] := 0C
    END
  END
END Split;

(* ── RelativeTo ────────────────────────────────────────── *)

PROCEDURE RelativeTo(base: ARRAY OF CHAR; target: ARRAY OF CHAR;
                     VAR out: ARRAY OF CHAR);
VAR
  normBase, normTarget: ARRAY [0..1023] OF CHAR;
  baseSegs, targSegs: SegArray;
  nBase, nTarg: CARDINAL;
  common, i, pos: CARDINAL;
  dotdotStr: ARRAY [0..3] OF CHAR;
  done: BOOLEAN;
BEGIN
  Normalize(base, normBase);
  Normalize(target, normTarget);

  dotdotStr[0] := '.'; dotdotStr[1] := '.'; dotdotStr[2] := 0C;

  ParseSegments(normBase, baseSegs, nBase);
  ParseSegments(normTarget, targSegs, nTarg);

  (* Find common prefix length *)
  common := 0;
  done := FALSE;
  WHILE (common < nBase) AND (common < nTarg) AND NOT done DO
    IF CompareStr(baseSegs[common], targSegs[common]) = 0 THEN
      INC(common)
    ELSE
      done := TRUE
    END
  END;

  pos := 0;
  out[0] := 0C;

  i := common;
  WHILE i < nBase DO
    IF pos > 0 THEN
      AppendChar(out, pos, '/')
    END;
    AppendStr(out, pos, dotdotStr);
    INC(i)
  END;

  i := common;
  WHILE i < nTarg DO
    IF pos > 0 THEN
      AppendChar(out, pos, '/')
    END;
    AppendStr(out, pos, targSegs[i]);
    INC(i)
  END;

  IF pos = 0 THEN
    out[0] := '.';
    IF HIGH(out) >= 1 THEN out[1] := 0C END
  ELSE
    IF pos <= HIGH(out) THEN
      out[pos] := 0C
    END
  END
END RelativeTo;

(* ── Join ──────────────────────────────────────────────── *)

PROCEDURE Join(a: ARRAY OF CHAR; b: ARRAY OF CHAR;
               VAR out: ARRAY OF CHAR);
VAR
  alen, pos: CARDINAL;
BEGIN
  IF (Length(b) > 0) AND (b[0] = '/') THEN
    Assign(b, out);
    RETURN
  END;

  alen := Length(a);

  IF alen = 0 THEN
    Assign(b, out);
    RETURN
  END;

  pos := 0;
  AppendStr(out, pos, a);

  IF (alen > 0) AND (a[alen - 1] # '/') THEN
    AppendChar(out, pos, '/')
  END;

  AppendStr(out, pos, b);

  IF pos <= HIGH(out) THEN
    out[pos] := 0C
  END
END Join;

(* ── Match ─────────────────────────────────────────────── *)

PROCEDURE DoMatch(VAR text: ARRAY OF CHAR; ti: CARDINAL;
                  VAR pat: ARRAY OF CHAR; pi: CARDINAL): BOOLEAN;
VAR
  tlen, plen: CARDINAL;
BEGIN
  tlen := StrLen(text);
  plen := StrLen(pat);

  LOOP
    IF pi >= plen THEN
      RETURN ti >= tlen
    END;

    IF pat[pi] = '*' THEN
      INC(pi);
      LOOP
        IF DoMatch(text, ti, pat, pi) THEN
          RETURN TRUE
        END;
        IF (ti >= tlen) OR (text[ti] = '/') THEN
          RETURN FALSE
        END;
        INC(ti)
      END
    ELSIF pat[pi] = '?' THEN
      IF (ti >= tlen) OR (text[ti] = '/') THEN
        RETURN FALSE
      END;
      INC(ti);
      INC(pi)
    ELSE
      IF (ti >= tlen) OR (text[ti] # pat[pi]) THEN
        RETURN FALSE
      END;
      INC(ti);
      INC(pi)
    END
  END
END DoMatch;

PROCEDURE Match(path: ARRAY OF CHAR; pattern: ARRAY OF CHAR): BOOLEAN;
VAR
  baseBuf: ARRAY [0..MaxSeg-1] OF CHAR;
  dirBuf: ARRAY [0..1023] OF CHAR;
BEGIN
  Split(path, dirBuf, baseBuf);
  RETURN DoMatch(baseBuf, 0, pattern, 0)
END Match;

END Path.
