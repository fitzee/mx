IMPLEMENTATION MODULE LLDBDriver;
FROM SYSTEM IMPORT ADR;
FROM Process IMPORT Spawn, SendLine, ReadUntilPrompt, Kill, IsRunning;
FROM Strings IMPORT Assign, Concat, Length;

CONST
  MaxResp = 32768;

VAR
  resp: ARRAY [0..MaxResp-1] OF CHAR;
  respLen: CARDINAL;

  (* Last stop info, parsed from lldb output *)
  lastReason: ARRAY [0..31] OF CHAR;
  lastThread: INTEGER;
  lastFile: ARRAY [0..255] OF CHAR;
  lastLine: INTEGER;
  sessionActive: BOOLEAN;

(* ── Helpers ────────────────────────────────────── *)

PROCEDURE IntToStr(n: INTEGER; VAR s: ARRAY OF CHAR);
VAR
  digits: ARRAY [0..15] OF CHAR;
  nd, i: CARDINAL;
  val: CARDINAL;
BEGIN
  IF n < 0 THEN
    s[0] := '-';
    val := VAL(CARDINAL, -n);
    nd := 0;
    REPEAT
      digits[nd] := CHR(VAL(CARDINAL, ORD('0')) + (val MOD 10));
      INC(nd);
      val := val DIV 10
    UNTIL val = 0;
    i := 0;
    WHILE i < nd DO
      s[i+1] := digits[nd - 1 - i];
      INC(i)
    END;
    s[nd+1] := CHR(0)
  ELSE
    val := VAL(CARDINAL, n);
    nd := 0;
    REPEAT
      digits[nd] := CHR(VAL(CARDINAL, ORD('0')) + (val MOD 10));
      INC(nd);
      val := val DIV 10
    UNTIL val = 0;
    i := 0;
    WHILE i < nd DO
      s[i] := digits[nd - 1 - i];
      INC(i)
    END;
    s[nd] := CHR(0)
  END
END IntToStr;

PROCEDURE SendCmd(VAR cmd: ARRAY OF CHAR): BOOLEAN;
BEGIN
  SendLine(cmd);
  RETURN ReadUntilPrompt(resp, respLen)
END SendCmd;

PROCEDURE RespMatchAt(pos: CARDINAL; VAR s: ARRAY OF CHAR): BOOLEAN;
VAR j: CARDINAL;
BEGIN
  j := 0;
  WHILE (j <= HIGH(s)) AND (s[j] # CHR(0)) DO
    IF (pos + j >= respLen) OR (resp[pos+j] # s[j]) THEN
      RETURN FALSE
    END;
    INC(j)
  END;
  RETURN TRUE
END RespMatchAt;

PROCEDURE RespExtractWord(VAR pos: CARDINAL; VAR dst: ARRAY OF CHAR);
VAR j: CARDINAL;
BEGIN
  j := 0;
  WHILE (pos < respLen) AND (resp[pos] # ' ') AND
        (resp[pos] # CHR(10)) AND (resp[pos] # CHR(13)) AND
        (j < HIGH(dst)) DO
    dst[j] := resp[pos];
    INC(pos); INC(j)
  END;
  dst[j] := CHR(0)
END RespExtractWord;

PROCEDURE ParseStopOutput;
VAR
  i, start, j: CARDINAL;
  srTag: ARRAY [0..15] OF CHAR;
  atTag: ARRAY [0..3] OF CHAR;
BEGIN
  lastReason[0] := CHR(0);
  lastThread := 1;
  lastFile[0] := CHR(0);
  lastLine := 0;
  srTag := "stop reason = ";
  atTag := " at ";

  i := 0;

  (* Find "thread #N" *)
  WHILE i < respLen DO
    IF (resp[i] = 't') AND (i + 9 < respLen) THEN
      IF (resp[i+1] = 'h') AND (resp[i+2] = 'r') AND
         (resp[i+3] = 'e') AND (resp[i+4] = 'a') AND
         (resp[i+5] = 'd') AND (resp[i+6] = ' ') AND
         (resp[i+7] = '#') THEN
        i := i + 8;
        lastThread := 0;
        WHILE (i < respLen) AND (resp[i] >= '0') AND (resp[i] <= '9') DO
          lastThread := lastThread * 10 +
                        (ORD(resp[i]) - ORD('0'));
          INC(i)
        END
      END
    END;
    INC(i)
  END;

  (* Find "stop reason = " *)
  i := 0;
  WHILE i + 14 < respLen DO
    IF RespMatchAt(i, srTag) THEN
      i := i + 14;
      RespExtractWord(i, lastReason);
      i := respLen
    ELSE
      INC(i)
    END
  END;

  (* Find " at File.mod:NN" *)
  i := 0;
  WHILE i + 4 < respLen DO
    IF RespMatchAt(i, atTag) THEN
      i := i + 4;
      start := i;
      WHILE (i < respLen) AND (resp[i] # ':') AND
            (resp[i] # CHR(10)) DO
        INC(i)
      END;
      IF i > start THEN
        j := 0;
        WHILE (start + j < i) AND (j < HIGH(lastFile)) DO
          lastFile[j] := resp[start + j];
          INC(j)
        END;
        lastFile[j] := CHR(0)
      END;
      IF (i < respLen) AND (resp[i] = ':') THEN
        INC(i);
        lastLine := 0;
        WHILE (i < respLen) AND (resp[i] >= '0') AND (resp[i] <= '9') DO
          lastLine := lastLine * 10 + (ORD(resp[i]) - ORD('0'));
          INC(i)
        END
      END;
      i := respLen
    ELSE
      INC(i)
    END
  END
END ParseStopOutput;

PROCEDURE ParseBreakpointId(): INTEGER;
(* Parse "Breakpoint N:" from resp to get breakpoint ID. *)
VAR
  i: CARDINAL;
  id: INTEGER;
  bpTag: ARRAY [0..11] OF CHAR;
BEGIN
  bpTag := "Breakpoint ";
  i := 0;
  WHILE i + 11 < respLen DO
    IF (resp[i] = 'B') AND (resp[i+1] = 'r') AND (resp[i+2] = 'e') AND
       (resp[i+3] = 'a') AND (resp[i+4] = 'k') AND (resp[i+5] = 'p') AND
       (resp[i+6] = 'o') AND (resp[i+7] = 'i') AND (resp[i+8] = 'n') AND
       (resp[i+9] = 't') AND (resp[i+10] = ' ') THEN
      i := i + 11;
      id := 0;
      WHILE (i < respLen) AND (resp[i] >= '0') AND (resp[i] <= '9') DO
        id := id * 10 + (ORD(resp[i]) - ORD('0'));
        INC(i)
      END;
      RETURN id
    END;
    INC(i)
  END;
  RETURN -1
END ParseBreakpointId;

(* ── Public API ─────────────────────────────────── *)

PROCEDURE StartSession(VAR targetPath: ARRAY OF CHAR): BOOLEAN;
VAR
  cmd: ARRAY [0..511] OF CHAR;
BEGIN
  IF NOT Spawn() THEN RETURN FALSE END;

  (* Use full paths in backtraces so DAP clients can open source files *)
  cmd := 'settings set frame-format "frame #${frame.index}: ${frame.pc}{ ${module.file.basename}`${function.name-with-args}{${function.pc-offset}}}{ at ${line.file.fullpath}:${line.number}}\n"';
  IF NOT SendCmd(cmd) THEN RETURN FALSE END;

  Assign("target create ", cmd);
  Concat(cmd, targetPath, cmd);
  IF NOT SendCmd(cmd) THEN RETURN FALSE END;
  sessionActive := TRUE;
  RETURN TRUE
END StartSession;

PROCEDURE SetBreakpoint(VAR file: ARRAY OF CHAR;
                        line: INTEGER): INTEGER;
VAR
  cmd: ARRAY [0..511] OF CHAR;
  lineStr: ARRAY [0..15] OF CHAR;
BEGIN
  Assign("breakpoint set --file ", cmd);
  Concat(cmd, file, cmd);
  Concat(cmd, " --line ", cmd);
  IntToStr(line, lineStr);
  Concat(cmd, lineStr, cmd);
  IF NOT SendCmd(cmd) THEN RETURN -1 END;
  RETURN ParseBreakpointId()
END SetBreakpoint;

PROCEDURE RemoveBreakpoint(id: INTEGER);
VAR
  cmd: ARRAY [0..63] OF CHAR;
  idStr: ARRAY [0..15] OF CHAR;
BEGIN
  Assign("breakpoint delete ", cmd);
  IntToStr(id, idStr);
  Concat(cmd, idStr, cmd);
  IF SendCmd(cmd) THEN END
END RemoveBreakpoint;

PROCEDURE Launch(VAR args: ARRAY OF CHAR;
                 stopOnEntry: BOOLEAN): BOOLEAN;
VAR
  cmd: ARRAY [0..511] OF CHAR;
BEGIN
  IF stopOnEntry THEN
    Assign("process launch --stop-at-entry", cmd)
  ELSE
    Assign("process launch", cmd)
  END;
  IF (args[0] # CHR(0)) THEN
    Concat(cmd, " -- ", cmd);
    Concat(cmd, args, cmd)
  END;
  IF NOT SendCmd(cmd) THEN RETURN FALSE END;
  ParseStopOutput;
  RETURN TRUE
END Launch;

PROCEDURE Continue(): BOOLEAN;
VAR cmd: ARRAY [0..31] OF CHAR;
BEGIN
  cmd := "process continue";
  IF NOT SendCmd(cmd) THEN RETURN FALSE END;
  ParseStopOutput;
  RETURN TRUE
END Continue;

PROCEDURE StepOver(): BOOLEAN;
VAR cmd: ARRAY [0..31] OF CHAR;
BEGIN
  cmd := "thread step-over";
  IF NOT SendCmd(cmd) THEN RETURN FALSE END;
  ParseStopOutput;
  RETURN TRUE
END StepOver;

PROCEDURE StepIn(): BOOLEAN;
VAR cmd: ARRAY [0..31] OF CHAR;
BEGIN
  cmd := "thread step-in";
  IF NOT SendCmd(cmd) THEN RETURN FALSE END;
  ParseStopOutput;
  RETURN TRUE
END StepIn;

PROCEDURE StepOut(): BOOLEAN;
VAR cmd: ARRAY [0..31] OF CHAR;
BEGIN
  cmd := "thread step-out";
  IF NOT SendCmd(cmd) THEN RETURN FALSE END;
  ParseStopOutput;
  RETURN TRUE
END StepOut;

PROCEDURE Pause(): BOOLEAN;
VAR cmd: ARRAY [0..31] OF CHAR;
BEGIN
  cmd := "process interrupt";
  IF NOT SendCmd(cmd) THEN RETURN FALSE END;
  ParseStopOutput;
  RETURN TRUE
END Pause;

PROCEDURE GetStopInfo(VAR reason: ARRAY OF CHAR;
                      VAR threadId: INTEGER;
                      VAR file: ARRAY OF CHAR;
                      VAR line: INTEGER);
BEGIN
  Assign(lastReason, reason);
  threadId := lastThread;
  Assign(lastFile, file);
  line := lastLine
END GetStopInfo;

PROCEDURE GetThreads(VAR buf: ARRAY OF CHAR;
                     VAR len: CARDINAL): INTEGER;
VAR
  cmd: ARRAY [0..31] OF CHAR;
  count, i: INTEGER;
BEGIN
  cmd := "thread list";
  IF NOT SendCmd(cmd) THEN
    len := 0;
    RETURN 0
  END;
  (* Copy response to caller *)
  i := 0;
  WHILE (VAL(CARDINAL, i) < respLen) AND (i <= VAL(INTEGER, HIGH(buf))) DO
    buf[i] := resp[i];
    INC(i)
  END;
  len := VAL(CARDINAL, i);
  IF VAL(CARDINAL, i) <= HIGH(buf) THEN buf[i] := CHR(0) END;

  (* Count threads — each "thread #" occurrence *)
  count := 0;
  i := 0;
  WHILE VAL(CARDINAL, i) + 8 < respLen DO
    IF (resp[i] = 't') AND (resp[i+1] = 'h') AND (resp[i+2] = 'r') AND
       (resp[i+3] = 'e') AND (resp[i+4] = 'a') AND (resp[i+5] = 'd') AND
       (resp[i+6] = ' ') AND (resp[i+7] = '#') THEN
      INC(count)
    END;
    INC(i)
  END;
  RETURN count
END GetThreads;

PROCEDURE GetBacktrace(VAR buf: ARRAY OF CHAR;
                       VAR len: CARDINAL);
VAR
  cmd: ARRAY [0..31] OF CHAR;
  i: INTEGER;
BEGIN
  cmd := "thread backtrace";
  IF NOT SendCmd(cmd) THEN
    len := 0;
    RETURN
  END;
  i := 0;
  WHILE (VAL(CARDINAL, i) < respLen) AND (i <= VAL(INTEGER, HIGH(buf))) DO
    buf[i] := resp[i];
    INC(i)
  END;
  len := VAL(CARDINAL, i);
  IF VAL(CARDINAL, i) <= HIGH(buf) THEN buf[i] := CHR(0) END
END GetBacktrace;

PROCEDURE GetFrameVars(frameIdx: INTEGER;
                       VAR buf: ARRAY OF CHAR;
                       VAR len: CARDINAL);
VAR
  cmd: ARRAY [0..63] OF CHAR;
  idxStr: ARRAY [0..15] OF CHAR;
  i: INTEGER;
BEGIN
  (* Select frame first *)
  Assign("frame select ", cmd);
  IntToStr(frameIdx, idxStr);
  Concat(cmd, idxStr, cmd);
  IF NOT SendCmd(cmd) THEN
    len := 0;
    RETURN
  END;

  (* Then get variables *)
  cmd := "frame variable";
  IF NOT SendCmd(cmd) THEN
    len := 0;
    RETURN
  END;
  i := 0;
  WHILE (VAL(CARDINAL, i) < respLen) AND (i <= VAL(INTEGER, HIGH(buf))) DO
    buf[i] := resp[i];
    INC(i)
  END;
  len := VAL(CARDINAL, i);
  IF VAL(CARDINAL, i) <= HIGH(buf) THEN buf[i] := CHR(0) END
END GetFrameVars;

PROCEDURE GetVarTypes(VAR buf: ARRAY OF CHAR;
                      VAR len: CARDINAL);
VAR
  cmd: ARRAY [0..63] OF CHAR;
  i: INTEGER;
BEGIN
  cmd := "image lookup -a $pc -v";
  IF NOT SendCmd(cmd) THEN
    len := 0;
    RETURN
  END;
  i := 0;
  WHILE (VAL(CARDINAL, i) < respLen) AND (i <= VAL(INTEGER, HIGH(buf))) DO
    buf[i] := resp[i];
    INC(i)
  END;
  len := VAL(CARDINAL, i);
  IF VAL(CARDINAL, i) <= HIGH(buf) THEN buf[i] := CHR(0) END
END GetVarTypes;

PROCEDURE EndSession;
VAR cmd: ARRAY [0..31] OF CHAR;
BEGIN
  IF sessionActive THEN
    IF IsRunning() THEN
      cmd := "quit";
      SendLine(cmd);
      Kill
    END;
    sessionActive := FALSE
  END
END EndSession;

BEGIN
  sessionActive := FALSE;
  lastReason[0] := CHR(0);
  lastThread := 1;
  lastFile[0] := CHR(0);
  lastLine := 0;
  respLen := 0
END LLDBDriver.
