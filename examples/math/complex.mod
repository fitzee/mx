MODULE Complex;
(* Complex number arithmetic using records and procedure types *)
FROM InOut IMPORT WriteString, WriteLn;
FROM RealInOut IMPORT WriteFixPt;

TYPE
  Complex = RECORD
    re, im: REAL
  END;
  ComplexOp = PROCEDURE(Complex, Complex): Complex;

VAR
  a, b, c: Complex;

PROCEDURE MakeComplex(re, im: REAL): Complex;
  VAR r: Complex;
BEGIN
  r.re := re;
  r.im := im;
  RETURN r
END MakeComplex;

PROCEDURE Add(a, b: Complex): Complex;
  VAR r: Complex;
BEGIN
  r.re := a.re + b.re;
  r.im := a.im + b.im;
  RETURN r
END Add;

PROCEDURE Sub(a, b: Complex): Complex;
  VAR r: Complex;
BEGIN
  r.re := a.re - b.re;
  r.im := a.im - b.im;
  RETURN r
END Sub;

PROCEDURE Mul(a, b: Complex): Complex;
  VAR r: Complex;
BEGIN
  r.re := a.re * b.re - a.im * b.im;
  r.im := a.re * b.im + a.im * b.re;
  RETURN r
END Mul;

PROCEDURE Magnitude2(a: Complex): REAL;
BEGIN
  RETURN a.re * a.re + a.im * a.im
END Magnitude2;

PROCEDURE Print(label: ARRAY OF CHAR; c: Complex);
BEGIN
  WriteString(label);
  WriteFixPt(c.re, 8, 2);
  WriteString(" + ");
  WriteFixPt(c.im, 8, 2);
  WriteString("i");
  WriteLn
END Print;

PROCEDURE Apply(op: ComplexOp; x, y: Complex): Complex;
BEGIN
  RETURN op(x, y)
END Apply;

BEGIN
  a := MakeComplex(3.0, 4.0);
  b := MakeComplex(1.0, 2.0);

  Print("a = ", a);
  Print("b = ", b);

  c := Add(a, b);
  Print("a+b = ", c);

  c := Sub(a, b);
  Print("a-b = ", c);

  c := Mul(a, b);
  Print("a*b = ", c);

  WriteString("|a|^2 = "); WriteFixPt(Magnitude2(a), 8, 2); WriteLn;

  (* Test procedure type variable *)
  c := Apply(Add, a, b);
  Print("Apply(Add) = ", c);

  c := Apply(Mul, a, b);
  Print("Apply(Mul) = ", c);

  WriteString("Done"); WriteLn
END Complex.
