MODULE Records;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

TYPE
  Point = RECORD
    x, y: INTEGER
  END;

VAR p: Point;

PROCEDURE PrintPoint(p: Point);
BEGIN
  WriteString("(");
  WriteInt(p.x, 0);
  WriteString(", ");
  WriteInt(p.y, 0);
  WriteString(")")
END PrintPoint;

BEGIN
  p.x := 10;
  p.y := 20;
  WriteString("Point: ");
  PrintPoint(p);
  WriteLn
END Records.
