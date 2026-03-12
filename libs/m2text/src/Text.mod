IMPLEMENTATION MODULE Text;

FROM SYSTEM IMPORT ADDRESS, ADR, LONGCARD, TSIZE;
FROM Strings IMPORT Assign, Length;

TYPE
  CharPtr = POINTER TO CHAR;

(* ── Byte/char read helpers via POINTER TO CHAR ── *)

PROCEDURE PtrAt(base: ADDRESS; idx: CARDINAL): CharPtr;
BEGIN
  RETURN CharPtr(LONGCARD(base) + LONGCARD(idx))
END PtrAt;

PROCEDURE Byte(base: ADDRESS; i: CARDINAL): CARDINAL;
VAR p: CharPtr;
BEGIN
  p := PtrAt(base, i);
  RETURN ORD(p^) MOD 256
END Byte;

PROCEDURE GetCh(base: ADDRESS; i: CARDINAL): CHAR;
VAR p: CharPtr;
BEGIN
  p := PtrAt(base, i);
  RETURN p^
END GetCh;

(* ── Encoding validation ──────────────────────────── *)

PROCEDURE IsValidUTF8(buf: ADDRESS; len: CARDINAL): BOOLEAN;
VAR
  i: CARDINAL;
  b, b1, b2, b3: CARDINAL;
BEGIN
  IF len = 0 THEN RETURN TRUE END;
  i := 0;
  WHILE i < len DO
    b := Byte(buf, i);
    IF b <= 07FH THEN
      (* Single byte ASCII *)
      INC(i)
    ELSIF (b >= 0C2H) AND (b <= 0DFH) THEN
      (* 2-byte sequence: reject 0xC0, 0xC1 (overlong) by range *)
      IF i + 1 >= len THEN RETURN FALSE END;
      b1 := Byte(buf, i + 1);
      IF (b1 < 080H) OR (b1 > 0BFH) THEN RETURN FALSE END;
      INC(i, 2)
    ELSIF (b >= 0E0H) AND (b <= 0EFH) THEN
      (* 3-byte sequence *)
      IF i + 2 >= len THEN RETURN FALSE END;
      b1 := Byte(buf, i + 1);
      b2 := Byte(buf, i + 2);
      (* Check continuation bytes *)
      IF (b1 < 080H) OR (b1 > 0BFH) THEN RETURN FALSE END;
      IF (b2 < 080H) OR (b2 > 0BFH) THEN RETURN FALSE END;
      (* Overlong check: E0 requires b1 >= A0 *)
      IF (b = 0E0H) AND (b1 < 0A0H) THEN RETURN FALSE END;
      (* Surrogate check: ED requires b1 <= 9F *)
      IF (b = 0EDH) AND (b1 > 09FH) THEN RETURN FALSE END;
      INC(i, 3)
    ELSIF (b >= 0F0H) AND (b <= 0F4H) THEN
      (* 4-byte sequence *)
      IF i + 3 >= len THEN RETURN FALSE END;
      b1 := Byte(buf, i + 1);
      b2 := Byte(buf, i + 2);
      b3 := Byte(buf, i + 3);
      (* Check continuation bytes *)
      IF (b1 < 080H) OR (b1 > 0BFH) THEN RETURN FALSE END;
      IF (b2 < 080H) OR (b2 > 0BFH) THEN RETURN FALSE END;
      IF (b3 < 080H) OR (b3 > 0BFH) THEN RETURN FALSE END;
      (* Overlong check: F0 requires b1 >= 90 *)
      IF (b = 0F0H) AND (b1 < 090H) THEN RETURN FALSE END;
      (* > U+10FFFF check: F4 requires b1 <= 8F *)
      IF (b = 0F4H) AND (b1 > 08FH) THEN RETURN FALSE END;
      INC(i, 4)
    ELSE
      (* 0x80-0xBF (lone continuation), 0xC0-0xC1 (overlong),
         0xF5+ (beyond Unicode): all invalid *)
      RETURN FALSE
    END
  END;
  RETURN TRUE
END IsValidUTF8;

PROCEDURE IsASCII(buf: ADDRESS; len: CARDINAL): BOOLEAN;
VAR
  i: CARDINAL;
BEGIN
  IF len = 0 THEN RETURN TRUE END;
  i := 0;
  WHILE i < len DO
    IF Byte(buf, i) >= 128 THEN RETURN FALSE END;
    INC(i)
  END;
  RETURN TRUE
END IsASCII;

(* ── Text / binary heuristic ──────────────────────── *)

PROCEDURE IsText(buf: ADDRESS; len: CARDINAL): BOOLEAN;
VAR
  i, scanLen: CARDINAL;
  b: CARDINAL;
  controlCount: CARDINAL;
BEGIN
  IF len = 0 THEN RETURN TRUE END;
  scanLen := len;
  IF scanLen > 8192 THEN scanLen := 8192 END;
  controlCount := 0;
  i := 0;
  WHILE i < scanLen DO
    b := Byte(buf, i);
    (* NUL byte: definitely binary *)
    IF b = 0 THEN RETURN FALSE END;
    (* Control chars: 0x01-0x08, 0x0E-0x1F *)
    IF ((b >= 1) AND (b <= 8)) OR ((b >= 14) AND (b <= 31)) THEN
      INC(controlCount)
    END;
    INC(i)
  END;
  (* Control ratio >= 5% means binary *)
  IF controlCount * 100 >= scanLen * 5 THEN RETURN FALSE END;
  RETURN TRUE
END IsText;

PROCEDURE IsBinary(buf: ADDRESS; len: CARDINAL): BOOLEAN;
BEGIN
  RETURN NOT IsText(buf, len)
END IsBinary;

(* ── BOM detection ─────────────────────────────────── *)

PROCEDURE HasBOM(buf: ADDRESS; len: CARDINAL): INTEGER;
BEGIN
  IF len < 3 THEN RETURN 0 END;
  IF (Byte(buf, 0) = 0EFH) AND
     (Byte(buf, 1) = 0BBH) AND
     (Byte(buf, 2) = 0BFH) THEN
    RETURN 3
  END;
  RETURN 0
END HasBOM;

(* ── Line counting ─────────────────────────────────── *)

PROCEDURE CountLines(buf: ADDRESS; len: CARDINAL): INTEGER;
VAR
  i: CARDINAL;
  count: INTEGER;
BEGIN
  IF len = 0 THEN RETURN 0 END;
  count := 0;
  i := 0;
  WHILE i < len DO
    IF Byte(buf, i) = 0AH THEN
      INC(count)
    END;
    INC(i)
  END;
  RETURN count + 1
END CountLines;

(* ── Shebang parsing ──────────────────────────────── *)

PROCEDURE ParseShebang(buf: ADDRESS; len: CARDINAL;
                       VAR interp: ARRAY OF CHAR);
VAR
  lineEnd: CARDINAL;
  i, start, nameLen: CARDINAL;
  hasEnv: BOOLEAN;
  envPos: CARDINAL;
  maxLen: CARDINAL;
BEGIN
  interp[0] := 0C;
  IF len < 2 THEN RETURN END;
  IF (GetCh(buf, 0) # '#') OR (GetCh(buf, 1) # '!') THEN RETURN END;

  (* Find end of first line *)
  lineEnd := len;
  i := 2;
  WHILE i < len DO
    IF (Byte(buf, i) = 0AH) OR (Byte(buf, i) = 0DH) THEN
      lineEnd := i;
      i := len (* break *)
    END;
    INC(i)
  END;

  (* Skip whitespace after #! *)
  i := 2;
  WHILE (i < lineEnd) AND ((GetCh(buf, i) = ' ') OR (GetCh(buf, i) = CHR(9))) DO
    INC(i)
  END;
  IF i >= lineEnd THEN RETURN END;

  (* Check for /env followed by space or tab *)
  hasEnv := FALSE;
  envPos := i;
  IF lineEnd - i >= 4 THEN
    start := i;
    WHILE start + 4 <= lineEnd DO
      IF (GetCh(buf, start) = '/') AND (GetCh(buf, start + 1) = 'e') AND
         (GetCh(buf, start + 2) = 'n') AND (GetCh(buf, start + 3) = 'v') AND
         (start + 4 < lineEnd) AND
         ((GetCh(buf, start + 4) = ' ') OR (GetCh(buf, start + 4) = CHR(9))) THEN
        hasEnv := TRUE;
        envPos := start + 5;
        start := lineEnd (* break *)
      END;
      INC(start)
    END
  END;

  IF hasEnv THEN
    WHILE (envPos < lineEnd) AND
          ((GetCh(buf, envPos) = ' ') OR (GetCh(buf, envPos) = CHR(9))) DO
      INC(envPos)
    END;
    start := envPos;
    WHILE (envPos < lineEnd) AND
          (GetCh(buf, envPos) # ' ') AND (GetCh(buf, envPos) # CHR(9)) DO
      INC(envPos)
    END;
    nameLen := envPos - start;
    maxLen := HIGH(interp);
    IF nameLen > maxLen THEN nameLen := maxLen END;
    i := 0;
    WHILE i < nameLen DO
      interp[i] := GetCh(buf, start + i);
      INC(i)
    END;
    IF i <= maxLen THEN
      interp[i] := 0C
    END
  ELSE
    start := i;
    envPos := i;
    WHILE (envPos < lineEnd) AND
          (GetCh(buf, envPos) # ' ') AND (GetCh(buf, envPos) # CHR(9)) DO
      INC(envPos)
    END;
    i := envPos;
    WHILE i > start DO
      DEC(i);
      IF GetCh(buf, i) = '/' THEN
        start := i + 1;
        i := start (* break *)
      END
    END;
    nameLen := envPos - start;
    maxLen := HIGH(interp);
    IF nameLen > maxLen THEN nameLen := maxLen END;
    i := 0;
    WHILE i < nameLen DO
      interp[i] := GetCh(buf, start + i);
      INC(i)
    END;
    IF i <= maxLen THEN
      interp[i] := 0C
    END
  END
END ParseShebang;

(* ── Line ending detection ─────────────────────────── *)

PROCEDURE DetectLineEnding(buf: ADDRESS; len: CARDINAL): INTEGER;
VAR
  i: CARDINAL;
  b: CARDINAL;
  crlfCount, lfCount, crCount: CARDINAL;
  types: CARDINAL;
BEGIN
  IF len = 0 THEN RETURN LineEndNone END;
  crlfCount := 0;
  lfCount := 0;
  crCount := 0;
  i := 0;
  WHILE i < len DO
    b := Byte(buf, i);
    IF b = 0DH THEN
      (* CR: check if followed by LF *)
      IF (i + 1 < len) AND (Byte(buf, i + 1) = 0AH) THEN
        INC(crlfCount);
        INC(i, 2)
      ELSE
        INC(crCount);
        INC(i)
      END
    ELSIF b = 0AH THEN
      INC(lfCount);
      INC(i)
    ELSE
      INC(i)
    END
  END;

  (* No line endings at all *)
  IF (crlfCount = 0) AND (lfCount = 0) AND (crCount = 0) THEN
    RETURN LineEndNone
  END;

  (* Count how many distinct types we saw *)
  types := 0;
  IF crlfCount > 0 THEN INC(types) END;
  IF lfCount > 0 THEN INC(types) END;
  IF crCount > 0 THEN INC(types) END;

  IF types > 1 THEN RETURN LineEndMixed END;

  IF crlfCount > 0 THEN RETURN LineEndCRLF END;
  IF lfCount > 0 THEN RETURN LineEndLF END;
  RETURN LineEndCR
END DetectLineEnding;

END Text.
