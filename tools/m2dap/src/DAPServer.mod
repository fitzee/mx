IMPLEMENTATION MODULE DAPServer;
FROM SYSTEM IMPORT ADR, ADDRESS;
FROM Json IMPORT Parser, Token, TokenKind,
                  Init, Next, GetString, GetInteger, Skip;
FROM Fmt IMPORT Buf, InitBuf, BufLen, BufClear,
                JsonStart, JsonEnd, JsonKey, JsonStr, JsonInt, JsonBool,
                JsonArrayStart, JsonArrayEnd, JsonNull;
FROM DAPTransport IMPORT WriteMessage;
FROM Strings IMPORT CompareStr, Assign, Length;
FROM LLDBDriver IMPORT StartSession, SetBreakpoint, RemoveBreakpoint,
                        Launch, Continue, StepOver, StepIn, StepOut,
                        Pause, GetStopInfo, GetThreads, GetBacktrace,
                        GetFrameVars, GetVarTypes, EndSession;
FROM Vars IMPORT Reset, AllocScopeRef, GetRefInfo,
                  ScopeLocals, ScopeArgs;
FROM M2Format IMPORT FormatValue, FormatType, Demangle;

CONST
  MaxBody = 65536;
  MaxBP   = 256;
  MaxVT   = 64;

TYPE
  BreakpointEntry = RECORD
    file: ARRAY [0..255] OF CHAR;
    line: INTEGER;
    lldbId: INTEGER;
  END;

  VarTypeEntry = RECORD
    name: ARRAY [0..63] OF CHAR;
    dwarfType: ARRAY [0..63] OF CHAR;
  END;

VAR
  outBacking: ARRAY [0..MaxBody-1] OF CHAR;
  outBuf: Buf;
  seq: INTEGER;
  responseSeq: INTEGER;

  (* Breakpoint state *)
  bps: ARRAY [0..MaxBP-1] OF BreakpointEntry;
  bpCount: INTEGER;

  (* Session state *)
  targetPath: ARRAY [0..511] OF CHAR;
  launched: BOOLEAN;
  stopOnEntry: BOOLEAN;

  (* DWARF variable type map — rebuilt on each frame *)
  varTypes: ARRAY [0..MaxVT-1] OF VarTypeEntry;
  varTypeCount: INTEGER;

  (* Shared parse state for backtrace/variable parsing *)
  parseBuf: ARRAY [0..8191] OF CHAR;
  parseLen: CARDINAL;
  parsePos: CARDINAL;

(* ── JSON helpers ──────────────────────────────── *)

PROCEDURE StrEq(VAR a, b: ARRAY OF CHAR): BOOLEAN;
BEGIN
  RETURN CompareStr(a, b) = 0
END StrEq;

PROCEDURE FindField(VAR p: Parser; VAR name: ARRAY OF CHAR): BOOLEAN;
VAR
  tok: Token;
  key: ARRAY [0..63] OF CHAR;
BEGIN
  LOOP
    IF NOT Next(p, tok) THEN RETURN FALSE END;
    IF tok.kind = JObjectEnd THEN RETURN FALSE END;
    IF tok.kind = JString THEN
      IF NOT GetString(p, tok, key) THEN RETURN FALSE END;
      IF NOT Next(p, tok) THEN RETURN FALSE END;
      IF tok.kind # JColon THEN RETURN FALSE END;
      IF StrEq(key, name) THEN
        RETURN TRUE
      ELSE
        Skip(p)
      END
    ELSIF tok.kind = JComma THEN
      (* skip *)
    ELSE
      RETURN FALSE
    END
  END
END FindField;

(* ── Response building ──────────────────────────── *)

PROCEDURE BeginResponse(command: ARRAY OF CHAR; success: BOOLEAN);
BEGIN
  INC(responseSeq);
  BufClear(outBuf);
  JsonStart(outBuf);
  JsonKey(outBuf, "seq");
  JsonInt(outBuf, responseSeq);
  JsonKey(outBuf, "type");
  JsonStr(outBuf, "response");
  JsonKey(outBuf, "request_seq");
  JsonInt(outBuf, seq);
  JsonKey(outBuf, "success");
  JsonBool(outBuf, success);
  JsonKey(outBuf, "command");
  JsonStr(outBuf, command)
END BeginResponse;

PROCEDURE EndAndSend;
VAR len: CARDINAL;
BEGIN
  JsonEnd(outBuf);
  len := BufLen(outBuf);
  WriteMessage(outBacking, len)
END EndAndSend;

PROCEDURE SendEvent(event: ARRAY OF CHAR);
BEGIN
  INC(responseSeq);
  BufClear(outBuf);
  JsonStart(outBuf);
  JsonKey(outBuf, "seq");
  JsonInt(outBuf, responseSeq);
  JsonKey(outBuf, "type");
  JsonStr(outBuf, "event");
  JsonKey(outBuf, "event");
  JsonStr(outBuf, event);
  JsonEnd(outBuf);
  WriteMessage(outBacking, BufLen(outBuf))
END SendEvent;

PROCEDURE SendStoppedEvent;
VAR
  reason: ARRAY [0..31] OF CHAR;
  file: ARRAY [0..255] OF CHAR;
  threadId, line: INTEGER;
BEGIN
  GetStopInfo(reason, threadId, file, line);
  INC(responseSeq);
  BufClear(outBuf);
  JsonStart(outBuf);
  JsonKey(outBuf, "seq");
  JsonInt(outBuf, responseSeq);
  JsonKey(outBuf, "type");
  JsonStr(outBuf, "event");
  JsonKey(outBuf, "event");
  JsonStr(outBuf, "stopped");
  JsonKey(outBuf, "body");
  JsonStart(outBuf);
    JsonKey(outBuf, "reason");
    JsonStr(outBuf, reason);
    JsonKey(outBuf, "threadId");
    JsonInt(outBuf, threadId);
    JsonKey(outBuf, "allThreadsStopped");
    JsonBool(outBuf, TRUE);
  JsonEnd(outBuf);
  JsonEnd(outBuf);
  WriteMessage(outBacking, BufLen(outBuf));

  (* Reset variable references on each stop *)
  Reset
END SendStoppedEvent;

(* ── Command handlers ──────────────────────────── *)

PROCEDURE HandleInitialize;
BEGIN
  BeginResponse("initialize", TRUE);
  JsonKey(outBuf, "body");
  JsonStart(outBuf);
    JsonKey(outBuf, "supportsConfigurationDoneRequest");
    JsonBool(outBuf, TRUE);
    JsonKey(outBuf, "supportsFunctionBreakpoints");
    JsonBool(outBuf, FALSE);
    JsonKey(outBuf, "supportsConditionalBreakpoints");
    JsonBool(outBuf, FALSE);
    JsonKey(outBuf, "supportsEvaluateForHovers");
    JsonBool(outBuf, FALSE);
    JsonKey(outBuf, "supportsStepBack");
    JsonBool(outBuf, FALSE);
    JsonKey(outBuf, "supportsSetVariable");
    JsonBool(outBuf, FALSE);
    JsonKey(outBuf, "supportsRestartFrame");
    JsonBool(outBuf, FALSE);
    JsonKey(outBuf, "supportsSteppingGranularity");
    JsonBool(outBuf, FALSE);
  JsonEnd(outBuf);
  EndAndSend;
  SendEvent("initialized")
END HandleInitialize;

PROCEDURE HandleDisconnect(): BOOLEAN;
BEGIN
  IF launched THEN
    EndSession;
    launched := FALSE
  END;
  BeginResponse("disconnect", TRUE);
  EndAndSend;
  RETURN FALSE
END HandleDisconnect;

PROCEDURE HandleConfigurationDone;
BEGIN
  BeginResponse("configurationDone", TRUE);
  EndAndSend;
  (* If not stopOnEntry, continue after configuration *)
  IF launched AND (NOT stopOnEntry) THEN
    IF Continue() THEN
      SendStoppedEvent
    END
  END
END HandleConfigurationDone;

PROCEDURE HandleLaunch(VAR buf: ARRAY OF CHAR; len: CARDINAL);
VAR
  p: Parser;
  tok: Token;
  fieldName: ARRAY [0..63] OF CHAR;
  program: ARRAY [0..511] OF CHAR;
  args: ARRAY [0..511] OF CHAR;
  soe: BOOLEAN;
BEGIN
  program[0] := CHR(0);
  args[0] := CHR(0);
  soe := FALSE;

  (* Re-parse to find arguments *)
  Init(p, ADR(buf), len);
  IF NOT Next(p, tok) THEN RETURN END;  (* { *)

  (* Find "arguments" object *)
  LOOP
    IF NOT Next(p, tok) THEN RETURN END;
    IF tok.kind = JObjectEnd THEN RETURN END;
    IF tok.kind = JComma THEN (* skip *) END;
    IF tok.kind = JString THEN
      IF NOT GetString(p, tok, fieldName) THEN RETURN END;
      IF NOT Next(p, tok) THEN RETURN END;  (* : *)
      IF StrEq(fieldName, "arguments") THEN
        (* Parse arguments object *)
        IF NOT Next(p, tok) THEN RETURN END;
        IF tok.kind # JObjectStart THEN RETURN END;
        LOOP
          IF NOT Next(p, tok) THEN EXIT END;
          IF tok.kind = JObjectEnd THEN EXIT END;
          IF tok.kind = JComma THEN (* skip *) END;
          IF tok.kind = JString THEN
            IF NOT GetString(p, tok, fieldName) THEN EXIT END;
            IF NOT Next(p, tok) THEN EXIT END;  (* : *)
            IF StrEq(fieldName, "program") THEN
              IF NOT Next(p, tok) THEN EXIT END;
              IF tok.kind = JString THEN
                IF NOT GetString(p, tok, program) THEN END
              END
            ELSIF StrEq(fieldName, "args") THEN
              IF NOT Next(p, tok) THEN EXIT END;
              IF tok.kind = JString THEN
                IF NOT GetString(p, tok, args) THEN END
              ELSE
                Skip(p)
              END
            ELSIF StrEq(fieldName, "stopOnEntry") THEN
              IF NOT Next(p, tok) THEN EXIT END;
              soe := (tok.kind = JTrue)
            ELSE
              Skip(p)
            END
          END
        END;
        EXIT  (* done with arguments *)
      ELSE
        Skip(p)
      END
    END
  END;

  IF program[0] = CHR(0) THEN
    BeginResponse("launch", FALSE);
    JsonKey(outBuf, "message");
    JsonStr(outBuf, "missing program path");
    EndAndSend;
    RETURN
  END;

  stopOnEntry := soe;

  IF NOT StartSession(program) THEN
    BeginResponse("launch", FALSE);
    JsonKey(outBuf, "message");
    JsonStr(outBuf, "failed to start lldb");
    EndAndSend;
    RETURN
  END;

  IF NOT Launch(args, TRUE) THEN
    BeginResponse("launch", FALSE);
    JsonKey(outBuf, "message");
    JsonStr(outBuf, "failed to launch process");
    EndAndSend;
    RETURN
  END;

  launched := TRUE;
  BeginResponse("launch", TRUE);
  EndAndSend;

  IF soe THEN
    SendStoppedEvent
  END
END HandleLaunch;

PROCEDURE HandleSetBreakpoints(VAR buf: ARRAY OF CHAR; len: CARDINAL);
VAR
  p: Parser;
  tok: Token;
  fieldName: ARRAY [0..63] OF CHAR;
  srcFile: ARRAY [0..255] OF CHAR;
  lines: ARRAY [0..63] OF INTEGER;
  lineCount: INTEGER;
  i, j: INTEGER;
  newId: INTEGER;
BEGIN
  srcFile[0] := CHR(0);
  lineCount := 0;

  (* Parse source.path and breakpoints[].line *)
  Init(p, ADR(buf), len);
  IF NOT Next(p, tok) THEN RETURN END;

  LOOP
    IF NOT Next(p, tok) THEN EXIT END;
    IF tok.kind = JObjectEnd THEN EXIT END;
    IF tok.kind = JComma THEN (* skip *) END;
    IF tok.kind = JString THEN
      IF NOT GetString(p, tok, fieldName) THEN EXIT END;
      IF NOT Next(p, tok) THEN EXIT END;
      IF StrEq(fieldName, "arguments") THEN
        IF NOT Next(p, tok) THEN EXIT END;
        IF tok.kind # JObjectStart THEN EXIT END;
        LOOP
          IF NOT Next(p, tok) THEN EXIT END;
          IF tok.kind = JObjectEnd THEN EXIT END;
          IF tok.kind = JComma THEN (* skip *) END;
          IF tok.kind = JString THEN
            IF NOT GetString(p, tok, fieldName) THEN EXIT END;
            IF NOT Next(p, tok) THEN EXIT END;
            IF StrEq(fieldName, "source") THEN
              IF NOT Next(p, tok) THEN EXIT END;
              IF tok.kind = JObjectStart THEN
                LOOP
                  IF NOT Next(p, tok) THEN EXIT END;
                  IF tok.kind = JObjectEnd THEN EXIT END;
                  IF tok.kind = JComma THEN (* skip *) END;
                  IF tok.kind = JString THEN
                    IF NOT GetString(p, tok, fieldName) THEN EXIT END;
                    IF NOT Next(p, tok) THEN EXIT END;
                    IF StrEq(fieldName, "path") THEN
                      IF NOT Next(p, tok) THEN EXIT END;
                      IF tok.kind = JString THEN
                        IF NOT GetString(p, tok, srcFile) THEN END
                      END
                    ELSE
                      Skip(p)
                    END
                  END
                END
              END
            ELSIF StrEq(fieldName, "breakpoints") THEN
              IF NOT Next(p, tok) THEN EXIT END;
              IF tok.kind = JArrayStart THEN
                LOOP
                  IF NOT Next(p, tok) THEN EXIT END;
                  IF tok.kind = JArrayEnd THEN EXIT END;
                  IF tok.kind = JComma THEN (* skip *) END;
                  IF tok.kind = JObjectStart THEN
                    LOOP
                      IF NOT Next(p, tok) THEN EXIT END;
                      IF tok.kind = JObjectEnd THEN EXIT END;
                      IF tok.kind = JComma THEN (* skip *) END;
                      IF tok.kind = JString THEN
                        IF NOT GetString(p, tok, fieldName) THEN EXIT END;
                        IF NOT Next(p, tok) THEN EXIT END;
                        IF StrEq(fieldName, "line") THEN
                          IF NOT Next(p, tok) THEN EXIT END;
                          IF (tok.kind = JNumber) AND (lineCount < 64) THEN
                            IF GetInteger(p, tok, lines[lineCount]) THEN
                              INC(lineCount)
                            END
                          END
                        ELSE
                          Skip(p)
                        END
                      END
                    END
                  END
                END
              END
            ELSE
              Skip(p)
            END
          END
        END;
        EXIT
      ELSE
        Skip(p)
      END
    END
  END;

  (* Remove existing breakpoints for this file *)
  i := 0;
  WHILE i < bpCount DO
    IF CompareStr(bps[i].file, srcFile) = 0 THEN
      RemoveBreakpoint(bps[i].lldbId);
      (* Shift array *)
      j := i;
      WHILE j < bpCount - 1 DO
        bps[j] := bps[j+1];
        INC(j)
      END;
      DEC(bpCount)
    ELSE
      INC(i)
    END
  END;

  (* Set new breakpoints and build response *)
  BeginResponse("setBreakpoints", TRUE);
  JsonKey(outBuf, "body");
  JsonStart(outBuf);
  JsonKey(outBuf, "breakpoints");
  JsonArrayStart(outBuf);

  i := 0;
  WHILE i < lineCount DO
    newId := SetBreakpoint(srcFile, lines[i]);
    JsonStart(outBuf);
    JsonKey(outBuf, "id");
    JsonInt(outBuf, newId);
    JsonKey(outBuf, "verified");
    JsonBool(outBuf, newId >= 0);
    JsonKey(outBuf, "line");
    JsonInt(outBuf, lines[i]);
    JsonEnd(outBuf);

    IF (newId >= 0) AND (bpCount < MaxBP) THEN
      Assign(srcFile, bps[bpCount].file);
      bps[bpCount].line := lines[i];
      bps[bpCount].lldbId := newId;
      INC(bpCount)
    END;
    INC(i)
  END;

  JsonArrayEnd(outBuf);
  JsonEnd(outBuf);
  EndAndSend
END HandleSetBreakpoints;

PROCEDURE HandleContinue;
BEGIN
  BeginResponse("continue", TRUE);
  JsonKey(outBuf, "body");
  JsonStart(outBuf);
  JsonKey(outBuf, "allThreadsContinued");
  JsonBool(outBuf, TRUE);
  JsonEnd(outBuf);
  EndAndSend;
  IF Continue() THEN
    SendStoppedEvent
  END
END HandleContinue;

PROCEDURE HandleNext;
BEGIN
  BeginResponse("next", TRUE);
  EndAndSend;
  IF StepOver() THEN
    SendStoppedEvent
  END
END HandleNext;

PROCEDURE HandleStepIn;
BEGIN
  BeginResponse("stepIn", TRUE);
  EndAndSend;
  IF StepIn() THEN
    SendStoppedEvent
  END
END HandleStepIn;

PROCEDURE HandleStepOut;
BEGIN
  BeginResponse("stepOut", TRUE);
  EndAndSend;
  IF StepOut() THEN
    SendStoppedEvent
  END
END HandleStepOut;

PROCEDURE HandlePause;
BEGIN
  IF Pause() THEN
    BeginResponse("pause", TRUE);
    EndAndSend;
    SendStoppedEvent
  ELSE
    BeginResponse("pause", FALSE);
    EndAndSend
  END
END HandlePause;

PROCEDURE HandleThreads;
VAR
  tbuf: ARRAY [0..4095] OF CHAR;
  tlen: CARDINAL;
  count: INTEGER;
BEGIN
  BeginResponse("threads", TRUE);
  JsonKey(outBuf, "body");
  JsonStart(outBuf);
  JsonKey(outBuf, "threads");
  JsonArrayStart(outBuf);

  IF launched THEN
    count := GetThreads(tbuf, tlen);
    (* For simplicity, report thread 1 as the main thread.
       Full thread parsing can be expanded later. *)
    JsonStart(outBuf);
    JsonKey(outBuf, "id");
    JsonInt(outBuf, 1);
    JsonKey(outBuf, "name");
    JsonStr(outBuf, "main");
    JsonEnd(outBuf)
  END;

  JsonArrayEnd(outBuf);
  JsonEnd(outBuf);
  EndAndSend
END HandleThreads;

(* ── Parse buffer helpers (operate on parseBuf/parseLen/parsePos) ── *)

PROCEDURE PSkipSpaces;
BEGIN
  WHILE (parsePos < parseLen) AND
        ((parseBuf[parsePos] = ' ') OR (parseBuf[parsePos] = CHR(9))) DO
    INC(parsePos)
  END
END PSkipSpaces;

PROCEDURE PParseInt(): INTEGER;
VAR n: INTEGER;
BEGIN
  n := 0;
  WHILE (parsePos < parseLen) AND
        (parseBuf[parsePos] >= '0') AND (parseBuf[parsePos] <= '9') DO
    n := n * 10 + (ORD(parseBuf[parsePos]) - ORD('0'));
    INC(parsePos)
  END;
  RETURN n
END PParseInt;

PROCEDURE PExtractUntil(stop: CHAR; VAR dst: ARRAY OF CHAR);
VAR j: CARDINAL;
BEGIN
  j := 0;
  WHILE (parsePos < parseLen) AND (parseBuf[parsePos] # stop) AND
        (parseBuf[parsePos] # CHR(10)) AND (j < HIGH(dst)) DO
    dst[j] := parseBuf[parsePos];
    INC(parsePos); INC(j)
  END;
  dst[j] := CHR(0)
END PExtractUntil;

PROCEDURE PExtractParen(VAR dst: ARRAY OF CHAR);
VAR j: CARDINAL;
BEGIN
  j := 0;
  IF (parsePos < parseLen) AND (parseBuf[parsePos] = '(') THEN
    INC(parsePos)
  END;
  WHILE (parsePos < parseLen) AND (parseBuf[parsePos] # ')') AND
        (j < HIGH(dst)) DO
    dst[j] := parseBuf[parsePos];
    INC(parsePos); INC(j)
  END;
  dst[j] := CHR(0);
  IF (parsePos < parseLen) AND (parseBuf[parsePos] = ')') THEN
    INC(parsePos)
  END
END PExtractParen;

PROCEDURE PExtractToEOL(VAR dst: ARRAY OF CHAR);
VAR j: CARDINAL;
BEGIN
  j := 0;
  WHILE (parsePos < parseLen) AND (parseBuf[parsePos] # CHR(10)) AND
        (parseBuf[parsePos] # CHR(13)) AND (j < HIGH(dst)) DO
    dst[j] := parseBuf[parsePos];
    INC(parsePos); INC(j)
  END;
  WHILE (j > 0) AND (dst[j-1] = ' ') DO DEC(j) END;
  dst[j] := CHR(0)
END PExtractToEOL;

(* ── DWARF type map ───────────────────────────── *)
(* Parse "image lookup -a $pc -v" output to extract DWARF type names.
   Lines look like: Variable: ... name = "x", type = "INTEGER", ... *)

PROCEDURE BuildTypeMap(frameIdx: INTEGER);
VAR
  typeBuf: ARRAY [0..8191] OF CHAR;
  typeLen: CARDINAL;
  pos: CARDINAL;
  varName: ARRAY [0..63] OF CHAR;
  varType: ARRAY [0..63] OF CHAR;
  j: CARDINAL;
  nameTag: ARRAY [0..9] OF CHAR;
  typeTag: ARRAY [0..9] OF CHAR;
  varTag: ARRAY [0..10] OF CHAR;
BEGIN
  varTypeCount := 0;
  GetVarTypes(typeBuf, typeLen);
  nameTag := "name = ";
  typeTag := "type = ";
  varTag := "Variable:";
  pos := 0;
  WHILE pos < typeLen DO
    (* Find "Variable:" on a line *)
    IF (pos + 9 < typeLen) AND
       (typeBuf[pos] = ' ') THEN
      (* Skip leading spaces *)
      WHILE (pos < typeLen) AND (typeBuf[pos] = ' ') DO INC(pos) END;
      IF (pos + 9 <= typeLen) AND
         (typeBuf[pos] = 'V') AND (typeBuf[pos+1] = 'a') AND
         (typeBuf[pos+2] = 'r') AND (typeBuf[pos+3] = 'i') AND
         (typeBuf[pos+4] = 'a') AND (typeBuf[pos+5] = 'b') AND
         (typeBuf[pos+6] = 'l') AND (typeBuf[pos+7] = 'e') AND
         (typeBuf[pos+8] = ':') THEN
        pos := pos + 9;
        (* Find name = "..." *)
        varName[0] := CHR(0);
        varType[0] := CHR(0);
        WHILE (pos < typeLen) AND (typeBuf[pos] # CHR(10)) DO
          (* Look for 'name = "' *)
          IF (pos + 8 < typeLen) AND
             (typeBuf[pos] = 'n') AND (typeBuf[pos+1] = 'a') AND
             (typeBuf[pos+2] = 'm') AND (typeBuf[pos+3] = 'e') AND
             (typeBuf[pos+4] = ' ') AND (typeBuf[pos+5] = '=') AND
             (typeBuf[pos+6] = ' ') AND (typeBuf[pos+7] = '"') THEN
            pos := pos + 8;
            j := 0;
            WHILE (pos < typeLen) AND (typeBuf[pos] # '"') AND
                  (j < HIGH(varName)) DO
              varName[j] := typeBuf[pos];
              INC(pos); INC(j)
            END;
            varName[j] := CHR(0);
            IF (pos < typeLen) AND (typeBuf[pos] = '"') THEN INC(pos) END
          (* Look for 'type = "' *)
          ELSIF (pos + 8 < typeLen) AND
             (typeBuf[pos] = 't') AND (typeBuf[pos+1] = 'y') AND
             (typeBuf[pos+2] = 'p') AND (typeBuf[pos+3] = 'e') AND
             (typeBuf[pos+4] = ' ') AND (typeBuf[pos+5] = '=') AND
             (typeBuf[pos+6] = ' ') AND (typeBuf[pos+7] = '"') THEN
            pos := pos + 8;
            j := 0;
            WHILE (pos < typeLen) AND (typeBuf[pos] # '"') AND
                  (j < HIGH(varType)) DO
              varType[j] := typeBuf[pos];
              INC(pos); INC(j)
            END;
            varType[j] := CHR(0);
            IF (pos < typeLen) AND (typeBuf[pos] = '"') THEN INC(pos) END
          ELSE
            INC(pos)
          END
        END;
        (* Store entry *)
        IF (varName[0] # CHR(0)) AND (varType[0] # CHR(0)) AND
           (varTypeCount < MaxVT) THEN
          Assign(varName, varTypes[varTypeCount].name);
          Assign(varType, varTypes[varTypeCount].dwarfType);
          INC(varTypeCount)
        END
      END
    END;
    (* Skip to next line *)
    WHILE (pos < typeLen) AND (typeBuf[pos] # CHR(10)) DO INC(pos) END;
    IF pos < typeLen THEN INC(pos) END
  END
END BuildTypeMap;

PROCEDURE LookupDwarfType(VAR name: ARRAY OF CHAR;
                          VAR out: ARRAY OF CHAR): BOOLEAN;
VAR i: INTEGER;
BEGIN
  i := 0;
  WHILE i < varTypeCount DO
    IF CompareStr(name, varTypes[i].name) = 0 THEN
      Assign(varTypes[i].dwarfType, out);
      RETURN TRUE
    END;
    INC(i)
  END;
  RETURN FALSE
END LookupDwarfType;

(* ── Stack trace / variables handlers ──────────── *)

PROCEDURE HandleStackTrace;
VAR
  frameIdx, line: INTEGER;
  funcName: ARRAY [0..127] OF CHAR;
  fileName: ARRAY [0..255] OF CHAR;
  demangled: ARRAY [0..127] OF CHAR;
BEGIN
  BeginResponse("stackTrace", TRUE);
  JsonKey(outBuf, "body");
  JsonStart(outBuf);
  JsonKey(outBuf, "stackFrames");
  JsonArrayStart(outBuf);

  IF launched THEN
    GetBacktrace(parseBuf, parseLen);
    parsePos := 0;
    WHILE parsePos < parseLen DO
      PSkipSpaces;
      IF (parsePos < parseLen) AND (parseBuf[parsePos] = '*') THEN
        INC(parsePos); PSkipSpaces
      END;

      IF (parsePos + 7 < parseLen) AND
         (parseBuf[parsePos] = 'f') AND (parseBuf[parsePos+1] = 'r') AND
         (parseBuf[parsePos+2] = 'a') AND (parseBuf[parsePos+3] = 'm') AND
         (parseBuf[parsePos+4] = 'e') AND (parseBuf[parsePos+5] = ' ') AND
         (parseBuf[parsePos+6] = '#') THEN
        parsePos := parsePos + 7;
        frameIdx := PParseInt();
        PSkipSpaces;

        WHILE (parsePos < parseLen) AND (parseBuf[parsePos] # '`') AND
              (parseBuf[parsePos] # CHR(10)) DO
          INC(parsePos)
        END;
        IF (parsePos < parseLen) AND (parseBuf[parsePos] = '`') THEN
          INC(parsePos);
          PExtractUntil(' ', funcName)
        ELSE
          funcName[0] := '?'; funcName[1] := CHR(0)
        END;

        fileName[0] := CHR(0);
        line := 0;
        WHILE (parsePos + 4 < parseLen) AND
              (parseBuf[parsePos] # CHR(10)) DO
          IF (parseBuf[parsePos] = ' ') AND (parseBuf[parsePos+1] = 'a') AND
             (parseBuf[parsePos+2] = 't') AND (parseBuf[parsePos+3] = ' ') THEN
            parsePos := parsePos + 4;
            PExtractUntil(':', fileName);
            IF (parsePos < parseLen) AND (parseBuf[parsePos] = ':') THEN
              INC(parsePos);
              line := PParseInt()
            END;
            parsePos := parseLen
          ELSE
            INC(parsePos)
          END
        END;

        Demangle(funcName, demangled);
        JsonStart(outBuf);
        JsonKey(outBuf, "id");
        JsonInt(outBuf, frameIdx);
        JsonKey(outBuf, "name");
        JsonStr(outBuf, demangled);
        IF fileName[0] # CHR(0) THEN
          JsonKey(outBuf, "source");
          JsonStart(outBuf);
          JsonKey(outBuf, "name");
          JsonStr(outBuf, fileName);
          JsonKey(outBuf, "path");
          JsonStr(outBuf, fileName);
          JsonEnd(outBuf);
          JsonKey(outBuf, "line");
          JsonInt(outBuf, line);
          JsonKey(outBuf, "column");
          JsonInt(outBuf, 1)
        END;
        JsonEnd(outBuf)
      END;

      WHILE (parsePos < parseLen) AND (parseBuf[parsePos] # CHR(10)) DO
        INC(parsePos)
      END;
      IF parsePos < parseLen THEN INC(parsePos) END
    END
  END;

  JsonArrayEnd(outBuf);
  JsonEnd(outBuf);
  EndAndSend
END HandleStackTrace;

PROCEDURE HandleScopes(VAR buf: ARRAY OF CHAR; len: CARDINAL);
VAR
  p: Parser;
  tok: Token;
  fieldName: ARRAY [0..63] OF CHAR;
  frameId: INTEGER;
  localsRef, argsRef: INTEGER;
BEGIN
  frameId := 0;

  (* Parse frameId from arguments *)
  Init(p, ADR(buf), len);
  IF NOT Next(p, tok) THEN RETURN END;
  LOOP
    IF NOT Next(p, tok) THEN EXIT END;
    IF tok.kind = JObjectEnd THEN EXIT END;
    IF tok.kind = JComma THEN (* skip *) END;
    IF tok.kind = JString THEN
      IF NOT GetString(p, tok, fieldName) THEN EXIT END;
      IF NOT Next(p, tok) THEN EXIT END;
      IF StrEq(fieldName, "arguments") THEN
        IF NOT Next(p, tok) THEN EXIT END;
        IF tok.kind = JObjectStart THEN
          LOOP
            IF NOT Next(p, tok) THEN EXIT END;
            IF tok.kind = JObjectEnd THEN EXIT END;
            IF tok.kind = JComma THEN (* skip *) END;
            IF tok.kind = JString THEN
              IF NOT GetString(p, tok, fieldName) THEN EXIT END;
              IF NOT Next(p, tok) THEN EXIT END;
              IF StrEq(fieldName, "frameId") THEN
                IF NOT Next(p, tok) THEN EXIT END;
                IF tok.kind = JNumber THEN
                  IF NOT GetInteger(p, tok, frameId) THEN END
                END
              ELSE
                Skip(p)
              END
            END
          END
        END;
        EXIT
      ELSE
        Skip(p)
      END
    END
  END;

  localsRef := AllocScopeRef(frameId, ScopeLocals);
  argsRef := AllocScopeRef(frameId, ScopeArgs);

  BeginResponse("scopes", TRUE);
  JsonKey(outBuf, "body");
  JsonStart(outBuf);
  JsonKey(outBuf, "scopes");
  JsonArrayStart(outBuf);

  JsonStart(outBuf);
  JsonKey(outBuf, "name");
  JsonStr(outBuf, "Locals");
  JsonKey(outBuf, "variablesReference");
  JsonInt(outBuf, localsRef);
  JsonKey(outBuf, "expensive");
  JsonBool(outBuf, FALSE);
  JsonEnd(outBuf);

  JsonStart(outBuf);
  JsonKey(outBuf, "name");
  JsonStr(outBuf, "Arguments");
  JsonKey(outBuf, "variablesReference");
  JsonInt(outBuf, argsRef);
  JsonKey(outBuf, "expensive");
  JsonBool(outBuf, FALSE);
  JsonEnd(outBuf);

  JsonArrayEnd(outBuf);
  JsonEnd(outBuf);
  EndAndSend
END HandleScopes;

PROCEDURE HandleVariables(VAR buf: ARRAY OF CHAR; len: CARDINAL);
VAR
  p: Parser;
  tok: Token;
  fieldName: ARRAY [0..63] OF CHAR;
  varRef: INTEGER;
  frameIdx, scopeKind, parentRef, childIdx: INTEGER;
  isScope: BOOLEAN;
  typeName: ARRAY [0..63] OF CHAR;
  fmtType: ARRAY [0..63] OF CHAR;
  varName: ARRAY [0..63] OF CHAR;
  rawValue: ARRAY [0..255] OF CHAR;
  fmtValue: ARRAY [0..255] OF CHAR;
BEGIN
  varRef := 0;

  Init(p, ADR(buf), len);
  IF NOT Next(p, tok) THEN RETURN END;
  LOOP
    IF NOT Next(p, tok) THEN EXIT END;
    IF tok.kind = JObjectEnd THEN EXIT END;
    IF tok.kind = JComma THEN (* skip *) END;
    IF tok.kind = JString THEN
      IF NOT GetString(p, tok, fieldName) THEN EXIT END;
      IF NOT Next(p, tok) THEN EXIT END;
      IF StrEq(fieldName, "arguments") THEN
        IF NOT Next(p, tok) THEN EXIT END;
        IF tok.kind = JObjectStart THEN
          LOOP
            IF NOT Next(p, tok) THEN EXIT END;
            IF tok.kind = JObjectEnd THEN EXIT END;
            IF tok.kind = JComma THEN (* skip *) END;
            IF tok.kind = JString THEN
              IF NOT GetString(p, tok, fieldName) THEN EXIT END;
              IF NOT Next(p, tok) THEN EXIT END;
              IF StrEq(fieldName, "variablesReference") THEN
                IF NOT Next(p, tok) THEN EXIT END;
                IF tok.kind = JNumber THEN
                  IF NOT GetInteger(p, tok, varRef) THEN END
                END
              ELSE
                Skip(p)
              END
            END
          END
        END;
        EXIT
      ELSE
        Skip(p)
      END
    END
  END;

  GetRefInfo(varRef, frameIdx, scopeKind, parentRef, childIdx, isScope);

  BeginResponse("variables", TRUE);
  JsonKey(outBuf, "body");
  JsonStart(outBuf);
  JsonKey(outBuf, "variables");
  JsonArrayStart(outBuf);

  IF isScope AND launched THEN
    (* Build DWARF type map for this frame *)
    BuildTypeMap(frameIdx);

    GetFrameVars(frameIdx, parseBuf, parseLen);
    parsePos := 0;
    WHILE parsePos < parseLen DO
      PSkipSpaces;
      IF (parsePos < parseLen) AND (parseBuf[parsePos] = '(') THEN
        PExtractParen(typeName);
        PSkipSpaces;
        PExtractUntil(' ', varName);
        PSkipSpaces;
        IF (parsePos < parseLen) AND (parseBuf[parsePos] = '=') THEN
          INC(parsePos);
          PSkipSpaces
        END;
        PExtractToEOL(rawValue);

        (* Look up DWARF type name; fall back to FormatType mapping *)
        IF NOT LookupDwarfType(varName, fmtType) THEN
          FormatType(typeName, fmtType)
        END;

        (* Format the value using the DWARF type for accurate dispatch *)
        IF NOT FormatValue(fmtType, rawValue, fmtValue) THEN
          Assign(rawValue, fmtValue)
        END;

        (* Handle record types: value starts with "{" — skip inner lines *)
        IF (rawValue[0] = '{') THEN
          Assign("{...}", fmtValue);
          (* Skip lines until closing "}" at start of line *)
          LOOP
            WHILE (parsePos < parseLen) AND (parseBuf[parsePos] # CHR(10)) DO
              INC(parsePos)
            END;
            IF parsePos < parseLen THEN INC(parsePos) END;
            PSkipSpaces;
            IF (parsePos >= parseLen) THEN EXIT END;
            IF (parseBuf[parsePos] = '}') THEN
              (* Skip the closing brace line *)
              WHILE (parsePos < parseLen) AND (parseBuf[parsePos] # CHR(10)) DO
                INC(parsePos)
              END;
              IF parsePos < parseLen THEN INC(parsePos) END;
              EXIT
            END;
            (* Not '}' — check if it's a new top-level var (starts with '(') *)
            IF (parseBuf[parsePos] = '(') THEN EXIT END
          END
        END;

        JsonStart(outBuf);
        JsonKey(outBuf, "name");
        JsonStr(outBuf, varName);
        JsonKey(outBuf, "value");
        JsonStr(outBuf, fmtValue);
        JsonKey(outBuf, "type");
        JsonStr(outBuf, fmtType);
        JsonKey(outBuf, "variablesReference");
        JsonInt(outBuf, 0);
        JsonEnd(outBuf)
      END;
      WHILE (parsePos < parseLen) AND (parseBuf[parsePos] # CHR(10)) DO
        INC(parsePos)
      END;
      IF parsePos < parseLen THEN INC(parsePos) END
    END
  END;

  JsonArrayEnd(outBuf);
  JsonEnd(outBuf);
  EndAndSend
END HandleVariables;

PROCEDURE HandleUnknown(command: ARRAY OF CHAR);
BEGIN
  BeginResponse(command, FALSE);
  JsonKey(outBuf, "message");
  JsonStr(outBuf, "unsupported");
  EndAndSend
END HandleUnknown;

(* ── Main dispatch ─────────────────────────────── *)

PROCEDURE HandleMessage(VAR buf: ARRAY OF CHAR;
                        len: CARDINAL): BOOLEAN;
VAR
  p: Parser;
  tok: Token;
  command: ARRAY [0..63] OF CHAR;
  fieldName: ARRAY [0..63] OF CHAR;
  foundCommand: BOOLEAN;
BEGIN
  Init(p, ADR(buf), len);

  IF NOT Next(p, tok) THEN RETURN TRUE END;
  IF tok.kind # JObjectStart THEN RETURN TRUE END;

  seq := 0;
  command[0] := CHR(0);
  foundCommand := FALSE;

  LOOP
    IF NOT Next(p, tok) THEN EXIT END;
    IF tok.kind = JObjectEnd THEN EXIT END;
    IF tok.kind = JComma THEN
      (* skip *)
    ELSIF tok.kind = JString THEN
      IF NOT GetString(p, tok, fieldName) THEN EXIT END;
      IF NOT Next(p, tok) THEN EXIT END;
      IF tok.kind # JColon THEN EXIT END;

      IF StrEq(fieldName, "seq") THEN
        IF NOT Next(p, tok) THEN EXIT END;
        IF tok.kind = JNumber THEN
          IF NOT GetInteger(p, tok, seq) THEN seq := 0 END
        END
      ELSIF StrEq(fieldName, "command") THEN
        IF NOT Next(p, tok) THEN EXIT END;
        IF tok.kind = JString THEN
          IF GetString(p, tok, command) THEN
            foundCommand := TRUE
          END
        END
      ELSE
        Skip(p)
      END
    ELSE
      EXIT
    END
  END;

  IF NOT foundCommand THEN RETURN TRUE END;

  IF StrEq(command, "initialize") THEN
    HandleInitialize
  ELSIF StrEq(command, "disconnect") THEN
    RETURN HandleDisconnect()
  ELSIF StrEq(command, "configurationDone") THEN
    HandleConfigurationDone
  ELSIF StrEq(command, "launch") THEN
    HandleLaunch(buf, len)
  ELSIF StrEq(command, "setBreakpoints") THEN
    HandleSetBreakpoints(buf, len)
  ELSIF StrEq(command, "continue") THEN
    HandleContinue
  ELSIF StrEq(command, "next") THEN
    HandleNext
  ELSIF StrEq(command, "stepIn") THEN
    HandleStepIn
  ELSIF StrEq(command, "stepOut") THEN
    HandleStepOut
  ELSIF StrEq(command, "pause") THEN
    HandlePause
  ELSIF StrEq(command, "threads") THEN
    HandleThreads
  ELSIF StrEq(command, "stackTrace") THEN
    HandleStackTrace
  ELSIF StrEq(command, "scopes") THEN
    HandleScopes(buf, len)
  ELSIF StrEq(command, "variables") THEN
    HandleVariables(buf, len)
  ELSE
    HandleUnknown(command)
  END;

  RETURN TRUE
END HandleMessage;

BEGIN
  seq := 0;
  responseSeq := 0;
  bpCount := 0;
  launched := FALSE;
  stopOnEntry := FALSE;
  targetPath[0] := CHR(0);
  varTypeCount := 0;
  InitBuf(outBuf, ADR(outBacking), MaxBody)
END DAPServer.
