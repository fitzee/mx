MODULE LogTests;
(* Unit tests for the m2log library.

   Tests:
     1. Level filtering fast path
     2. Deterministic formatting with field ordering and escaping
     3. Category inclusion
     4. MemorySink capture correctness
     5. Recursion guard (sink triggers log -> suppressed)
     6. Field constructors
     7. LogKV with structured fields
     8. Category propagation
     9. Multiple sinks
    10. Sink level override
    11. Negative integer formatting *)

FROM InOut IMPORT WriteString, WriteLn, WriteInt;
FROM Log IMPORT Logger, Record, Field, Sink, SinkProc,
                Level, FieldKind, MaxFields,
                Init, InitDefault, SetLevel, AddSink,
                SetCategory, WithCategory,
                LogMsg, LogKV,
                Trace, Debug, Info, Warn, Error, Fatal,
                InfoD, WarnD,
                KVStr, KVInt, KVBool,
                GetDropCount, MakeConsoleSink,
                Format, LineBuf, MaxLine;
FROM LogSinkMemory IMPORT MemorySink, Create, GetCount, GetTotal,
                          GetLine, Clear, Contains;
FROM SYSTEM IMPORT ADDRESS, ADR;
FROM Strings IMPORT Pos, Length;

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

(* ── Test 1: Level filtering ────────────────────────── *)

PROCEDURE TestLevelFiltering;
VAR
  l: Logger;
  ms: Sink;
  mem: MemorySink;
BEGIN
  Init(l);
  Create(mem, ms);
  IF AddSink(l, ms) THEN END;

  Trace(l, "trace msg");
  Debug(l, "debug msg");
  Check("filter: trace+debug filtered", GetCount(mem) = 0);

  Info(l, "info msg");
  Check("filter: info passes", GetCount(mem) = 1);

  Warn(l, "warn msg");
  Error(l, "error msg");
  Fatal(l, "fatal msg");
  Check("filter: warn+error+fatal pass", GetCount(mem) = 4);

  Clear(mem);
  SetLevel(l, ERROR);
  Info(l, "info");
  Warn(l, "warn");
  Check("filter: info+warn filtered at ERROR", GetCount(mem) = 0);

  Error(l, "error");
  Fatal(l, "fatal");
  Check("filter: error+fatal pass at ERROR", GetCount(mem) = 2);

  Clear(mem);
  SetLevel(l, TRACE);
  Trace(l, "t");
  Debug(l, "d");
  Check("filter: trace+debug pass at TRACE", GetCount(mem) = 2)
END TestLevelFiltering;

(* ── Test 2: Formatting ─────────────────────────────── *)

PROCEDURE TestFormatting;
VAR
  rec: Record;
  buf: LineBuf;
  len: INTEGER;
  p, pa, pm, pz: CARDINAL;
BEGIN
  rec.level := INFO;
  rec.msg := "hello world";
  rec.category[0] := 0C;
  rec.nFields := 0;
  Format(rec, buf, len);

  p := Pos("INFO", buf);
  Check("fmt: starts with INFO", p = 0);
  p := Pos('msg="hello world"', buf);
  Check("fmt: contains msg=", p < CARDINAL(len));

  rec.category := "net.tls";
  Format(rec, buf, len);
  p := Pos('category="net.tls"', buf);
  Check("fmt: contains category=", p < CARDINAL(len));

  rec.category[0] := 0C;
  rec.nFields := 3;
  rec.fields[0].key := "zebra";
  rec.fields[0].kind := FkInt;
  rec.fields[0].intVal := 99;
  rec.fields[0].strVal[0] := 0C;
  rec.fields[1].key := "alpha";
  rec.fields[1].kind := FkStr;
  rec.fields[1].strVal := "first";
  rec.fields[1].intVal := 0;
  rec.fields[2].key := "mid";
  rec.fields[2].kind := FkBool;
  rec.fields[2].boolVal := TRUE;
  rec.fields[2].strVal[0] := 0C;
  rec.fields[2].intVal := 0;

  Format(rec, buf, len);
  pa := Pos("alpha=", buf);
  pm := Pos("mid=", buf);
  pz := Pos("zebra=", buf);
  Check("fmt: fields sorted (alpha < mid)", pa < pm);
  Check("fmt: fields sorted (mid < zebra)", pm < pz);
  Check("fmt: alpha has quoted string", Pos('alpha="first"', buf) < CARDINAL(len));
  Check("fmt: mid has bool", Pos("mid=true", buf) < CARDINAL(len));
  Check("fmt: zebra has int", Pos("zebra=99", buf) < CARDINAL(len))
END TestFormatting;

(* ── Test 3: Escaping ────────────────────────────────── *)

PROCEDURE TestEscaping;
VAR
  rec: Record;
  buf: LineBuf;
  len: INTEGER;
  p: CARDINAL;
BEGIN
  rec.level := WARN;
  rec.msg := 'say "hello"';
  rec.category[0] := 0C;
  rec.nFields := 0;
  Format(rec, buf, len);
  p := Pos('\"hello\"', buf);
  Check("esc: quotes escaped in msg", p < CARDINAL(len))
END TestEscaping;

(* ── Test 4: MemorySink capture ──────────────────────── *)

PROCEDURE TestMemorySink;
VAR
  l: Logger;
  ms: Sink;
  mem: MemorySink;
  line: ARRAY [0..511] OF CHAR;
  ok: BOOLEAN;
  p: CARDINAL;
BEGIN
  Init(l);
  SetLevel(l, TRACE);
  Create(mem, ms);
  IF AddSink(l, ms) THEN END;

  Info(l, "first");
  Info(l, "second");
  Info(l, "third");
  Check("memsink: count=3", GetCount(mem) = 3);
  Check("memsink: total=3", GetTotal(mem) = 3);

  ok := GetLine(mem, 0, line);
  Check("memsink: getline 0 ok", ok);
  p := Pos('msg="first"', line);
  Check("memsink: line 0 is first", p < 512);

  ok := GetLine(mem, 2, line);
  Check("memsink: getline 2 ok", ok);
  p := Pos('msg="third"', line);
  Check("memsink: line 2 is third", p < 512);

  ok := GetLine(mem, 3, line);
  Check("memsink: getline 3 false", NOT ok);

  Check("memsink: contains 'second'", Contains(mem, "second"));
  Check("memsink: not contains 'bogus'", NOT Contains(mem, "bogus"));

  Clear(mem);
  Check("memsink: clear count=0", GetCount(mem) = 0);
  Check("memsink: clear total=0", GetTotal(mem) = 0)
END TestMemorySink;

(* ── Test 5: Recursion guard ─────────────────────────── *)

VAR
  recursiveLogger: Logger;

PROCEDURE RecursiveSinkProc(ctx: ADDRESS; VAR rec: Record);
BEGIN
  Info(recursiveLogger, "recursive call")
END RecursiveSinkProc;

PROCEDURE TestRecursionGuard;
VAR
  ms: Sink;
  mem: MemorySink;
  recSink: Sink;
BEGIN
  Init(recursiveLogger);
  SetLevel(recursiveLogger, TRACE);

  Create(mem, ms);
  IF AddSink(recursiveLogger, ms) THEN END;

  recSink.proc := RecursiveSinkProc;
  recSink.ctx := NIL;
  recSink.minLevel := TRACE;
  IF AddSink(recursiveLogger, recSink) THEN END;

  Info(recursiveLogger, "trigger");
  Check("recurse: only 1 line captured", GetCount(mem) = 1);
  Check("recurse: drop count > 0", GetDropCount(recursiveLogger) > 0)
END TestRecursionGuard;

(* ── Test 6: Field constructors ──────────────────────── *)

PROCEDURE TestFieldConstructors;
VAR f: Field;
BEGIN
  KVStr("name", "Alice", f);
  Check("kvstr: key", f.key[0] = 'n');
  Check("kvstr: kind", f.kind = FkStr);
  Check("kvstr: val", f.strVal[0] = 'A');

  KVInt("count", 42, f);
  Check("kvint: key", f.key[0] = 'c');
  Check("kvint: kind", f.kind = FkInt);
  Check("kvint: val", f.intVal = 42);

  KVBool("active", TRUE, f);
  Check("kvbool: key", f.key[0] = 'a');
  Check("kvbool: kind", f.kind = FkBool);
  Check("kvbool: val", f.boolVal)
END TestFieldConstructors;

(* ── Test 7: LogKV with fields ───────────────────────── *)

PROCEDURE TestLogKV;
VAR
  l: Logger;
  ms: Sink;
  mem: MemorySink;
  fs: ARRAY [0..2] OF Field;
  line: ARRAY [0..511] OF CHAR;
  ok: BOOLEAN;
BEGIN
  Init(l);
  SetLevel(l, TRACE);
  Create(mem, ms);
  IF AddSink(l, ms) THEN END;

  KVStr("peer", "127.0.0.1", fs[0]);
  KVInt("fd", 12, fs[1]);
  KVBool("tls", TRUE, fs[2]);

  LogKV(l, INFO, "connected", fs, 3);
  Check("logkv: captured", GetCount(mem) = 1);

  ok := GetLine(mem, 0, line);
  Check("logkv: has peer", Pos("peer=", line) < 512);
  Check("logkv: has fd", Pos("fd=12", line) < 512);
  Check("logkv: has tls", Pos("tls=true", line) < 512)
END TestLogKV;

(* ── Test 8: Category ────────────────────────────────── *)

PROCEDURE TestCategory;
VAR
  l: Logger;
  ms: Sink;
  mem: MemorySink;
  line: ARRAY [0..511] OF CHAR;
  ok: BOOLEAN;
BEGIN
  Init(l);
  Create(mem, ms);
  IF AddSink(l, ms) THEN END;
  SetCategory(l, "app");

  Info(l, "parent");
  ok := GetLine(mem, 0, line);
  Check("cat: parent has category", Pos('category="app"', line) < 512)
END TestCategory;

(* ── Test 9: Multiple sinks ─────────────────────────── *)

PROCEDURE TestMultipleSinks;
VAR
  l: Logger;
  ms1, ms2: Sink;
  mem1, mem2: MemorySink;
BEGIN
  Init(l);
  Create(mem1, ms1);
  Create(mem2, ms2);
  IF AddSink(l, ms1) THEN END;
  IF AddSink(l, ms2) THEN END;

  Info(l, "broadcast");
  Check("multi: sink1 got msg", GetCount(mem1) = 1);
  Check("multi: sink2 got msg", GetCount(mem2) = 1)
END TestMultipleSinks;

(* ── Test 10: Sink level override ────────────────────── *)

PROCEDURE TestSinkLevelOverride;
VAR
  l: Logger;
  ms: Sink;
  mem: MemorySink;
BEGIN
  Init(l);
  SetLevel(l, TRACE);
  Create(mem, ms);
  ms.minLevel := WARN;
  IF AddSink(l, ms) THEN END;

  Info(l, "info");
  Check("sinklevel: info filtered by sink", GetCount(mem) = 0);

  Warn(l, "warn");
  Check("sinklevel: warn passes sink", GetCount(mem) = 1)
END TestSinkLevelOverride;

(* ── Test 11: Negative integer formatting ────────────── *)

PROCEDURE TestNegativeInt;
VAR
  l: Logger;
  ms: Sink;
  mem: MemorySink;
  fs: ARRAY [0..0] OF Field;
  line: ARRAY [0..511] OF CHAR;
  ok: BOOLEAN;
BEGIN
  Init(l);
  Create(mem, ms);
  IF AddSink(l, ms) THEN END;

  KVInt("val", -42, fs[0]);
  LogKV(l, INFO, "neg", fs, 1);
  ok := GetLine(mem, 0, line);
  Check("negint: has -42", Pos("val=-42", line) < 512)
END TestNegativeInt;

BEGIN
  passed := 0;
  failed := 0;
  total := 0;

  WriteString("m2log test suite"); WriteLn;
  WriteString("================"); WriteLn;

  TestLevelFiltering;
  TestFormatting;
  TestEscaping;
  TestMemorySink;
  TestRecursionGuard;
  TestFieldConstructors;
  TestLogKV;
  TestCategory;
  TestMultipleSinks;
  TestSinkLevelOverride;
  TestNegativeInt;

  WriteLn;
  WriteInt(total, 0); WriteString(" tests, ");
  WriteInt(passed, 0); WriteString(" passed, ");
  WriteInt(failed, 0); WriteString(" failed"); WriteLn;

  IF failed > 0 THEN
    WriteString("*** FAILURES ***"); WriteLn
  ELSE
    WriteString("*** ALL TESTS PASSED ***"); WriteLn
  END
END LogTests.
