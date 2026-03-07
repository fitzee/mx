IMPLEMENTATION MODULE Glob;

FROM SYSTEM IMPORT ADDRESS, ADR;
FROM Strings IMPORT Assign, Length;
IMPORT Sys;

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

(* ── Directory walking ───────────────────────────────── *)

CONST
  MaxPath = 1024;
  MaxDirBuf = 8192;
  MaxSegments = 32;
  MaxRecurse = 32;

(* ── Path helper: append component to path with / separator ── *)

PROCEDURE PathAppend(VAR base: ARRAY OF CHAR;
                     VAR baseLen: CARDINAL;
                     VAR comp: ARRAY OF CHAR;
                     compLen: CARDINAL;
                     maxLen: CARDINAL);
VAR i: CARDINAL;
BEGIN
  IF (baseLen > 0) AND (baseLen < maxLen) THEN
    base[baseLen] := '/';
    INC(baseLen)
  END;
  i := 0;
  WHILE (i < compLen) AND (baseLen < maxLen) DO
    base[baseLen] := comp[i];
    INC(baseLen);
    INC(i)
  END;
  IF baseLen <= HIGH(base) THEN
    base[baseLen] := 0C
  END
END PathAppend;

(* ── Copy null-terminated string ─────────────────────── *)

PROCEDURE CopyStr(VAR src: ARRAY OF CHAR; srcLen: CARDINAL;
                  VAR dst: ARRAY OF CHAR; VAR dstLen: CARDINAL);
VAR i: CARDINAL;
BEGIN
  i := 0;
  WHILE (i < srcLen) AND (i <= HIGH(dst)) DO
    dst[i] := src[i];
    INC(i)
  END;
  dstLen := i;
  IF i <= HIGH(dst) THEN
    dst[i] := 0C
  END
END CopyStr;

(* ── Check if segment is ** ──────────────────────────── *)

PROCEDURE IsDoubleStar(VAR seg: ARRAY OF CHAR; segLen: CARDINAL): BOOLEAN;
BEGIN
  RETURN (segLen = 2) AND (seg[0] = '*') AND (seg[1] = '*')
END IsDoubleStar;

(* ── Split pattern into segments by '/' ──────────────── *)

TYPE
  Segment = RECORD
    start: CARDINAL;
    len:   CARDINAL;
  END;

PROCEDURE SplitPattern(VAR pat: ARRAY OF CHAR; patLen: CARDINAL;
                       VAR segs: ARRAY OF Segment;
                       VAR nSegs: CARDINAL);
VAR i, segStart: CARDINAL;
BEGIN
  nSegs := 0;
  IF patLen = 0 THEN RETURN END;
  segStart := 0;
  i := 0;
  WHILE i < patLen DO
    IF pat[i] = '/' THEN
      IF (nSegs < MaxSegments) AND (i > segStart) THEN
        segs[nSegs].start := segStart;
        segs[nSegs].len := i - segStart;
        INC(nSegs)
      END;
      segStart := i + 1
    END;
    INC(i)
  END;
  (* Last segment *)
  IF (segStart <= patLen) AND (nSegs < MaxSegments) THEN
    segs[nSegs].start := segStart;
    segs[nSegs].len := patLen - segStart;
    IF segs[nSegs].len > 0 THEN
      INC(nSegs)
    END
  END
END SplitPattern;

(* ── Extract a segment as a null-terminated string ───── *)

PROCEDURE ExtractSeg(VAR pat: ARRAY OF CHAR;
                     VAR seg: Segment;
                     VAR out: ARRAY OF CHAR;
                     VAR outLen: CARDINAL);
VAR i: CARDINAL;
BEGIN
  i := 0;
  WHILE (i < seg.len) AND (i <= HIGH(out)) DO
    out[i] := pat[seg.start + i];
    INC(i)
  END;
  outLen := i;
  IF i <= HIGH(out) THEN
    out[i] := 0C
  END
END ExtractSeg;

(* ── Parse directory listing buffer (newline-separated) ── *)

PROCEDURE NextEntry(VAR buf: ARRAY OF CHAR; bufLen: CARDINAL;
                    VAR pos: CARDINAL;
                    VAR name: ARRAY OF CHAR;
                    VAR nameLen: CARDINAL): BOOLEAN;
VAR start, i: CARDINAL;
BEGIN
  IF pos >= bufLen THEN RETURN FALSE END;
  start := pos;
  WHILE (pos < bufLen) AND (buf[pos] # 12C) DO  (* 12C = newline *)
    INC(pos)
  END;
  nameLen := pos - start;
  IF nameLen = 0 THEN
    IF pos < bufLen THEN INC(pos) END;
    RETURN FALSE
  END;
  i := 0;
  WHILE (i < nameLen) AND (i <= HIGH(name)) DO
    name[i] := buf[start + i];
    INC(i)
  END;
  IF i <= HIGH(name) THEN
    name[i] := 0C
  END;
  IF pos < bufLen THEN INC(pos) END;  (* skip newline *)
  RETURN TRUE
END NextEntry;

(* ── Recursive walk engine ───────────────────────────── *)

PROCEDURE WalkRecurse(VAR basePath: ARRAY OF CHAR;
                      baseLen: CARDINAL;
                      VAR pat: ARRAY OF CHAR;
                      VAR segs: ARRAY OF Segment;
                      nSegs: CARDINAL;
                      segIdx: CARDINAL;
                      callback: MatchProc;
                      ctx: ADDRESS;
                      VAR count: CARDINAL;
                      VAR stopped: BOOLEAN;
                      depth: CARDINAL);
VAR
  dirBuf: ARRAY [0..MaxDirBuf-1] OF CHAR;
  entName: ARRAY [0..255] OF CHAR;
  segStr: ARRAY [0..255] OF CHAR;
  childPath: ARRAY [0..MaxPath-1] OF CHAR;
  entLen, segLen: CARDINAL;
  dirResult: INTEGER;
  dirLen, pos: CARDINAL;
  childLen, i: CARDINAL;
  isDir: BOOLEAN;
  dirPath: ARRAY [0..MaxPath-1] OF CHAR;
BEGIN
  IF stopped THEN RETURN END;
  IF depth > MaxRecurse THEN RETURN END;

  (* If we've consumed all segments, the current basePath is a match *)
  IF segIdx >= nSegs THEN
    IF NOT callback(basePath, ctx) THEN
      stopped := TRUE
    END;
    INC(count);
    RETURN
  END;

  (* Extract current segment *)
  ExtractSeg(pat, segs[segIdx], segStr, segLen);

  IF IsDoubleStar(segStr, segLen) THEN
    (* ** : try matching remaining segments from here (skip **),
       and also recurse into every subdirectory *)

    (* First: try without descending... matches zero directories *)
    WalkRecurse(basePath, baseLen, pat, segs, nSegs, segIdx + 1,
                callback, ctx, count, stopped, depth);
    IF stopped THEN RETURN END;

    (* List directory *)
    (* Build null-terminated directory path *)
    IF baseLen = 0 THEN
      dirPath[0] := '.';
      dirPath[1] := 0C
    ELSE
      i := 0;
      WHILE i < baseLen DO
        dirPath[i] := basePath[i];
        INC(i)
      END;
      dirPath[baseLen] := 0C
    END;

    dirResult := Sys.m2sys_list_dir(ADR(dirPath), ADR(dirBuf), MaxDirBuf);
    IF dirResult <= 0 THEN RETURN END;
    dirLen := CARDINAL(dirResult);

    pos := 0;
    WHILE NextEntry(dirBuf, dirLen, pos, entName, entLen) AND
          (NOT stopped) DO
      (* Build child path *)
      childLen := 0;
      IF baseLen > 0 THEN
        i := 0;
        WHILE i < baseLen DO
          childPath[i] := basePath[i];
          INC(i)
        END;
        childLen := baseLen
      END;
      PathAppend(childPath, childLen, entName, entLen, MaxPath - 1);

      (* Check if directory *)
      isDir := Sys.m2sys_is_dir(ADR(childPath)) = 1;

      IF isDir THEN
        (* Recurse with same segIdx -- ** can match more levels *)
        WalkRecurse(childPath, childLen, pat, segs, nSegs, segIdx,
                    callback, ctx, count, stopped, depth + 1)
      ELSE
        (* Try matching remaining segments against this file *)
        IF segIdx + 1 >= nSegs THEN
          (* ** at end matches everything *)
          IF NOT callback(childPath, ctx) THEN
            stopped := TRUE
          END;
          INC(count)
        ELSIF segIdx + 1 = nSegs - 1 THEN
          (* One more segment after **: match filename *)
          ExtractSeg(pat, segs[segIdx + 1], segStr, segLen);
          IF Match(segStr, entName) THEN
            IF NOT callback(childPath, ctx) THEN
              stopped := TRUE
            END;
            INC(count)
          END
        END
      END
    END

  ELSE
    (* Normal segment: list directory and match entries *)
    IF baseLen = 0 THEN
      dirPath[0] := '.';
      dirPath[1] := 0C
    ELSE
      i := 0;
      WHILE i < baseLen DO
        dirPath[i] := basePath[i];
        INC(i)
      END;
      dirPath[baseLen] := 0C
    END;

    dirResult := Sys.m2sys_list_dir(ADR(dirPath), ADR(dirBuf), MaxDirBuf);
    IF dirResult <= 0 THEN RETURN END;
    dirLen := CARDINAL(dirResult);

    pos := 0;
    WHILE NextEntry(dirBuf, dirLen, pos, entName, entLen) AND
          (NOT stopped) DO
      IF Match(segStr, entName) THEN
        (* Build child path *)
        childLen := 0;
        IF baseLen > 0 THEN
          i := 0;
          WHILE i < baseLen DO
            childPath[i] := basePath[i];
            INC(i)
          END;
          childLen := baseLen
        END;
        PathAppend(childPath, childLen, entName, entLen, MaxPath - 1);

        IF segIdx + 1 >= nSegs THEN
          (* Last segment: this is a match *)
          IF NOT callback(childPath, ctx) THEN
            stopped := TRUE
          END;
          INC(count)
        ELSE
          (* More segments: must be a directory to continue *)
          isDir := Sys.m2sys_is_dir(ADR(childPath)) = 1;
          IF isDir THEN
            WalkRecurse(childPath, childLen, pat, segs, nSegs,
                        segIdx + 1, callback, ctx, count, stopped,
                        depth + 1)
          END
        END
      END
    END
  END
END WalkRecurse;

(* ── Public Walk ─────────────────────────────────────── *)

PROCEDURE Walk(pattern: ARRAY OF CHAR;
               callback: MatchProc;
               ctx: ADDRESS): CARDINAL;
VAR
  segs: ARRAY [0..MaxSegments-1] OF Segment;
  nSegs, patLen: CARDINAL;
  count: CARDINAL;
  stopped: BOOLEAN;
  basePath: ARRAY [0..MaxPath-1] OF CHAR;
BEGIN
  patLen := StrLen(pattern);
  IF patLen = 0 THEN RETURN 0 END;

  SplitPattern(pattern, patLen, segs, nSegs);
  IF nSegs = 0 THEN RETURN 0 END;

  count := 0;
  stopped := FALSE;
  basePath[0] := 0C;

  WalkRecurse(basePath, 0, pattern, segs, nSegs, 0,
              callback, ctx, count, stopped, 0);

  RETURN count
END Walk;

END Glob.
