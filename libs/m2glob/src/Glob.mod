IMPLEMENTATION MODULE Glob;

FROM Strings IMPORT Assign, Length;

(* ── String helpers ──────────────────────────────────── *)

PROCEDURE StrLen(VAR s: ARRAY OF CHAR): CARDINAL;
VAR i: CARDINAL;
BEGIN
  i := 0;
  WHILE (i <= HIGH(s)) AND (s[i] # 0C) DO
    INC(i)
  END;
  RETURN i
END StrLen;

(* ── Backtrack stack for iterative ** matching ───────── *)

CONST
  MaxStack = 32;

TYPE
  BacktrackEntry = RECORD
    pi: CARDINAL;  (* pattern index to resume *)
    ti: CARDINAL;  (* text index to resume *)
  END;

(* ── MatchClass ──────────────────────────────────────── *)

(* Parse a [...] bracket expression starting after '['.
   pi enters pointing at first char inside brackets.
   On return pi points past the closing ']'.
   Returns TRUE if ch is in the class. *)

PROCEDURE MatchClass(VAR pat: ARRAY OF CHAR; plen: CARDINAL;
                     VAR pi: CARDINAL; ch: CHAR): BOOLEAN;
VAR
  negate, matched: BOOLEAN;
  lo, hi: CHAR;
  startPi: CARDINAL;
BEGIN
  negate := FALSE;
  matched := FALSE;

  (* Check for negation *)
  IF (pi < plen) AND (pat[pi] = '!') THEN
    negate := TRUE;
    INC(pi)
  END;

  startPi := pi;

  (* Scan characters until ']' *)
  WHILE (pi < plen) AND ((pat[pi] # ']') OR (pi = startPi)) DO
    lo := pat[pi];
    INC(pi);

    (* Check for range a-z *)
    IF (pi + 1 < plen) AND (pat[pi] = '-') AND (pat[pi+1] # ']') THEN
      INC(pi);  (* skip '-' *)
      hi := pat[pi];
      INC(pi);
      IF (ch >= lo) AND (ch <= hi) THEN
        matched := TRUE
      END
    ELSE
      IF ch = lo THEN
        matched := TRUE
      END
    END
  END;

  (* Skip closing ']' *)
  IF (pi < plen) AND (pat[pi] = ']') THEN
    INC(pi)
  END;

  IF negate THEN
    RETURN NOT matched
  ELSE
    RETURN matched
  END
END MatchClass;

(* ── Core Match ──────────────────────────────────────── *)

PROCEDURE Match(pattern: ARRAY OF CHAR;
                text: ARRAY OF CHAR): BOOLEAN;
VAR
  pi, ti, plen, tlen: CARDINAL;
  stack: ARRAY [0..MaxStack-1] OF BacktrackEntry;
  sp: CARDINAL;

  (* Star backtrack: single * saves a resume point *)
  starPi, starTi: CARDINAL;
  hasStar: BOOLEAN;

  classPi: CARDINAL;
  classMatch: BOOLEAN;

BEGIN
  plen := StrLen(pattern);
  tlen := StrLen(text);
  pi := 0;
  ti := 0;
  sp := 0;
  hasStar := FALSE;

  LOOP
    IF (pi < plen) AND (pi + 1 < plen) AND
       (pattern[pi] = '*') AND (pattern[pi+1] = '*') THEN

      (* ── Handle ** ──────────────────────────────── *)

      (* Skip all consecutive *'s *)
      WHILE (pi < plen) AND (pattern[pi] = '*') DO
        INC(pi)
      END;

      (* Trailing **: matches everything remaining *)
      IF pi >= plen THEN
        RETURN TRUE
      END;

      (* **/ prefix: skip the slash *)
      IF (pi < plen) AND (pattern[pi] = '/') THEN
        INC(pi)
      END;

      (* Push backtrack: try matching rest from current ti,
         and on failure advance ti past next component *)
      IF sp < MaxStack THEN
        stack[sp].pi := pi;
        stack[sp].ti := ti;
        INC(sp)
      END;

      (* Continue matching from current positions *)

    ELSIF (pi < plen) AND (pattern[pi] = '*') THEN

      (* ── Handle single * ───────────────────────── *)
      INC(pi);

      (* Save star backtrack point *)
      hasStar := TRUE;
      starPi := pi;
      starTi := ti;

      (* Try matching zero chars: continue loop *)

    ELSIF (pi < plen) AND (ti < tlen) THEN

      IF pattern[pi] = '?' THEN
        (* Match any single non-/ char *)
        IF text[ti] = '/' THEN
          (* Mismatch: try backtrack *)
          IF hasStar THEN
            (* Can't advance star past / *)
            IF (starTi < tlen) AND (text[starTi] # '/') THEN
              INC(starTi);
              pi := starPi;
              ti := starTi
            ELSE
              hasStar := FALSE;
              IF sp > 0 THEN
                DEC(sp);
                pi := stack[sp].pi;
                ti := stack[sp].ti;
                (* Advance ti to next / or end *)
                WHILE (ti < tlen) AND (text[ti] # '/') DO
                  INC(ti)
                END;
                IF (ti < tlen) AND (text[ti] = '/') THEN
                  INC(ti);
                  IF sp < MaxStack THEN
                    stack[sp].pi := pi;
                    stack[sp].ti := ti;
                    INC(sp)
                  END
                END
              ELSE
                RETURN FALSE
              END
            END
          ELSIF sp > 0 THEN
            DEC(sp);
            pi := stack[sp].pi;
            ti := stack[sp].ti;
            WHILE (ti < tlen) AND (text[ti] # '/') DO
              INC(ti)
            END;
            IF (ti < tlen) AND (text[ti] = '/') THEN
              INC(ti);
              IF sp < MaxStack THEN
                stack[sp].pi := pi;
                stack[sp].ti := ti;
                INC(sp)
              END
            END
          ELSE
            RETURN FALSE
          END
        ELSE
          INC(pi);
          INC(ti)
        END

      ELSIF pattern[pi] = '[' THEN
        (* Character class *)
        INC(pi);
        classPi := pi;
        classMatch := MatchClass(pattern, plen, pi, text[ti]);
        IF classMatch THEN
          INC(ti)
        ELSE
          (* Mismatch: try backtrack *)
          IF hasStar THEN
            IF (starTi < tlen) AND (text[starTi] # '/') THEN
              INC(starTi);
              pi := starPi;
              ti := starTi
            ELSE
              hasStar := FALSE;
              IF sp > 0 THEN
                DEC(sp);
                pi := stack[sp].pi;
                ti := stack[sp].ti;
                WHILE (ti < tlen) AND (text[ti] # '/') DO
                  INC(ti)
                END;
                IF (ti < tlen) AND (text[ti] = '/') THEN
                  INC(ti);
                  IF sp < MaxStack THEN
                    stack[sp].pi := pi;
                    stack[sp].ti := ti;
                    INC(sp)
                  END
                END
              ELSE
                RETURN FALSE
              END
            END
          ELSIF sp > 0 THEN
            DEC(sp);
            pi := stack[sp].pi;
            ti := stack[sp].ti;
            WHILE (ti < tlen) AND (text[ti] # '/') DO
              INC(ti)
            END;
            IF (ti < tlen) AND (text[ti] = '/') THEN
              INC(ti);
              IF sp < MaxStack THEN
                stack[sp].pi := pi;
                stack[sp].ti := ti;
                INC(sp)
              END
            END
          ELSE
            RETURN FALSE
          END
        END

      ELSE
        (* Literal character *)
        IF pattern[pi] = text[ti] THEN
          INC(pi);
          INC(ti)
        ELSE
          (* Mismatch: try backtrack *)
          IF hasStar THEN
            IF (starTi < tlen) AND (text[starTi] # '/') THEN
              INC(starTi);
              pi := starPi;
              ti := starTi
            ELSE
              hasStar := FALSE;
              IF sp > 0 THEN
                DEC(sp);
                pi := stack[sp].pi;
                ti := stack[sp].ti;
                WHILE (ti < tlen) AND (text[ti] # '/') DO
                  INC(ti)
                END;
                IF (ti < tlen) AND (text[ti] = '/') THEN
                  INC(ti);
                  IF sp < MaxStack THEN
                    stack[sp].pi := pi;
                    stack[sp].ti := ti;
                    INC(sp)
                  END
                END
              ELSE
                RETURN FALSE
              END
            END
          ELSIF sp > 0 THEN
            DEC(sp);
            pi := stack[sp].pi;
            ti := stack[sp].ti;
            WHILE (ti < tlen) AND (text[ti] # '/') DO
              INC(ti)
            END;
            IF (ti < tlen) AND (text[ti] = '/') THEN
              INC(ti);
              IF sp < MaxStack THEN
                stack[sp].pi := pi;
                stack[sp].ti := ti;
                INC(sp)
              END
            END
          ELSE
            RETURN FALSE
          END
        END
      END

    ELSIF (pi >= plen) AND (ti >= tlen) THEN
      (* Both exhausted: match *)
      RETURN TRUE

    ELSIF (pi >= plen) AND (ti < tlen) THEN
      (* Pattern exhausted but text remains: backtrack *)
      IF hasStar THEN
        IF (starTi < tlen) AND (text[starTi] # '/') THEN
          INC(starTi);
          pi := starPi;
          ti := starTi
        ELSE
          hasStar := FALSE;
          IF sp > 0 THEN
            DEC(sp);
            pi := stack[sp].pi;
            ti := stack[sp].ti;
            WHILE (ti < tlen) AND (text[ti] # '/') DO
              INC(ti)
            END;
            IF (ti < tlen) AND (text[ti] = '/') THEN
              INC(ti);
              IF sp < MaxStack THEN
                stack[sp].pi := pi;
                stack[sp].ti := ti;
                INC(sp)
              END
            END
          ELSE
            RETURN FALSE
          END
        END
      ELSIF sp > 0 THEN
        DEC(sp);
        pi := stack[sp].pi;
        ti := stack[sp].ti;
        WHILE (ti < tlen) AND (text[ti] # '/') DO
          INC(ti)
        END;
        IF (ti < tlen) AND (text[ti] = '/') THEN
          INC(ti);
          IF sp < MaxStack THEN
            stack[sp].pi := pi;
            stack[sp].ti := ti;
            INC(sp)
          END
        END
      ELSE
        RETURN FALSE
      END

    ELSIF (ti >= tlen) AND (pi < plen) THEN
      (* Text exhausted but pattern remains: only ok if rest is * or ** *)
      IF pattern[pi] = '*' THEN
        INC(pi)
      ELSE
        IF sp > 0 THEN
          DEC(sp);
          pi := stack[sp].pi;
          ti := stack[sp].ti;
          WHILE (ti < tlen) AND (text[ti] # '/') DO
            INC(ti)
          END;
          IF (ti < tlen) AND (text[ti] = '/') THEN
            INC(ti);
            IF sp < MaxStack THEN
              stack[sp].pi := pi;
              stack[sp].ti := ti;
              INC(sp)
            END
          END
        ELSE
          RETURN FALSE
        END
      END

    ELSE
      RETURN FALSE
    END
  END (* LOOP *)
END Match;

(* ── Utility procedures ──────────────────────────────── *)

PROCEDURE IsNegated(pattern: ARRAY OF CHAR): BOOLEAN;
BEGIN
  IF (HIGH(pattern) >= 0) AND (StrLen(pattern) > 0) THEN
    RETURN pattern[0] = '!'
  END;
  RETURN FALSE
END IsNegated;

PROCEDURE IsAnchored(pattern: ARRAY OF CHAR): BOOLEAN;
BEGIN
  IF (HIGH(pattern) >= 0) AND (StrLen(pattern) > 0) THEN
    RETURN pattern[0] = '/'
  END;
  RETURN FALSE
END IsAnchored;

PROCEDURE HasPathSep(pattern: ARRAY OF CHAR): BOOLEAN;
VAR i, len: CARDINAL;
BEGIN
  len := StrLen(pattern);
  i := 0;
  WHILE i < len DO
    IF pattern[i] = '/' THEN
      RETURN TRUE
    END;
    INC(i)
  END;
  RETURN FALSE
END HasPathSep;

PROCEDURE StripNegation(pattern: ARRAY OF CHAR;
                        VAR out: ARRAY OF CHAR);
VAR i, j, len: CARDINAL;
BEGIN
  len := StrLen(pattern);
  IF (len > 0) AND (pattern[0] = '!') THEN
    j := 0;
    i := 1;
    WHILE (i < len) AND (j <= HIGH(out)) DO
      out[j] := pattern[i];
      INC(i);
      INC(j)
    END;
    IF j <= HIGH(out) THEN
      out[j] := 0C
    END
  ELSE
    Assign(pattern, out)
  END
END StripNegation;

PROCEDURE StripAnchor(pattern: ARRAY OF CHAR;
                      VAR out: ARRAY OF CHAR);
VAR i, j, len: CARDINAL;
BEGIN
  len := StrLen(pattern);
  IF (len > 0) AND (pattern[0] = '/') THEN
    j := 0;
    i := 1;
    WHILE (i < len) AND (j <= HIGH(out)) DO
      out[j] := pattern[i];
      INC(i);
      INC(j)
    END;
    IF j <= HIGH(out) THEN
      out[j] := 0C
    END
  ELSE
    Assign(pattern, out)
  END
END StripAnchor;

END Glob.
