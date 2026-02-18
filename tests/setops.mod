MODULE SetOps;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

VAR
  s: BITSET;
  i: INTEGER;

BEGIN
  s := {1, 3, 5, 7};
  WriteString("Set bits: ");
  FOR i := 0 TO 15 DO
    IF i IN s THEN
      WriteInt(i, 3)
    END
  END;
  WriteLn;

  INCL(s, 4);
  EXCL(s, 3);
  WriteString("After INCL(4), EXCL(3): ");
  FOR i := 0 TO 15 DO
    IF i IN s THEN
      WriteInt(i, 3)
    END
  END;
  WriteLn
END SetOps.
