IMPLEMENTATION MODULE Fmt;
(* JSON/CSV/table output formatting.
   All output to caller-provided Buf.  No heap allocation. *)

FROM SYSTEM IMPORT ADDRESS, ADR, LONGCARD, TSIZE;
FROM Strings IMPORT Assign, Length, Concat;

TYPE
  CharPtr = POINTER TO CHAR;

(* ── Buffer helpers (private) ─────────────────────────── *)

PROCEDURE PutCh(base: ADDRESS; idx: CARDINAL; ch: CHAR);
VAR p: CharPtr;
BEGIN
  p := CharPtr(LONGCARD(base) + LONGCARD(idx));
  p^ := ch
END PutCh;

PROCEDURE BufAppendChar(VAR b: Buf; ch: CHAR);
BEGIN
  IF b.pos < b.cap THEN
    PutCh(b.data, b.pos, ch);
    INC(b.pos)
  END
END BufAppendChar;

PROCEDURE BufAppendStr(VAR b: Buf; s: ARRAY OF CHAR);
VAR i: CARDINAL;
BEGIN
  i := 0;
  WHILE (i <= HIGH(s)) AND (s[i] # 0C) DO
    BufAppendChar(b, s[i]);
    INC(i)
  END
END BufAppendStr;

PROCEDURE BufNullTerm(VAR b: Buf);
BEGIN
  IF b.pos < b.cap THEN
    PutCh(b.data, b.pos, 0C)
  END
END BufNullTerm;

(* ── Buffer public operations ─────────────────────────── *)

PROCEDURE InitBuf(VAR b: Buf; data: ADDRESS; cap: CARDINAL);
BEGIN
  b.data := data;
  b.cap := cap;
  b.pos := 0
END InitBuf;

PROCEDURE BufLen(VAR b: Buf): CARDINAL;
BEGIN
  RETURN b.pos
END BufLen;

PROCEDURE BufClear(VAR b: Buf);
BEGIN
  b.pos := 0
END BufClear;

(* ── JSON nesting/comma state ─────────────────────────── *)

CONST
  MaxNest = 16;

VAR
  nestStack: ARRAY [0..MaxNest-1] OF BOOLEAN;
  nestTop: INTEGER;

PROCEDURE NestPush(needComma: BOOLEAN);
BEGIN
  IF nestTop < MaxNest - 1 THEN
    INC(nestTop);
    nestStack[nestTop] := needComma
  END
END NestPush;

PROCEDURE NestPop;
BEGIN
  IF nestTop >= 0 THEN
    DEC(nestTop)
  END
END NestPop;

PROCEDURE WriteCommaIfNeeded(VAR b: Buf);
BEGIN
  IF (nestTop >= 0) AND nestStack[nestTop] THEN
    BufAppendChar(b, ',')
  END
END WriteCommaIfNeeded;

PROCEDURE SetNeedComma(val: BOOLEAN);
BEGIN
  IF nestTop >= 0 THEN
    nestStack[nestTop] := val
  END
END SetNeedComma;

(* ── JSON mini-writer ─────────────────────────────────── *)

PROCEDURE JsonStart(VAR b: Buf);
BEGIN
  WriteCommaIfNeeded(b);
  BufAppendChar(b, '{');
  NestPush(FALSE)
END JsonStart;

PROCEDURE JsonEnd(VAR b: Buf);
BEGIN
  NestPop;
  BufAppendChar(b, '}');
  SetNeedComma(TRUE);
  BufNullTerm(b)
END JsonEnd;

PROCEDURE JsonArrayStart(VAR b: Buf);
BEGIN
  WriteCommaIfNeeded(b);
  BufAppendChar(b, '[');
  NestPush(FALSE)
END JsonArrayStart;

PROCEDURE JsonArrayEnd(VAR b: Buf);
BEGIN
  NestPop;
  BufAppendChar(b, ']');
  SetNeedComma(TRUE);
  BufNullTerm(b)
END JsonArrayEnd;

PROCEDURE JsonKey(VAR b: Buf; key: ARRAY OF CHAR);
BEGIN
  WriteCommaIfNeeded(b);
  BufAppendChar(b, '"');
  BufAppendStr(b, key);
  BufAppendChar(b, '"');
  BufAppendChar(b, ':');
  SetNeedComma(FALSE)
END JsonKey;

PROCEDURE WriteJsonEscapedStr(VAR b: Buf; s: ARRAY OF CHAR);
VAR i: CARDINAL;
    ch: CHAR;
BEGIN
  BufAppendChar(b, '"');
  i := 0;
  WHILE (i <= HIGH(s)) AND (s[i] # 0C) DO
    ch := s[i];
    IF ch = '"' THEN
      BufAppendChar(b, '\');
      BufAppendChar(b, '"')
    ELSIF ch = '\' THEN
      BufAppendChar(b, '\');
      BufAppendChar(b, '\')
    ELSIF ch = CHR(10) THEN
      BufAppendChar(b, '\');
      BufAppendChar(b, 'n')
    ELSIF ch = CHR(9) THEN
      BufAppendChar(b, '\');
      BufAppendChar(b, 't')
    ELSE
      BufAppendChar(b, ch)
    END;
    INC(i)
  END;
  BufAppendChar(b, '"')
END WriteJsonEscapedStr;

PROCEDURE JsonStr(VAR b: Buf; val: ARRAY OF CHAR);
BEGIN
  WriteCommaIfNeeded(b);
  WriteJsonEscapedStr(b, val);
  SetNeedComma(TRUE);
  BufNullTerm(b)
END JsonStr;

PROCEDURE WriteIntToBuf(VAR b: Buf; val: INTEGER);
VAR digits: ARRAY [0..19] OF CHAR;
    n, i, j: INTEGER;
    neg: BOOLEAN;
BEGIN
  IF val = 0 THEN
    BufAppendChar(b, '0');
    RETURN
  END;
  neg := val < 0;
  IF neg THEN
    n := -val
  ELSE
    n := val
  END;
  i := 0;
  WHILE n > 0 DO
    digits[i] := CHR(ORD('0') + (n MOD 10));
    n := n DIV 10;
    INC(i)
  END;
  IF neg THEN
    BufAppendChar(b, '-')
  END;
  j := i - 1;
  WHILE j >= 0 DO
    BufAppendChar(b, digits[j]);
    DEC(j)
  END
END WriteIntToBuf;

PROCEDURE JsonInt(VAR b: Buf; val: INTEGER);
BEGIN
  WriteCommaIfNeeded(b);
  WriteIntToBuf(b, val);
  SetNeedComma(TRUE);
  BufNullTerm(b)
END JsonInt;

PROCEDURE JsonBool(VAR b: Buf; val: BOOLEAN);
BEGIN
  WriteCommaIfNeeded(b);
  IF val THEN
    BufAppendStr(b, "true")
  ELSE
    BufAppendStr(b, "false")
  END;
  SetNeedComma(TRUE);
  BufNullTerm(b)
END JsonBool;

PROCEDURE JsonNull(VAR b: Buf);
BEGIN
  WriteCommaIfNeeded(b);
  BufAppendStr(b, "null");
  SetNeedComma(TRUE);
  BufNullTerm(b)
END JsonNull;

(* ── CSV encoder ──────────────────────────────────────── *)

PROCEDURE CsvNeedsQuoting(VAR s: ARRAY OF CHAR): BOOLEAN;
VAR i: CARDINAL;
    ch: CHAR;
BEGIN
  i := 0;
  WHILE (i <= HIGH(s)) AND (s[i] # 0C) DO
    ch := s[i];
    IF (ch = ',') OR (ch = '"') OR (ch = CHR(10)) OR (ch = CHR(13)) THEN
      RETURN TRUE
    END;
    INC(i)
  END;
  RETURN FALSE
END CsvNeedsQuoting;

PROCEDURE CsvField(VAR b: Buf; val: ARRAY OF CHAR);
VAR i: CARDINAL;
    ch: CHAR;
BEGIN
  IF CsvNeedsQuoting(val) THEN
    BufAppendChar(b, '"');
    i := 0;
    WHILE (i <= HIGH(val)) AND (val[i] # 0C) DO
      ch := val[i];
      IF ch = '"' THEN
        BufAppendChar(b, '"');
        BufAppendChar(b, '"')
      ELSE
        BufAppendChar(b, ch)
      END;
      INC(i)
    END;
    BufAppendChar(b, '"')
  ELSE
    BufAppendStr(b, val)
  END;
  BufNullTerm(b)
END CsvField;

PROCEDURE CsvSep(VAR b: Buf);
BEGIN
  BufAppendChar(b, ',');
  BufNullTerm(b)
END CsvSep;

PROCEDURE CsvNewline(VAR b: Buf);
BEGIN
  BufAppendChar(b, CHR(13));
  BufAppendChar(b, CHR(10));
  BufNullTerm(b)
END CsvNewline;

(* ── Text table renderer ─────────────────────────────── *)

CONST
  MaxCols = 16;
  MaxRows = 64;
  MaxCellLen = 128;

VAR
  tblCols: INTEGER;
  tblRows: INTEGER;
  tblHeaders: ARRAY [0..MaxCols-1] OF ARRAY [0..MaxCellLen-1] OF CHAR;
  tblCells: ARRAY [0..MaxRows-1] OF ARRAY [0..MaxCols-1] OF ARRAY [0..MaxCellLen-1] OF CHAR;

PROCEDURE TableSetColumns(n: INTEGER);
VAR i, j: INTEGER;
BEGIN
  IF n > MaxCols THEN
    tblCols := MaxCols
  ELSE
    tblCols := n
  END;
  tblRows := 0;
  (* Clear headers *)
  i := 0;
  WHILE i < MaxCols DO
    tblHeaders[i][0] := 0C;
    INC(i)
  END;
  (* Clear cells *)
  i := 0;
  WHILE i < MaxRows DO
    j := 0;
    WHILE j < MaxCols DO
      tblCells[i][j][0] := 0C;
      INC(j)
    END;
    INC(i)
  END
END TableSetColumns;

PROCEDURE TableSetHeader(col: INTEGER; name: ARRAY OF CHAR);
VAR i: CARDINAL;
BEGIN
  IF (col >= 0) AND (col < tblCols) THEN
    i := 0;
    WHILE (i <= HIGH(name)) AND (name[i] # 0C) AND (i < CARDINAL(MaxCellLen - 1)) DO
      tblHeaders[col][i] := name[i];
      INC(i)
    END;
    tblHeaders[col][i] := 0C
  END
END TableSetHeader;

PROCEDURE TableAddRow(): INTEGER;
BEGIN
  IF tblRows < MaxRows THEN
    INC(tblRows);
    RETURN tblRows - 1
  ELSE
    RETURN -1
  END
END TableAddRow;

PROCEDURE TableSetCell(row: INTEGER; col: INTEGER; value: ARRAY OF CHAR);
VAR i: CARDINAL;
BEGIN
  IF (row >= 0) AND (row < tblRows) AND (col >= 0) AND (col < tblCols) THEN
    i := 0;
    WHILE (i <= HIGH(value)) AND (value[i] # 0C) AND (i < CARDINAL(MaxCellLen - 1)) DO
      tblCells[row][col][i] := value[i];
      INC(i)
    END;
    tblCells[row][col][i] := 0C
  END
END TableSetCell;

PROCEDURE StrLen(VAR s: ARRAY OF CHAR): CARDINAL;
VAR i: CARDINAL;
BEGIN
  i := 0;
  WHILE (i <= HIGH(s)) AND (s[i] # 0C) DO
    INC(i)
  END;
  RETURN i
END StrLen;

PROCEDURE BufAppendPadded(VAR b: Buf; VAR s: ARRAY OF CHAR; width: CARDINAL);
VAR slen, pad, i: CARDINAL;
BEGIN
  slen := StrLen(s);
  i := 0;
  WHILE (i < slen) DO
    BufAppendChar(b, s[i]);
    INC(i)
  END;
  IF slen < width THEN
    pad := width - slen;
    i := 0;
    WHILE i < pad DO
      BufAppendChar(b, ' ');
      INC(i)
    END
  END
END BufAppendPadded;

PROCEDURE TableRender(VAR b: Buf);
VAR colWidths: ARRAY [0..MaxCols-1] OF CARDINAL;
    c, r: INTEGER;
    w: CARDINAL;
    i: CARDINAL;
BEGIN
  (* Pass 1: compute column widths *)
  c := 0;
  WHILE c < tblCols DO
    colWidths[c] := StrLen(tblHeaders[c]);
    r := 0;
    WHILE r < tblRows DO
      w := StrLen(tblCells[r][c]);
      IF w > colWidths[c] THEN
        colWidths[c] := w
      END;
      INC(r)
    END;
    INC(c)
  END;

  (* Pass 2: render header row *)
  c := 0;
  WHILE c < tblCols DO
    IF c > 0 THEN
      BufAppendChar(b, ' ');
      BufAppendChar(b, ' ')
    END;
    BufAppendPadded(b, tblHeaders[c], colWidths[c]);
    INC(c)
  END;
  BufAppendChar(b, CHR(10));

  (* Separator row *)
  c := 0;
  WHILE c < tblCols DO
    IF c > 0 THEN
      BufAppendChar(b, ' ');
      BufAppendChar(b, ' ')
    END;
    i := 0;
    WHILE i < colWidths[c] DO
      BufAppendChar(b, '-');
      INC(i)
    END;
    INC(c)
  END;
  BufAppendChar(b, CHR(10));

  (* Data rows *)
  r := 0;
  WHILE r < tblRows DO
    c := 0;
    WHILE c < tblCols DO
      IF c > 0 THEN
        BufAppendChar(b, ' ');
        BufAppendChar(b, ' ')
      END;
      BufAppendPadded(b, tblCells[r][c], colWidths[c]);
      INC(c)
    END;
    BufAppendChar(b, CHR(10));
    INC(r)
  END;
  BufNullTerm(b)
END TableRender;

BEGIN
  nestTop := -1
END Fmt.
