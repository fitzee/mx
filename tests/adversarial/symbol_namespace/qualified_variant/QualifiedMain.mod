MODULE QualifiedMain;
FROM InOut IMPORT WriteInt, WriteLn;
IMPORT QA;
IMPORT QB;
VAR a, b, c: INTEGER;
BEGIN
  a := QA.MakeOK();
  b := QA.MakeErr();
  c := QB.MakeTimeout();
  WriteInt(a, 0); WriteLn;
  WriteInt(b, 0); WriteLn;
  WriteInt(c, 0); WriteLn
END QualifiedMain.
