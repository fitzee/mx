MODULE TokTests;
(* Test suite for m2tok Tokenizer.

   Tests:
     1.  empty          Empty input yields no tokens
     2.  ident          Single identifier
     3.  multi_ident    Multiple identifiers
     4.  operators      Punctuation yields Operator tokens
     5.  mixed          Identifiers and operators interleaved
     6.  skip_dquote    Double-quoted strings stripped
     7.  skip_squote    Single-quoted strings stripped
     8.  skip_escape    Backslash escapes inside strings
     9.  skip_line_sl   // line comments stripped
    10.  skip_line_hash # line comments stripped
    11.  skip_block     Block comments stripped
    12.  skip_nested    Nested block comments stripped
    13.  shebang        Shebang line yields Shebang token
    14.  digits         Digit runs yield Ident tokens
    15.  underscore     Underscore in identifiers
    16.  copy_token     CopyToken extracts correct text
    17.  copy_truncate  CopyToken truncates long tokens
    18.  real_code      Realistic code snippet *)

FROM SYSTEM IMPORT ADDRESS, ADR;
FROM InOut IMPORT WriteString, WriteLn, WriteInt, WriteCard;
FROM Tokenizer IMPORT TokenKind, Token, State, Init, Next, CopyToken;
FROM Strings IMPORT Length;

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

(* Helper: count tokens in a string *)
PROCEDURE CountTokens(src: ARRAY OF CHAR): CARDINAL;
VAR s: State; t: Token; n: CARDINAL;
BEGIN
  Init(s, ADR(src), Length(src));
  n := 0;
  WHILE Next(s, t) DO INC(n) END;
  RETURN n
END CountTokens;

(* ── Test 1: Empty ─────────────────────────────────── *)

PROCEDURE TestEmpty;
VAR s: State; t: Token;
    buf: ARRAY [0..0] OF CHAR;
BEGIN
  buf[0] := 0C;
  Init(s, ADR(buf), 0);
  Check("empty: no tokens", NOT Next(s, t))
END TestEmpty;

(* ── Test 2: Single ident ──────────────────────────── *)

PROCEDURE TestIdent;
VAR s: State; t: Token;
    src: ARRAY [0..15] OF CHAR;
    word: ARRAY [0..31] OF CHAR;
BEGIN
  src := "hello";
  Init(s, ADR(src), Length(src));
  Check("ident: has token", Next(s, t));
  Check("ident: kind=Ident", t.kind = Ident);
  CopyToken(s, t, word);
  Check("ident: text=hello", StrEq(word, "hello"));
  Check("ident: no more", NOT Next(s, t))
END TestIdent;

(* ── Test 3: Multiple idents ──────────────────────── *)

PROCEDURE TestMultiIdent;
VAR s: State; t: Token;
    src: ARRAY [0..31] OF CHAR;
    word: ARRAY [0..31] OF CHAR;
BEGIN
  src := "foo bar baz";
  Init(s, ADR(src), Length(src));
  Check("multi: tok1", Next(s, t));
  CopyToken(s, t, word);
  Check("multi: foo", StrEq(word, "foo"));
  Check("multi: tok2", Next(s, t));
  CopyToken(s, t, word);
  Check("multi: bar", StrEq(word, "bar"));
  Check("multi: tok3", Next(s, t));
  CopyToken(s, t, word);
  Check("multi: baz", StrEq(word, "baz"));
  Check("multi: done", NOT Next(s, t))
END TestMultiIdent;

(* ── Test 4: Operators ─────────────────────────────── *)

PROCEDURE TestOperators;
VAR s: State; t: Token;
    src: ARRAY [0..15] OF CHAR;
    word: ARRAY [0..7] OF CHAR;
BEGIN
  src := "+-*";
  Init(s, ADR(src), Length(src));
  Check("ops: tok1", Next(s, t));
  Check("ops: kind=Op", t.kind = Operator);
  CopyToken(s, t, word);
  Check("ops: +", StrEq(word, "+"));
  Check("ops: tok2", Next(s, t));
  CopyToken(s, t, word);
  Check("ops: -", StrEq(word, "-"));
  Check("ops: tok3", Next(s, t));
  CopyToken(s, t, word);
  Check("ops: *", StrEq(word, "*"))
END TestOperators;

(* ── Test 5: Mixed ─────────────────────────────────── *)

PROCEDURE TestMixed;
VAR s: State; t: Token;
    src: ARRAY [0..31] OF CHAR;
    word: ARRAY [0..31] OF CHAR;
BEGIN
  src := "x=42";
  Init(s, ADR(src), Length(src));
  Check("mixed: tok1", Next(s, t));
  CopyToken(s, t, word);
  Check("mixed: x", StrEq(word, "x"));
  Check("mixed: tok2", Next(s, t));
  Check("mixed: op", t.kind = Operator);
  CopyToken(s, t, word);
  Check("mixed: =", StrEq(word, "="));
  Check("mixed: tok3", Next(s, t));
  Check("mixed: ident", t.kind = Ident);
  CopyToken(s, t, word);
  Check("mixed: 42", StrEq(word, "42"))
END TestMixed;

(* ── Test 6: Double-quoted strings ─────────────────── *)

PROCEDURE TestSkipDQuote;
VAR n: CARDINAL;
    src: ARRAY [0..31] OF CHAR;
BEGIN
  src := 'a "hello" b';
  n := CountTokens(src);
  Check("dquote: 2 tokens", n = 2)
END TestSkipDQuote;

(* ── Test 7: Single-quoted strings ─────────────────── *)

PROCEDURE TestSkipSQuote;
VAR n: CARDINAL;
    src: ARRAY [0..31] OF CHAR;
BEGIN
  src := "a 'hello' b";
  n := CountTokens(src);
  Check("squote: 2 tokens", n = 2)
END TestSkipSQuote;

(* ── Test 8: Backslash escapes ─────────────────────── *)

PROCEDURE TestSkipEscape;
VAR n: CARDINAL;
    src: ARRAY [0..31] OF CHAR;
BEGIN
  (* "he\"lo" should be one string, tokens: a b *)
  src[0] := 'a';
  src[1] := ' ';
  src[2] := '"';
  src[3] := 'h';
  src[4] := CHR(92);  (* backslash *)
  src[5] := '"';
  src[6] := 'o';
  src[7] := '"';
  src[8] := ' ';
  src[9] := 'b';
  src[10] := 0C;
  n := CountTokens(src);
  Check("escape: 2 tokens", n = 2)
END TestSkipEscape;

(* ── Test 9: // line comments ──────────────────────── *)

PROCEDURE TestSkipLineSlash;
VAR s: State; t: Token;
    src: ARRAY [0..63] OF CHAR;
    word: ARRAY [0..31] OF CHAR;
BEGIN
  src[0] := 'x';
  src[1] := ' ';
  src[2] := '/';
  src[3] := '/';
  src[4] := ' ';
  src[5] := 'c';
  src[6] := 'o';
  src[7] := 'm';
  src[8] := 'm';
  src[9] := CHR(10);
  src[10] := 'y';
  src[11] := 0C;
  Init(s, ADR(src), 11);
  Check("slashcmt: tok1", Next(s, t));
  CopyToken(s, t, word);
  Check("slashcmt: x", StrEq(word, "x"));
  Check("slashcmt: tok2", Next(s, t));
  CopyToken(s, t, word);
  Check("slashcmt: y", StrEq(word, "y"));
  Check("slashcmt: done", NOT Next(s, t))
END TestSkipLineSlash;

(* ── Test 10: # line comments ──────────────────────── *)

PROCEDURE TestSkipLineHash;
VAR s: State; t: Token;
    src: ARRAY [0..63] OF CHAR;
    word: ARRAY [0..31] OF CHAR;
BEGIN
  src[0] := 'a';
  src[1] := ' ';
  src[2] := '#';
  src[3] := ' ';
  src[4] := 'c';
  src[5] := 'o';
  src[6] := 'm';
  src[7] := CHR(10);
  src[8] := 'b';
  src[9] := 0C;
  Init(s, ADR(src), 9);
  Check("hashcmt: tok1", Next(s, t));
  CopyToken(s, t, word);
  Check("hashcmt: a", StrEq(word, "a"));
  Check("hashcmt: tok2", Next(s, t));
  CopyToken(s, t, word);
  Check("hashcmt: b", StrEq(word, "b"));
  Check("hashcmt: done", NOT Next(s, t))
END TestSkipLineHash;

(* ── Test 11: Block comments ──────────────────────── *)

PROCEDURE TestSkipBlock;
VAR s: State; t: Token;
    src: ARRAY [0..63] OF CHAR;
    word: ARRAY [0..31] OF CHAR;
BEGIN
  src[0] := 'x';
  src[1] := ' ';
  src[2] := '/';
  src[3] := '*';
  src[4] := ' ';
  src[5] := 'c';
  src[6] := ' ';
  src[7] := '*';
  src[8] := '/';
  src[9] := ' ';
  src[10] := 'y';
  src[11] := 0C;
  Init(s, ADR(src), 11);
  Check("block: tok1", Next(s, t));
  CopyToken(s, t, word);
  Check("block: x", StrEq(word, "x"));
  Check("block: tok2", Next(s, t));
  CopyToken(s, t, word);
  Check("block: y", StrEq(word, "y"));
  Check("block: done", NOT Next(s, t))
END TestSkipBlock;

(* ── Test 12: Nested block comments ───────────────── *)

PROCEDURE TestSkipNested;
VAR s: State; t: Token;
    src: ARRAY [0..63] OF CHAR;
    word: ARRAY [0..31] OF CHAR;
BEGIN
  (* a /* outer /* inner */ still */ b *)
  src[0] := 'a';
  src[1] := ' ';
  src[2] := '/';
  src[3] := '*';
  src[4] := ' ';
  src[5] := '/';
  src[6] := '*';
  src[7] := ' ';
  src[8] := '*';
  src[9] := '/';
  src[10] := ' ';
  src[11] := '*';
  src[12] := '/';
  src[13] := ' ';
  src[14] := 'b';
  src[15] := 0C;
  Init(s, ADR(src), 15);
  Check("nested: tok1", Next(s, t));
  CopyToken(s, t, word);
  Check("nested: a", StrEq(word, "a"));
  Check("nested: tok2", Next(s, t));
  CopyToken(s, t, word);
  Check("nested: b", StrEq(word, "b"));
  Check("nested: done", NOT Next(s, t))
END TestSkipNested;

(* ── Test 13: Shebang ─────────────────────────────── *)

PROCEDURE TestShebang;
VAR s: State; t: Token;
    src: ARRAY [0..63] OF CHAR;
    word: ARRAY [0..31] OF CHAR;
BEGIN
  src[0] := '#';
  src[1] := '!';
  src[2] := '/';
  src[3] := 'b';
  src[4] := 'i';
  src[5] := 'n';
  src[6] := '/';
  src[7] := 's';
  src[8] := 'h';
  src[9] := CHR(10);
  src[10] := 'x';
  src[11] := 0C;
  Init(s, ADR(src), 11);
  Check("shebang: tok1", Next(s, t));
  Check("shebang: kind", t.kind = Shebang);
  CopyToken(s, t, word);
  Check("shebang: text", StrEq(word, "#!/bin/sh"));
  Check("shebang: tok2", Next(s, t));
  CopyToken(s, t, word);
  Check("shebang: x", StrEq(word, "x"));
  Check("shebang: done", NOT Next(s, t))
END TestShebang;

(* ── Test 14: Digits ───────────────────────────────── *)

PROCEDURE TestDigits;
VAR s: State; t: Token;
    src: ARRAY [0..15] OF CHAR;
    word: ARRAY [0..31] OF CHAR;
BEGIN
  src := "123 456";
  Init(s, ADR(src), Length(src));
  Check("digits: tok1", Next(s, t));
  Check("digits: kind=Ident", t.kind = Ident);
  CopyToken(s, t, word);
  Check("digits: 123", StrEq(word, "123"));
  Check("digits: tok2", Next(s, t));
  CopyToken(s, t, word);
  Check("digits: 456", StrEq(word, "456"))
END TestDigits;

(* ── Test 15: Underscore ───────────────────────────── *)

PROCEDURE TestUnderscore;
VAR s: State; t: Token;
    src: ARRAY [0..31] OF CHAR;
    word: ARRAY [0..31] OF CHAR;
BEGIN
  src := "_foo __bar_2";
  Init(s, ADR(src), Length(src));
  Check("under: tok1", Next(s, t));
  CopyToken(s, t, word);
  Check("under: _foo", StrEq(word, "_foo"));
  Check("under: tok2", Next(s, t));
  CopyToken(s, t, word);
  Check("under: __bar_2", StrEq(word, "__bar_2"))
END TestUnderscore;

(* ── Test 16: CopyToken ───────────────────────────── *)

PROCEDURE TestCopyToken;
VAR s: State; t: Token;
    src: ARRAY [0..31] OF CHAR;
    word: ARRAY [0..31] OF CHAR;
BEGIN
  src := "hello";
  Init(s, ADR(src), Length(src));
  Check("copy: has token", Next(s, t));
  Check("copy: start=0", t.start = 0);
  Check("copy: len=5", t.len = 5);
  CopyToken(s, t, word);
  Check("copy: text", StrEq(word, "hello"))
END TestCopyToken;

(* ── Test 17: CopyToken truncation ─────────────────── *)

PROCEDURE TestCopyTruncate;
VAR s: State; t: Token;
    src: ARRAY [0..31] OF CHAR;
    tiny: ARRAY [0..2] OF CHAR;
BEGIN
  src := "abcdefgh";
  Init(s, ADR(src), Length(src));
  Check("trunc: has token", Next(s, t));
  CopyToken(s, t, tiny);
  (* tiny has HIGH=2, copies 2 chars + NUL terminator *)
  Check("trunc: [0]=a", tiny[0] = 'a');
  Check("trunc: [1]=b", tiny[1] = 'b');
  Check("trunc: [2]=NUL", tiny[2] = 0C)
END TestCopyTruncate;

(* ── Test 18: Realistic code ───────────────────────── *)

PROCEDURE TestRealCode;
VAR s: State; t: Token;
    src: ARRAY [0..63] OF CHAR;
    word: ARRAY [0..31] OF CHAR;
    count: CARDINAL;
BEGIN
  src := "int main() { return 0; }";
  Init(s, ADR(src), Length(src));
  count := 0;
  WHILE Next(s, t) DO INC(count) END;
  (* int main ( ) { return 0 ; } = 9 tokens *)
  Check("real: 9 tokens", count = 9);

  (* Verify specific tokens *)
  Init(s, ADR(src), Length(src));
  Check("real: tok1", Next(s, t));
  CopyToken(s, t, word);
  Check("real: int", StrEq(word, "int"));
  Check("real: tok2", Next(s, t));
  CopyToken(s, t, word);
  Check("real: main", StrEq(word, "main"));
  Check("real: tok3", Next(s, t));
  Check("real: (", t.kind = Operator)
END TestRealCode;

BEGIN
  passed := 0;
  failed := 0;
  total := 0;

  TestEmpty;
  TestIdent;
  TestMultiIdent;
  TestOperators;
  TestMixed;
  TestSkipDQuote;
  TestSkipSQuote;
  TestSkipEscape;
  TestSkipLineSlash;
  TestSkipLineHash;
  TestSkipBlock;
  TestSkipNested;
  TestShebang;
  TestDigits;
  TestUnderscore;
  TestCopyToken;
  TestCopyTruncate;
  TestRealCode;

  WriteLn;
  WriteString("m2tok: ");
  WriteInt(passed, 0); WriteString(" passed, ");
  WriteInt(failed, 0); WriteString(" failed, ");
  WriteInt(total, 0); WriteString(" total"); WriteLn;
  IF failed = 0 THEN
    WriteString("ALL TESTS PASSED"); WriteLn
  END
END TokTests.
