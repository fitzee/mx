MODULE UseModule;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;
IMPORT MathUtils;

BEGIN
  WriteString("Square(7) = "); WriteInt(MathUtils.Square(7), 1); WriteLn;
  WriteString("Cube(3) = "); WriteInt(MathUtils.Cube(3), 1); WriteLn
END UseModule.
