MODULE RecordParamCross;
(* Regression: LLVM backend type mismatches for:
   1. Record passed by value to cross-module procedure
   2. Pointer deref → field access → open array param (ptr^.charArray
      passed to PROCEDURE(VAR ARRAY OF CHAR)) *)

FROM SYSTEM IMPORT ADDRESS, ADR;
FROM SinkLib IMPORT Sink, Logger, Node, NodePtr,
                    Init, MakeSink, AddSink, Dispatch;
FROM Strings IMPORT Assign;
FROM Storage IMPORT ALLOCATE;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

VAR
  logger: Logger;
  sink: Sink;
  ok: BOOLEAN;
  np: NodePtr;
  buf: ARRAY [0..63] OF CHAR;

PROCEDURE MyHandler(ctx: ADDRESS; msg: INTEGER);
BEGIN
  WriteString("msg=");
  WriteInt(msg, 0);
  WriteLn
END MyHandler;

PROCEDURE ExtractName(p: NodePtr; VAR out: ARRAY OF CHAR);
BEGIN
  Assign(p^.name, out)
END ExtractName;

BEGIN
  Init(logger);
  sink := MakeSink(MyHandler, 0);
  ok := AddSink(logger, sink);
  WriteString("added=");
  IF ok THEN WriteString("TRUE") ELSE WriteString("FALSE") END;
  WriteLn;
  Dispatch(logger, 42);

  ALLOCATE(np, 72);
  np^.tag := 1;
  Assign("hello", np^.name);
  np^.value := 99;
  ExtractName(np, buf);
  WriteString("name=");
  WriteString(buf);
  WriteLn
END RecordParamCross.
