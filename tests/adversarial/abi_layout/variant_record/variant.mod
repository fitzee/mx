MODULE Variant;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

TYPE
  ShapeKind = (Circle, Rectangle, Triangle);
  Shape = RECORD
    x, y: INTEGER;
    CASE kind: ShapeKind OF
      Circle:    radius: INTEGER |
      Rectangle: width, height: INTEGER |
      Triangle:  base, side: INTEGER
    END
  END;

VAR
  s: Shape;

BEGIN
  s.x := 10;
  s.y := 20;
  s.kind := 0;  (* Circle *)
  s.radius := 5;
  WriteString("Circle at (");
  WriteInt(s.x, 1); WriteString(", ");
  WriteInt(s.y, 1); WriteString(") r=");
  WriteInt(s.radius, 1);
  WriteLn;

  s.kind := 1;  (* Rectangle *)
  s.width := 30;
  s.height := 40;
  WriteString("Rect at (");
  WriteInt(s.x, 1); WriteString(", ");
  WriteInt(s.y, 1); WriteString(") ");
  WriteInt(s.width, 1); WriteString("x");
  WriteInt(s.height, 1);
  WriteLn
END Variant.
