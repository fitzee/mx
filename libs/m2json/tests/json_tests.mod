MODULE JsonTests;
(* Test suite for m2json SAX-style JSON tokenizer.

   Tests:
     1.  empty            Empty input yields JEnd
     2.  null_true_false  Keyword tokens
     3.  integer          Integer number tokenisation
     4.  negative         Negative number
     5.  float            Decimal number
     6.  exponent         Number with exponent
     7.  simple_string    Plain string
     8.  escape_string    String with escape sequences
     9.  empty_string     Zero-length string
    10.  simple_array     Array with mixed values
    11.  simple_object    Object with key-value pairs
    12.  nested           Nested objects and arrays
    13.  get_integer      GetInteger extraction
    14.  get_real         GetReal extraction
    15.  get_string       GetString with escapes
    16.  skip_object      Skip over entire object
    17.  skip_array       Skip over entire array
    18.  skip_scalar      Skip over scalar value
    19.  error_bad_token  Invalid input triggers JError
    20.  error_unterm_str Unterminated string
    21.  whitespace       Various whitespace characters
    22.  unicode_escape   \uXXXX escape sequences
    23.  get_error        GetError copies message *)

FROM SYSTEM IMPORT ADDRESS, ADR;
FROM InOut IMPORT WriteString, WriteLn, WriteInt;
FROM Json IMPORT Parser, Token, TokenKind, Init, Next,
                 GetString, GetInteger, GetReal, Skip, GetError;

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

PROCEDURE StrEq(a, b: ARRAY OF CHAR): BOOLEAN;
VAR i: CARDINAL;
BEGIN
  i := 0;
  WHILE (i <= HIGH(a)) AND (i <= HIGH(b)) DO
    IF a[i] # b[i] THEN RETURN FALSE END;
    IF a[i] = 0C THEN RETURN TRUE END;
    INC(i)
  END;
  IF (i <= HIGH(a)) AND (a[i] # 0C) THEN RETURN FALSE END;
  IF (i <= HIGH(b)) AND (b[i] # 0C) THEN RETURN FALSE END;
  RETURN TRUE
END StrEq;

PROCEDURE Len(s: ARRAY OF CHAR): CARDINAL;
VAR i: CARDINAL;
BEGIN
  i := 0;
  WHILE (i <= HIGH(s)) AND (s[i] # 0C) DO INC(i) END;
  RETURN i
END Len;

(* ── Test 1: Empty input ─────────────────────────────── *)

PROCEDURE TestEmpty;
VAR p: Parser; t: Token;
    buf: ARRAY [0..0] OF CHAR;
BEGIN
  buf[0] := 0C;
  Init(p, ADR(buf), 0);
  Check("empty: returns FALSE", NOT Next(p, t));
  Check("empty: kind=JEnd", t.kind = JEnd)
END TestEmpty;

(* ── Test 2: null, true, false ───────────────────────── *)

PROCEDURE TestKeywords;
VAR p: Parser; t: Token;
    src: ARRAY [0..31] OF CHAR;
BEGIN
  src := "null true false";
  Init(p, ADR(src), Len(src));

  Check("kw: tok1", Next(p, t));
  Check("kw: null", t.kind = JNull);

  Check("kw: tok2", Next(p, t));
  Check("kw: true", t.kind = JTrue);

  Check("kw: tok3", Next(p, t));
  Check("kw: false", t.kind = JFalse);

  Check("kw: done", NOT Next(p, t))
END TestKeywords;

(* ── Test 3: Integer number ──────────────────────────── *)

PROCEDURE TestInteger;
VAR p: Parser; t: Token;
    src: ARRAY [0..15] OF CHAR;
BEGIN
  src := "42";
  Init(p, ADR(src), Len(src));
  Check("int: tok", Next(p, t));
  Check("int: kind=JNumber", t.kind = JNumber);
  Check("int: len=2", t.len = 2);
  Check("int: done", NOT Next(p, t))
END TestInteger;

(* ── Test 4: Negative number ─────────────────────────── *)

PROCEDURE TestNegative;
VAR p: Parser; t: Token;
    src: ARRAY [0..15] OF CHAR;
    val: INTEGER;
BEGIN
  src := "-17";
  Init(p, ADR(src), Len(src));
  Check("neg: tok", Next(p, t));
  Check("neg: kind=JNumber", t.kind = JNumber);
  Check("neg: getint", GetInteger(p, t, val));
  Check("neg: val=-17", val = -17)
END TestNegative;

(* ── Test 5: Float number ────────────────────────────── *)

PROCEDURE TestFloat;
VAR p: Parser; t: Token;
    src: ARRAY [0..15] OF CHAR;
    val: REAL;
BEGIN
  src := "3.14";
  Init(p, ADR(src), Len(src));
  Check("float: tok", Next(p, t));
  Check("float: kind=JNumber", t.kind = JNumber);
  Check("float: getreal", GetReal(p, t, val));
  (* check approximate equality *)
  Check("float: val~3.14", (val > 3.13) AND (val < 3.15))
END TestFloat;

(* ── Test 6: Exponent number ─────────────────────────── *)

PROCEDURE TestExponent;
VAR p: Parser; t: Token;
    src: ARRAY [0..15] OF CHAR;
    val: REAL;
BEGIN
  src := "1e3";
  Init(p, ADR(src), Len(src));
  Check("exp: tok", Next(p, t));
  Check("exp: kind=JNumber", t.kind = JNumber);
  Check("exp: getreal", GetReal(p, t, val));
  Check("exp: val~1000", (val > 999.0) AND (val < 1001.0))
END TestExponent;

(* ── Test 7: Simple string ───────────────────────────── *)

PROCEDURE TestSimpleString;
VAR p: Parser; t: Token;
    src: ARRAY [0..31] OF CHAR;
    buf: ARRAY [0..63] OF CHAR;
BEGIN
  src := '"hello"';
  Init(p, ADR(src), Len(src));
  Check("str: tok", Next(p, t));
  Check("str: kind=JString", t.kind = JString);
  Check("str: getstr", GetString(p, t, buf));
  Check("str: val=hello", StrEq(buf, "hello"))
END TestSimpleString;

(* ── Test 8: String with escapes ─────────────────────── *)

PROCEDURE TestEscapeString;
VAR p: Parser; t: Token;
    src: ARRAY [0..63] OF CHAR;
    buf: ARRAY [0..63] OF CHAR;
    i: CARDINAL;
BEGIN
  (* Build: "a\tb\nc\\d\"e\/f" *)
  i := 0;
  src[i] := '"'; INC(i);
  src[i] := 'a'; INC(i);
  src[i] := CHR(92); INC(i);  (* \ *)
  src[i] := 't'; INC(i);
  src[i] := 'b'; INC(i);
  src[i] := CHR(92); INC(i);  (* \ *)
  src[i] := 'n'; INC(i);
  src[i] := 'c'; INC(i);
  src[i] := CHR(92); INC(i);  (* \ *)
  src[i] := CHR(92); INC(i);  (* \ — escaped backslash *)
  src[i] := 'd'; INC(i);
  src[i] := CHR(92); INC(i);  (* \ *)
  src[i] := '"'; INC(i);      (* escaped quote *)
  src[i] := 'e'; INC(i);
  src[i] := CHR(92); INC(i);  (* \ *)
  src[i] := '/'; INC(i);      (* escaped slash *)
  src[i] := 'f'; INC(i);
  src[i] := '"'; INC(i);
  src[i] := 0C;

  Init(p, ADR(src), i - 1);
  Check("esc: tok", Next(p, t));
  Check("esc: kind=JString", t.kind = JString);
  Check("esc: getstr", GetString(p, t, buf));

  (* Expected: a<TAB>b<LF>c\d"e/f *)
  Check("esc: [0]=a", buf[0] = 'a');
  Check("esc: [1]=TAB", buf[1] = CHR(9));
  Check("esc: [2]=b", buf[2] = 'b');
  Check("esc: [3]=LF", buf[3] = CHR(10));
  Check("esc: [4]=c", buf[4] = 'c');
  Check("esc: [5]=backslash", buf[5] = CHR(92));
  Check("esc: [6]=d", buf[6] = 'd');
  Check("esc: [7]=quote", buf[7] = '"');
  Check("esc: [8]=e", buf[8] = 'e');
  Check("esc: [9]=slash", buf[9] = '/');
  Check("esc: [10]=f", buf[10] = 'f');
  Check("esc: [11]=NUL", buf[11] = 0C)
END TestEscapeString;

(* ── Test 9: Empty string ────────────────────────────── *)

PROCEDURE TestEmptyString;
VAR p: Parser; t: Token;
    src: ARRAY [0..7] OF CHAR;
    buf: ARRAY [0..31] OF CHAR;
BEGIN
  src := '""';
  Init(p, ADR(src), 2);
  Check("estr: tok", Next(p, t));
  Check("estr: kind=JString", t.kind = JString);
  Check("estr: len=0", t.len = 0);
  Check("estr: getstr", GetString(p, t, buf));
  Check("estr: empty", buf[0] = 0C)
END TestEmptyString;

(* ── Test 10: Simple array ───────────────────────────── *)

PROCEDURE TestSimpleArray;
VAR p: Parser; t: Token;
    src: ARRAY [0..31] OF CHAR;
BEGIN
  src := "[1, 2, 3]";
  Init(p, ADR(src), Len(src));

  Check("arr: [", Next(p, t));
  Check("arr: ArrayStart", t.kind = JArrayStart);

  Check("arr: 1", Next(p, t));
  Check("arr: Number", t.kind = JNumber);

  Check("arr: ,1", Next(p, t));
  Check("arr: Comma1", t.kind = JComma);

  Check("arr: 2", Next(p, t));
  Check("arr: Number2", t.kind = JNumber);

  Check("arr: ,2", Next(p, t));
  Check("arr: Comma2", t.kind = JComma);

  Check("arr: 3", Next(p, t));
  Check("arr: Number3", t.kind = JNumber);

  Check("arr: ]", Next(p, t));
  Check("arr: ArrayEnd", t.kind = JArrayEnd);

  Check("arr: done", NOT Next(p, t))
END TestSimpleArray;

(* ── Test 11: Simple object ──────────────────────────── *)

PROCEDURE TestSimpleObject;
VAR p: Parser; t: Token;
    src: ARRAY [0..63] OF CHAR;
    buf: ARRAY [0..31] OF CHAR;
    val: INTEGER;
BEGIN
  src := '{"x": 10, "y": 20}';
  Init(p, ADR(src), Len(src));

  Check("obj: {", Next(p, t));
  Check("obj: ObjectStart", t.kind = JObjectStart);

  Check("obj: key-x", Next(p, t));
  Check("obj: key-x str", t.kind = JString);
  Check("obj: getstr x", GetString(p, t, buf));
  Check("obj: x=x", StrEq(buf, "x"));

  Check("obj: colon1", Next(p, t));
  Check("obj: Colon1", t.kind = JColon);

  Check("obj: val10", Next(p, t));
  Check("obj: val10 num", t.kind = JNumber);
  Check("obj: getint 10", GetInteger(p, t, val));
  Check("obj: val=10", val = 10);

  Check("obj: comma", Next(p, t));
  Check("obj: Comma", t.kind = JComma);

  Check("obj: key-y", Next(p, t));
  Check("obj: key-y str", t.kind = JString);
  Check("obj: getstr y", GetString(p, t, buf));
  Check("obj: y=y", StrEq(buf, "y"));

  Check("obj: colon2", Next(p, t));
  Check("obj: Colon2", t.kind = JColon);

  Check("obj: val20", Next(p, t));
  Check("obj: val20 num", t.kind = JNumber);
  Check("obj: getint 20", GetInteger(p, t, val));
  Check("obj: val=20", val = 20);

  Check("obj: }", Next(p, t));
  Check("obj: ObjectEnd", t.kind = JObjectEnd);

  Check("obj: done", NOT Next(p, t))
END TestSimpleObject;

(* ── Test 12: Nested structures ──────────────────────── *)

PROCEDURE TestNested;
VAR p: Parser; t: Token;
    src: ARRAY [0..63] OF CHAR;
    count: CARDINAL;
BEGIN
  src := '{"a":[1,{"b":2}]}';
  Init(p, ADR(src), Len(src));

  count := 0;
  WHILE Next(p, t) DO INC(count) END;
  (* { "a" : [ 1 , { "b" : 2 } ] } = 13 tokens *)
  Check("nest: 13 tokens", count = 13)
END TestNested;

(* ── Test 13: GetInteger ─────────────────────────────── *)

PROCEDURE TestGetInteger;
VAR p: Parser; t: Token;
    src: ARRAY [0..15] OF CHAR;
    val: INTEGER;
    ok: BOOLEAN;
BEGIN
  src := "12345";
  Init(p, ADR(src), Len(src));
  Check("geti: tok", Next(p, t));
  ok := GetInteger(p, t, val);
  Check("geti: ok", ok);
  Check("geti: val=12345", val = 12345);

  (* float should fail GetInteger *)
  src := "3.14";
  Init(p, ADR(src), Len(src));
  Check("geti.f: tok", Next(p, t));
  ok := GetInteger(p, t, val);
  Check("geti.f: fails", NOT ok)
END TestGetInteger;

(* ── Test 14: GetReal ────────────────────────────────── *)

PROCEDURE TestGetReal;
VAR p: Parser; t: Token;
    src: ARRAY [0..31] OF CHAR;
    val: REAL;
BEGIN
  src := "2.5e2";
  Init(p, ADR(src), Len(src));
  Check("getr: tok", Next(p, t));
  Check("getr: ok", GetReal(p, t, val));
  Check("getr: val~250", (val > 249.0) AND (val < 251.0));

  (* negative exponent *)
  src := "5e-1";
  Init(p, ADR(src), Len(src));
  Check("getr.ne: tok", Next(p, t));
  Check("getr.ne: ok", GetReal(p, t, val));
  Check("getr.ne: val~0.5", (val > 0.49) AND (val < 0.51))
END TestGetReal;

(* ── Test 15: GetString with escapes ─────────────────── *)

PROCEDURE TestGetString;
VAR p: Parser; t: Token;
    src: ARRAY [0..31] OF CHAR;
    buf: ARRAY [0..63] OF CHAR;
    i: CARDINAL;
BEGIN
  (* Build: "ab\tcd" *)
  i := 0;
  src[i] := '"'; INC(i);
  src[i] := 'a'; INC(i);
  src[i] := 'b'; INC(i);
  src[i] := CHR(92); INC(i);
  src[i] := 't'; INC(i);
  src[i] := 'c'; INC(i);
  src[i] := 'd'; INC(i);
  src[i] := '"'; INC(i);
  src[i] := 0C;

  Init(p, ADR(src), i - 1);
  Check("gets: tok", Next(p, t));
  Check("gets: ok", GetString(p, t, buf));
  Check("gets: [0]=a", buf[0] = 'a');
  Check("gets: [1]=b", buf[1] = 'b');
  Check("gets: [2]=TAB", buf[2] = CHR(9));
  Check("gets: [3]=c", buf[3] = 'c');
  Check("gets: [4]=d", buf[4] = 'd');
  Check("gets: [5]=NUL", buf[5] = 0C)
END TestGetString;

(* ── Test 16: Skip object ────────────────────────────── *)

PROCEDURE TestSkipObject;
VAR p: Parser; t: Token;
    src: ARRAY [0..63] OF CHAR;
BEGIN
  src := '{"a":{"b":1}} 42';
  Init(p, ADR(src), Len(src));

  (* Skip the entire outer object *)
  Skip(p);

  (* Next token should be 42 *)
  Check("skipo: tok", Next(p, t));
  Check("skipo: kind=JNumber", t.kind = JNumber);
  Check("skipo: done", NOT Next(p, t))
END TestSkipObject;

(* ── Test 17: Skip array ─────────────────────────────── *)

PROCEDURE TestSkipArray;
VAR p: Parser; t: Token;
    src: ARRAY [0..63] OF CHAR;
BEGIN
  src := '[1, [2, 3], 4] true';
  Init(p, ADR(src), Len(src));

  Skip(p);

  Check("skipa: tok", Next(p, t));
  Check("skipa: kind=JTrue", t.kind = JTrue);
  Check("skipa: done", NOT Next(p, t))
END TestSkipArray;

(* ── Test 18: Skip scalar ────────────────────────────── *)

PROCEDURE TestSkipScalar;
VAR p: Parser; t: Token;
    src: ARRAY [0..31] OF CHAR;
BEGIN
  src := "123 456";
  Init(p, ADR(src), Len(src));

  Skip(p);

  Check("skips: tok", Next(p, t));
  Check("skips: kind=JNumber", t.kind = JNumber);
  Check("skips: start correct", t.start = 4)
END TestSkipScalar;

(* ── Test 19: Error bad token ────────────────────────── *)

PROCEDURE TestErrorBadToken;
VAR p: Parser; t: Token;
    src: ARRAY [0..7] OF CHAR;
BEGIN
  src := "@";
  Init(p, ADR(src), 1);
  Check("errbad: fails", NOT Next(p, t));
  Check("errbad: kind=JError", t.kind = JError);
  Check("errbad: hasError", p.hasError)
END TestErrorBadToken;

(* ── Test 20: Unterminated string ────────────────────── *)

PROCEDURE TestErrorUntermStr;
VAR p: Parser; t: Token;
    src: ARRAY [0..15] OF CHAR;
BEGIN
  src[0] := '"';
  src[1] := 'a';
  src[2] := 'b';
  src[3] := 0C;
  Init(p, ADR(src), 3);
  Check("errunterm: fails", NOT Next(p, t));
  Check("errunterm: kind=JError", t.kind = JError)
END TestErrorUntermStr;

(* ── Test 21: Whitespace handling ────────────────────── *)

PROCEDURE TestWhitespace;
VAR p: Parser; t: Token;
    src: ARRAY [0..31] OF CHAR;
    i: CARDINAL;
BEGIN
  (* Build: <SP><TAB><LF><CR>42 *)
  i := 0;
  src[i] := ' '; INC(i);
  src[i] := CHR(9); INC(i);
  src[i] := CHR(10); INC(i);
  src[i] := CHR(13); INC(i);
  src[i] := '4'; INC(i);
  src[i] := '2'; INC(i);
  src[i] := 0C;

  Init(p, ADR(src), 6);
  Check("ws: tok", Next(p, t));
  Check("ws: kind=JNumber", t.kind = JNumber);
  Check("ws: start=4", t.start = 4);
  Check("ws: done", NOT Next(p, t))
END TestWhitespace;

(* ── Test 22: Unicode escape ─────────────────────────── *)

PROCEDURE TestUnicodeEscape;
VAR p: Parser; t: Token;
    src: ARRAY [0..31] OF CHAR;
    buf: ARRAY [0..31] OF CHAR;
    i: CARDINAL;
BEGIN
  (* Build: "\u0041\u0042" which is "AB" *)
  i := 0;
  src[i] := '"'; INC(i);
  src[i] := CHR(92); INC(i);
  src[i] := 'u'; INC(i);
  src[i] := '0'; INC(i);
  src[i] := '0'; INC(i);
  src[i] := '4'; INC(i);
  src[i] := '1'; INC(i);
  src[i] := CHR(92); INC(i);
  src[i] := 'u'; INC(i);
  src[i] := '0'; INC(i);
  src[i] := '0'; INC(i);
  src[i] := '4'; INC(i);
  src[i] := '2'; INC(i);
  src[i] := '"'; INC(i);
  src[i] := 0C;

  Init(p, ADR(src), i - 1);
  Check("uni: tok", Next(p, t));
  Check("uni: kind=JString", t.kind = JString);
  Check("uni: getstr", GetString(p, t, buf));
  Check("uni: [0]=A", buf[0] = 'A');
  Check("uni: [1]=B", buf[1] = 'B');
  Check("uni: [2]=NUL", buf[2] = 0C)
END TestUnicodeEscape;

(* ── Test 23: GetError ───────────────────────────────── *)

PROCEDURE TestGetError;
VAR p: Parser; t: Token;
    src: ARRAY [0..7] OF CHAR;
    ebuf: ARRAY [0..127] OF CHAR;
BEGIN
  (* No error initially *)
  src := "42";
  Init(p, ADR(src), 2);
  GetError(p, ebuf);
  Check("geterr: no error", ebuf[0] = 0C);

  (* Trigger an error *)
  src := "@";
  Init(p, ADR(src), 1);
  Next(p, t);
  GetError(p, ebuf);
  Check("geterr: has msg", ebuf[0] # 0C)
END TestGetError;

BEGIN
  passed := 0;
  failed := 0;
  total := 0;

  TestEmpty;
  TestKeywords;
  TestInteger;
  TestNegative;
  TestFloat;
  TestExponent;
  TestSimpleString;
  TestEscapeString;
  TestEmptyString;
  TestSimpleArray;
  TestSimpleObject;
  TestNested;
  TestGetInteger;
  TestGetReal;
  TestGetString;
  TestSkipObject;
  TestSkipArray;
  TestSkipScalar;
  TestErrorBadToken;
  TestErrorUntermStr;
  TestWhitespace;
  TestUnicodeEscape;
  TestGetError;

  WriteLn;
  WriteString("m2json: ");
  WriteInt(passed, 0); WriteString(" passed, ");
  WriteInt(failed, 0); WriteString(" failed, ");
  WriteInt(total, 0); WriteString(" total"); WriteLn;
  IF failed = 0 THEN
    WriteString("ALL TESTS PASSED"); WriteLn
  END
END JsonTests.
