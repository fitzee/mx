MODULE TextTests;
(* Deterministic test suite for m2text.

   Tests:
     1.  ascii.hello          "Hello" is ASCII
     2.  ascii.highbit         Buffer with byte >= 128 is not ASCII
     3.  utf8.ascii            Pure ASCII is valid UTF-8
     4.  utf8.2byte            Valid 2-byte (C3 A9 = e-acute)
     5.  utf8.3byte            Valid 3-byte (E2 9C 93 = checkmark)
     6.  utf8.4byte            Valid 4-byte (F0 9F 98 80 = grinning)
     7.  utf8.invalid_ff       0xFF byte is invalid UTF-8
     8.  utf8.overlong          Overlong C0 80 is invalid
     9.  utf8.truncated         Truncated C3 alone is invalid
    10.  text.hello             "Hello world" is text
    11.  text.nul               Buffer with NUL is not text
    12.  binary.nul             Buffer with NUL is binary
    13.  text.control           Many control chars is not text
    14.  bom.present            EF BB BF -> 3
    15.  bom.absent             "Hello" -> 0
    16.  lines.three            "a\nb\nc" -> 3
    17.  lines.empty            "" -> 0
    18.  lines.one              "a" -> 1
    19.  lines.trailing         "\n" -> 2
    20.  shebang.bash           "#!/bin/bash\n" -> "bash"
    21.  shebang.env            "#!/usr/bin/env python\n" -> "python"
    22.  shebang.none           "no shebang" -> ""
    23.  lineend.lf             "a\nb" -> LF
    24.  lineend.crlf           "a\r\nb" -> CRLF
    25.  lineend.cr             "a\rb" -> CR
    26.  lineend.none           "abc" -> None *)

FROM SYSTEM IMPORT ADDRESS, ADR, TSIZE;
FROM InOut IMPORT WriteString, WriteLn, WriteInt;
FROM Text IMPORT IsValidUTF8, IsASCII, IsText, IsBinary,
                 HasBOM, CountLines, ParseShebang,
                 DetectLineEnding,
                 LineEndNone, LineEndLF, LineEndCRLF,
                 LineEndCR, LineEndMixed;

VAR
  passed, failed, total: INTEGER;

PROCEDURE Check(name: ARRAY OF CHAR; cond: BOOLEAN);
BEGIN
  INC(total);
  IF cond THEN
    INC(passed)
  ELSE
    INC(failed);
    WriteString("FAIL: "); WriteString(name); WriteLn
  END
END Check;

(* ── Helper: string length for stack buffers ──────── *)

PROCEDURE SLen(VAR s: ARRAY OF CHAR): CARDINAL;
VAR i: CARDINAL;
BEGIN
  i := 0;
  WHILE (i <= HIGH(s)) AND (s[i] # 0C) DO INC(i) END;
  RETURN i
END SLen;

(* ── Test 1-2: IsASCII ────────────────────────────── *)

PROCEDURE TestIsASCII;
VAR buf: ARRAY [0..15] OF CHAR;
BEGIN
  buf[0] := 'H'; buf[1] := 'e'; buf[2] := 'l';
  buf[3] := 'l'; buf[4] := 'o'; buf[5] := 0C;
  Check("ascii.hello", IsASCII(ADR(buf), 5));

  buf[0] := CHR(0C3H); buf[1] := CHR(0A9H); buf[2] := 0C;
  Check("ascii.highbit", NOT IsASCII(ADR(buf), 2))
END TestIsASCII;

(* ── Test 3-9: IsValidUTF8 ────────────────────────── *)

PROCEDURE TestIsValidUTF8;
VAR buf: ARRAY [0..15] OF CHAR;
BEGIN
  (* Pure ASCII *)
  buf[0] := 'A'; buf[1] := 'B'; buf[2] := 'C'; buf[3] := 0C;
  Check("utf8.ascii", IsValidUTF8(ADR(buf), 3));

  (* Valid 2-byte: C3 A9 = U+00E9 e-acute *)
  buf[0] := CHR(0C3H); buf[1] := CHR(0A9H); buf[2] := 0C;
  Check("utf8.2byte", IsValidUTF8(ADR(buf), 2));

  (* Valid 3-byte: E2 9C 93 = U+2713 checkmark *)
  buf[0] := CHR(0E2H); buf[1] := CHR(09CH); buf[2] := CHR(093H);
  buf[3] := 0C;
  Check("utf8.3byte", IsValidUTF8(ADR(buf), 3));

  (* Valid 4-byte: F0 9F 98 80 = U+1F600 grinning face *)
  buf[0] := CHR(0F0H); buf[1] := CHR(09FH);
  buf[2] := CHR(098H); buf[3] := CHR(080H);
  buf[4] := 0C;
  Check("utf8.4byte", IsValidUTF8(ADR(buf), 4));

  (* Invalid: 0xFF byte *)
  buf[0] := CHR(0FFH); buf[1] := 0C;
  Check("utf8.invalid_ff", NOT IsValidUTF8(ADR(buf), 1));

  (* Invalid: overlong C0 80 *)
  buf[0] := CHR(0C0H); buf[1] := CHR(080H); buf[2] := 0C;
  Check("utf8.overlong", NOT IsValidUTF8(ADR(buf), 2));

  (* Invalid: truncated sequence - C3 alone *)
  buf[0] := CHR(0C3H); buf[1] := 0C;
  Check("utf8.truncated", NOT IsValidUTF8(ADR(buf), 1))
END TestIsValidUTF8;

(* ── Test 10-13: IsText / IsBinary ───────────────── *)

PROCEDURE TestIsTextBinary;
VAR buf: ARRAY [0..127] OF CHAR;
    i: CARDINAL;
BEGIN
  (* "Hello world" is text *)
  buf[0] := 'H'; buf[1] := 'e'; buf[2] := 'l'; buf[3] := 'l';
  buf[4] := 'o'; buf[5] := ' '; buf[6] := 'w'; buf[7] := 'o';
  buf[8] := 'r'; buf[9] := 'l'; buf[10] := 'd'; buf[11] := 0C;
  Check("text.hello", IsText(ADR(buf), 11));

  (* Buffer with NUL byte is not text *)
  buf[0] := 'A'; buf[1] := CHR(0); buf[2] := 'B'; buf[3] := 0C;
  Check("text.nul", NOT IsText(ADR(buf), 3));

  (* Buffer with NUL byte is binary *)
  Check("binary.nul", IsBinary(ADR(buf), 3));

  (* Buffer with many control chars is not text *)
  (* Fill 100 bytes: 10 control chars (10%) > 5% threshold *)
  i := 0;
  WHILE i < 100 DO
    buf[i] := 'A';
    INC(i)
  END;
  (* Set 10 bytes to control char 0x01 *)
  buf[0] := CHR(1); buf[10] := CHR(1); buf[20] := CHR(1);
  buf[30] := CHR(1); buf[40] := CHR(1); buf[50] := CHR(1);
  buf[60] := CHR(1); buf[70] := CHR(1); buf[80] := CHR(1);
  buf[90] := CHR(1);
  Check("text.control", NOT IsText(ADR(buf), 100))
END TestIsTextBinary;

(* ── Test 14-15: HasBOM ───────────────────────────── *)

PROCEDURE TestHasBOM;
VAR buf: ARRAY [0..15] OF CHAR;
BEGIN
  (* UTF-8 BOM present *)
  buf[0] := CHR(0EFH); buf[1] := CHR(0BBH); buf[2] := CHR(0BFH);
  buf[3] := 'H'; buf[4] := 'i'; buf[5] := 0C;
  Check("bom.present", HasBOM(ADR(buf), 5) = 3);

  (* No BOM *)
  buf[0] := 'H'; buf[1] := 'e'; buf[2] := 'l';
  buf[3] := 'l'; buf[4] := 'o'; buf[5] := 0C;
  Check("bom.absent", HasBOM(ADR(buf), 5) = 0)
END TestHasBOM;

(* ── Test 16-19: CountLines ───────────────────────── *)

PROCEDURE TestCountLines;
VAR buf: ARRAY [0..15] OF CHAR;
BEGIN
  (* "a\nb\nc" -> 3 lines *)
  buf[0] := 'a'; buf[1] := CHR(0AH); buf[2] := 'b';
  buf[3] := CHR(0AH); buf[4] := 'c'; buf[5] := 0C;
  Check("lines.three", CountLines(ADR(buf), 5) = 3);

  (* Empty buffer -> 0 *)
  Check("lines.empty", CountLines(ADR(buf), 0) = 0);

  (* "a" -> 1 *)
  buf[0] := 'a'; buf[1] := 0C;
  Check("lines.one", CountLines(ADR(buf), 1) = 1);

  (* "\n" -> 2 *)
  buf[0] := CHR(0AH); buf[1] := 0C;
  Check("lines.trailing", CountLines(ADR(buf), 1) = 2)
END TestCountLines;

(* ── Test 20-22: ParseShebang ─────────────────────── *)

PROCEDURE TestParseShebang;
VAR buf: ARRAY [0..63] OF CHAR;
    interp: ARRAY [0..63] OF CHAR;
BEGIN
  (* "#!/bin/bash\n" -> "bash" *)
  buf[0] := '#'; buf[1] := '!'; buf[2] := '/'; buf[3] := 'b';
  buf[4] := 'i'; buf[5] := 'n'; buf[6] := '/'; buf[7] := 'b';
  buf[8] := 'a'; buf[9] := 's'; buf[10] := 'h'; buf[11] := CHR(0AH);
  buf[12] := 0C;
  ParseShebang(ADR(buf), 12, interp);
  Check("shebang.bash",
        (interp[0] = 'b') AND (interp[1] = 'a') AND
        (interp[2] = 's') AND (interp[3] = 'h') AND
        (interp[4] = 0C));

  (* "#!/usr/bin/env python\n" -> "python" *)
  buf[0]  := '#'; buf[1]  := '!'; buf[2]  := '/'; buf[3]  := 'u';
  buf[4]  := 's'; buf[5]  := 'r'; buf[6]  := '/'; buf[7]  := 'b';
  buf[8]  := 'i'; buf[9]  := 'n'; buf[10] := '/'; buf[11] := 'e';
  buf[12] := 'n'; buf[13] := 'v'; buf[14] := ' '; buf[15] := 'p';
  buf[16] := 'y'; buf[17] := 't'; buf[18] := 'h'; buf[19] := 'o';
  buf[20] := 'n'; buf[21] := CHR(0AH); buf[22] := 0C;
  ParseShebang(ADR(buf), 22, interp);
  Check("shebang.env",
        (interp[0] = 'p') AND (interp[1] = 'y') AND
        (interp[2] = 't') AND (interp[3] = 'h') AND
        (interp[4] = 'o') AND (interp[5] = 'n') AND
        (interp[6] = 0C));

  (* "no shebang" -> "" *)
  buf[0] := 'n'; buf[1] := 'o'; buf[2] := ' '; buf[3] := 's';
  buf[4] := 'h'; buf[5] := 'e'; buf[6] := 'b'; buf[7] := 'a';
  buf[8] := 'n'; buf[9] := 'g'; buf[10] := 0C;
  ParseShebang(ADR(buf), 10, interp);
  Check("shebang.none", interp[0] = 0C)
END TestParseShebang;

(* ── Test 23-26: DetectLineEnding ─────────────────── *)

PROCEDURE TestDetectLineEnding;
VAR buf: ARRAY [0..15] OF CHAR;
BEGIN
  (* "a\nb" -> LF *)
  buf[0] := 'a'; buf[1] := CHR(0AH); buf[2] := 'b'; buf[3] := 0C;
  Check("lineend.lf", DetectLineEnding(ADR(buf), 3) = LineEndLF);

  (* "a\r\nb" -> CRLF *)
  buf[0] := 'a'; buf[1] := CHR(0DH); buf[2] := CHR(0AH);
  buf[3] := 'b'; buf[4] := 0C;
  Check("lineend.crlf", DetectLineEnding(ADR(buf), 4) = LineEndCRLF);

  (* "a\rb" -> CR *)
  buf[0] := 'a'; buf[1] := CHR(0DH); buf[2] := 'b'; buf[3] := 0C;
  Check("lineend.cr", DetectLineEnding(ADR(buf), 3) = LineEndCR);

  (* "abc" -> None *)
  buf[0] := 'a'; buf[1] := 'b'; buf[2] := 'c'; buf[3] := 0C;
  Check("lineend.none", DetectLineEnding(ADR(buf), 3) = LineEndNone)
END TestDetectLineEnding;

BEGIN
  passed := 0;
  failed := 0;
  total := 0;

  TestIsASCII;
  TestIsValidUTF8;
  TestIsTextBinary;
  TestHasBOM;
  TestCountLines;
  TestParseShebang;
  TestDetectLineEnding;

  WriteLn;
  WriteString("m2text: ");
  WriteInt(passed, 0); WriteString(" passed, ");
  WriteInt(failed, 0); WriteString(" failed, ");
  WriteInt(total, 0); WriteString(" total"); WriteLn;
  IF failed = 0 THEN
    WriteString("ALL TESTS PASSED"); WriteLn
  END
END TextTests.
