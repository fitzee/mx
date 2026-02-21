MODULE EnumCollisionMain;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;
FROM A IMPORT GetVal;
IMPORT B;
VAR a, b: INTEGER;
BEGIN
  a := GetVal();
  b := B.GetVal();
  WriteInt(a, 0); WriteLn;
  WriteInt(b, 0); WriteLn
END EnumCollisionMain.
