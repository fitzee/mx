MODULE WithTest2;
(* Advanced WITH statement tests - nested WITH and record of records *)
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

TYPE
  Point = RECORD x, y: INTEGER END;
  Rect = RECORD
    origin: Point;
    size: Point
  END;
  Circle = RECORD
    center: Point;
    radius: INTEGER
  END;

VAR
  r: Rect;
  c: Circle;

BEGIN
  (* Initialize rect *)
  r.origin.x := 10;
  r.origin.y := 20;
  r.size.x := 100;
  r.size.y := 50;

  (* WITH on outer record *)
  WITH r DO
    WriteString("Rect origin: (");
    WriteInt(origin.x, 1); WriteString(", ");
    WriteInt(origin.y, 1); WriteString(")"); WriteLn;
    WriteString("Rect size: ");
    WriteInt(size.x, 1); WriteString("x");
    WriteInt(size.y, 1); WriteLn;
    WriteString("Area: ");
    WriteInt(size.x * size.y, 1); WriteLn
  END;

  (* Initialize circle *)
  c.center.x := 50;
  c.center.y := 60;
  c.radius := 25;

  WITH c DO
    WriteString("Circle center: (");
    WriteInt(center.x, 1); WriteString(", ");
    WriteInt(center.y, 1); WriteString(")"); WriteLn;
    WriteString("Radius: "); WriteInt(radius, 1); WriteLn
  END;

  (* Modify via WITH *)
  WITH r DO
    origin.x := 0;
    origin.y := 0;
    size.x := 200;
    size.y := 100
  END;
  WriteString("Modified rect: (");
  WriteInt(r.origin.x, 1); WriteString(",");
  WriteInt(r.origin.y, 1); WriteString(") ");
  WriteInt(r.size.x, 1); WriteString("x");
  WriteInt(r.size.y, 1); WriteLn;

  WriteString("Done"); WriteLn
END WithTest2.
