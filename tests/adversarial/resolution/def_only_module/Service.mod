IMPLEMENTATION MODULE Service;

FROM SharedTypes IMPORT Color, Coord, Status, Red, StOk;
FROM Backend IMPORT Handle, Open;

VAR
  gColor: Color;
  gCoord: Coord;

PROCEDURE Init(VAR h: Handle): Status;
VAR ok: BOOLEAN;
BEGIN
  ok := Open(h);
  gColor := Red;
  gCoord.x := 0;
  gCoord.y := 0;
  RETURN StOk
END Init;

PROCEDURE SetColor(h: Handle; c: Color): Status;
BEGIN
  gColor := c;
  RETURN StOk
END SetColor;

PROCEDURE GetCoord(h: Handle; VAR p: Coord): Status;
BEGIN
  p := gCoord;
  RETURN StOk
END GetCoord;

END Service.
