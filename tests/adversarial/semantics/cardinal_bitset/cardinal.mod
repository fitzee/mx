MODULE Cardinal;
FROM InOut IMPORT WriteString, WriteInt, WriteCard, WriteLn;

VAR
  c: CARDINAL;
  i: INTEGER;
  b: BITSET;

BEGIN
  (* CARDINAL operations *)
  c := 42;
  WriteString("Cardinal: "); WriteCard(c, 1); WriteLn;

  c := c + 10;
  WriteString("After +10: "); WriteCard(c, 1); WriteLn;

  c := c * 2;
  WriteString("After *2: "); WriteCard(c, 1); WriteLn;

  c := c DIV 3;
  WriteString("After DIV 3: "); WriteCard(c, 1); WriteLn;

  c := c MOD 7;
  WriteString("After MOD 7: "); WriteCard(c, 1); WriteLn;

  (* BITSET operations *)
  b := {0, 1, 4, 7};
  WriteString("BITSET {0,1,4,7}"); WriteLn;

  IF 4 IN b THEN
    WriteString("4 IN b: YES")
  ELSE
    WriteString("4 IN b: NO")
  END;
  WriteLn;

  IF 3 IN b THEN
    WriteString("3 IN b: YES")
  ELSE
    WriteString("3 IN b: NO")
  END;
  WriteLn;

  INCL(b, 3);
  IF 3 IN b THEN
    WriteString("After INCL(3): YES")
  ELSE
    WriteString("After INCL(3): NO")
  END;
  WriteLn;

  EXCL(b, 1);
  IF 1 IN b THEN
    WriteString("After EXCL(1): YES")
  ELSE
    WriteString("After EXCL(1): NO")
  END;
  WriteLn;

  (* VAL type transfer *)
  i := 65;
  WriteString("VAL(CARDINAL, 65) = "); WriteCard(VAL(CARDINAL, i), 1); WriteLn;

  (* SIZE *)
  WriteString("SIZE(INTEGER) = "); WriteInt(SIZE(INTEGER), 1); WriteLn;
  WriteString("SIZE(CARDINAL) = "); WriteInt(SIZE(CARDINAL), 1); WriteLn;
  WriteString("SIZE(CHAR) = "); WriteInt(SIZE(CHAR), 1); WriteLn;
  WriteString("SIZE(BOOLEAN) = "); WriteInt(SIZE(BOOLEAN), 1); WriteLn;

  WriteString("Done"); WriteLn
END Cardinal.
