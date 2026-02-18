MODULE StrTest;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

TYPE
  String20 = ARRAY [0..19] OF CHAR;

VAR
  s1, s2: String20;
  ch: CHAR;

BEGIN
  (* String assignment *)
  s1 := "Hello";
  WriteString("s1 = "); WriteString(s1); WriteLn;

  (* String comparison *)
  s2 := "Hello";
  IF s1 = s2 THEN
    WriteString("s1 = s2: TRUE")
  ELSE
    WriteString("s1 = s2: FALSE")
  END;
  WriteLn;

  s2 := "World";
  IF s1 < s2 THEN
    WriteString("Hello < World: TRUE")
  ELSE
    WriteString("Hello < World: FALSE")
  END;
  WriteLn;

  (* Char extraction *)
  ch := s1[0];
  WriteString("s1[0] = ");
  IF ch = 'H' THEN
    WriteString("H")
  ELSE
    WriteString("not H")
  END;
  WriteLn
END StrTest.
