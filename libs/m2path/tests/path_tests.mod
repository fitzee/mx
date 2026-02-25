MODULE PathTests;
(* Test suite for m2path.

   Tests:
     1.  normalize_basic        Simple paths stay unchanged
     2.  normalize_double_slash  "//" merged to single "/"
     3.  normalize_dot          "." segments removed
     4.  normalize_dotdot       ".." pops parent
     5.  normalize_root_dotdot  Can't go above root
     6.  normalize_dot_only     "." -> "."
     7.  normalize_dotdot_only  ".." -> ".."
     8.  normalize_trailing     Trailing "/" stripped
     9.  extension_basic        ".mod" from "foo.mod"
    10.  extension_none         No dot -> ""
    11.  extension_dotfile       ".gitignore" -> ""
    12.  extension_double       "foo.tar.gz" -> ".gz"
    13.  extension_dir_dot      "/a/b.c/d" -> ""
    14.  stripext_basic         "foo.mod" -> "foo"
    15.  stripext_none          "foo" -> "foo"
    16.  stripext_path          "/a/b.txt" -> "/a/b"
    17.  isabsolute_yes         "/foo" -> TRUE
    18.  isabsolute_no          "foo" -> FALSE
    19.  isabsolute_empty       "" -> FALSE
    20.  split_full             "/a/b/c" -> ("/a/b", "c")
    21.  split_bare             "foo" -> ("", "foo")
    22.  split_root             "/" -> ("/", "")
    23.  split_rootfile         "/a" -> ("/", "a")
    24.  relative_sibling       "../c/d" from /a/b to /a/c/d
    25.  relative_same          "." from /a/b to /a/b
    26.  relative_up            "../.." from /a/b/c to /a
    27.  join_basic             "a" + "b" -> "a/b"
    28.  join_trailing_slash    "a/" + "b" -> "a/b"
    29.  join_abs_b             "a" + "/b" -> "/b"
    30.  join_empty_a           "" + "b" -> "b"
    31.  match_star             "*.mod" matches "foo.mod"
    32.  match_star_no          "*.mod" rejects "foo.txt"
    33.  match_question_yes     "?" matches "a"
    34.  match_question_no      "?" rejects "ab" *)

FROM InOut IMPORT WriteString, WriteLn, WriteInt;
FROM Strings IMPORT CompareStr;
FROM Path IMPORT Normalize, Extension, StripExt, IsAbsolute,
                 Split, RelativeTo, Join, Match;

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

PROCEDURE StrEq(VAR a: ARRAY OF CHAR; b: ARRAY OF CHAR): BOOLEAN;
BEGIN
  RETURN CompareStr(a, b) = 0
END StrEq;

(* ── Normalize tests ───────────────────────────────────── *)

PROCEDURE TestNormalize;
VAR out: ARRAY [0..1023] OF CHAR;
BEGIN
  Normalize("a/b/c", out);
  Check("normalize: a/b/c", StrEq(out, "a/b/c"));

  Normalize("a//b", out);
  Check("normalize: a//b -> a/b", StrEq(out, "a/b"));

  Normalize("a/./b", out);
  Check("normalize: a/./b -> a/b", StrEq(out, "a/b"));

  Normalize("a/b/../c", out);
  Check("normalize: a/b/../c -> a/c", StrEq(out, "a/c"));

  Normalize("/a/b/..", out);
  Check("normalize: /a/b/.. -> /a", StrEq(out, "/a"));

  Normalize("/../a", out);
  Check("normalize: /../a -> /a", StrEq(out, "/a"));

  Normalize(".", out);
  Check("normalize: . -> .", StrEq(out, "."));

  Normalize("..", out);
  Check("normalize: .. -> ..", StrEq(out, ".."));

  Normalize("a/b/c/", out);
  Check("normalize: a/b/c/ -> a/b/c", StrEq(out, "a/b/c"))
END TestNormalize;

(* ── Extension tests ───────────────────────────────────── *)

PROCEDURE TestExtension;
VAR out: ARRAY [0..255] OF CHAR;
BEGIN
  Extension("foo.mod", out);
  Check("ext: foo.mod -> .mod", StrEq(out, ".mod"));

  Extension("foo", out);
  Check("ext: foo -> empty", StrEq(out, ""));

  Extension(".gitignore", out);
  Check("ext: .gitignore -> empty", StrEq(out, ""));

  Extension("foo.tar.gz", out);
  Check("ext: foo.tar.gz -> .gz", StrEq(out, ".gz"));

  Extension("/a/b.c/d", out);
  Check("ext: /a/b.c/d -> empty", StrEq(out, ""))
END TestExtension;

(* ── StripExt tests ────────────────────────────────────── *)

PROCEDURE TestStripExt;
VAR out: ARRAY [0..1023] OF CHAR;
BEGIN
  StripExt("foo.mod", out);
  Check("stripext: foo.mod -> foo", StrEq(out, "foo"));

  StripExt("foo", out);
  Check("stripext: foo -> foo", StrEq(out, "foo"));

  StripExt("/a/b.txt", out);
  Check("stripext: /a/b.txt -> /a/b", StrEq(out, "/a/b"))
END TestStripExt;

(* ── IsAbsolute tests ──────────────────────────────────── *)

PROCEDURE TestIsAbsolute;
BEGIN
  Check("isabs: /foo -> TRUE", IsAbsolute("/foo"));
  Check("isabs: foo -> FALSE", NOT IsAbsolute("foo"));
  Check("isabs: empty -> FALSE", NOT IsAbsolute(""))
END TestIsAbsolute;

(* ── Split tests ───────────────────────────────────────── *)

PROCEDURE TestSplit;
VAR
  dir, base: ARRAY [0..1023] OF CHAR;
BEGIN
  Split("/a/b/c", dir, base);
  Check("split: /a/b/c dir=/a/b", StrEq(dir, "/a/b"));
  Check("split: /a/b/c base=c", StrEq(base, "c"));

  Split("foo", dir, base);
  Check("split: foo dir=empty", StrEq(dir, ""));
  Check("split: foo base=foo", StrEq(base, "foo"));

  Split("/", dir, base);
  Check("split: / dir=/", StrEq(dir, "/"));
  Check("split: / base=empty", StrEq(base, ""));

  Split("/a", dir, base);
  Check("split: /a dir=/", StrEq(dir, "/"));
  Check("split: /a base=a", StrEq(base, "a"))
END TestSplit;

(* ── RelativeTo tests ──────────────────────────────────── *)

PROCEDURE TestRelativeTo;
VAR out: ARRAY [0..1023] OF CHAR;
BEGIN
  RelativeTo("/a/b", "/a/c/d", out);
  Check("rel: /a/b -> /a/c/d = ../c/d", StrEq(out, "../c/d"));

  RelativeTo("/a/b", "/a/b", out);
  Check("rel: /a/b -> /a/b = .", StrEq(out, "."));

  RelativeTo("/a/b/c", "/a", out);
  Check("rel: /a/b/c -> /a = ../..", StrEq(out, "../.."))
END TestRelativeTo;

(* ── Join tests ────────────────────────────────────────── *)

PROCEDURE TestJoin;
VAR out: ARRAY [0..1023] OF CHAR;
BEGIN
  Join("a", "b", out);
  Check("join: a + b = a/b", StrEq(out, "a/b"));

  Join("a/", "b", out);
  Check("join: a/ + b = a/b", StrEq(out, "a/b"));

  Join("a", "/b", out);
  Check("join: a + /b = /b", StrEq(out, "/b"));

  Join("", "b", out);
  Check("join: empty + b = b", StrEq(out, "b"))
END TestJoin;

(* ── Match tests ───────────────────────────────────────── *)

PROCEDURE TestMatch;
BEGIN
  Check("match: foo.mod vs *.mod", Match("foo.mod", "*.mod"));
  Check("match: foo.txt vs *.mod", NOT Match("foo.txt", "*.mod"));
  Check("match: a vs ?", Match("a", "?"));
  Check("match: ab vs ?", NOT Match("ab", "?"))
END TestMatch;

BEGIN
  passed := 0;
  failed := 0;
  total := 0;

  TestNormalize;
  TestExtension;
  TestStripExt;
  TestIsAbsolute;
  TestSplit;
  TestRelativeTo;
  TestJoin;
  TestMatch;

  WriteLn;
  WriteString("m2path: ");
  WriteInt(passed, 0); WriteString(" passed, ");
  WriteInt(failed, 0); WriteString(" failed, ");
  WriteInt(total, 0); WriteString(" total"); WriteLn;
  IF failed = 0 THEN
    WriteString("ALL TESTS PASSED"); WriteLn
  END
END PathTests.
