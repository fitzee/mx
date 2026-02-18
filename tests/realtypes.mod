MODULE RealTypes;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;
FROM RealInOut IMPORT WriteReal, WriteFixPt;
FROM MathLib0 IMPORT sqrt;

VAR
  r: REAL;
  lr: LONGREAL;
  i: INTEGER;

BEGIN
  (* REAL arithmetic *)
  r := 3.14;
  WriteString("PI ~= "); WriteFixPt(r, 8, 4); WriteLn;

  r := r * 2.0;
  WriteString("2*PI ~= "); WriteFixPt(r, 8, 4); WriteLn;

  (* LONGREAL *)
  lr := 2.718281828;
  WriteString("e ~= "); WriteFixPt(lr, 12, 8); WriteLn;

  (* Mixed integer/real *)
  i := 7;
  r := FLOAT(i);
  WriteString("FLOAT(7) = "); WriteFixPt(r, 8, 1); WriteLn;

  r := sqrt(2.0);
  WriteString("sqrt(2) = "); WriteFixPt(r, 10, 6); WriteLn;

  i := TRUNC(r);
  WriteString("TRUNC(sqrt(2)) = "); WriteInt(i, 1); WriteLn;

  (* Real comparisons *)
  r := 3.14;
  IF r > 3.0 THEN
    WriteString("3.14 > 3.0: YES")
  ELSE
    WriteString("3.14 > 3.0: NO")
  END;
  WriteLn;

  IF r < 4.0 THEN
    WriteString("3.14 < 4.0: YES")
  ELSE
    WriteString("3.14 < 4.0: NO")
  END;
  WriteLn;

  WriteString("Done"); WriteLn
END RealTypes.
