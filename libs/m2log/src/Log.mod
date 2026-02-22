IMPLEMENTATION MODULE Log;

FROM SYSTEM IMPORT ADDRESS, ADR;
FROM Strings IMPORT Assign, Length;
FROM InOut IMPORT WriteString, WriteLn;

(* ── Module state ────────────────────────────────────── *)

VAR
  defaultLogger: Logger;
  defaultReady: BOOLEAN;

(* ── Format helpers (use LineBuf directly to avoid open-array codegen issues) *)

PROCEDURE FmtChar(VAR buf: LineBuf; VAR pos: INTEGER; ch: CHAR);
BEGIN
  IF pos < MaxLine - 1 THEN
    buf[pos] := ch;
    INC(pos)
  END
END FmtChar;

PROCEDURE FmtStr(VAR buf: LineBuf; VAR pos: INTEGER;
                 s: ARRAY OF CHAR);
VAR i: INTEGER;
BEGIN
  i := 0;
  WHILE (i <= HIGH(s)) AND (s[i] # 0C) AND (pos < MaxLine - 1) DO
    buf[pos] := s[i];
    INC(pos);
    INC(i)
  END
END FmtStr;

PROCEDURE FmtEscaped(VAR buf: LineBuf; VAR pos: INTEGER;
                     s: ARRAY OF CHAR);
VAR i: INTEGER; ch: CHAR;
BEGIN
  FmtChar(buf, pos, '"');
  i := 0;
  WHILE (i <= HIGH(s)) AND (s[i] # 0C) AND (pos < MaxLine - 3) DO
    ch := s[i];
    IF ch = '"' THEN
      FmtChar(buf, pos, '\'); FmtChar(buf, pos, '"')
    ELSIF ch = '\' THEN
      FmtChar(buf, pos, '\'); FmtChar(buf, pos, '\')
    ELSIF ch = 12C THEN
      FmtChar(buf, pos, '\'); FmtChar(buf, pos, 'n')
    ELSIF ch = 11C THEN
      FmtChar(buf, pos, '\'); FmtChar(buf, pos, 't')
    ELSIF ch = 15C THEN
      FmtChar(buf, pos, '\'); FmtChar(buf, pos, 'r')
    ELSIF ch < ' ' THEN
      (* skip other control chars *)
    ELSE
      FmtChar(buf, pos, ch)
    END;
    INC(i)
  END;
  FmtChar(buf, pos, '"')
END FmtEscaped;

PROCEDURE FmtInt(VAR buf: LineBuf; VAR pos: INTEGER; n: INTEGER);
VAR
  digits: ARRAY [0..11] OF CHAR;
  dpos, val: INTEGER;
  neg: BOOLEAN;
BEGIN
  IF n = 0 THEN
    FmtChar(buf, pos, '0');
    RETURN
  END;
  neg := n < 0;
  IF neg THEN
    IF n = -2147483648 THEN
      FmtStr(buf, pos, "-2147483648");
      RETURN
    END;
    val := -n
  ELSE
    val := n
  END;
  dpos := 0;
  WHILE val > 0 DO
    digits[dpos] := CHR(ORD('0') + (val MOD 10));
    INC(dpos);
    val := val DIV 10
  END;
  IF neg THEN FmtChar(buf, pos, '-') END;
  WHILE dpos > 0 DO
    DEC(dpos);
    FmtChar(buf, pos, digits[dpos])
  END
END FmtInt;

PROCEDURE FmtLevel(level: Level; VAR buf: LineBuf; VAR pos: INTEGER);
BEGIN
  IF level = TRACE THEN FmtStr(buf, pos, "TRACE")
  ELSIF level = DEBUG THEN FmtStr(buf, pos, "DEBUG")
  ELSIF level = INFO THEN FmtStr(buf, pos, "INFO")
  ELSIF level = WARN THEN FmtStr(buf, pos, "WARN")
  ELSIF level = ERROR THEN FmtStr(buf, pos, "ERROR")
  ELSIF level = FATAL THEN FmtStr(buf, pos, "FATAL")
  ELSE FmtStr(buf, pos, "???")
  END
END FmtLevel;

(* ── Field sorting ───────────────────────────────────── *)

PROCEDURE KeyLess(VAR a, b: Field): BOOLEAN;
VAR i: INTEGER;
BEGIN
  i := 0;
  WHILE (i <= MaxKey) AND (a.key[i] # 0C) AND (b.key[i] # 0C) DO
    IF a.key[i] < b.key[i] THEN RETURN TRUE
    ELSIF a.key[i] > b.key[i] THEN RETURN FALSE
    END;
    INC(i)
  END;
  IF (i > MaxKey) OR (a.key[i] = 0C) THEN
    IF (i <= MaxKey) AND (b.key[i] # 0C) THEN RETURN TRUE END
  END;
  RETURN FALSE
END KeyLess;

PROCEDURE CopyField(VAR src, dst: Field);
VAR i: INTEGER;
BEGIN
  i := 0;
  WHILE (i <= MaxKey) AND (src.key[i] # 0C) DO
    dst.key[i] := src.key[i]; INC(i)
  END;
  IF i <= MaxKey THEN dst.key[i] := 0C END;
  dst.kind := src.kind;
  dst.intVal := src.intVal;
  dst.boolVal := src.boolVal;
  i := 0;
  WHILE (i <= MaxStrVal) AND (src.strVal[i] # 0C) DO
    dst.strVal[i] := src.strVal[i]; INC(i)
  END;
  IF i <= MaxStrVal THEN dst.strVal[i] := 0C END
END CopyField;

PROCEDURE SortFields(VAR fs: ARRAY OF Field; n: INTEGER);
VAR i, j: INTEGER; tmp: Field;
BEGIN
  i := 1;
  WHILE i < n DO
    CopyField(fs[i], tmp);
    j := i;
    WHILE (j > 0) AND KeyLess(tmp, fs[j-1]) DO
      CopyField(fs[j-1], fs[j]);
      DEC(j)
    END;
    CopyField(tmp, fs[j]);
    INC(i)
  END
END SortFields;

(* ── Format ──────────────────────────────────────────── *)

PROCEDURE Format(VAR rec: Record; VAR buf: LineBuf;
                 VAR len: INTEGER);
VAR
  pos, i, nf: INTEGER;
  sorted: ARRAY [0..MaxFields-1] OF Field;
BEGIN
  pos := 0;
  FmtLevel(rec.level, buf, pos);
  FmtStr(buf, pos, " msg=");
  FmtEscaped(buf, pos, rec.msg);

  IF rec.category[0] # 0C THEN
    FmtStr(buf, pos, " category=");
    FmtEscaped(buf, pos, rec.category)
  END;

  nf := rec.nFields;
  IF nf > MaxFields THEN nf := MaxFields END;
  IF nf < 0 THEN nf := 0 END;

  i := 0;
  WHILE i < nf DO
    CopyField(rec.fields[i], sorted[i]);
    INC(i)
  END;
  IF nf > 1 THEN SortFields(sorted, nf) END;

  i := 0;
  WHILE i < nf DO
    FmtChar(buf, pos, ' ');
    FmtStr(buf, pos, sorted[i].key);
    FmtChar(buf, pos, '=');
    IF sorted[i].kind = FkStr THEN
      FmtEscaped(buf, pos, sorted[i].strVal)
    ELSIF sorted[i].kind = FkInt THEN
      FmtInt(buf, pos, sorted[i].intVal)
    ELSIF sorted[i].kind = FkBool THEN
      IF sorted[i].boolVal THEN FmtStr(buf, pos, "true")
      ELSE FmtStr(buf, pos, "false")
      END
    END;
    INC(i)
  END;

  IF pos < MaxLine THEN buf[pos] := 0C
  ELSE buf[MaxLine - 1] := 0C
  END;
  len := pos
END Format;

(* ── Built-in console sink (stdout) ──────────────────── *)

PROCEDURE ConsoleSinkProc(ctx: ADDRESS; VAR rec: Record);
VAR buf: LineBuf; len: INTEGER;
BEGIN
  Format(rec, buf, len);
  WriteString(buf);
  WriteLn
END ConsoleSinkProc;

(* ── Initialization ──────────────────────────────────── *)

PROCEDURE Init(VAR l: Logger);
VAR i: INTEGER;
BEGIN
  l.minLevel := INFO;
  l.sinkCount := 0;
  l.category[0] := 0C;
  l.inSink := FALSE;
  l.dropCount := 0;
  i := 0;
  WHILE i < MaxSinks DO
    l.sinks[i].proc := NIL;
    l.sinks[i].ctx := NIL;
    l.sinks[i].minLevel := TRACE;
    INC(i)
  END
END Init;

PROCEDURE InitDefault;
VAR s: Sink;
BEGIN
  Init(defaultLogger);
  MakeConsoleSink(s);
  IF AddSink(defaultLogger, s) THEN END;
  defaultReady := TRUE
END InitDefault;

(* ── Configuration ───────────────────────────────────── *)

PROCEDURE SetLevel(VAR l: Logger; level: Level);
BEGIN l.minLevel := level END SetLevel;

PROCEDURE AddSink(VAR l: Logger; s: Sink): BOOLEAN;
BEGIN
  IF l.sinkCount >= MaxSinks THEN RETURN FALSE END;
  l.sinks[l.sinkCount] := s;
  INC(l.sinkCount);
  RETURN TRUE
END AddSink;

PROCEDURE SetCategory(VAR l: Logger; cat: ARRAY OF CHAR);
BEGIN Assign(cat, l.category) END SetCategory;

PROCEDURE WithCategory(VAR src: Logger; cat: ARRAY OF CHAR;
                        VAR out: Logger);
VAR i: INTEGER;
BEGIN
  out.minLevel := src.minLevel;
  out.sinkCount := src.sinkCount;
  out.inSink := FALSE;
  out.dropCount := 0;
  i := 0;
  WHILE i < src.sinkCount DO
    out.sinks[i] := src.sinks[i];
    INC(i)
  END;
  Assign(cat, out.category)
END WithCategory;

(* ── Internal dispatch ───────────────────────────────── *)

PROCEDURE Dispatch(VAR l: Logger; VAR rec: Record);
VAR i: INTEGER; sp: SinkProc;
BEGIN
  IF l.inSink THEN INC(l.dropCount); RETURN END;
  l.inSink := TRUE;
  i := 0;
  WHILE i < l.sinkCount DO
    IF ORD(rec.level) >= ORD(l.sinks[i].minLevel) THEN
      sp := l.sinks[i].proc;
      IF sp # NIL THEN
        sp(l.sinks[i].ctx, rec)
      END
    END;
    INC(i)
  END;
  l.inSink := FALSE
END Dispatch;

PROCEDURE FillRecord(VAR l: Logger; level: Level;
                     msg: ARRAY OF CHAR; VAR rec: Record);
BEGIN
  rec.level := level;
  Assign(msg, rec.msg);
  Assign(l.category, rec.category);
  rec.nFields := 0
END FillRecord;

(* ── Logging ─────────────────────────────────────────── *)

PROCEDURE LogMsg(VAR l: Logger; level: Level;
                 msg: ARRAY OF CHAR);
VAR rec: Record;
BEGIN
  IF ORD(level) < ORD(l.minLevel) THEN RETURN END;
  FillRecord(l, level, msg, rec);
  Dispatch(l, rec)
END LogMsg;

PROCEDURE LogKV(VAR l: Logger; level: Level;
                msg: ARRAY OF CHAR;
                fields: ARRAY OF Field; nFields: INTEGER);
VAR rec: Record; i, n: INTEGER;
BEGIN
  IF ORD(level) < ORD(l.minLevel) THEN RETURN END;
  FillRecord(l, level, msg, rec);
  n := nFields;
  IF n > MaxFields THEN n := MaxFields END;
  IF n > HIGH(fields) + 1 THEN n := HIGH(fields) + 1 END;
  IF n < 0 THEN n := 0 END;
  i := 0;
  WHILE i < n DO rec.fields[i] := fields[i]; INC(i) END;
  rec.nFields := n;
  Dispatch(l, rec)
END LogKV;

(* ── Convenience (explicit logger) ───────────────────── *)

PROCEDURE Trace(VAR l: Logger; msg: ARRAY OF CHAR);
BEGIN LogMsg(l, TRACE, msg) END Trace;
PROCEDURE Debug(VAR l: Logger; msg: ARRAY OF CHAR);
BEGIN LogMsg(l, DEBUG, msg) END Debug;
PROCEDURE Info(VAR l: Logger; msg: ARRAY OF CHAR);
BEGIN LogMsg(l, INFO, msg) END Info;
PROCEDURE Warn(VAR l: Logger; msg: ARRAY OF CHAR);
BEGIN LogMsg(l, WARN, msg) END Warn;
PROCEDURE Error(VAR l: Logger; msg: ARRAY OF CHAR);
BEGIN LogMsg(l, ERROR, msg) END Error;
PROCEDURE Fatal(VAR l: Logger; msg: ARRAY OF CHAR);
BEGIN LogMsg(l, FATAL, msg) END Fatal;

(* ── Convenience (default logger) ────────────────────── *)

PROCEDURE EnsureDefault;
BEGIN IF NOT defaultReady THEN InitDefault END
END EnsureDefault;

PROCEDURE TraceD(msg: ARRAY OF CHAR);
BEGIN EnsureDefault; LogMsg(defaultLogger, TRACE, msg) END TraceD;
PROCEDURE DebugD(msg: ARRAY OF CHAR);
BEGIN EnsureDefault; LogMsg(defaultLogger, DEBUG, msg) END DebugD;
PROCEDURE InfoD(msg: ARRAY OF CHAR);
BEGIN EnsureDefault; LogMsg(defaultLogger, INFO, msg) END InfoD;
PROCEDURE WarnD(msg: ARRAY OF CHAR);
BEGIN EnsureDefault; LogMsg(defaultLogger, WARN, msg) END WarnD;
PROCEDURE ErrorD(msg: ARRAY OF CHAR);
BEGIN EnsureDefault; LogMsg(defaultLogger, ERROR, msg) END ErrorD;
PROCEDURE FatalD(msg: ARRAY OF CHAR);
BEGIN EnsureDefault; LogMsg(defaultLogger, FATAL, msg) END FatalD;

(* ── Field constructors ──────────────────────────────── *)

PROCEDURE KVStr(key: ARRAY OF CHAR; val: ARRAY OF CHAR;
                VAR out: Field);
BEGIN
  Assign(key, out.key);
  out.kind := FkStr;
  Assign(val, out.strVal);
  out.intVal := 0;
  out.boolVal := FALSE
END KVStr;

PROCEDURE KVInt(key: ARRAY OF CHAR; val: INTEGER;
                VAR out: Field);
BEGIN
  Assign(key, out.key);
  out.kind := FkInt;
  out.intVal := val;
  out.boolVal := FALSE;
  out.strVal[0] := 0C
END KVInt;

PROCEDURE KVBool(key: ARRAY OF CHAR; val: BOOLEAN;
                 VAR out: Field);
BEGIN
  Assign(key, out.key);
  out.kind := FkBool;
  out.boolVal := val;
  out.intVal := 0;
  out.strVal[0] := 0C
END KVBool;

(* ── Diagnostics ─────────────────────────────────────── *)

PROCEDURE GetDropCount(VAR l: Logger): INTEGER;
BEGIN RETURN l.dropCount END GetDropCount;

(* ── Sink helpers ────────────────────────────────────── *)

PROCEDURE MakeConsoleSink(VAR out: Sink);
BEGIN
  out.proc := ConsoleSinkProc;
  out.ctx := NIL;
  out.minLevel := TRACE
END MakeConsoleSink;

BEGIN
  defaultReady := FALSE
END Log.
