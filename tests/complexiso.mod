MODULE ComplexISO;
(* Test ISO Modula-2 COMPLEX and LONGCOMPLEX types *)
FROM InOut IMPORT WriteString, WriteLn;
FROM RealInOut IMPORT WriteFixPt;

VAR
  a, b, c: COMPLEX;
  d: LONGCOMPLEX;
  r: REAL;

BEGIN
  a := CMPLX(3.0, 4.0);
  b := CMPLX(1.0, 2.0);

  WriteString("a = "); WriteFixPt(RE(a), 6, 1); WriteString(" + ");
  WriteFixPt(IM(a), 6, 1); WriteString("i"); WriteLn;

  WriteString("b = "); WriteFixPt(RE(b), 6, 1); WriteString(" + ");
  WriteFixPt(IM(b), 6, 1); WriteString("i"); WriteLn;

  (* Addition *)
  c := a + b;
  WriteString("a+b = "); WriteFixPt(RE(c), 6, 1); WriteString(" + ");
  WriteFixPt(IM(c), 6, 1); WriteString("i"); WriteLn;

  (* Subtraction *)
  c := a - b;
  WriteString("a-b = "); WriteFixPt(RE(c), 6, 1); WriteString(" + ");
  WriteFixPt(IM(c), 6, 1); WriteString("i"); WriteLn;

  (* Multiplication: (3+4i)(1+2i) = 3+6i+4i+8i² = 3+10i-8 = -5+10i *)
  c := a * b;
  WriteString("a*b = "); WriteFixPt(RE(c), 6, 1); WriteString(" + ");
  WriteFixPt(IM(c), 6, 1); WriteString("i"); WriteLn;

  (* Division *)
  c := a / b;
  WriteString("a/b = "); WriteFixPt(RE(c), 6, 1); WriteString(" + ");
  WriteFixPt(IM(c), 6, 1); WriteString("i"); WriteLn;

  (* Negation *)
  c := -a;
  WriteString("-a = "); WriteFixPt(RE(c), 6, 1); WriteString(" + ");
  WriteFixPt(IM(c), 6, 1); WriteString("i"); WriteLn;

  (* Equality *)
  a := CMPLX(1.0, 2.0);
  IF a = b THEN WriteString("a = b: TRUE") ELSE WriteString("a = b: FALSE") END; WriteLn;

  c := CMPLX(5.0, 0.0);
  IF a = c THEN WriteString("a = c: TRUE") ELSE WriteString("a = c: FALSE") END; WriteLn;

  WriteString("Done"); WriteLn
END ComplexISO.
