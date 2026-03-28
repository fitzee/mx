MODULE ProcLocalDecls;
(* Adversarial test: all PIM4 local declaration kinds inside procedures.
   Exercises proc-local TYPE, CONST, VAR, and nested PROCEDURE. *)

FROM InOut IMPORT WriteString, WriteInt, WriteLn;

PROCEDURE TestLocalTypes;
TYPE
  SmallBuf = ARRAY [0..7] OF CHAR;
  Color = (Red, Green, Blue);
VAR
  buf: SmallBuf;
  c: Color;
BEGIN
  buf := "hello";
  c := Green;
  WriteString("buf=");
  WriteString(buf);
  WriteLn;
  WriteString("color=");
  WriteInt(ORD(c), 0);
  WriteLn;
END TestLocalTypes;

PROCEDURE TestLocalConsts;
CONST
  MaxLen = 15;
  Pi = 3;
  Greeting = "hi";
TYPE
  Buf = ARRAY [0..MaxLen] OF CHAR;
VAR
  b: Buf;
  n: INTEGER;
BEGIN
  b := Greeting;
  n := MaxLen + Pi;
  WriteString("const=");
  WriteInt(n, 0);
  WriteLn;
  WriteString("cbuf=");
  WriteString(b);
  WriteLn;
END TestLocalConsts;

PROCEDURE TestNestedProc;
VAR total: INTEGER;

  PROCEDURE Add(a, b: INTEGER): INTEGER;
  BEGIN
    RETURN a + b
  END Add;

  PROCEDURE Accumulate(n: INTEGER);
  BEGIN
    total := total + n
  END Accumulate;

BEGIN
  total := 0;
  Accumulate(Add(3, 4));
  Accumulate(Add(10, 20));
  WriteString("nested=");
  WriteInt(total, 0);
  WriteLn;
END TestNestedProc;

PROCEDURE TestConstBeforeType;
CONST Size = 3;
TYPE
  Vec = ARRAY [0..Size-1] OF INTEGER;
VAR
  v: Vec;
BEGIN
  v[0] := 100;
  v[1] := 200;
  v[2] := 300;
  WriteString("vec=");
  WriteInt(v[0] + v[1] + v[2], 0);
  WriteLn;
END TestConstBeforeType;

BEGIN
  TestLocalTypes;
  TestLocalConsts;
  TestNestedProc;
  TestConstBeforeType;
END ProcLocalDecls.
