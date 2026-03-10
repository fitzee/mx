MODULE NestedVariant;

FROM InOut IMPORT WriteString, WriteInt, WriteLn;

TYPE
  Kind = (circle, rect, tri);
  Shape = RECORD
    x, y: INTEGER;
    CASE kind: Kind OF
      circle: radius: CARDINAL |
      rect:   width, height: CARDINAL;
              CASE fill: Kind OF
                circle: fillRadius: CARDINAL |
                rect:   fillW, fillH: CARDINAL
              END |
      tri:    base, side: CARDINAL
    END
  END;

VAR
  s: Shape;

BEGIN
  s.x := 10;
  s.y := 20;
  s.kind := rect;
  s.width := 100;
  s.height := 50;
  s.fill := rect;
  s.fillW := 80;
  s.fillH := 40;

  WriteString("x="); WriteInt(s.x, 1);
  WriteString(" y="); WriteInt(s.y, 1);
  WriteString(" w="); WriteInt(s.width, 1);
  WriteString(" h="); WriteInt(s.height, 1);
  WriteString(" fw="); WriteInt(s.fillW, 1);
  WriteString(" fh="); WriteInt(s.fillH, 1);
  WriteLn
END NestedVariant.
