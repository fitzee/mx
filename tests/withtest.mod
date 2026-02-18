MODULE WithTest;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

TYPE
  Point = RECORD
    x, y: INTEGER
  END;
  Rect = RECORD
    origin: Point;
    width, height: INTEGER
  END;

VAR
  p: Point;
  r: Rect;

PROCEDURE PrintPoint(pt: Point);
BEGIN
  WriteString("(");
  WriteInt(pt.x, 1);
  WriteString(", ");
  WriteInt(pt.y, 1);
  WriteString(")")
END PrintPoint;

BEGIN
  (* Simple WITH on a record *)
  WITH p DO
    x := 10;
    y := 20
  END;
  WriteString("Point: "); PrintPoint(p); WriteLn;

  (* WITH on nested record *)
  WITH r DO
    origin.x := 5;
    origin.y := 15;
    width := 100;
    height := 50
  END;
  WriteString("Rect origin: "); PrintPoint(r.origin); WriteLn;
  WriteString("Rect size: ");
  WriteInt(r.width, 1); WriteString("x"); WriteInt(r.height, 1); WriteLn
END WithTest.
