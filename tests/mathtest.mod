MODULE MathTest;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;
FROM RealInOut IMPORT WriteReal, WriteFixPt;
FROM MathLib0 IMPORT sqrt, sin, cos, exp, ln;

VAR
  x, y: REAL;
  i: INTEGER;

BEGIN
  (* Test sqrt *)
  x := sqrt(4.0);
  WriteString("sqrt(4.0) = "); WriteFixPt(x, 8, 4); WriteLn;

  x := sqrt(2.0);
  WriteString("sqrt(2.0) = "); WriteFixPt(x, 8, 4); WriteLn;

  (* Test sin/cos *)
  x := sin(0.0);
  WriteString("sin(0.0) = "); WriteFixPt(x, 8, 4); WriteLn;

  x := cos(0.0);
  WriteString("cos(0.0) = "); WriteFixPt(x, 8, 4); WriteLn;

  (* Test exp/ln *)
  x := exp(1.0);
  WriteString("exp(1.0) = "); WriteFixPt(x, 8, 4); WriteLn;

  x := ln(1.0);
  WriteString("ln(1.0) = "); WriteFixPt(x, 8, 4); WriteLn;

  (* Test FLOAT and TRUNC *)
  i := 42;
  x := FLOAT(i);
  WriteString("FLOAT(42) = "); WriteFixPt(x, 8, 1); WriteLn;

  x := 3.7;
  i := TRUNC(x);
  WriteString("TRUNC(3.7) = "); WriteInt(i, 1); WriteLn;

  x := -2.9;
  i := TRUNC(x);
  WriteString("TRUNC(-2.9) = "); WriteInt(i, 1); WriteLn;

  WriteString("Done"); WriteLn
END MathTest.
