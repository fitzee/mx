MODULE RegexTests;
(* Deterministic test suite for m2regex.

   Tests:
     1.  compile_ok       Compile a valid pattern
     2.  test_match       Test matches simple patterns
     3.  test_no_match    Test rejects non-matching text
     4.  find_basic       Find locates first match position
     5.  find_offset      Find returns correct offset in middle of string
     6.  find_no_match    Find returns NoMatch for absent pattern
     7.  find_all_multi   FindAll locates multiple non-overlapping matches
     8.  find_all_single  FindAll with one match
     9.  find_all_none    FindAll with no matches
    10.  bad_pattern      Compile rejects invalid regex
    11.  error_message    GetError returns non-empty message after bad compile
    12.  free_nil         Free on NIL does not crash
    13.  test_nil         Test on NIL returns FALSE
    14.  digit_class      Character class \d+ matching
    15.  alternation      Alternation pattern (a|b) *)

FROM InOut IMPORT WriteString, WriteLn, WriteInt, WriteCard;
FROM Regex IMPORT Regex, Match, Status, Ok, NoMatch, BadPattern, Error,
                  Compile, Free, Test, Find, FindAll, GetError,
                  MaxMatches;

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

(* ── Test 1: Compile valid pattern ───────────────── *)

PROCEDURE TestCompileOk;
VAR re: Regex; s: Status;
BEGIN
  s := Compile("hello", re);
  Check("compile: ok status", s = Ok);
  Check("compile: re not nil", re # NIL);
  Free(re)
END TestCompileOk;

(* ── Test 2: Test match ──────────────────────────── *)

PROCEDURE TestMatch;
VAR re: Regex; s: Status;
BEGIN
  s := Compile("world", re);
  Check("match: compile ok", s = Ok);
  Check("match: hello world", Test(re, "hello world"));
  Check("match: world alone", Test(re, "world"));
  Free(re)
END TestMatch;

(* ── Test 3: Test no match ───────────────────────── *)

PROCEDURE TestNoMatch;
VAR re: Regex; s: Status;
BEGIN
  s := Compile("xyz", re);
  Check("nomatch: compile ok", s = Ok);
  Check("nomatch: abc", NOT Test(re, "abc"));
  Check("nomatch: empty", NOT Test(re, ""));
  Free(re)
END TestNoMatch;

(* ── Test 4: Find basic ─────────────────────────── *)

PROCEDURE TestFindBasic;
VAR re: Regex; m: Match; s: Status;
BEGIN
  s := Compile("foo", re);
  Check("find: compile ok", s = Ok);
  s := Find(re, "foo bar", m);
  Check("find: status ok", s = Ok);
  Check("find: start=0", m.start = 0);
  Check("find: len=3", m.len = 3);
  Free(re)
END TestFindBasic;

(* ── Test 5: Find offset ────────────────────────── *)

PROCEDURE TestFindOffset;
VAR re: Regex; m: Match; s: Status;
BEGIN
  s := Compile("[0-9]+", re);
  Check("offset: compile ok", s = Ok);
  s := Find(re, "abc 42 def", m);
  Check("offset: status ok", s = Ok);
  Check("offset: start=4", m.start = 4);
  Check("offset: len=2", m.len = 2);
  Free(re)
END TestFindOffset;

(* ── Test 6: Find no match ──────────────────────── *)

PROCEDURE TestFindNoMatch;
VAR re: Regex; m: Match; s: Status;
BEGIN
  s := Compile("[0-9]+", re);
  Check("findnm: compile ok", s = Ok);
  s := Find(re, "no digits here", m);
  Check("findnm: NoMatch", s = NoMatch);
  Free(re)
END TestFindNoMatch;

(* ── Test 7: FindAll multiple ────────────────────── *)

PROCEDURE TestFindAllMulti;
VAR
  re: Regex;
  ms: ARRAY [0..MaxMatches-1] OF Match;
  count: CARDINAL;
  s: Status;
BEGIN
  s := Compile("[0-9]+", re);
  Check("findall: compile ok", s = Ok);
  s := FindAll(re, "a1b22c333d", ms, MaxMatches, count);
  Check("findall: status ok", s = Ok);
  Check("findall: count=3", count = 3);

  (* first match: "1" at position 1 *)
  Check("findall: m0 start=1", ms[0].start = 1);
  Check("findall: m0 len=1", ms[0].len = 1);

  (* second match: "22" at position 3 *)
  Check("findall: m1 start=3", ms[1].start = 3);
  Check("findall: m1 len=2", ms[1].len = 2);

  (* third match: "333" at position 6 *)
  Check("findall: m2 start=6", ms[2].start = 6);
  Check("findall: m2 len=3", ms[2].len = 3);
  Free(re)
END TestFindAllMulti;

(* ── Test 8: FindAll single ──────────────────────── *)

PROCEDURE TestFindAllSingle;
VAR
  re: Regex;
  ms: ARRAY [0..MaxMatches-1] OF Match;
  count: CARDINAL;
  s: Status;
BEGIN
  s := Compile("only", re);
  Check("findall1: compile ok", s = Ok);
  s := FindAll(re, "the only one", ms, MaxMatches, count);
  Check("findall1: status ok", s = Ok);
  Check("findall1: count=1", count = 1);
  Check("findall1: start=4", ms[0].start = 4);
  Check("findall1: len=4", ms[0].len = 4);
  Free(re)
END TestFindAllSingle;

(* ── Test 9: FindAll none ────────────────────────── *)

PROCEDURE TestFindAllNone;
VAR
  re: Regex;
  ms: ARRAY [0..MaxMatches-1] OF Match;
  count: CARDINAL;
  s: Status;
BEGIN
  s := Compile("zzz", re);
  Check("findall0: compile ok", s = Ok);
  s := FindAll(re, "no match here", ms, MaxMatches, count);
  Check("findall0: NoMatch", s = NoMatch);
  Check("findall0: count=0", count = 0);
  Free(re)
END TestFindAllNone;

(* ── Test 10: Bad pattern ────────────────────────── *)

PROCEDURE TestBadPattern;
VAR re: Regex; s: Status;
BEGIN
  s := Compile("[invalid", re);
  Check("badpat: BadPattern", s = BadPattern);
  Check("badpat: re is nil", re = NIL)
END TestBadPattern;

(* ── Test 11: Error message ──────────────────────── *)

PROCEDURE TestErrorMessage;
VAR
  re: Regex;
  s: Status;
  buf: ARRAY [0..255] OF CHAR;
BEGIN
  s := Compile("[bad", re);
  Check("errmsg: BadPattern", s = BadPattern);
  GetError(buf);
  (* error message should not be empty *)
  Check("errmsg: non-empty", buf[0] # 0C)
END TestErrorMessage;

(* ── Test 12: Free NIL ───────────────────────────── *)

PROCEDURE TestFreeNil;
VAR re: Regex;
BEGIN
  re := NIL;
  Free(re);
  Check("freenil: no crash", TRUE)
END TestFreeNil;

(* ── Test 13: Test NIL ───────────────────────────── *)

PROCEDURE TestNilRegex;
VAR re: Regex;
BEGIN
  re := NIL;
  Check("testnil: returns false", NOT Test(re, "anything"))
END TestNilRegex;

(* ── Test 14: Digit class ────────────────────────── *)

PROCEDURE TestDigitClass;
VAR re: Regex; m: Match; s: Status;
BEGIN
  s := Compile("[[:digit:]]+", re);
  Check("digit: compile ok", s = Ok);
  Check("digit: matches 123", Test(re, "abc123def"));
  Check("digit: no match letters", NOT Test(re, "abcdef"));
  s := Find(re, "abc123def", m);
  Check("digit: find ok", s = Ok);
  Check("digit: start=3", m.start = 3);
  Check("digit: len=3", m.len = 3);
  Free(re)
END TestDigitClass;

(* ── Test 15: Alternation ────────────────────────── *)

PROCEDURE TestAlternation;
VAR re: Regex; s: Status;
BEGIN
  s := Compile("cat|dog", re);
  Check("alt: compile ok", s = Ok);
  Check("alt: matches cat", Test(re, "I have a cat"));
  Check("alt: matches dog", Test(re, "I have a dog"));
  Check("alt: no match fish", NOT Test(re, "I have a fish"));
  Free(re)
END TestAlternation;

(* ── Main ────────────────────────────────────────── *)

BEGIN
  passed := 0;
  failed := 0;
  total := 0;

  WriteString("m2regex test suite"); WriteLn;
  WriteString("=================="); WriteLn;

  TestCompileOk;
  TestMatch;
  TestNoMatch;
  TestFindBasic;
  TestFindOffset;
  TestFindNoMatch;
  TestFindAllMulti;
  TestFindAllSingle;
  TestFindAllNone;
  TestBadPattern;
  TestErrorMessage;
  TestFreeNil;
  TestNilRegex;
  TestDigitClass;
  TestAlternation;

  WriteLn;
  WriteString("m2regex: ");
  WriteInt(passed, 0); WriteString(" passed, ");
  WriteInt(failed, 0); WriteString(" failed, ");
  WriteInt(total, 0); WriteString(" total"); WriteLn;

  IF failed > 0 THEN
    WriteString("*** FAILURES ***"); WriteLn
  ELSE
    WriteString("*** ALL TESTS PASSED ***"); WriteLn
  END
END RegexTests.
