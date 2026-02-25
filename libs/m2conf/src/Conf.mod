IMPLEMENTATION MODULE Conf;

FROM Strings IMPORT Assign, Length, CompareStr;

CONST
  MaxSections   = 16;
  MaxKeysPerSec = 32;
  MaxKeyLen     = 64;
  MaxValLen     = 256;
  MaxSecNameLen = 64;

VAR
  secNames:    ARRAY [0..MaxSections-1] OF ARRAY [0..MaxSecNameLen-1] OF CHAR;
  secKeyCount: ARRAY [0..MaxSections-1] OF INTEGER;
  keys:        ARRAY [0..MaxSections-1] OF ARRAY [0..MaxKeysPerSec-1] OF ARRAY [0..MaxKeyLen-1] OF CHAR;
  vals:        ARRAY [0..MaxSections-1] OF ARRAY [0..MaxKeysPerSec-1] OF ARRAY [0..MaxValLen-1] OF CHAR;
  numSections: INTEGER;

(* ── Internal helpers ───────────────────────────────── *)

PROCEDURE IsWhitespace(ch: CHAR): BOOLEAN;
BEGIN
  RETURN (ch = ' ') OR (ch = CHR(9))
END IsWhitespace;

PROCEDURE TrimCopy(src: ARRAY OF CHAR; start, end0: INTEGER;
                   VAR dst: ARRAY OF CHAR);
(* Copy src[start..end0-1] into dst, trimming leading and trailing
   spaces and tabs. *)
VAR
  lo, hi, p, dstHigh: INTEGER;
BEGIN
  lo := start;
  hi := end0 - 1;
  WHILE (lo <= hi) AND IsWhitespace(src[lo]) DO INC(lo) END;
  WHILE (hi >= lo) AND IsWhitespace(src[hi]) DO DEC(hi) END;
  p := 0;
  dstHigh := HIGH(dst);
  WHILE (lo <= hi) AND (p < dstHigh) DO
    dst[p] := src[lo];
    INC(p);
    INC(lo)
  END;
  dst[p] := 0C
END TrimCopy;

PROCEDURE FindSection(name: ARRAY OF CHAR): INTEGER;
VAR j: INTEGER;
BEGIN
  FOR j := 0 TO numSections - 1 DO
    IF CompareStr(name, secNames[j]) = 0 THEN
      RETURN j
    END
  END;
  RETURN -1
END FindSection;

PROCEDURE FindOrAddSection(name: ARRAY OF CHAR): INTEGER;
VAR idx: INTEGER;
BEGIN
  idx := FindSection(name);
  IF idx >= 0 THEN RETURN idx END;
  IF numSections >= MaxSections THEN RETURN -1 END;
  idx := numSections;
  Assign(name, secNames[idx]);
  secKeyCount[idx] := 0;
  INC(numSections);
  RETURN idx
END FindOrAddSection;

PROCEDURE AddKeyValue(si: INTEGER; k, v: ARRAY OF CHAR);
VAR ki: INTEGER;
BEGIN
  IF si < 0 THEN RETURN END;
  IF secKeyCount[si] >= MaxKeysPerSec THEN RETURN END;
  ki := secKeyCount[si];
  Assign(k, keys[si][ki]);
  Assign(v, vals[si][ki]);
  INC(secKeyCount[si])
END AddKeyValue;

(* ── Public procedures ──────────────────────────────── *)

PROCEDURE Parse(buf: ARRAY OF CHAR; len: CARDINAL): BOOLEAN;
VAR
  pos:      INTEGER;
  lineStart: INTEGER;
  lineEnd:  INTEGER;
  curSec:   INTEGER;
  ch:       CHAR;
  i, eqPos: INTEGER;
  nameLen:  INTEGER;
  tmp:      ARRAY [0..MaxSecNameLen-1] OF CHAR;
  kBuf:     ARRAY [0..MaxKeyLen-1] OF CHAR;
  vBuf:     ARRAY [0..MaxValLen-1] OF CHAR;
  first:    INTEGER;
  bufLen:   INTEGER;
BEGIN
  Clear;
  bufLen := VAL(INTEGER, len);
  IF bufLen = 0 THEN RETURN TRUE END;

  (* auto-create default empty-name section at index 0 *)
  curSec := FindOrAddSection("");

  pos := 0;
  WHILE pos < bufLen DO
    (* find start and end of current line *)
    lineStart := pos;
    WHILE (pos < bufLen) AND (buf[pos] # CHR(10)) AND (buf[pos] # CHR(13)) DO
      INC(pos)
    END;
    lineEnd := pos;

    (* skip CR/LF *)
    IF (pos < bufLen) AND (buf[pos] = CHR(13)) THEN INC(pos) END;
    IF (pos < bufLen) AND (buf[pos] = CHR(10)) THEN INC(pos) END;

    (* skip leading whitespace to find first meaningful char *)
    first := lineStart;
    WHILE (first < lineEnd) AND IsWhitespace(buf[first]) DO INC(first) END;

    IF first >= lineEnd THEN
      (* blank line, skip *)
    ELSIF buf[first] = '#' THEN
      (* comment, skip *)
    ELSIF buf[first] = '[' THEN
      (* section header *)
      i := first + 1;
      WHILE (i < lineEnd) AND (buf[i] # ']') DO INC(i) END;
      IF i < lineEnd THEN
        TrimCopy(buf, first + 1, i, tmp);
        curSec := FindOrAddSection(tmp)
      END
    ELSE
      (* key=value line *)
      eqPos := first;
      WHILE (eqPos < lineEnd) AND (buf[eqPos] # '=') DO INC(eqPos) END;
      IF eqPos < lineEnd THEN
        TrimCopy(buf, first, eqPos, kBuf);
        TrimCopy(buf, eqPos + 1, lineEnd, vBuf);
        AddKeyValue(curSec, kBuf, vBuf)
      END
      (* lines without '=' are silently ignored *)
    END
  END;
  RETURN TRUE
END Parse;

PROCEDURE Clear;
BEGIN
  numSections := 0
END Clear;

PROCEDURE SectionCount(): INTEGER;
BEGIN
  RETURN numSections
END SectionCount;

PROCEDURE GetSectionName(i: INTEGER; VAR name: ARRAY OF CHAR): BOOLEAN;
BEGIN
  IF (i < 0) OR (i >= numSections) THEN
    name[0] := 0C;
    RETURN FALSE
  END;
  Assign(secNames[i], name);
  RETURN TRUE
END GetSectionName;

PROCEDURE KeyCount(section: ARRAY OF CHAR): INTEGER;
VAR idx: INTEGER;
BEGIN
  idx := FindSection(section);
  IF idx < 0 THEN RETURN -1 END;
  RETURN secKeyCount[idx]
END KeyCount;

PROCEDURE GetKey(section: ARRAY OF CHAR; i: INTEGER;
                 VAR key: ARRAY OF CHAR): BOOLEAN;
VAR si: INTEGER;
BEGIN
  si := FindSection(section);
  IF si < 0 THEN
    key[0] := 0C;
    RETURN FALSE
  END;
  IF (i < 0) OR (i >= secKeyCount[si]) THEN
    key[0] := 0C;
    RETURN FALSE
  END;
  Assign(keys[si][i], key);
  RETURN TRUE
END GetKey;

PROCEDURE GetValue(section: ARRAY OF CHAR; key: ARRAY OF CHAR;
                   VAR value: ARRAY OF CHAR): BOOLEAN;
VAR si, j: INTEGER;
BEGIN
  si := FindSection(section);
  IF si < 0 THEN
    value[0] := 0C;
    RETURN FALSE
  END;
  FOR j := 0 TO secKeyCount[si] - 1 DO
    IF CompareStr(key, keys[si][j]) = 0 THEN
      Assign(vals[si][j], value);
      RETURN TRUE
    END
  END;
  value[0] := 0C;
  RETURN FALSE
END GetValue;

PROCEDURE HasKey(section: ARRAY OF CHAR; key: ARRAY OF CHAR): BOOLEAN;
VAR si, j: INTEGER;
BEGIN
  si := FindSection(section);
  IF si < 0 THEN RETURN FALSE END;
  FOR j := 0 TO secKeyCount[si] - 1 DO
    IF CompareStr(key, keys[si][j]) = 0 THEN
      RETURN TRUE
    END
  END;
  RETURN FALSE
END HasKey;

BEGIN
  numSections := 0
END Conf.
