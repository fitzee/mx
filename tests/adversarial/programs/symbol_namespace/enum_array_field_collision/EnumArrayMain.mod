MODULE EnumArrayMain;
(* Regression test: when two imported records have a field named 'status'
   but one is ARRAY OF CHAR and the other is an enumeration, assignment
   to the enum-typed field must emit direct assignment, not memcpy.

   ArrayStatus.LogEntry.status  = ARRAY [0..31] OF CHAR
   EnumStatus.Span.status       = SpanStatus (enumeration)

   Also tests CARDINAL fields to cover the integer-to-pointer conversion
   error pattern (uint32_t passed where const void * expected). *)

FROM InOut IMPORT WriteString, WriteCard, WriteLn;
FROM ArrayStatus IMPORT LogEntry, InitLog;
FROM EnumStatus IMPORT Span, SpanStatus, InitSpan, Active, Completed, Error, Cancelled;

VAR
  entry: LogEntry;
  s1, s2: Span;
  st: SpanStatus;
  items: ARRAY [0..3] OF Span;
  i: CARDINAL;

BEGIN
  (* Array-typed status: should use memcpy/strcpy *)
  InitLog(entry, "OK", 200);
  WriteString("log="); WriteString(entry.status); WriteLn;
  WriteString("code="); WriteCard(entry.code, 1); WriteLn;

  (* Enum-typed status: must use direct assignment *)
  InitSpan(s1, "request", Active, 100);
  WriteString("s1="); WriteCard(ORD(s1.status), 1); WriteLn;
  WriteString("d1="); WriteCard(s1.duration, 1); WriteLn;

  (* Enum field-to-field: s2.status := s1.status *)
  InitSpan(s2, "child", Cancelled, 0);
  s2.status := s1.status;
  WriteString("s2="); WriteCard(ORD(s2.status), 1); WriteLn;

  (* Enum literal: s2.status := Error *)
  s2.status := Error;
  WriteString("s3="); WriteCard(ORD(s2.status), 1); WriteLn;

  (* Enum from variable: st := Completed; s2.status := st *)
  st := Completed;
  s2.status := st;
  WriteString("s4="); WriteCard(ORD(s2.status), 1); WriteLn;

  (* CARDINAL field via array index: items[i].duration := scalar *)
  i := 0;
  InitSpan(items[0], "a", Active, 0);
  items[i].duration := 999;
  WriteString("dur="); WriteCard(items[i].duration, 1); WriteLn;

  (* Enum field via array index: items[i].status := enum *)
  items[i].status := Cancelled;
  WriteString("s5="); WriteCard(ORD(items[i].status), 1); WriteLn;

  WriteString("ok"); WriteLn
END EnumArrayMain.
