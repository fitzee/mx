MODULE ProcVar;
(* Test procedure variables and procedure types *)
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

TYPE
  IntFunc = PROCEDURE(INTEGER): INTEGER;
  VoidProc = PROCEDURE;
  BinOp = PROCEDURE(INTEGER, INTEGER): INTEGER;

VAR
  f: IntFunc;
  g: VoidProc;
  op: BinOp;
  result: INTEGER;

PROCEDURE Double(x: INTEGER): INTEGER;
BEGIN
  RETURN x * 2
END Double;

PROCEDURE Triple(x: INTEGER): INTEGER;
BEGIN
  RETURN x * 3
END Triple;

PROCEDURE Greet;
BEGIN
  WriteString("Hello from procedure variable!"); WriteLn
END Greet;

PROCEDURE Add(a, b: INTEGER): INTEGER;
BEGIN
  RETURN a + b
END Add;

PROCEDURE Mul(a, b: INTEGER): INTEGER;
BEGIN
  RETURN a * b
END Mul;

PROCEDURE Apply(fn: IntFunc; val: INTEGER): INTEGER;
BEGIN
  RETURN fn(val)
END Apply;

PROCEDURE ApplyBin(fn: BinOp; a, b: INTEGER): INTEGER;
BEGIN
  RETURN fn(a, b)
END ApplyBin;

BEGIN
  (* Test basic procedure variable assignment and call *)
  f := Double;
  result := f(5);
  WriteString("Double(5) = "); WriteInt(result, 1); WriteLn;

  f := Triple;
  result := f(5);
  WriteString("Triple(5) = "); WriteInt(result, 1); WriteLn;

  (* Test void procedure variable *)
  g := Greet;
  g;

  (* Test binary operation procedure variable *)
  op := Add;
  result := op(3, 4);
  WriteString("Add(3,4) = "); WriteInt(result, 1); WriteLn;

  op := Mul;
  result := op(3, 4);
  WriteString("Mul(3,4) = "); WriteInt(result, 1); WriteLn;

  (* Test passing procedure as parameter *)
  result := Apply(Double, 7);
  WriteString("Apply(Double,7) = "); WriteInt(result, 1); WriteLn;

  result := Apply(Triple, 7);
  WriteString("Apply(Triple,7) = "); WriteInt(result, 1); WriteLn;

  result := ApplyBin(Add, 10, 20);
  WriteString("ApplyBin(Add,10,20) = "); WriteInt(result, 1); WriteLn;

  result := ApplyBin(Mul, 10, 20);
  WriteString("ApplyBin(Mul,10,20) = "); WriteInt(result, 1); WriteLn;

  WriteString("Done"); WriteLn
END ProcVar.
