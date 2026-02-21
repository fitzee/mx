IMPLEMENTATION MODULE Canvas;

FROM GfxBridge IMPORT gfx_set_color, gfx_get_color, gfx_clear,
     gfx_draw_point, gfx_draw_line,
     gfx_draw_rect, gfx_fill_rect,
     gfx_set_clip, gfx_clear_clip,
     gfx_get_clip_x, gfx_get_clip_y, gfx_get_clip_w, gfx_get_clip_h,
     gfx_set_blend,
     gfx_set_viewport, gfx_reset_viewport;

FROM DrawAlgo IMPORT Ctx, PointFn, HLineFn, LineFn;

FROM MathLib0 IMPORT sqrt, sin, cos;

(* ---- Renderer adapter callbacks ---- *)

PROCEDURE RenPt(ctx: Ctx; x, y: INTEGER);
BEGIN gfx_draw_point(ctx, x, y) END RenPt;

PROCEDURE RenHL(ctx: Ctx; x1, x2, y: INTEGER);
BEGIN gfx_draw_line(ctx, x1, y, x2, y) END RenHL;

PROCEDURE RenLn(ctx: Ctx; x1, y1, x2, y2: INTEGER);
BEGIN gfx_draw_line(ctx, x1, y1, x2, y2) END RenLn;

(* ---- Color & clear (thin SDL wrappers) ---- *)

PROCEDURE SetColor(ren: Renderer; r, g, b, a: INTEGER);
BEGIN gfx_set_color(ren, r, g, b, a) END SetColor;

PROCEDURE GetColor(ren: Renderer; VAR r, g, b, a: INTEGER);
BEGIN gfx_get_color(ren, r, g, b, a) END GetColor;

PROCEDURE Clear(ren: Renderer);
BEGIN gfx_clear(ren) END Clear;

(* ---- Rectangles (thin SDL wrappers) ---- *)

PROCEDURE DrawRect(ren: Renderer; x, y, w, h: INTEGER);
BEGIN gfx_draw_rect(ren, x, y, w, h) END DrawRect;

PROCEDURE FillRect(ren: Renderer; x, y, w, h: INTEGER);
BEGIN gfx_fill_rect(ren, x, y, w, h) END FillRect;

(* ---- Lines & points (thin SDL wrappers) ---- *)

PROCEDURE DrawLine(ren: Renderer; x1, y1, x2, y2: INTEGER);
BEGIN gfx_draw_line(ren, x1, y1, x2, y2) END DrawLine;

PROCEDURE DrawPoint(ren: Renderer; x, y: INTEGER);
BEGIN gfx_draw_point(ren, x, y) END DrawPoint;

(* ---- Shared algorithms via DrawAlgo ---- *)

PROCEDURE DrawCircle(ren: Renderer; cx, cy, radius: INTEGER);
BEGIN DrawAlgo.Circle(ren, RenPt, cx, cy, radius) END DrawCircle;

PROCEDURE FillCircle(ren: Renderer; cx, cy, radius: INTEGER);
BEGIN DrawAlgo.FillCircle(ren, RenHL, cx, cy, radius) END FillCircle;

PROCEDURE DrawEllipse(ren: Renderer; cx, cy, rx, ry: INTEGER);
BEGIN DrawAlgo.Ellipse(ren, RenPt, cx, cy, rx, ry) END DrawEllipse;

PROCEDURE FillEllipse(ren: Renderer; cx, cy, rx, ry: INTEGER);
BEGIN DrawAlgo.FillEllipse(ren, RenHL, cx, cy, rx, ry) END FillEllipse;

PROCEDURE DrawTriangle(ren: Renderer; x1, y1, x2, y2, x3, y3: INTEGER);
BEGIN DrawAlgo.Triangle(ren, RenLn, x1, y1, x2, y2, x3, y3) END DrawTriangle;

PROCEDURE FillTriangle(ren: Renderer; x1, y1, x2, y2, x3, y3: INTEGER);
BEGIN DrawAlgo.FillTriangle(ren, RenHL, x1, y1, x2, y2, x3, y3) END FillTriangle;

PROCEDURE DrawBezier(ren: Renderer;
                     x1, y1, cx1, cy1, cx2, cy2, x2, y2,
                     steps: INTEGER);
BEGIN DrawAlgo.Bezier(ren, RenLn, x1, y1, cx1, cy1, cx2, cy2, x2, y2, steps) END DrawBezier;

(* ---- Canvas-only algorithms (not in PixBuf) ---- *)

PROCEDURE DrawRoundRect(ren: Renderer; x, y, w, h, radius: INTEGER);
VAR rad: INTEGER;
    tlCx, tlCy, trCx, trCy, blCx, blCy, brCx, brCy: INTEGER;
    cx, cy, d: INTEGER;
BEGIN
  rad := radius;
  IF rad > w DIV 2 THEN rad := w DIV 2 END;
  IF rad > h DIV 2 THEN rad := h DIV 2 END;
  IF rad < 0 THEN rad := 0 END;
  tlCx := x + rad;         tlCy := y + rad;
  trCx := x + w - 1 - rad; trCy := y + rad;
  blCx := x + rad;         blCy := y + h - 1 - rad;
  brCx := x + w - 1 - rad; brCy := y + h - 1 - rad;
  gfx_draw_line(ren, tlCx, y, trCx, y);
  gfx_draw_line(ren, tlCx, y + h - 1, brCx, y + h - 1);
  gfx_draw_line(ren, x, tlCy, x, blCy);
  gfx_draw_line(ren, x + w - 1, trCy, x + w - 1, brCy);
  cx := rad; cy := 0; d := 1 - rad;
  WHILE cx >= cy DO
    gfx_draw_point(ren, trCx + cx, trCy - cy);
    gfx_draw_point(ren, trCx + cy, trCy - cx);
    gfx_draw_point(ren, tlCx - cx, tlCy - cy);
    gfx_draw_point(ren, tlCx - cy, tlCy - cx);
    gfx_draw_point(ren, brCx + cx, brCy + cy);
    gfx_draw_point(ren, brCx + cy, brCy + cx);
    gfx_draw_point(ren, blCx - cx, blCy + cy);
    gfx_draw_point(ren, blCx - cy, blCy + cx);
    INC(cy);
    IF d <= 0 THEN
      d := d + 2 * cy + 1
    ELSE
      DEC(cx);
      d := d + 2 * (cy - cx) + 1
    END
  END
END DrawRoundRect;

PROCEDURE FillRoundRect(ren: Renderer; x, y, w, h, radius: INTEGER);
VAR rad: INTEGER;
    tlCx, tlCy, trCx, trCy, blCx, blCy, brCx, brCy: INTEGER;
    cx, cy, d: INTEGER;
BEGIN
  rad := radius;
  IF rad > w DIV 2 THEN rad := w DIV 2 END;
  IF rad > h DIV 2 THEN rad := h DIV 2 END;
  IF rad < 0 THEN rad := 0 END;
  gfx_fill_rect(ren, x, y + rad, w, h - 2 * rad);
  gfx_fill_rect(ren, x + rad, y, w - 2 * rad, rad);
  gfx_fill_rect(ren, x + rad, y + h - rad, w - 2 * rad, rad);
  tlCx := x + rad;         tlCy := y + rad;
  trCx := x + w - 1 - rad; trCy := y + rad;
  blCx := x + rad;         blCy := y + h - 1 - rad;
  brCx := x + w - 1 - rad; brCy := y + h - 1 - rad;
  cx := rad; cy := 0; d := 1 - rad;
  WHILE cx >= cy DO
    gfx_draw_line(ren, tlCx - cx, tlCy - cy, trCx + cx, trCy - cy);
    gfx_draw_line(ren, tlCx - cy, tlCy - cx, trCx + cy, trCy - cx);
    gfx_draw_line(ren, blCx - cx, blCy + cy, brCx + cx, brCy + cy);
    gfx_draw_line(ren, blCx - cy, blCy + cx, brCx + cy, brCy + cx);
    INC(cy);
    IF d <= 0 THEN
      d := d + 2 * cy + 1
    ELSE
      DEC(cx);
      d := d + 2 * (cy - cx) + 1
    END
  END
END FillRoundRect;

PROCEDURE DrawThickLine(ren: Renderer; x1, y1, x2, y2, thickness: INTEGER);
VAR dx, dy, len, half, px, py: REAL;
    qx0, qy0, qx1, qy1, qx2, qy2, qx3, qy3: INTEGER;
BEGIN
  IF thickness <= 0 THEN RETURN END;
  IF thickness = 1 THEN
    gfx_draw_line(ren, x1, y1, x2, y2);
    RETURN
  END;
  dx := FLOAT(x2 - x1);
  dy := FLOAT(y2 - y1);
  len := sqrt(dx * dx + dy * dy);
  IF len < 0.001 THEN
    FillCircle(ren, x1, y1, thickness DIV 2);
    RETURN
  END;
  half := FLOAT(thickness) / 2.0;
  px := (-dy / len) * half;
  py := (dx / len) * half;
  qx0 := TRUNC(FLOAT(x1) + px + 0.5);
  qy0 := TRUNC(FLOAT(y1) + py + 0.5);
  qx1 := TRUNC(FLOAT(x1) - px + 0.5);
  qy1 := TRUNC(FLOAT(y1) - py + 0.5);
  qx2 := TRUNC(FLOAT(x2) - px + 0.5);
  qy2 := TRUNC(FLOAT(y2) - py + 0.5);
  qx3 := TRUNC(FLOAT(x2) + px + 0.5);
  qy3 := TRUNC(FLOAT(y2) + py + 0.5);
  FillTriangle(ren, qx0, qy0, qx1, qy1, qx2, qy2);
  FillTriangle(ren, qx0, qy0, qx2, qy2, qx3, qy3)
END DrawThickLine;

PROCEDURE DrawArc(ren: Renderer; cx, cy, radius, startDeg, endDeg: INTEGER);
CONST PI = 3.14159265;
VAR steps, i, prevX, prevY, nextX, nextY, sDeg, eDeg: INTEGER;
    startRad, endRad, step, angle: REAL;
BEGIN
  IF radius <= 0 THEN RETURN END;
  sDeg := startDeg; eDeg := endDeg;
  WHILE sDeg < 0 DO INC(sDeg, 360) END;
  WHILE eDeg < 0 DO INC(eDeg, 360) END;
  sDeg := sDeg MOD 360;
  eDeg := eDeg MOD 360;
  steps := TRUNC(2.0 * PI * FLOAT(radius));
  IF steps < 36 THEN steps := 36 END;
  startRad := FLOAT(sDeg) * PI / 180.0;
  endRad := FLOAT(eDeg) * PI / 180.0;
  IF endRad <= startRad THEN endRad := endRad + 2.0 * PI END;
  step := (endRad - startRad) / FLOAT(steps);
  prevX := cx + TRUNC(cos(startRad) * FLOAT(radius) + 0.5);
  prevY := cy - TRUNC(sin(startRad) * FLOAT(radius) + 0.5);
  FOR i := 1 TO steps DO
    angle := startRad + step * FLOAT(i);
    nextX := cx + TRUNC(cos(angle) * FLOAT(radius) + 0.5);
    nextY := cy - TRUNC(sin(angle) * FLOAT(radius) + 0.5);
    gfx_draw_line(ren, prevX, prevY, nextX, nextY);
    prevX := nextX;
    prevY := nextY
  END
END DrawArc;

(* ---- Clipping (thin SDL wrappers) ---- *)

PROCEDURE SetClip(ren: Renderer; x, y, w, h: INTEGER);
BEGIN gfx_set_clip(ren, x, y, w, h) END SetClip;

PROCEDURE ClearClip(ren: Renderer);
BEGIN gfx_clear_clip(ren) END ClearClip;

PROCEDURE GetClipX(ren: Renderer): INTEGER;
BEGIN RETURN gfx_get_clip_x(ren) END GetClipX;

PROCEDURE GetClipY(ren: Renderer): INTEGER;
BEGIN RETURN gfx_get_clip_y(ren) END GetClipY;

PROCEDURE GetClipW(ren: Renderer): INTEGER;
BEGIN RETURN gfx_get_clip_w(ren) END GetClipW;

PROCEDURE GetClipH(ren: Renderer): INTEGER;
BEGIN RETURN gfx_get_clip_h(ren) END GetClipH;

(* ---- Blend mode (thin SDL wrapper) ---- *)

PROCEDURE SetBlendMode(ren: Renderer; mode: INTEGER);
BEGIN gfx_set_blend(ren, mode) END SetBlendMode;

(* ---- Viewport (thin SDL wrappers) ---- *)

PROCEDURE SetViewport(ren: Renderer; x, y, w, h: INTEGER);
BEGIN gfx_set_viewport(ren, x, y, w, h) END SetViewport;

PROCEDURE ResetViewport(ren: Renderer);
BEGIN gfx_reset_viewport(ren) END ResetViewport;

END Canvas.
