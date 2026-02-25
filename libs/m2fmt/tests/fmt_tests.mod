MODULE FmtTests;
(* Deterministic test suite for m2fmt.

   Tests:
     1.  json.empty_object       {} from JsonStart/JsonEnd
     2.  json.string_escape      "key":"a\"b"
     3.  json.int                {"n":42}
     4.  json.bool               true/false values
     5.  json.null               null value
     6.  json.multi_key_comma    {"a":1,"b":2}
     7.  json.array              [1,2,3]
     8.  json.nested             {"a":[1,2]}
     9.  csv.plain               plain field
    10.  csv.comma               field with comma
    11.  csv.quote               field with quote
    12.  csv.row                 field,sep,field,newline
    13.  table.basic             2 cols, 1 row
    14.  table.column_widths     padding verification
    15.  json.empty_array        []
    16.  buf.clear               BufClear resets length *)

FROM SYSTEM IMPORT ADDRESS, ADR, TSIZE;
FROM InOut IMPORT WriteString, WriteLn, WriteInt;
FROM Fmt IMPORT Buf, InitBuf, BufLen, BufClear,
                JsonStart, JsonEnd, JsonArrayStart, JsonArrayEnd,
                JsonKey, JsonStr, JsonInt, JsonBool, JsonNull,
                CsvField, CsvSep, CsvNewline,
                TableSetColumns, TableSetHeader, TableAddRow,
                TableSetCell, TableRender;

TYPE
  ByteArray = ARRAY [0..65535] OF CHAR;
  BytePtr = POINTER TO ByteArray;

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

PROCEDURE BufEq(VAR b: Buf; expected: ARRAY OF CHAR): BOOLEAN;
VAR bp: BytePtr;
    i: CARDINAL;
    blen, elen: CARDINAL;
BEGIN
  blen := BufLen(b);
  (* compute expected length *)
  elen := 0;
  WHILE (elen <= HIGH(expected)) AND (expected[elen] # 0C) DO
    INC(elen)
  END;
  IF blen # elen THEN
    RETURN FALSE
  END;
  bp := b.data;
  i := 0;
  WHILE i < blen DO
    IF bp^[i] # expected[i] THEN
      RETURN FALSE
    END;
    INC(i)
  END;
  RETURN TRUE
END BufEq;

PROCEDURE BufContains(VAR b: Buf; needle: ARRAY OF CHAR): BOOLEAN;
VAR bp: BytePtr;
    blen, nlen: CARDINAL;
    i, j: CARDINAL;
    found: BOOLEAN;
BEGIN
  blen := BufLen(b);
  nlen := 0;
  WHILE (nlen <= HIGH(needle)) AND (needle[nlen] # 0C) DO
    INC(nlen)
  END;
  IF nlen = 0 THEN RETURN TRUE END;
  IF nlen > blen THEN RETURN FALSE END;
  bp := b.data;
  i := 0;
  WHILE i <= blen - nlen DO
    found := TRUE;
    j := 0;
    WHILE j < nlen DO
      IF bp^[i + j] # needle[j] THEN
        found := FALSE;
        j := nlen (* break *)
      ELSE
        INC(j)
      END
    END;
    IF found THEN RETURN TRUE END;
    INC(i)
  END;
  RETURN FALSE
END BufContains;

(* ── Test 1: JSON empty object ────────────────────────── *)

PROCEDURE TestJsonEmptyObject;
VAR backing: ARRAY [0..4095] OF CHAR;
    b: Buf;
BEGIN
  InitBuf(b, ADR(backing), 4096);
  JsonStart(b);
  JsonEnd(b);
  Check("json.empty_object", BufEq(b, "{}"))
END TestJsonEmptyObject;

(* ── Test 2: JSON string escaping ─────────────────────── *)

PROCEDURE TestJsonStringEscape;
VAR backing: ARRAY [0..4095] OF CHAR;
    b: Buf;
BEGIN
  InitBuf(b, ADR(backing), 4096);
  JsonStart(b);
  JsonKey(b, "k");
  JsonStr(b, 'a"b');
  JsonEnd(b);
  Check("json.string_escape", BufEq(b, '{"k":"a\"b"}'))
END TestJsonStringEscape;

(* ── Test 3: JSON integer ─────────────────────────────── *)

PROCEDURE TestJsonInt;
VAR backing: ARRAY [0..4095] OF CHAR;
    b: Buf;
BEGIN
  InitBuf(b, ADR(backing), 4096);
  JsonStart(b);
  JsonKey(b, "n");
  JsonInt(b, 42);
  JsonEnd(b);
  Check("json.int", BufEq(b, '{"n":42}'))
END TestJsonInt;

(* ── Test 4: JSON booleans ────────────────────────────── *)

PROCEDURE TestJsonBool;
VAR backing: ARRAY [0..4095] OF CHAR;
    b: Buf;
BEGIN
  InitBuf(b, ADR(backing), 4096);
  JsonStart(b);
  JsonKey(b, "t");
  JsonBool(b, TRUE);
  JsonKey(b, "f");
  JsonBool(b, FALSE);
  JsonEnd(b);
  Check("json.bool", BufEq(b, '{"t":true,"f":false}'))
END TestJsonBool;

(* ── Test 5: JSON null ────────────────────────────────── *)

PROCEDURE TestJsonNull;
VAR backing: ARRAY [0..4095] OF CHAR;
    b: Buf;
BEGIN
  InitBuf(b, ADR(backing), 4096);
  JsonStart(b);
  JsonKey(b, "x");
  JsonNull(b);
  JsonEnd(b);
  Check("json.null", BufEq(b, '{"x":null}'))
END TestJsonNull;

(* ── Test 6: JSON multi-key comma ─────────────────────── *)

PROCEDURE TestJsonMultiKeyComma;
VAR backing: ARRAY [0..4095] OF CHAR;
    b: Buf;
BEGIN
  InitBuf(b, ADR(backing), 4096);
  JsonStart(b);
  JsonKey(b, "a");
  JsonInt(b, 1);
  JsonKey(b, "b");
  JsonInt(b, 2);
  JsonEnd(b);
  Check("json.multi_key_comma", BufEq(b, '{"a":1,"b":2}'))
END TestJsonMultiKeyComma;

(* ── Test 7: JSON array ───────────────────────────────── *)

PROCEDURE TestJsonArray;
VAR backing: ARRAY [0..4095] OF CHAR;
    b: Buf;
BEGIN
  InitBuf(b, ADR(backing), 4096);
  JsonArrayStart(b);
  JsonInt(b, 1);
  JsonInt(b, 2);
  JsonInt(b, 3);
  JsonArrayEnd(b);
  Check("json.array", BufEq(b, "[1,2,3]"))
END TestJsonArray;

(* ── Test 8: JSON nested ──────────────────────────────── *)

PROCEDURE TestJsonNested;
VAR backing: ARRAY [0..4095] OF CHAR;
    b: Buf;
BEGIN
  InitBuf(b, ADR(backing), 4096);
  JsonStart(b);
  JsonKey(b, "a");
  JsonArrayStart(b);
  JsonInt(b, 1);
  JsonInt(b, 2);
  JsonArrayEnd(b);
  JsonEnd(b);
  Check("json.nested", BufEq(b, '{"a":[1,2]}'))
END TestJsonNested;

(* ── Test 9: CSV plain field ──────────────────────────── *)

PROCEDURE TestCsvPlain;
VAR backing: ARRAY [0..4095] OF CHAR;
    b: Buf;
BEGIN
  InitBuf(b, ADR(backing), 4096);
  CsvField(b, "hello");
  Check("csv.plain", BufEq(b, "hello"))
END TestCsvPlain;

(* ── Test 10: CSV field with comma ────────────────────── *)

PROCEDURE TestCsvComma;
VAR backing: ARRAY [0..4095] OF CHAR;
    b: Buf;
BEGIN
  InitBuf(b, ADR(backing), 4096);
  CsvField(b, "a,b");
  Check("csv.comma", BufEq(b, '"a,b"'))
END TestCsvComma;

(* ── Test 11: CSV field with quote ────────────────────── *)

PROCEDURE TestCsvQuote;
VAR backing: ARRAY [0..4095] OF CHAR;
    b: Buf;
BEGIN
  InitBuf(b, ADR(backing), 4096);
  CsvField(b, 'a"b');
  Check("csv.quote", BufEq(b, '"a""b"'))
END TestCsvQuote;

(* ── Test 12: CSV row ─────────────────────────────────── *)

PROCEDURE TestCsvRow;
VAR backing: ARRAY [0..4095] OF CHAR;
    b: Buf;
    expected: ARRAY [0..63] OF CHAR;
BEGIN
  InitBuf(b, ADR(backing), 4096);
  CsvField(b, "name");
  CsvSep(b);
  CsvField(b, "age");
  CsvNewline(b);
  (* expected: name,age\r\n *)
  expected[0] := 'n'; expected[1] := 'a'; expected[2] := 'm'; expected[3] := 'e';
  expected[4] := ',';
  expected[5] := 'a'; expected[6] := 'g'; expected[7] := 'e';
  expected[8] := CHR(13); expected[9] := CHR(10);
  expected[10] := 0C;
  Check("csv.row", BufEq(b, expected))
END TestCsvRow;

(* ── Test 13: Table basic ─────────────────────────────── *)

PROCEDURE TestTableBasic;
VAR backing: ARRAY [0..4095] OF CHAR;
    b: Buf;
    r: INTEGER;
BEGIN
  InitBuf(b, ADR(backing), 4096);
  TableSetColumns(2);
  TableSetHeader(0, "Name");
  TableSetHeader(1, "Age");
  r := TableAddRow();
  TableSetCell(r, 0, "Alice");
  TableSetCell(r, 1, "30");
  TableRender(b);
  Check("table.basic: has Name", BufContains(b, "Name"));
  Check("table.basic: has Age", BufContains(b, "Age"));
  Check("table.basic: has Alice", BufContains(b, "Alice"));
  Check("table.basic: has 30", BufContains(b, "30"));
  Check("table.basic: has separator", BufContains(b, "-----"))
END TestTableBasic;

(* ── Test 14: Table column widths ─────────────────────── *)

PROCEDURE TestTableColumnWidths;
VAR backing: ARRAY [0..4095] OF CHAR;
    b: Buf;
    r: INTEGER;
BEGIN
  InitBuf(b, ADR(backing), 4096);
  TableSetColumns(2);
  TableSetHeader(0, "X");
  TableSetHeader(1, "Y");
  r := TableAddRow();
  TableSetCell(r, 0, "Hello");
  TableSetCell(r, 1, "W");
  TableRender(b);
  (* "X" header should be padded to width 5 ("Hello" is longest in col 0) *)
  (* Header row: "X     Y" — X followed by spaces then separator then Y *)
  Check("table.widths: has X padded", BufContains(b, "X    "));
  Check("table.widths: has Hello", BufContains(b, "Hello"))
END TestTableColumnWidths;

(* ── Test 15: JSON empty array ────────────────────────── *)

PROCEDURE TestJsonEmptyArray;
VAR backing: ARRAY [0..4095] OF CHAR;
    b: Buf;
BEGIN
  InitBuf(b, ADR(backing), 4096);
  JsonArrayStart(b);
  JsonArrayEnd(b);
  Check("json.empty_array", BufEq(b, "[]"))
END TestJsonEmptyArray;

(* ── Test 16: BufClear resets length ──────────────────── *)

PROCEDURE TestBufClear;
VAR backing: ARRAY [0..4095] OF CHAR;
    b: Buf;
BEGIN
  InitBuf(b, ADR(backing), 4096);
  JsonStart(b);
  JsonEnd(b);
  Check("buf.clear: len>0", BufLen(b) > 0);
  BufClear(b);
  Check("buf.clear: len=0", BufLen(b) = 0)
END TestBufClear;

BEGIN
  passed := 0;
  failed := 0;
  total := 0;

  TestJsonEmptyObject;
  TestJsonStringEscape;
  TestJsonInt;
  TestJsonBool;
  TestJsonNull;
  TestJsonMultiKeyComma;
  TestJsonArray;
  TestJsonNested;
  TestCsvPlain;
  TestCsvComma;
  TestCsvQuote;
  TestCsvRow;
  TestTableBasic;
  TestTableColumnWidths;
  TestJsonEmptyArray;
  TestBufClear;

  WriteLn;
  WriteString("m2fmt: ");
  WriteInt(passed, 0); WriteString(" passed, ");
  WriteInt(failed, 0); WriteString(" failed, ");
  WriteInt(total, 0); WriteString(" total"); WriteLn;
  IF failed = 0 THEN
    WriteString("ALL TESTS PASSED"); WriteLn
  END
END FmtTests.
