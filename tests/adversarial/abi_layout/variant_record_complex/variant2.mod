MODULE Variant2;
(* Test variant records with multiple variants *)
FROM InOut IMPORT WriteString, WriteInt, WriteLn;
FROM RealInOut IMPORT WriteFixPt;

TYPE
  ShapeKind = (SKCircle, SKRect, SKTriangle);
  Shape = RECORD
    name: ARRAY [0..19] OF CHAR;
    CASE kind: ShapeKind OF
      SKCircle:   radius: REAL |
      SKRect:     width, height: REAL |
      SKTriangle: base, side1, side2: REAL
    END
  END;

VAR
  s: Shape;

PROCEDURE PrintShape(s: Shape);
BEGIN
  WriteString(s.name);
  WriteString(": ");
  CASE s.kind OF
    SKCircle:
      WriteString("Circle r="); WriteFixPt(s.radius, 6, 2) |
    SKRect:
      WriteString("Rect ");
      WriteFixPt(s.width, 6, 2); WriteString("x");
      WriteFixPt(s.height, 6, 2) |
    SKTriangle:
      WriteString("Tri ");
      WriteFixPt(s.base, 6, 2); WriteString(" ");
      WriteFixPt(s.side1, 6, 2); WriteString(" ");
      WriteFixPt(s.side2, 6, 2)
  END;
  WriteLn
END PrintShape;

BEGIN
  s.name := "MyCircle";
  s.kind := SKCircle;
  s.radius := 5.0;
  PrintShape(s);

  s.name := "MyRect";
  s.kind := SKRect;
  s.width := 10.0;
  s.height := 20.0;
  PrintShape(s);

  s.name := "MyTriangle";
  s.kind := SKTriangle;
  s.base := 3.0;
  s.side1 := 4.0;
  s.side2 := 5.0;
  PrintShape(s);

  WriteString("Done"); WriteLn
END Variant2.
