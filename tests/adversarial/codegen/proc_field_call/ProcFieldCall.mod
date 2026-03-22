MODULE ProcFieldCall;
(* Test indirect calls through procedure-type fields in records,
   accessed via pointer dereference: cp^.callback(args) *)
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

TYPE
  TransformProc = PROCEDURE(INTEGER): INTEGER;
  Context = RECORD
    fn: TransformProc;
    label: ARRAY [0..15] OF CHAR;
    value: INTEGER;
  END;
  CtxPtr = POINTER TO Context;

PROCEDURE Double(x: INTEGER): INTEGER;
BEGIN
  RETURN x * 2
END Double;

PROCEDURE Triple(x: INTEGER): INTEGER;
BEGIN
  RETURN x * 3
END Triple;

PROCEDURE Apply(cp: CtxPtr);
VAR result: INTEGER;
BEGIN
  result := cp^.fn(cp^.value);
  WriteString(cp^.label);
  WriteInt(result, 1);
  WriteLn
END Apply;

VAR
  c1, c2: Context;
  p: CtxPtr;

BEGIN
  c1.fn := Double;
  c1.label := "double=";
  c1.value := 7;
  p := ADR(c1);
  Apply(p);

  c2.fn := Triple;
  c2.label := "triple=";
  c2.value := 5;
  p := ADR(c2);
  Apply(p);

  WriteString("done"); WriteLn
END ProcFieldCall.
