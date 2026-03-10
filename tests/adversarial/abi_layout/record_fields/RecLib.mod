IMPLEMENTATION MODULE RecLib;
PROCEDURE MakePoint(px, py: INTEGER; VAR p: Point);
BEGIN
  p.x := px;
  p.y := py
END MakePoint;
PROCEDURE SumPoint(p: Point): INTEGER;
BEGIN
  RETURN p.x + p.y
END SumPoint;
PROCEDURE InitInfo(VAR i: Info; t, v: INTEGER);
BEGIN
  i.tag := t;
  i.flag := TRUE;
  i.ch := "A";
  i.val := v
END InitInfo;
PROCEDURE InfoTag(i: Info): INTEGER;
BEGIN
  RETURN i.tag
END InfoTag;
END RecLib.
