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
  s.variant.v0.radius := 5;
  WriteString("Circle at (");
  WriteInt(s.x, 1); WriteString(", ");
  WriteInt(s.y, 1); WriteString(") r=");
  WriteInt(s.variant.v0.radius, 1);
  WriteLn;

  s.kind := 1;  (* Rectangle *)
  s.variant.v1.width := 30;
  s.variant.v1.height := 40;
  WriteString("Rect at (");
  WriteInt(s.x, 1); WriteString(", ");
  WriteInt(s.y, 1); WriteString(") ");
  WriteInt(s.variant.v1.width, 1); WriteString("x");
  WriteInt(s.variant.v1.height, 1);
  WriteLn
END Variant.
