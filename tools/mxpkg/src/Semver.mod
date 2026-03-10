IMPLEMENTATION MODULE Semver;

FROM Strings IMPORT Length;

(* Internal: convert a substring of digits starting at pos to integer.
   Updates pos to first non-digit. Returns -1 if no digits found. *)
PROCEDURE ReadInt(s: ARRAY OF CHAR; slen: INTEGER; VAR pos: INTEGER): INTEGER;
VAR n: INTEGER; started: INTEGER;
BEGIN
  n := 0;
  started := 0;
  WHILE (pos < slen) AND (s[pos] >= '0') AND (s[pos] <= '9') DO
    n := n * 10 + (ORD(s[pos]) - ORD('0'));
    INC(pos);
    started := 1
  END;
  IF started = 0 THEN RETURN -1 END;
  RETURN n
END ReadInt;

PROCEDURE Parse(s: ARRAY OF CHAR; VAR v: Version): INTEGER;
VAR pos, slen: INTEGER;
BEGIN
  slen := Length(s);
  pos := 0;
  v.major := ReadInt(s, slen, pos);
  IF v.major < 0 THEN RETURN -1 END;
  IF (pos >= slen) OR (s[pos] # '.') THEN RETURN -1 END;
  INC(pos);
  v.minor := ReadInt(s, slen, pos);
  IF v.minor < 0 THEN RETURN -1 END;
  IF (pos >= slen) OR (s[pos] # '.') THEN RETURN -1 END;
  INC(pos);
  v.patch := ReadInt(s, slen, pos);
  IF v.patch < 0 THEN RETURN -1 END;
  IF pos # slen THEN RETURN -1 END;
  RETURN 0
END Parse;

PROCEDURE Compare(a, b: Version): INTEGER;
BEGIN
  IF a.major < b.major THEN RETURN -1
  ELSIF a.major > b.major THEN RETURN 1
  ELSIF a.minor < b.minor THEN RETURN -1
  ELSIF a.minor > b.minor THEN RETURN 1
  ELSIF a.patch < b.patch THEN RETURN -1
  ELSIF a.patch > b.patch THEN RETURN 1
  ELSE RETURN 0
  END
END Compare;

PROCEDURE IsValid(s: ARRAY OF CHAR): INTEGER;
VAR v: Version;
BEGIN
  IF Parse(s, v) = 0 THEN RETURN 1 ELSE RETURN 0 END
END IsValid;

PROCEDURE MatchesRange(v: Version; rangeSpec: ARRAY OF CHAR): INTEGER;
VAR
  rlen, pos: INTEGER;
  rv: Version;
  ch: CHAR;
  spec: ARRAY [0..63] OF CHAR;
  si, ri: INTEGER;
BEGIN
  rlen := Length(rangeSpec);
  IF rlen = 0 THEN RETURN 0 END;

  ch := rangeSpec[0];

  IF ch = '^' THEN
    (* Caret: >=given, <next major (or next minor if major=0) *)
    pos := 1;
    si := 0;
    WHILE pos < rlen DO
      spec[si] := rangeSpec[pos];
      INC(si); INC(pos)
    END;
    spec[si] := 0C;
    IF Parse(spec, rv) # 0 THEN RETURN 0 END;
    IF Compare(v, rv) < 0 THEN RETURN 0 END;
    IF rv.major = 0 THEN
      (* ^0.x.y means >=0.x.y, <0.(x+1).0 *)
      IF v.major # 0 THEN RETURN 0 END;
      IF v.minor >= rv.minor + 1 THEN RETURN 0 END
    ELSE
      IF v.major >= rv.major + 1 THEN RETURN 0 END
    END;
    RETURN 1

  ELSIF ch = '~' THEN
    (* Tilde: >=given, <next minor *)
    pos := 1;
    si := 0;
    WHILE pos < rlen DO
      spec[si] := rangeSpec[pos];
      INC(si); INC(pos)
    END;
    spec[si] := 0C;
    IF Parse(spec, rv) # 0 THEN RETURN 0 END;
    IF Compare(v, rv) < 0 THEN RETURN 0 END;
    IF v.major # rv.major THEN RETURN 0 END;
    IF v.minor >= rv.minor + 1 THEN RETURN 0 END;
    RETURN 1

  ELSIF (ch = '>') AND (rlen > 1) AND (rangeSpec[1] = '=') THEN
    (* >=version *)
    pos := 2;
    si := 0;
    WHILE pos < rlen DO
      spec[si] := rangeSpec[pos];
      INC(si); INC(pos)
    END;
    spec[si] := 0C;
    IF Parse(spec, rv) # 0 THEN RETURN 0 END;
    IF Compare(v, rv) >= 0 THEN RETURN 1 ELSE RETURN 0 END

  ELSE
    (* Exact match *)
    IF Parse(rangeSpec, rv) # 0 THEN RETURN 0 END;
    IF Compare(v, rv) = 0 THEN RETURN 1 ELSE RETURN 0 END
  END
END MatchesRange;

PROCEDURE IntToStr(n: INTEGER; VAR buf: ARRAY OF CHAR; VAR pos: INTEGER);
VAR tmp: ARRAY [0..15] OF CHAR;
    ti, i: INTEGER;
BEGIN
  IF n = 0 THEN
    buf[pos] := '0'; INC(pos);
    RETURN
  END;
  ti := 0;
  WHILE n > 0 DO
    tmp[ti] := CHR(ORD('0') + (n MOD 10));
    n := n DIV 10;
    INC(ti)
  END;
  (* Reverse *)
  i := ti - 1;
  WHILE i >= 0 DO
    buf[pos] := tmp[i];
    INC(pos);
    DEC(i)
  END
END IntToStr;

PROCEDURE ToString(v: Version; VAR buf: ARRAY OF CHAR);
VAR pos: INTEGER;
BEGIN
  pos := 0;
  IntToStr(v.major, buf, pos);
  buf[pos] := '.'; INC(pos);
  IntToStr(v.minor, buf, pos);
  buf[pos] := '.'; INC(pos);
  IntToStr(v.patch, buf, pos);
  buf[pos] := 0C
END ToString;

END Semver.
