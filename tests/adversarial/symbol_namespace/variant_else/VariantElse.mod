MODULE VariantElse;
FROM InOut IMPORT WriteString, WriteLn, WriteInt;

TYPE
  Shape = RECORD
    CASE kind: CARDINAL OF
      1: x, y: INTEGER |
      2: radius: INTEGER
    ELSE
      code: INTEGER
    END
  END;

VAR
  s: Shape;

BEGIN
  s.kind := 1;
  s.x := 10;
  s.y := 20;
  WriteString("x=");
  WriteInt(s.x, 1);
  WriteLn;
  WriteString("y=");
  WriteInt(s.y, 1);
  WriteLn;

  s.kind := 2;
  s.radius := 42;
  WriteString("r=");
  WriteInt(s.radius, 1);
  WriteLn;

  s.kind := 99;
  s.code := 7;
  WriteString("code=");
  WriteInt(s.code, 1);
  WriteLn;
END VariantElse.
