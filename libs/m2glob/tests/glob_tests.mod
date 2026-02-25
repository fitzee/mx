MODULE GlobTests;
(* Test suite for m2glob: gitignore-grade glob pattern matching.

   Tests:
     1.  literal         exact string match and mismatch
     2.  star            single * matches non-/ sequences
     3.  question        ? matches single non-/ char
     4.  char_class      [abc], [a-z], [!x] bracket expressions
     5.  double_star     ** for multi-directory matching
     6.  negation        IsNegated detection
     7.  anchor          IsAnchored detection
     8.  path_sep        HasPathSep detection
     9.  strip           StripNegation and StripAnchor
    10.  empty           edge cases with empty strings *)

FROM InOut IMPORT WriteString, WriteLn, WriteInt;
FROM Glob IMPORT Match, IsNegated, IsAnchored, HasPathSep,
                 StripNegation, StripAnchor;

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

(* ── Test 1: Literal matching ────────────────────────── *)

PROCEDURE TestLiteral;
BEGIN
  Check("literal: foo=foo", Match("foo", "foo"));
  Check("literal: foo#bar", NOT Match("foo", "bar"))
END TestLiteral;

(* ── Test 2: Single star ─────────────────────────────── *)

PROCEDURE TestStar;
BEGIN
  Check("star: *.mod=test.mod", Match("*.mod", "test.mod"));
  Check("star: *.mod#test.txt", NOT Match("*.mod", "test.txt"));
  Check("star: src/*=src/foo", Match("src/*", "src/foo"));
  Check("star: src/*#src/a/b", NOT Match("src/*", "src/a/b"))
END TestStar;

(* ── Test 3: Question mark ───────────────────────────── *)

PROCEDURE TestQuestion;
BEGIN
  Check("question: ?.c=a.c", Match("?.c", "a.c"));
  Check("question: ?.c#ab.c", NOT Match("?.c", "ab.c"))
END TestQuestion;

(* ── Test 4: Character classes ───────────────────────── *)

PROCEDURE TestCharClass;
BEGIN
  Check("class: [abc]=a", Match("[abc]", "a"));
  Check("class: [abc]#d", NOT Match("[abc]", "d"));
  Check("class: [a-z]=m", Match("[a-z]", "m"));
  Check("class: [a-z]#M", NOT Match("[a-z]", "M"));
  Check("class: [!a]=b", Match("[!a]", "b"));
  Check("class: [!a]#a", NOT Match("[!a]", "a"))
END TestCharClass;

(* ── Test 5: Double star ─────────────────────────────── *)

PROCEDURE TestDoubleStar;
BEGIN
  Check("dstar: **/foo=foo", Match("**/foo", "foo"));
  Check("dstar: **/foo=a/foo", Match("**/foo", "a/foo"));
  Check("dstar: **/foo=a/b/foo", Match("**/foo", "a/b/foo"));
  Check("dstar: src/**=src/a", Match("src/**", "src/a"));
  Check("dstar: src/**=src/a/b/c", Match("src/**", "src/a/b/c"));
  Check("dstar: a/**/b=a/b", Match("a/**/b", "a/b"));
  Check("dstar: a/**/b=a/x/y/b", Match("a/**/b", "a/x/y/b"))
END TestDoubleStar;

(* ── Test 6: Negation detection ──────────────────────── *)

PROCEDURE TestNegation;
BEGIN
  Check("negated: !foo", IsNegated("!foo"));
  Check("negated: foo", NOT IsNegated("foo"))
END TestNegation;

(* ── Test 7: Anchor detection ────────────────────────── *)

PROCEDURE TestAnchor;
BEGIN
  Check("anchor: /foo", IsAnchored("/foo"));
  Check("anchor: foo", NOT IsAnchored("foo"))
END TestAnchor;

(* ── Test 8: Path separator detection ────────────────── *)

PROCEDURE TestPathSep;
BEGIN
  Check("pathsep: a/b", HasPathSep("a/b"));
  Check("pathsep: foo", NOT HasPathSep("foo"))
END TestPathSep;

(* ── Test 9: Strip procedures ────────────────────────── *)

PROCEDURE TestStrip;
VAR buf: ARRAY [0..255] OF CHAR;
BEGIN
  StripNegation("!foo.mod", buf);
  Check("strip neg: !foo.mod", Match(buf, "foo.mod"));

  StripAnchor("/src/main.mod", buf);
  Check("strip anch: /src/main.mod", Match(buf, "src/main.mod"));

  (* No-op cases *)
  StripNegation("foo", buf);
  Check("strip neg noop", Match(buf, "foo"));

  StripAnchor("foo", buf);
  Check("strip anch noop", Match(buf, "foo"))
END TestStrip;

(* ── Test 10: Empty edge cases ───────────────────────── *)

PROCEDURE TestEmpty;
BEGIN
  Check("empty: ''=''", Match("", ""));
  Check("empty: *=''", Match("*", ""))
END TestEmpty;

BEGIN
  passed := 0;
  failed := 0;
  total := 0;

  TestLiteral;
  TestStar;
  TestQuestion;
  TestCharClass;
  TestDoubleStar;
  TestNegation;
  TestAnchor;
  TestPathSep;
  TestStrip;
  TestEmpty;

  WriteLn;
  WriteString("m2glob: ");
  WriteInt(passed, 0); WriteString(" passed, ");
  WriteInt(failed, 0); WriteString(" failed, ");
  WriteInt(total, 0); WriteString(" total"); WriteLn;
  IF failed = 0 THEN
    WriteString("ALL TESTS PASSED"); WriteLn
  END
END GlobTests.
