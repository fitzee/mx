MODULE LFloat;
(* Test LFLOAT and FLOAT builtins *)
FROM InOut IMPORT WriteString, WriteInt, WriteLn;
FROM RealInOut IMPORT WriteFixPt;

VAR
  i: INTEGER;
  r: REAL;
  lr: LONGREAL;

BEGIN
  i := 42;

  (* FLOAT: INTEGER -> REAL *)
  r := FLOAT(i);
  WriteString("FLOAT(42) = "); WriteFixPt(r, 10, 2); WriteLn;

  (* LFLOAT: INTEGER -> LONGREAL *)
  lr := LFLOAT(i);
  WriteString("LFLOAT(42) = "); WriteFixPt(lr, 10, 2); WriteLn;

  (* TRUNC: REAL -> INTEGER *)
  r := 3.7;
  i := TRUNC(r);
  WriteString("TRUNC(3.7) = "); WriteInt(i, 1); WriteLn;

  (* Arithmetic with FLOAT *)
  r := FLOAT(10) / FLOAT(3);
  WriteString("FLOAT(10)/FLOAT(3) = "); WriteFixPt(r, 10, 4); WriteLn;

  (* Arithmetic with LFLOAT *)
  lr := LFLOAT(10) / LFLOAT(3);
  WriteString("LFLOAT(10)/LFLOAT(3) = "); WriteFixPt(lr, 10, 4); WriteLn;

  WriteString("Done"); WriteLn
END LFloat.
