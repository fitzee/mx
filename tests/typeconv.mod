MODULE TypeConv;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

VAR
  i: INTEGER;
  c: CARDINAL;
  ch: CHAR;
  b: BOOLEAN;

BEGIN
  (* INTEGER to CARDINAL *)
  i := 42;
  c := VAL(CARDINAL, i);
  WriteString("VAL(CARDINAL, 42) = "); WriteInt(VAL(INTEGER, c), 1); WriteLn;

  (* CARDINAL to INTEGER *)
  c := 100;
  i := VAL(INTEGER, c);
  WriteString("VAL(INTEGER, 100) = "); WriteInt(i, 1); WriteLn;

  (* CHR and ORD *)
  ch := CHR(65);
  WriteString("CHR(65) = "); WriteInt(ORD(ch), 1); WriteLn;

  i := ORD('Z');
  WriteString("ORD('Z') = "); WriteInt(i, 1); WriteLn;

  (* ODD *)
  WriteString("ODD(7) = ");
  IF ODD(7) THEN WriteString("TRUE") ELSE WriteString("FALSE") END; WriteLn;
  WriteString("ODD(8) = ");
  IF ODD(8) THEN WriteString("TRUE") ELSE WriteString("FALSE") END; WriteLn;

  (* ABS *)
  WriteString("ABS(-99) = "); WriteInt(ABS(-99), 1); WriteLn;

  (* SIZE *)
  WriteString("SIZE(INTEGER) = "); WriteInt(SIZE(INTEGER), 1); WriteLn;
  WriteString("SIZE(CHAR) = "); WriteInt(SIZE(CHAR), 1); WriteLn;

  (* FLOAT and TRUNC *)
  i := TRUNC(3.99);
  WriteString("TRUNC(3.99) = "); WriteInt(i, 1); WriteLn;

  (* MAX/MIN *)
  WriteString("MAX(INTEGER) = "); WriteInt(MAX(INTEGER), 1); WriteLn
END TypeConv.
