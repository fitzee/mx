IMPLEMENTATION MODULE Regex;

FROM SYSTEM IMPORT ADDRESS, ADR, TSIZE;
FROM RegexBridge IMPORT m2_regex_compile, m2_regex_free,
                         m2_regex_test, m2_regex_find,
                         m2_regex_find_all, m2_regex_error;

(* ── Lifecycle ─────────────────────────────────────── *)

PROCEDURE Compile(pattern: ARRAY OF CHAR; VAR re: Regex): Status;
VAR p: ADDRESS;
BEGIN
  p := m2_regex_compile(ADR(pattern));
  IF p = NIL THEN
    re := NIL;
    RETURN BadPattern
  END;
  re := p;
  RETURN Ok
END Compile;

PROCEDURE Free(VAR re: Regex);
BEGIN
  IF re # NIL THEN
    m2_regex_free(re);
    re := NIL
  END
END Free;

(* ── Matching ──────────────────────────────────────── *)

PROCEDURE Test(re: Regex; text: ARRAY OF CHAR): BOOLEAN;
BEGIN
  IF re = NIL THEN RETURN FALSE END;
  RETURN m2_regex_test(re, ADR(text)) = 1
END Test;

PROCEDURE Find(re: Regex; text: ARRAY OF CHAR; VAR m: Match): Status;
VAR
  rc: INTEGER;
  s, l: INTEGER;
BEGIN
  IF re = NIL THEN RETURN Error END;
  rc := m2_regex_find(re, ADR(text), s, l);
  IF rc = 0 THEN
    m.start := CARDINAL(s);
    m.len := CARDINAL(l);
    RETURN Ok
  ELSIF rc = 1 THEN
    RETURN NoMatch
  ELSE
    RETURN Error
  END
END Find;

PROCEDURE FindAll(re: Regex; text: ARRAY OF CHAR;
                  VAR matches: ARRAY OF Match; maxMatches: CARDINAL;
                  VAR count: CARDINAL): Status;
VAR
  rc: INTEGER;
  i: CARDINAL;
  max: INTEGER;
  cnt: INTEGER;
  starts: ARRAY [0..MaxMatches-1] OF INTEGER;
  lens:   ARRAY [0..MaxMatches-1] OF INTEGER;
BEGIN
  IF re = NIL THEN RETURN Error END;
  count := 0;

  (* clamp to our internal array limit *)
  IF maxMatches > MaxMatches THEN
    max := MaxMatches
  ELSE
    max := INTEGER(maxMatches)
  END;

  rc := m2_regex_find_all(re, ADR(text),
                           ADR(starts), ADR(lens),
                           max, cnt);
  IF rc = 0 THEN
    count := CARDINAL(cnt);
    i := 0;
    WHILE i < count DO
      matches[i].start := CARDINAL(starts[i]);
      matches[i].len := CARDINAL(lens[i]);
      INC(i)
    END;
    RETURN Ok
  ELSIF rc = 1 THEN
    RETURN NoMatch
  ELSE
    RETURN Error
  END
END FindAll;

(* ── Error reporting ───────────────────────────────── *)

PROCEDURE GetError(VAR buf: ARRAY OF CHAR);
BEGIN
  m2_regex_error(ADR(buf), HIGH(buf) + 1)
END GetError;

END Regex.
