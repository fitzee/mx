IMPLEMENTATION MODULE DrawAlgo;

(* ---- Bresenham Line ---- *)

PROCEDURE Line(ctx: Ctx; pt: PointFn;
               x1, y1, x2, y2: INTEGER);
VAR dx, dy, sx, sy, err, e2: INTEGER;
BEGIN
  dx := x2 - x1; IF dx < 0 THEN dx := -dx END;
  dy := y2 - y1; IF dy < 0 THEN dy := -dy END;
  IF x1 < x2 THEN sx := 1 ELSE sx := -1 END;
  IF y1 < y2 THEN sy := 1 ELSE sy := -1 END;
  err := dx - dy;
  LOOP
    pt(ctx, x1, y1);
    IF (x1 = x2) AND (y1 = y2) THEN EXIT END;
    e2 := 2 * err;
    IF e2 > -dy THEN err := err - dy; x1 := x1 + sx END;
    IF e2 < dx THEN err := err + dx; y1 := y1 + sy END
  END
END Line;

(* ---- Midpoint Circle ---- *)

PROCEDURE Circle(ctx: Ctx; pt: PointFn;
                 cx, cy, radius: INTEGER);
VAR x, y, d: INTEGER;
BEGIN
  IF radius < 0 THEN RETURN END;
  x := radius; y := 0; d := 1 - radius;
  WHILE x >= y DO
    pt(ctx, cx + x, cy + y);
    pt(ctx, cx - x, cy + y);
    pt(ctx, cx + x, cy - y);
    pt(ctx, cx - x, cy - y);
    pt(ctx, cx + y, cy + x);
    pt(ctx, cx - y, cy + x);
    pt(ctx, cx + y, cy - x);
    pt(ctx, cx - y, cy - x);
    INC(y);
    IF d <= 0 THEN
      d := d + 2 * y + 1
    ELSE
      DEC(x);
      d := d + 2 * (y - x) + 1
    END
  END
END Circle;

PROCEDURE FillCircle(ctx: Ctx; hl: HLineFn;
                     cx, cy, radius: INTEGER);
VAR x, y, d: INTEGER;
BEGIN
  IF radius < 0 THEN RETURN END;
  x := radius; y := 0; d := 1 - radius;
  WHILE x >= y DO
    hl(ctx, cx - x, cx + x, cy + y);
    hl(ctx, cx - x, cx + x, cy - y);
    hl(ctx, cx - y, cx + y, cy + x);
    hl(ctx, cx - y, cx + y, cy - x);
    INC(y);
    IF d <= 0 THEN
      d := d + 2 * y + 1
    ELSE
      DEC(x);
      d := d + 2 * (y - x) + 1
    END
  END
END FillCircle;

(* ---- Midpoint Ellipse — two-region ---- *)

PROCEDURE Ellipse(ctx: Ctx; pt: PointFn;
                  cx, cy, rx, ry: INTEGER);
VAR x, y: INTEGER;
    rx2, ry2, twoRx2, twoRy2, px, py, d1, d2: LONGREAL;
BEGIN
  IF (rx < 0) OR (ry < 0) THEN RETURN END;
  rx2 := LFLOAT(rx) * LFLOAT(rx);
  ry2 := LFLOAT(ry) * LFLOAT(ry);
  twoRx2 := 2.0 * rx2;
  twoRy2 := 2.0 * ry2;
  x := 0; y := ry;
  px := 0.0; py := twoRx2 * LFLOAT(y);
  d1 := ry2 - rx2 * LFLOAT(ry) + rx2 / 4.0;
  WHILE px < py DO
    pt(ctx, cx + x, cy + y);
    pt(ctx, cx - x, cy + y);
    pt(ctx, cx + x, cy - y);
    pt(ctx, cx - x, cy - y);
    INC(x);
    px := px + twoRy2;
    IF d1 < 0.0 THEN
      d1 := d1 + ry2 + px
    ELSE
      DEC(y);
      py := py - twoRx2;
      d1 := d1 + ry2 + px - py
    END
  END;
  d2 := ry2 * LFLOAT(2 * x + 1) * LFLOAT(2 * x + 1) / 4.0
      + rx2 * LFLOAT(y - 1) * LFLOAT(y - 1)
      - rx2 * ry2;
  WHILE y >= 0 DO
    pt(ctx, cx + x, cy + y);
    pt(ctx, cx - x, cy + y);
    pt(ctx, cx + x, cy - y);
    pt(ctx, cx - x, cy - y);
    DEC(y);
    py := py - twoRx2;
    IF d2 > 0.0 THEN
      d2 := d2 + rx2 - py
    ELSE
      INC(x);
      px := px + twoRy2;
      d2 := d2 + rx2 - py + px
    END
  END
END Ellipse;

PROCEDURE FillEllipse(ctx: Ctx; hl: HLineFn;
                      cx, cy, rx, ry: INTEGER);
VAR x, y, lastY: INTEGER;
    rx2, ry2, twoRx2, twoRy2, px, py, d1, d2: LONGREAL;
BEGIN
  IF (rx < 0) OR (ry < 0) THEN RETURN END;
  rx2 := LFLOAT(rx) * LFLOAT(rx);
  ry2 := LFLOAT(ry) * LFLOAT(ry);
  twoRx2 := 2.0 * rx2;
  twoRy2 := 2.0 * ry2;
  x := 0; y := ry;
  px := 0.0; py := twoRx2 * LFLOAT(y);
  lastY := -1;
  d1 := ry2 - rx2 * LFLOAT(ry) + rx2 / 4.0;
  WHILE px < py DO
    IF y # lastY THEN
      hl(ctx, cx - x, cx + x, cy + y);
      hl(ctx, cx - x, cx + x, cy - y);
      lastY := y
    END;
    INC(x);
    px := px + twoRy2;
    IF d1 < 0.0 THEN
      d1 := d1 + ry2 + px
    ELSE
      hl(ctx, cx - x, cx + x, cy + y);
      hl(ctx, cx - x, cx + x, cy - y);
      DEC(y);
      py := py - twoRx2;
      d1 := d1 + ry2 + px - py;
      lastY := y
    END
  END;
  d2 := ry2 * LFLOAT(2 * x + 1) * LFLOAT(2 * x + 1) / 4.0
      + rx2 * LFLOAT(y - 1) * LFLOAT(y - 1)
      - rx2 * ry2;
  WHILE y >= 0 DO
    hl(ctx, cx - x, cx + x, cy + y);
    hl(ctx, cx - x, cx + x, cy - y);
    DEC(y);
    py := py - twoRx2;
    IF d2 > 0.0 THEN
      d2 := d2 + rx2 - py
    ELSE
      INC(x);
      px := px + twoRy2;
      d2 := d2 + rx2 - py + px
    END
  END
END FillEllipse;

(* ---- Triangle ---- *)

PROCEDURE Triangle(ctx: Ctx; ln: LineFn;
                   x1, y1, x2, y2, x3, y3: INTEGER);
BEGIN
  ln(ctx, x1, y1, x2, y2);
  ln(ctx, x2, y2, x3, y3);
  ln(ctx, x3, y3, x1, y1)
END Triangle;

PROCEDURE FillTriangle(ctx: Ctx; hl: HLineFn;
                       x1, y1, x2, y2, x3, y3: INTEGER);
VAR vx0, vy0, vx1, vy1, vx2, vy2, tmp: INTEGER;
    scanY, xa, xb, minX, maxX: INTEGER;
BEGIN
  vx0 := x1; vy0 := y1;
  vx1 := x2; vy1 := y2;
  vx2 := x3; vy2 := y3;
  IF vy0 > vy1 THEN
    tmp := vx0; vx0 := vx1; vx1 := tmp;
    tmp := vy0; vy0 := vy1; vy1 := tmp
  END;
  IF vy0 > vy2 THEN
    tmp := vx0; vx0 := vx2; vx2 := tmp;
    tmp := vy0; vy0 := vy2; vy2 := tmp
  END;
  IF vy1 > vy2 THEN
    tmp := vx1; vx1 := vx2; vx2 := tmp;
    tmp := vy1; vy1 := vy2; vy2 := tmp
  END;
  IF vy0 = vy2 THEN
    minX := vx0;
    IF vx1 < minX THEN minX := vx1 END;
    IF vx2 < minX THEN minX := vx2 END;
    maxX := vx0;
    IF vx1 > maxX THEN maxX := vx1 END;
    IF vx2 > maxX THEN maxX := vx2 END;
    hl(ctx, minX, maxX, vy0);
    RETURN
  END;
  FOR scanY := vy0 TO vy2 DO
    IF vy2 # vy0 THEN
      xa := vx0 + TRUNC(LFLOAT(vx2 - vx0) * LFLOAT(scanY - vy0) / LFLOAT(vy2 - vy0))
    ELSE
      xa := vx0
    END;
    IF scanY <= vy1 THEN
      IF vy1 # vy0 THEN
        xb := vx0 + TRUNC(LFLOAT(vx1 - vx0) * LFLOAT(scanY - vy0) / LFLOAT(vy1 - vy0))
      ELSE
        xb := vx1
      END
    ELSE
      IF vy2 # vy1 THEN
        xb := vx1 + TRUNC(LFLOAT(vx2 - vx1) * LFLOAT(scanY - vy1) / LFLOAT(vy2 - vy1))
      ELSE
        xb := vx2
      END
    END;
    IF xa > xb THEN tmp := xa; xa := xb; xb := tmp END;
    hl(ctx, xa, xb, scanY)
  END
END FillTriangle;

(* ---- Cubic Bezier ---- *)

PROCEDURE Bezier(ctx: Ctx; ln: LineFn;
                 x1, y1, cx1, cy1, cx2, cy2, x2, y2,
                 steps: INTEGER);
VAR i, prevX, prevY, nextX, nextY: INTEGER;
    t, u, u2, u3, t2, t3, bezX, bezY: REAL;
BEGIN
  IF steps < 1 THEN RETURN END;
  prevX := x1; prevY := y1;
  FOR i := 1 TO steps DO
    t := FLOAT(i) / FLOAT(steps);
    u := 1.0 - t;
    u2 := u * u; u3 := u2 * u;
    t2 := t * t; t3 := t2 * t;
    bezX := u3 * FLOAT(x1) + 3.0 * u2 * t * FLOAT(cx1)
          + 3.0 * u * t2 * FLOAT(cx2) + t3 * FLOAT(x2);
    bezY := u3 * FLOAT(y1) + 3.0 * u2 * t * FLOAT(cy1)
          + 3.0 * u * t2 * FLOAT(cy2) + t3 * FLOAT(y2);
    nextX := TRUNC(bezX + 0.5);
    nextY := TRUNC(bezY + 0.5);
    ln(ctx, prevX, prevY, nextX, nextY);
    prevX := nextX;
    prevY := nextY
  END
END Bezier;

END DrawAlgo.
