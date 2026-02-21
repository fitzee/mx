IMPLEMENTATION MODULE PixBuf;

FROM SYSTEM IMPORT ADR;
FROM MathLib0 IMPORT sin, cos;
FROM Color IMPORT UnpackR, UnpackG, UnpackB;
FROM FileSystem IMPORT File, Lookup, Close, ReadChar, WriteChar, Done;
FROM DrawAlgo IMPORT Ctx, PointFn, HLineFn, LineFn;
FROM GfxBridge IMPORT
     gfx_pb_create, gfx_pb_free, gfx_pb_clear,
     gfx_pb_width, gfx_pb_height,
     gfx_pb_set_pal, gfx_pb_pal_packed,
     gfx_pb_set, gfx_pb_get,
     gfx_pb_fill_row, gfx_pb_mark_dirty, gfx_pb_total,
     gfx_alloc, gfx_dealloc, gfx_buf_get, gfx_buf_set,
     gfx_pb_pixel_ptr, gfx_pb_composite, gfx_pb_copy_pixels,
     gfx_pb_stamp_text,
     gfx_pb_render,
     gfx_pb_save, gfx_pb_restore, gfx_pb_save_w, gfx_pb_save_h,
     gfx_pb_free_save,
     gfx_pb_save_png, gfx_pb_load_png,
     gfx_pb_render_alpha,
     gfx_pb_render_ham,
     gfx_pb_pal_to_screen, gfx_pb_rgba_get32, gfx_pb_rgba_set32,
     gfx_pb_flush_tex,
     gfx_log;

(* ================================================================
   Module variables — polygon, layer, frame state (Phase 5)
   ================================================================ *)

CONST
  MaxLayers = 16;
  MaxFrames = 256;

VAR
  polyXs, polyYs: ARRAY [0..255] OF INTEGER;
  polyN: INTEGER;
  drawIdx: INTEGER;
  (* Layer system state *)
  layers: ARRAY [0..15] OF PBuf;
  layerVis: ARRAY [0..15] OF INTEGER;
  layerCount: INTEGER;
  layerActive: INTEGER;
  (* Frame system state *)
  frames: ARRAY [0..255] OF PBuf;
  frameTiming: ARRAY [0..255] OF INTEGER;
  nFrames: INTEGER;
  currentFrame: INTEGER;

(* ================================================================
   Management — thin wrappers (stay in C)
   ================================================================ *)

PROCEDURE Create(w, h: INTEGER): PBuf;
BEGIN RETURN gfx_pb_create(w, h) END Create;

PROCEDURE Free(pb: PBuf);
BEGIN gfx_pb_free(pb) END Free;

PROCEDURE Clear(pb: PBuf; idx: INTEGER);
BEGIN gfx_pb_clear(pb, idx) END Clear;

PROCEDURE Width(pb: PBuf): INTEGER;
BEGIN RETURN gfx_pb_width(pb) END Width;

PROCEDURE Height(pb: PBuf): INTEGER;
BEGIN RETURN gfx_pb_height(pb) END Height;

(* ================================================================
   Palette — thin wrappers (stay in C)
   ================================================================ *)

PROCEDURE SetPal(pb: PBuf; idx, r, g, b: INTEGER);
BEGIN gfx_pb_set_pal(pb, idx, r, g, b) END SetPal;

PROCEDURE PalPacked(pb: PBuf; idx: INTEGER): CARDINAL;
BEGIN RETURN CARDINAL(gfx_pb_pal_packed(pb, idx)) END PalPacked;

PROCEDURE PalR(pb: PBuf; idx: INTEGER): INTEGER;
BEGIN RETURN UnpackR(PalPacked(pb, idx)) END PalR;

PROCEDURE PalG(pb: PBuf; idx: INTEGER): INTEGER;
BEGIN RETURN UnpackG(PalPacked(pb, idx)) END PalG;

PROCEDURE PalB(pb: PBuf; idx: INTEGER): INTEGER;
BEGIN RETURN UnpackB(PalPacked(pb, idx)) END PalB;

(* ================================================================
   Pixel access — thin wrappers (stay in C)
   ================================================================ *)

PROCEDURE SetPix(pb: PBuf; x, y, idx: INTEGER);
BEGIN gfx_pb_set(pb, x, y, idx) END SetPix;

PROCEDURE GetPix(pb: PBuf; x, y: INTEGER): INTEGER;
BEGIN RETURN gfx_pb_get(pb, x, y) END GetPix;

(* ================================================================
   Phase 2: Drawing Primitives — full M2 algorithms
   ================================================================ *)

(* Adapter callbacks for DrawAlgo — read drawIdx for palette index *)

PROCEDURE PBPoint(ctx: Ctx; x, y: INTEGER);
BEGIN gfx_pb_set(ctx, x, y, drawIdx) END PBPoint;

PROCEDURE PBHLine(ctx: Ctx; x1, x2, y: INTEGER);
VAR left, w: INTEGER;
BEGIN
  IF x1 <= x2 THEN left := x1; w := x2 - x1 + 1
  ELSE left := x2; w := x1 - x2 + 1 END;
  gfx_pb_fill_row(ctx, left, y, w, drawIdx)
END PBHLine;

PROCEDURE PBLine(ctx: Ctx; x1, y1, x2, y2: INTEGER);
BEGIN DrawAlgo.Line(ctx, PBPoint, x1, y1, x2, y2) END PBLine;

PROCEDURE Line(pb: PBuf; x1, y1, x2, y2, idx: INTEGER);
BEGIN drawIdx := idx; DrawAlgo.Line(pb, PBPoint, x1, y1, x2, y2)
END Line;

PROCEDURE ThickLine(pb: PBuf; x1, y1, x2, y2, idx, thick: INTEGER);
VAR dx, dy, sx, sy, err, i, r: INTEGER;
    cx, cy: INTEGER;
BEGIN
  drawIdx := idx;
  IF thick <= 1 THEN DrawAlgo.Line(pb, PBPoint, x1, y1, x2, y2); RETURN END;
  r := thick DIV 2;
  cx := x1; cy := y1;
  dx := x2 - x1; IF dx < 0 THEN dx := -dx END;
  dy := y2 - y1; IF dy < 0 THEN dy := -dy END;
  IF x1 < x2 THEN sx := 1 ELSE sx := -1 END;
  IF y1 < y2 THEN sy := 1 ELSE sy := -1 END;
  IF dx >= dy THEN
    err := dx DIV 2;
    FOR i := 0 TO dx DO
      DrawAlgo.FillCircle(pb, PBHLine, cx, cy, r);
      err := err - dy;
      IF err < 0 THEN cy := cy + sy; err := err + dx END;
      cx := cx + sx
    END
  ELSE
    err := dy DIV 2;
    FOR i := 0 TO dy DO
      DrawAlgo.FillCircle(pb, PBHLine, cx, cy, r);
      err := err - dx;
      IF err < 0 THEN cx := cx + sx; err := err + dy END;
      cy := cy + sy
    END
  END
END ThickLine;

PROCEDURE Rect(pb: PBuf; x, y, w, h, idx: INTEGER);
BEGIN
  drawIdx := idx;
  DrawAlgo.Line(pb, PBPoint, x, y, x + w - 1, y);
  DrawAlgo.Line(pb, PBPoint, x, y + h - 1, x + w - 1, y + h - 1);
  DrawAlgo.Line(pb, PBPoint, x, y, x, y + h - 1);
  DrawAlgo.Line(pb, PBPoint, x + w - 1, y, x + w - 1, y + h - 1)
END Rect;

PROCEDURE FillRect(pb: PBuf; x, y, w, h, idx: INTEGER);
VAR row, pw, ph, x0, y0, x1, y1: INTEGER;
BEGIN
  pw := Width(pb); ph := Height(pb);
  x0 := x; IF x0 < 0 THEN x0 := 0 END;
  y0 := y; IF y0 < 0 THEN y0 := 0 END;
  x1 := x + w; IF x1 > pw THEN x1 := pw END;
  y1 := y + h; IF y1 > ph THEN y1 := ph END;
  FOR row := y0 TO y1 - 1 DO
    gfx_pb_fill_row(pb, x0, row, x1 - x0, idx)
  END
END FillRect;

PROCEDURE Circle(pb: PBuf; cx, cy, radius, idx: INTEGER);
BEGIN
  drawIdx := idx;
  DrawAlgo.Circle(pb, PBPoint, cx, cy, radius)
END Circle;

PROCEDURE FillCircle(pb: PBuf; cx, cy, radius, idx: INTEGER);
BEGIN
  drawIdx := idx;
  DrawAlgo.FillCircle(pb, PBHLine, cx, cy, radius)
END FillCircle;

PROCEDURE Ellipse(pb: PBuf; cx, cy, rx, ry, idx: INTEGER);
BEGIN
  drawIdx := idx;
  DrawAlgo.Ellipse(pb, PBPoint, cx, cy, rx, ry)
END Ellipse;

PROCEDURE FillEllipse(pb: PBuf; cx, cy, rx, ry, idx: INTEGER);
BEGIN
  drawIdx := idx;
  DrawAlgo.FillEllipse(pb, PBHLine, cx, cy, rx, ry)
END FillEllipse;

PROCEDURE Triangle(pb: PBuf; x1, y1, x2, y2, x3, y3, idx: INTEGER);
BEGIN
  drawIdx := idx;
  DrawAlgo.Triangle(pb, PBLine, x1, y1, x2, y2, x3, y3)
END Triangle;

PROCEDURE FillTriangle(pb: PBuf; x1, y1, x2, y2, x3, y3, idx: INTEGER);
BEGIN
  drawIdx := idx;
  DrawAlgo.FillTriangle(pb, PBHLine, x1, y1, x2, y2, x3, y3)
END FillTriangle;

PROCEDURE LinePerfect(pb: PBuf; x0, y0, x1, y1, idx: INTEGER);
VAR dx, dy, adx, ady, sx, sy, err, x, y: INTEGER;
BEGIN
  dx := x1 - x0; dy := y1 - y0;
  adx := dx; IF adx < 0 THEN adx := -adx END;
  ady := dy; IF ady < 0 THEN ady := -ady END;
  IF dx > 0 THEN sx := 1 ELSE sx := -1 END;
  IF dy > 0 THEN sy := 1 ELSE sy := -1 END;
  x := x0; y := y0;
  IF adx >= ady THEN
    err := adx DIV 2;
    LOOP
      gfx_pb_set(pb, x, y, idx);
      IF x = x1 THEN EXIT END;
      err := err - ady;
      IF err < 0 THEN y := y + sy; err := err + adx END;
      x := x + sx
    END
  ELSE
    err := ady DIV 2;
    LOOP
      gfx_pb_set(pb, x, y, idx);
      IF y = y1 THEN EXIT END;
      err := err - adx;
      IF err < 0 THEN x := x + sx; err := err + ady END;
      y := y + sy
    END
  END
END LinePerfect;

(* ================================================================
   Phase 3: Math & Fill Algorithms
   ================================================================ *)

PROCEDURE NearestColor(pb: PBuf; r, g, b, ncolors: INTEGER): INTEGER;
VAR i, best, bestd, dr, dg, db, d: INTEGER;
BEGIN
  best := 0; bestd := 7FFFFFFFH;
  FOR i := 0 TO ncolors - 1 DO
    dr := r - PalR(pb, i);
    dg := g - PalG(pb, i);
    db := b - PalB(pb, i);
    d := dr*dr + dg*dg + db*db;
    IF d < bestd THEN bestd := d; best := i END
  END;
  RETURN best
END NearestColor;

PROCEDURE ReplaceColor(pb: PBuf; oldIdx, newIdx: INTEGER);
VAR pw, ph, x, y: INTEGER;
BEGIN
  pw := Width(pb); ph := Height(pb);
  FOR y := 0 TO ph - 1 DO
    FOR x := 0 TO pw - 1 DO
      IF GetPix(pb, x, y) = oldIdx THEN
        gfx_pb_set(pb, x, y, newIdx)
      END
    END
  END
END ReplaceColor;

PROCEDURE FloodFill(pb: PBuf; x, y, idx: INTEGER);
VAR stkX, stkY: ARRAY [0..8191] OF INTEGER;
    sp, target, pw, ph: INTEGER;
    cx, cy, left, right, xi, ny, dir: INTEGER;
    inSpan: BOOLEAN;
BEGIN
  pw := Width(pb); ph := Height(pb);
  IF (x < 0) OR (x >= pw) OR (y < 0) OR (y >= ph) THEN RETURN END;
  target := GetPix(pb, x, y);
  IF target = idx THEN RETURN END;
  sp := 0;
  stkX[sp] := x; stkY[sp] := y; INC(sp);
  WHILE sp > 0 DO
    DEC(sp); cx := stkX[sp]; cy := stkY[sp];
    IF (cx < 0) OR (cx >= pw) OR (cy < 0) OR (cy >= ph) THEN (* skip *)
    ELSIF GetPix(pb, cx, cy) # target THEN (* skip *)
    ELSE
      left := cx;
      WHILE (left > 0) AND (GetPix(pb, left - 1, cy) = target) DO DEC(left) END;
      right := cx;
      WHILE (right < pw - 1) AND (GetPix(pb, right + 1, cy) = target) DO INC(right) END;
      gfx_pb_fill_row(pb, left, cy, right - left + 1, idx);
      FOR dir := 0 TO 1 DO
        IF dir = 0 THEN ny := cy - 1 ELSE ny := cy + 1 END;
        IF (ny >= 0) AND (ny < ph) THEN
          inSpan := FALSE;
          FOR xi := left TO right DO
            IF GetPix(pb, xi, ny) = target THEN
              IF NOT inSpan THEN
                IF sp < 8192 THEN
                  stkX[sp] := xi; stkY[sp] := ny; INC(sp)
                END;
                inSpan := TRUE
              END
            ELSE
              inSpan := FALSE
            END
          END
        END
      END
    END
  END;
  gfx_pb_mark_dirty(pb, 0, 0, pw, ph)
END FloodFill;

PROCEDURE Gradient(pb: PBuf; x, y, w, h, c1, c2: INTEGER;
                   horiz: BOOLEAN; ncolors: INTEGER);
VAR r1, g1, b1, r2, g2, b2: INTEGER;
    n, col, row, mr, mg, mb, ci: INTEGER;
BEGIN
  IF ncolors < 1 THEN ncolors := 32 END;
  r1 := PalR(pb, c1 MOD 256); g1 := PalG(pb, c1 MOD 256); b1 := PalB(pb, c1 MOD 256);
  r2 := PalR(pb, c2 MOD 256); g2 := PalG(pb, c2 MOD 256); b2 := PalB(pb, c2 MOD 256);
  IF horiz THEN
    n := w - 1; IF n < 1 THEN n := 1 END;
    FOR col := 0 TO w - 1 DO
      mr := r1 + (r2 - r1) * col DIV n;
      mg := g1 + (g2 - g1) * col DIV n;
      mb := b1 + (b2 - b1) * col DIV n;
      ci := NearestColor(pb, mr, mg, mb, ncolors);
      FOR row := y TO y + h - 1 DO
        gfx_pb_set(pb, x + col, row, ci)
      END
    END
  ELSE
    n := h - 1; IF n < 1 THEN n := 1 END;
    FOR row := 0 TO h - 1 DO
      mr := r1 + (r2 - r1) * row DIV n;
      mg := g1 + (g2 - g1) * row DIV n;
      mb := b1 + (b2 - b1) * row DIV n;
      ci := NearestColor(pb, mr, mg, mb, ncolors);
      FOR col := x TO x + w - 1 DO
        gfx_pb_set(pb, col, y + row, ci)
      END
    END
  END
END Gradient;

PROCEDURE GradientAngle(pb: PBuf; x, y, w, h, c1, c2,
                        angleDeg, ncolors: INTEGER);
VAR r1, g1, b1, r2, g2, b2: INTEGER;
    rad, cosA, sinA: REAL;
    corners: ARRAY [0..3] OF REAL;
    pmin, pmax, range, proj, t: REAL;
    row, col, mr, mg, mb, ci, i: INTEGER;
BEGIN
  IF (w <= 0) OR (h <= 0) THEN RETURN END;
  IF ncolors < 1 THEN ncolors := 32 END;
  rad := FLOAT(angleDeg) * 3.14159265 / 180.0;
  cosA := cos(rad); sinA := sin(rad);
  corners[0] := 0.0;
  corners[1] := FLOAT(w - 1) * cosA;
  corners[2] := FLOAT(h - 1) * sinA;
  corners[3] := FLOAT(w - 1) * cosA + FLOAT(h - 1) * sinA;
  pmin := corners[0]; pmax := corners[0];
  FOR i := 1 TO 3 DO
    IF corners[i] < pmin THEN pmin := corners[i] END;
    IF corners[i] > pmax THEN pmax := corners[i] END
  END;
  range := pmax - pmin;
  IF range < 1.0 THEN range := 1.0 END;
  r1 := PalR(pb, c1 MOD 256); g1 := PalG(pb, c1 MOD 256); b1 := PalB(pb, c1 MOD 256);
  r2 := PalR(pb, c2 MOD 256); g2 := PalG(pb, c2 MOD 256); b2 := PalB(pb, c2 MOD 256);
  FOR row := 0 TO h - 1 DO
    FOR col := 0 TO w - 1 DO
      proj := FLOAT(col) * cosA + FLOAT(row) * sinA;
      t := (proj - pmin) / range;
      IF t < 0.0 THEN t := 0.0 END;
      IF t > 1.0 THEN t := 1.0 END;
      mr := TRUNC(FLOAT(r1) + FLOAT(r2 - r1) * t);
      mg := TRUNC(FLOAT(g1) + FLOAT(g2 - g1) * t);
      mb := TRUNC(FLOAT(b1) + FLOAT(b2 - b1) * t);
      ci := NearestColor(pb, mr, mg, mb, ncolors);
      gfx_pb_set(pb, x + col, y + row, ci)
    END
  END
END GradientAngle;

PROCEDURE PatternFill(pb: PBuf; x, y, w, h, fg, bg, pattern: INTEGER);
CONST
  B00 =  0; B01 =  8; B02 =  2; B03 = 10;
  B10 = 12; B11 =  4; B12 = 14; B13 =  6;
  B20 =  3; B21 = 11; B22 =  1; B23 =  9;
  B30 = 15; B31 =  7; B32 = 13; B33 =  5;
VAR row, col, thr, val, ci: INTEGER;
    bayer4: ARRAY [0..3],[0..3] OF INTEGER;
BEGIN
  bayer4[0][0] := B00; bayer4[0][1] := B01; bayer4[0][2] := B02; bayer4[0][3] := B03;
  bayer4[1][0] := B10; bayer4[1][1] := B11; bayer4[1][2] := B12; bayer4[1][3] := B13;
  bayer4[2][0] := B20; bayer4[2][1] := B21; bayer4[2][2] := B22; bayer4[2][3] := B23;
  bayer4[3][0] := B30; bayer4[3][1] := B31; bayer4[3][2] := B32; bayer4[3][3] := B33;
  thr := pattern;
  IF thr < 0 THEN thr := 0 END;
  IF thr > 16 THEN thr := 16 END;
  FOR row := y TO y + h - 1 DO
    FOR col := x TO x + w - 1 DO
      val := bayer4[row MOD 4][col MOD 4];
      IF val < thr THEN ci := fg ELSE ci := bg END;
      gfx_pb_set(pb, col, row, ci)
    END
  END
END PatternFill;

PROCEDURE DitherFill(pb: PBuf; x, y, w, h, fg, bg,
                     matrixType, threshold: INTEGER);
VAR row, col, val, ci, thr: INTEGER;
    bayer2: ARRAY [0..1],[0..1] OF INTEGER;
    bayer4: ARRAY [0..3],[0..3] OF INTEGER;
    bayer8: ARRAY [0..7],[0..7] OF INTEGER;
BEGIN
  bayer2[0][0] := 0; bayer2[0][1] := 2;
  bayer2[1][0] := 3; bayer2[1][1] := 1;

  bayer4[0][0] :=  0; bayer4[0][1] :=  8; bayer4[0][2] :=  2; bayer4[0][3] := 10;
  bayer4[1][0] := 12; bayer4[1][1] :=  4; bayer4[1][2] := 14; bayer4[1][3] :=  6;
  bayer4[2][0] :=  3; bayer4[2][1] := 11; bayer4[2][2] :=  1; bayer4[2][3] :=  9;
  bayer4[3][0] := 15; bayer4[3][1] :=  7; bayer4[3][2] := 13; bayer4[3][3] :=  5;

  bayer8[0][0] :=  0; bayer8[0][1] := 32; bayer8[0][2] :=  8; bayer8[0][3] := 40;
  bayer8[0][4] :=  2; bayer8[0][5] := 34; bayer8[0][6] := 10; bayer8[0][7] := 42;
  bayer8[1][0] := 48; bayer8[1][1] := 16; bayer8[1][2] := 56; bayer8[1][3] := 24;
  bayer8[1][4] := 50; bayer8[1][5] := 18; bayer8[1][6] := 58; bayer8[1][7] := 26;
  bayer8[2][0] := 12; bayer8[2][1] := 44; bayer8[2][2] :=  4; bayer8[2][3] := 36;
  bayer8[2][4] := 14; bayer8[2][5] := 46; bayer8[2][6] :=  6; bayer8[2][7] := 38;
  bayer8[3][0] := 60; bayer8[3][1] := 28; bayer8[3][2] := 52; bayer8[3][3] := 20;
  bayer8[3][4] := 62; bayer8[3][5] := 30; bayer8[3][6] := 54; bayer8[3][7] := 22;
  bayer8[4][0] :=  3; bayer8[4][1] := 35; bayer8[4][2] := 11; bayer8[4][3] := 43;
  bayer8[4][4] :=  1; bayer8[4][5] := 33; bayer8[4][6] :=  9; bayer8[4][7] := 41;
  bayer8[5][0] := 51; bayer8[5][1] := 19; bayer8[5][2] := 59; bayer8[5][3] := 27;
  bayer8[5][4] := 49; bayer8[5][5] := 17; bayer8[5][6] := 57; bayer8[5][7] := 25;
  bayer8[6][0] := 15; bayer8[6][1] := 47; bayer8[6][2] :=  7; bayer8[6][3] := 39;
  bayer8[6][4] := 13; bayer8[6][5] := 45; bayer8[6][6] :=  5; bayer8[6][7] := 37;
  bayer8[7][0] := 63; bayer8[7][1] := 31; bayer8[7][2] := 55; bayer8[7][3] := 23;
  bayer8[7][4] := 61; bayer8[7][5] := 29; bayer8[7][6] := 53; bayer8[7][7] := 21;

  thr := threshold;
  FOR row := y TO y + h - 1 DO
    FOR col := x TO x + w - 1 DO
      IF matrixType = 0 THEN
        val := bayer2[row MOD 2][col MOD 2];
        IF thr < 0 THEN thr := 0 END;
        IF thr > 4 THEN thr := 4 END
      ELSIF matrixType = 2 THEN
        val := bayer8[row MOD 8][col MOD 8];
        IF thr < 0 THEN thr := 0 END;
        IF thr > 64 THEN thr := 64 END
      ELSE
        val := bayer4[row MOD 4][col MOD 4];
        IF thr < 0 THEN thr := 0 END;
        IF thr > 16 THEN thr := 16 END
      END;
      IF val < thr THEN ci := fg ELSE ci := bg END;
      gfx_pb_set(pb, col, row, ci)
    END
  END
END DitherFill;

PROCEDURE Bezier(pb: PBuf; x1, y1, cx1, cy1, cx2, cy2,
                 x2, y2, idx, steps: INTEGER);
VAR st: INTEGER;
BEGIN
  st := steps;
  IF st < 4 THEN st := 32 END;
  drawIdx := idx;
  DrawAlgo.Bezier(pb, PBLine, x1, y1, cx1, cy1, cx2, cy2, x2, y2, st)
END Bezier;

PROCEDURE FlipH(pb: PBuf; x, y, w, h: INTEGER);
VAR row, c, lx, rx, t, pw, ph: INTEGER;
BEGIN
  pw := Width(pb); ph := Height(pb);
  FOR row := y TO y + h - 1 DO
    IF (row >= 0) AND (row < ph) THEN
      FOR c := 0 TO w DIV 2 - 1 DO
        lx := x + c; rx := x + w - 1 - c;
        IF (lx >= 0) AND (lx < pw) AND (rx >= 0) AND (rx < pw) THEN
          t := GetPix(pb, lx, row);
          gfx_pb_set(pb, lx, row, GetPix(pb, rx, row));
          gfx_pb_set(pb, rx, row, t)
        END
      END
    END
  END
END FlipH;

PROCEDURE FlipV(pb: PBuf; x, y, w, h: INTEGER);
VAR r, c, topY, botY, t, pw, ph: INTEGER;
BEGIN
  pw := Width(pb); ph := Height(pb);
  FOR r := 0 TO h DIV 2 - 1 DO
    topY := y + r; botY := y + h - 1 - r;
    IF (topY >= 0) AND (topY < ph) AND (botY >= 0) AND (botY < ph) THEN
      FOR c := x TO x + w - 1 DO
        IF (c >= 0) AND (c < pw) THEN
          t := GetPix(pb, c, topY);
          gfx_pb_set(pb, c, topY, GetPix(pb, c, botY));
          gfx_pb_set(pb, c, botY, t)
        END
      END
    END
  END
END FlipV;

(* ================================================================
   Phase 4: Temp-Buffer Algorithms
   ================================================================ *)

PROCEDURE Rotate90(pb: PBuf; x, y, w, h: INTEGER);
VAR buf: ADDRESS;
    r, c, sx, sy, dx, dy, pw, ph, m: INTEGER;
BEGIN
  IF (w <= 0) OR (h <= 0) THEN RETURN END;
  pw := Width(pb); ph := Height(pb);
  buf := gfx_alloc(w * h);
  IF buf = NIL THEN RETURN END;
  FOR r := 0 TO h - 1 DO
    FOR c := 0 TO w - 1 DO
      sx := x + c; sy := y + r;
      IF (sx >= 0) AND (sx < pw) AND (sy >= 0) AND (sy < ph) THEN
        gfx_buf_set(buf, r * w + c, GetPix(pb, sx, sy))
      ELSE
        gfx_buf_set(buf, r * w + c, 0)
      END
    END
  END;
  (* 90 CW: (c,r) -> (h-1-r, c) *)
  FOR r := 0 TO h - 1 DO
    FOR c := 0 TO w - 1 DO
      dx := x + (h - 1 - r); dy := y + c;
      IF (dx >= 0) AND (dx < pw) AND (dy >= 0) AND (dy < ph) THEN
        gfx_pb_set(pb, dx, dy, gfx_buf_get(buf, r * w + c))
      END
    END
  END;
  gfx_dealloc(buf);
  m := w; IF h > m THEN m := h END;
  gfx_pb_mark_dirty(pb, x, y, m, m)
END Rotate90;

PROCEDURE Rotate180(pb: PBuf; x, y, w, h: INTEGER);
VAR r, c, dx, dy, pw, ph, t: INTEGER;
    buf: ADDRESS;
BEGIN
  IF (w <= 0) OR (h <= 0) THEN RETURN END;
  pw := Width(pb); ph := Height(pb);
  buf := gfx_alloc(w * h);
  IF buf = NIL THEN RETURN END;
  FOR r := 0 TO h - 1 DO
    FOR c := 0 TO w - 1 DO
      IF (x+c >= 0) AND (x+c < pw) AND (y+r >= 0) AND (y+r < ph) THEN
        gfx_buf_set(buf, r * w + c, GetPix(pb, x + c, y + r))
      ELSE
        gfx_buf_set(buf, r * w + c, 0)
      END
    END
  END;
  FOR r := 0 TO h - 1 DO
    FOR c := 0 TO w - 1 DO
      dx := x + (w - 1 - c); dy := y + (h - 1 - r);
      IF (dx >= 0) AND (dx < pw) AND (dy >= 0) AND (dy < ph) THEN
        gfx_pb_set(pb, dx, dy, gfx_buf_get(buf, r * w + c))
      END
    END
  END;
  gfx_dealloc(buf);
  gfx_pb_mark_dirty(pb, x, y, w, h)
END Rotate180;

PROCEDURE Rotate270(pb: PBuf; x, y, w, h: INTEGER);
VAR buf: ADDRESS;
    r, c, sx, sy, dx, dy, pw, ph, m: INTEGER;
BEGIN
  IF (w <= 0) OR (h <= 0) THEN RETURN END;
  pw := Width(pb); ph := Height(pb);
  buf := gfx_alloc(w * h);
  IF buf = NIL THEN RETURN END;
  FOR r := 0 TO h - 1 DO
    FOR c := 0 TO w - 1 DO
      sx := x + c; sy := y + r;
      IF (sx >= 0) AND (sx < pw) AND (sy >= 0) AND (sy < ph) THEN
        gfx_buf_set(buf, r * w + c, GetPix(pb, sx, sy))
      ELSE
        gfx_buf_set(buf, r * w + c, 0)
      END
    END
  END;
  (* 270 CW = 90 CCW: (c,r) -> (r, w-1-c) *)
  FOR r := 0 TO h - 1 DO
    FOR c := 0 TO w - 1 DO
      dx := x + r; dy := y + (w - 1 - c);
      IF (dx >= 0) AND (dx < pw) AND (dy >= 0) AND (dy < ph) THEN
        gfx_pb_set(pb, dx, dy, gfx_buf_get(buf, r * w + c))
      END
    END
  END;
  gfx_dealloc(buf);
  m := w; IF h > m THEN m := h END;
  gfx_pb_mark_dirty(pb, x, y, m, m)
END Rotate270;

PROCEDURE CopyRegion(pb: PBuf; sx, sy, w, h, dx, dy: INTEGER);
VAR buf: ADDRESS;
    r, c, rx, ry, cx, pw, ph: INTEGER;
BEGIN
  IF (w <= 0) OR (h <= 0) THEN RETURN END;
  pw := Width(pb); ph := Height(pb);
  buf := gfx_alloc(w * h);
  IF buf = NIL THEN RETURN END;
  FOR r := 0 TO h - 1 DO
    ry := sy + r;
    FOR c := 0 TO w - 1 DO
      cx := sx + c;
      IF (cx >= 0) AND (cx < pw) AND (ry >= 0) AND (ry < ph) THEN
        gfx_buf_set(buf, r * w + c, GetPix(pb, cx, ry))
      ELSE
        gfx_buf_set(buf, r * w + c, 0)
      END
    END
  END;
  FOR r := 0 TO h - 1 DO
    ry := dy + r;
    IF (ry >= 0) AND (ry < ph) THEN
      FOR c := 0 TO w - 1 DO
        cx := dx + c;
        IF (cx >= 0) AND (cx < pw) THEN
          gfx_pb_set(pb, cx, ry, gfx_buf_get(buf, r * w + c))
        END
      END
    END
  END;
  gfx_dealloc(buf);
  gfx_pb_mark_dirty(pb, dx, dy, w, h)
END CopyRegion;

PROCEDURE AntiAlias(pb: PBuf; x, y, w, h, ncolors: INTEGER);
VAR buf: ADDRESS;
    r, c, sx, sy, pw, ph, center, diff: INTEGER;
    tr, tg, tb, cnt, ci, dr, dc: INTEGER;
    pi: INTEGER;
BEGIN
  IF (w <= 0) OR (h <= 0) THEN RETURN END;
  pw := Width(pb); ph := Height(pb);
  IF ncolors < 1 THEN ncolors := 32 END;
  buf := gfx_alloc(w * h);
  IF buf = NIL THEN RETURN END;
  FOR r := 0 TO h - 1 DO
    FOR c := 0 TO w - 1 DO
      sx := x + c; sy := y + r;
      IF (sx >= 0) AND (sx < pw) AND (sy >= 0) AND (sy < ph) THEN
        gfx_buf_set(buf, r * w + c, GetPix(pb, sx, sy))
      ELSE
        gfx_buf_set(buf, r * w + c, 0)
      END
    END
  END;
  FOR r := 1 TO h - 2 DO
    FOR c := 1 TO w - 2 DO
      center := gfx_buf_get(buf, r * w + c);
      diff := 0;
      IF gfx_buf_get(buf, (r-1)*w+c) # center THEN INC(diff) END;
      IF gfx_buf_get(buf, (r+1)*w+c) # center THEN INC(diff) END;
      IF gfx_buf_get(buf, r*w+c-1) # center THEN INC(diff) END;
      IF gfx_buf_get(buf, r*w+c+1) # center THEN INC(diff) END;
      IF diff >= 2 THEN
        tr := 0; tg := 0; tb := 0; cnt := 0;
        FOR dr := -1 TO 1 DO
          FOR dc := -1 TO 1 DO
            pi := gfx_buf_get(buf, (r+dr)*w+(c+dc));
            tr := tr + PalR(pb, pi);
            tg := tg + PalG(pb, pi);
            tb := tb + PalB(pb, pi);
            INC(cnt)
          END
        END;
        ci := NearestColor(pb, tr DIV cnt, tg DIV cnt, tb DIV cnt, ncolors);
        gfx_pb_set(pb, x + c, y + r, ci)
      END
    END
  END;
  gfx_dealloc(buf)
END AntiAlias;

(* ================================================================
   Phase 5: Polygon System
   ================================================================ *)

PROCEDURE PolyReset;
BEGIN polyN := 0 END PolyReset;

PROCEDURE PolyAdd(x, y: INTEGER);
BEGIN
  IF polyN < 256 THEN
    polyXs[polyN] := x; polyYs[polyN] := y;
    INC(polyN)
  END
END PolyAdd;

PROCEDURE PolyCount(): INTEGER;
BEGIN RETURN polyN END PolyCount;

PROCEDURE PolyX(i: INTEGER): INTEGER;
BEGIN
  IF (i >= 0) AND (i < polyN) THEN RETURN polyXs[i]
  ELSE RETURN 0 END
END PolyX;

PROCEDURE PolyY(i: INTEGER): INTEGER;
BEGIN
  IF (i >= 0) AND (i < polyN) THEN RETURN polyYs[i]
  ELSE RETURN 0 END
END PolyY;

PROCEDURE PolyDraw(pb: PBuf; idx: INTEGER);
VAR i: INTEGER;
BEGIN
  IF polyN < 2 THEN RETURN END;
  drawIdx := idx;
  FOR i := 0 TO polyN - 2 DO
    DrawAlgo.Line(pb, PBPoint, polyXs[i], polyYs[i], polyXs[i+1], polyYs[i+1])
  END;
  DrawAlgo.Line(pb, PBPoint, polyXs[polyN-1], polyYs[polyN-1], polyXs[0], polyYs[0])
END PolyDraw;

PROCEDURE PolyFill(pb: PBuf; idx: INTEGER);
VAR nx: ARRAY [0..255] OF INTEGER;
    ymin, ymax, sy, nodes, i, j, k, c: INTEGER;
    t: INTEGER;
BEGIN
  IF polyN < 3 THEN RETURN END;
  ymin := polyYs[0]; ymax := polyYs[0];
  FOR i := 1 TO polyN - 1 DO
    IF polyYs[i] < ymin THEN ymin := polyYs[i] END;
    IF polyYs[i] > ymax THEN ymax := polyYs[i] END
  END;
  FOR sy := ymin TO ymax DO
    nodes := 0; j := polyN - 1;
    FOR i := 0 TO polyN - 1 DO
      IF ((polyYs[i] < sy) AND (polyYs[j] >= sy))
      OR ((polyYs[j] < sy) AND (polyYs[i] >= sy)) THEN
        IF nodes < 256 THEN
          nx[nodes] := polyXs[i]
            + (sy - polyYs[i]) * (polyXs[j] - polyXs[i])
              DIV (polyYs[j] - polyYs[i]);
          INC(nodes)
        END
      END;
      j := i
    END;
    (* Sort intersection list *)
    FOR i := 0 TO nodes - 2 DO
      FOR k := i + 1 TO nodes - 1 DO
        IF nx[k] < nx[i] THEN t := nx[i]; nx[i] := nx[k]; nx[k] := t END
      END
    END;
    (* Fill spans *)
    i := 0;
    WHILE i < nodes - 1 DO
      FOR c := nx[i] TO nx[i+1] DO
        gfx_pb_set(pb, c, sy, idx)
      END;
      i := i + 2
    END
  END
END PolyFill;

(* ================================================================
   Layer System — pure M2 (state in module variables)
   ================================================================ *)

PROCEDURE CopyPal(src, dst: PBuf);
VAR i: INTEGER; packed: CARDINAL;
BEGIN
  FOR i := 0 TO 255 DO
    packed := CARDINAL(gfx_pb_pal_packed(src, i));
    gfx_pb_set_pal(dst, i,
      UnpackR(packed), UnpackG(packed), UnpackB(packed))
  END
END CopyPal;

PROCEDURE LayerInit(pb: PBuf);
BEGIN
  layers[0] := pb;
  layerVis[0] := 1;
  layerCount := 1;
  layerActive := 0
END LayerInit;

PROCEDURE LayerCount(): INTEGER;
BEGIN RETURN layerCount END LayerCount;

PROCEDURE LayerActive(): INTEGER;
BEGIN RETURN layerActive END LayerActive;

PROCEDURE LayerSetActive(idx: INTEGER);
BEGIN
  IF (idx >= 0) AND (idx < layerCount) THEN layerActive := idx END
END LayerSetActive;

PROCEDURE LayerGet(idx: INTEGER): PBuf;
BEGIN
  IF (idx >= 0) AND (idx < layerCount) THEN RETURN layers[idx]
  ELSE RETURN NIL END
END LayerGet;

PROCEDURE LayerGetActive(): PBuf;
BEGIN RETURN layers[layerActive] END LayerGetActive;

PROCEDURE LayerAdd(w, h: INTEGER): INTEGER;
VAR p: PBuf; idx: INTEGER;
BEGIN
  IF layerCount >= MaxLayers THEN RETURN -1 END;
  p := gfx_pb_create(w, h);
  IF p = NIL THEN RETURN -1 END;
  IF layers[0] # NIL THEN CopyPal(layers[0], p) END;
  gfx_pb_clear(p, 0);
  idx := layerCount;
  layers[idx] := p;
  layerVis[idx] := 1;
  INC(layerCount);
  RETURN idx
END LayerAdd;

PROCEDURE LayerRemove(idx: INTEGER);
VAR i: INTEGER;
BEGIN
  IF (idx <= 0) OR (idx >= layerCount) THEN RETURN END;
  gfx_pb_free(layers[idx]);
  FOR i := idx TO layerCount - 2 DO
    layers[i] := layers[i + 1];
    layerVis[i] := layerVis[i + 1]
  END;
  DEC(layerCount);
  IF layerActive >= layerCount THEN layerActive := layerCount - 1 END
END LayerRemove;

PROCEDURE LayerVisible(idx: INTEGER): BOOLEAN;
BEGIN
  IF (idx >= 0) AND (idx < layerCount) THEN RETURN layerVis[idx] # 0
  ELSE RETURN FALSE END
END LayerVisible;

PROCEDURE LayerSetVisible(idx: INTEGER; vis: BOOLEAN);
BEGIN
  IF (idx >= 0) AND (idx < layerCount) THEN
    IF vis THEN layerVis[idx] := 1 ELSE layerVis[idx] := 0 END
  END
END LayerSetVisible;

PROCEDURE LayerMoveUp(idx: INTEGER);
VAR tmp: PBuf; tv: INTEGER;
BEGIN
  IF (idx <= 0) OR (idx >= layerCount) THEN RETURN END;
  tmp := layers[idx]; layers[idx] := layers[idx - 1]; layers[idx - 1] := tmp;
  tv := layerVis[idx]; layerVis[idx] := layerVis[idx - 1]; layerVis[idx - 1] := tv;
  IF layerActive = idx THEN layerActive := idx - 1
  ELSIF layerActive = idx - 1 THEN layerActive := idx END
END LayerMoveUp;

PROCEDURE LayerMoveDown(idx: INTEGER);
BEGIN
  IF (idx < 0) OR (idx >= layerCount - 1) THEN RETURN END;
  LayerMoveUp(idx + 1)
END LayerMoveDown;

PROCEDURE LayerFlatten(dst: PBuf; transparentIdx: INTEGER);
VAR li: INTEGER;
BEGIN
  IF (layerCount > 0) AND (layerVis[0] # 0) AND (layers[0] # NIL) THEN
    gfx_pb_copy_pixels(layers[0], dst)
  ELSE
    gfx_pb_clear(dst, 0)
  END;
  FOR li := 1 TO layerCount - 1 DO
    IF (layerVis[li] # 0) AND (layers[li] # NIL) THEN
      gfx_pb_composite(dst, layers[li], transparentIdx)
    END
  END
END LayerFlatten;

(* ================================================================
   Text stamp — stays in C (SDL2_ttf)
   ================================================================ *)

PROCEDURE StampText(pb: PBuf; ren: Renderer; font: FontHandle;
                    text: ARRAY OF CHAR; x, y, idx: INTEGER);
BEGIN gfx_pb_stamp_text(pb, ren, font, ADR(text), x, y, idx) END StampText;

(* ================================================================
   Render — stays in C (SDL2)
   ================================================================ *)

PROCEDURE Render(ren: Renderer; tex: ADDRESS; pb: PBuf);
BEGIN gfx_pb_render(ren, tex, pb) END Render;

(* ================================================================
   Region save/restore — stays in C (malloc-based PBRegion)
   ================================================================ *)

PROCEDURE Save(pb: PBuf; x, y, w, h: INTEGER): Region;
BEGIN RETURN gfx_pb_save(pb, x, y, w, h) END Save;

PROCEDURE Restore(pb: PBuf; region: Region; x, y: INTEGER);
BEGIN gfx_pb_restore(pb, region, x, y) END Restore;

PROCEDURE SaveW(region: Region): INTEGER;
BEGIN RETURN gfx_pb_save_w(region) END SaveW;

PROCEDURE SaveH(region: Region): INTEGER;
BEGIN RETURN gfx_pb_save_h(region) END SaveH;

PROCEDURE FreeSave(region: Region);
BEGIN gfx_pb_free_save(region) END FreeSave;

(* ================================================================
   File I/O
   ================================================================ *)

(* ── Binary I/O helpers ─────────────────────────────────── *)

PROCEDURE WriteByteF(VAR f: File; v: INTEGER);
BEGIN WriteChar(f, CHR(BAND(CARDINAL(v), 0FFH))) END WriteByteF;

PROCEDURE Write16LE(VAR f: File; v: INTEGER);
BEGIN
  WriteByteF(f, BAND(CARDINAL(v), 0FFH));
  WriteByteF(f, BAND(SHR(CARDINAL(v), 8), 0FFH))
END Write16LE;

PROCEDURE Write32LE(VAR f: File; v: INTEGER);
BEGIN
  WriteByteF(f, BAND(CARDINAL(v), 0FFH));
  WriteByteF(f, BAND(SHR(CARDINAL(v), 8), 0FFH));
  WriteByteF(f, BAND(SHR(CARDINAL(v), 16), 0FFH));
  WriteByteF(f, BAND(SHR(CARDINAL(v), 24), 0FFH))
END Write32LE;

PROCEDURE ReadByteF(VAR f: File; VAR v: INTEGER);
VAR ch: CHAR;
BEGIN
  ReadChar(f, ch);
  IF Done THEN v := ORD(ch) ELSE v := 0 END
END ReadByteF;

PROCEDURE Read16LE(VAR f: File; VAR v: INTEGER);
VAR lo, hi: INTEGER;
BEGIN
  ReadByteF(f, lo); ReadByteF(f, hi);
  v := BOR(CARDINAL(lo), SHL(CARDINAL(hi), 8))
END Read16LE;

PROCEDURE Read32LE(VAR f: File; VAR v: INTEGER);
VAR b0, b1, b2, b3: INTEGER;
BEGIN
  ReadByteF(f, b0); ReadByteF(f, b1);
  ReadByteF(f, b2); ReadByteF(f, b3);
  v := BOR(BOR(CARDINAL(b0), SHL(CARDINAL(b1), 8)),
           BOR(SHL(CARDINAL(b2), 16), SHL(CARDINAL(b3), 24)))
END Read32LE;

(* ── SaveBMP — 8-bit indexed BMP ─────────────────────────── *)

PROCEDURE SaveBMP(pb: PBuf; path: ARRAY OF CHAR): BOOLEAN;
VAR f: File;
    w, h, rowSz, pixSz, off, fsz, padN: INTEGER;
    row, col, i: INTEGER;
    packed: CARDINAL;
    pix: ADDRESS;
BEGIN
  IF pb = NIL THEN RETURN FALSE END;
  w := gfx_pb_width(pb); h := gfx_pb_height(pb);
  rowSz := BAND(w + 3, INTEGER(BNOT(3)));  (* (w+3) & ~3 *)
  pixSz := rowSz * h;
  off := 14 + 40 + 1024;
  fsz := off + pixSz;
  padN := rowSz - w;

  Lookup(f, path, TRUE);
  IF NOT Done THEN RETURN FALSE END;

  (* BMP file header — 14 bytes *)
  WriteChar(f, "B"); WriteChar(f, "M");
  Write32LE(f, fsz);
  Write16LE(f, 0); Write16LE(f, 0);  (* reserved *)
  Write32LE(f, off);

  (* BITMAPINFOHEADER — 40 bytes *)
  Write32LE(f, 40);      (* header size *)
  Write32LE(f, w);
  Write32LE(f, h);
  Write16LE(f, 1);       (* planes *)
  Write16LE(f, 8);       (* bpp *)
  Write32LE(f, 0);       (* compression *)
  Write32LE(f, pixSz);
  Write32LE(f, 0);       (* h-res *)
  Write32LE(f, 0);       (* v-res *)
  Write32LE(f, 0);       (* colors used *)
  Write32LE(f, 0);       (* colors important *)

  (* Color table — 256 entries, BGRA order *)
  FOR i := 0 TO 255 DO
    packed := PalPacked(pb, i);
    WriteByteF(f, UnpackB(packed));
    WriteByteF(f, UnpackG(packed));
    WriteByteF(f, UnpackR(packed));
    WriteByteF(f, 0)
  END;

  (* Pixel data — bottom-up, padded rows *)
  pix := gfx_pb_pixel_ptr(pb);
  FOR row := h - 1 TO 0 BY -1 DO
    FOR col := 0 TO w - 1 DO
      WriteByteF(f, gfx_buf_get(pix, row * w + col))
    END;
    FOR i := 0 TO padN - 1 DO WriteByteF(f, 0) END
  END;
  Close(f);
  RETURN TRUE
END SaveBMP;

(* ── PNG — stays in C (stb_image) ────────────────────────── *)

PROCEDURE SavePNG(pb: PBuf; path: ARRAY OF CHAR): BOOLEAN;
BEGIN RETURN gfx_pb_save_png(pb, ADR(path)) # 0 END SavePNG;

PROCEDURE LoadPNG(path: ARRAY OF CHAR; ncolors: INTEGER): PBuf;
BEGIN RETURN gfx_pb_load_png(ADR(path), ncolors) END LoadPNG;

(* ── SaveDP2/LoadDP2 — pure M2 ──────────────────────────── *)

PROCEDURE SaveDP2(path: ARRAY OF CHAR): BOOLEAN;
VAR f: File;
    base: PBuf;
    li, i, total: INTEGER;
    packed: CARDINAL;
    pix: ADDRESS;
BEGIN
  IF layerCount < 1 THEN RETURN FALSE END;
  base := layers[0];
  IF base = NIL THEN RETURN FALSE END;

  Lookup(f, path, TRUE);
  IF NOT Done THEN RETURN FALSE END;

  (* Magic + version *)
  WriteChar(f, "D"); WriteChar(f, "P"); WriteChar(f, "2");
  WriteByteF(f, 0);  (* major ver byte *)
  WriteByteF(f, 1);  (* version 1 *)
  Write16LE(f, 256);  (* ncolors *)
  WriteByteF(f, layerCount);
  Write32LE(f, gfx_pb_width(base));
  Write32LE(f, gfx_pb_height(base));

  (* Palette — 256 * RGB *)
  FOR i := 0 TO 255 DO
    packed := PalPacked(base, i);
    WriteByteF(f, UnpackR(packed));
    WriteByteF(f, UnpackG(packed));
    WriteByteF(f, UnpackB(packed))
  END;

  (* Layer data *)
  FOR li := 0 TO layerCount - 1 DO
    WriteByteF(f, layerVis[li]);
    IF layers[li] # NIL THEN
      pix := gfx_pb_pixel_ptr(layers[li]);
      total := gfx_pb_width(layers[li]) * gfx_pb_height(layers[li]);
      FOR i := 0 TO total - 1 DO
        WriteByteF(f, gfx_buf_get(pix, i))
      END
    END
  END;
  Close(f);
  RETURN TRUE
END SaveDP2;

PROCEDURE LoadDP2(path: ARRAY OF CHAR): BOOLEAN;
VAR f: File;
    ch: CHAR;
    ver, nc, nl, w, h: INTEGER;
    pr, pg, pbb: ARRAY [0..255] OF INTEGER;
    i, li, idx, total: INTEGER;
    base, lp: PBuf;
    pix: ADDRESS;
    byt: INTEGER;
BEGIN
  Lookup(f, path, FALSE);
  IF NOT Done THEN RETURN FALSE END;

  (* Read and verify magic *)
  ReadChar(f, ch); IF ch # "D" THEN Close(f); RETURN FALSE END;
  ReadChar(f, ch); IF ch # "P" THEN Close(f); RETURN FALSE END;
  ReadChar(f, ch); IF ch # "2" THEN Close(f); RETURN FALSE END;
  ReadByteF(f, ver);  (* major byte — skip *)
  ReadByteF(f, ver);
  IF ver # 1 THEN Close(f); RETURN FALSE END;

  Read16LE(f, nc);  (* ncolors — skip *)
  ReadByteF(f, nl);  (* number of layers *)
  Read32LE(f, w); Read32LE(f, h);
  IF (nl < 1) OR (w < 1) OR (h < 1) OR (w > 8192) OR (h > 8192) THEN
    Close(f); RETURN FALSE
  END;

  (* Read palette *)
  FOR i := 0 TO 255 DO
    ReadByteF(f, pr[i]); ReadByteF(f, pg[i]); ReadByteF(f, pbb[i])
  END;

  (* Tear down existing layers *)
  WHILE layerCount > 1 DO LayerRemove(layerCount - 1) END;
  IF layers[0] # NIL THEN gfx_pb_free(layers[0]) END;

  (* Create base layer *)
  layers[0] := gfx_pb_create(w, h);
  IF layers[0] = NIL THEN Close(f); RETURN FALSE END;
  layerCount := 1;
  base := layers[0];

  (* Apply palette *)
  FOR i := 0 TO 255 DO gfx_pb_set_pal(base, i, pr[i], pg[i], pbb[i]) END;

  (* Read layer 0 visibility + pixels *)
  ReadByteF(f, layerVis[0]);
  pix := gfx_pb_pixel_ptr(base);
  total := w * h;
  FOR i := 0 TO total - 1 DO
    ReadByteF(f, byt);
    gfx_buf_set(pix, i, byt)
  END;
  gfx_pb_mark_dirty(base, 0, 0, w, h);

  (* Read additional layers *)
  FOR li := 1 TO nl - 1 DO
    idx := LayerAdd(w, h);
    IF idx < 0 THEN Close(f); RETURN TRUE END;  (* partial load *)
    ReadByteF(f, layerVis[idx]);
    lp := layers[idx];
    IF lp # NIL THEN
      CopyPal(base, lp);
      pix := gfx_pb_pixel_ptr(lp);
      FOR i := 0 TO total - 1 DO
        ReadByteF(f, byt);
        gfx_buf_set(pix, i, byt)
      END;
      gfx_pb_mark_dirty(lp, 0, 0, w, h)
    END
  END;
  layerActive := 0;
  Close(f);
  RETURN TRUE
END LoadDP2;

(* ── Palette file I/O helpers ─────────────────────────────── *)

PROCEDURE WriteIntF(VAR f: File; n: INTEGER);
VAR buf: ARRAY [0..10] OF CHAR;
    i, k: INTEGER;
    neg: BOOLEAN;
BEGIN
  IF n < 0 THEN neg := TRUE; n := -n ELSE neg := FALSE END;
  i := 0;
  REPEAT
    buf[i] := CHR(ORD("0") + n MOD 10);
    n := n DIV 10;
    INC(i)
  UNTIL n = 0;
  IF neg THEN WriteChar(f, "-") END;
  FOR k := i - 1 TO 0 BY -1 DO WriteChar(f, buf[k]) END
END WriteIntF;

PROCEDURE SkipWhite(VAR f: File; VAR ch: CHAR);
BEGIN
  WHILE Done AND ((ch = " ") OR (ch = 11C) OR  (* tab *)
        (ch = 12C) OR (ch = 15C)) DO            (* LF, CR *)
    ReadChar(f, ch)
  END
END SkipWhite;

PROCEDURE ReadIntF(VAR f: File; VAR n: INTEGER): BOOLEAN;
VAR ch: CHAR;
    neg: BOOLEAN;
    got: BOOLEAN;
BEGIN
  n := 0; got := FALSE; neg := FALSE;
  ReadChar(f, ch);
  SkipWhite(f, ch);
  IF NOT Done THEN RETURN FALSE END;
  IF ch = "-" THEN neg := TRUE; ReadChar(f, ch) END;
  WHILE Done AND (ch >= "0") AND (ch <= "9") DO
    n := n * 10 + (ORD(ch) - ORD("0"));
    got := TRUE;
    ReadChar(f, ch)
  END;
  IF neg THEN n := -n END;
  RETURN got
END ReadIntF;

(* ── Palette file I/O ────────────────────────────────────── *)

PROCEDURE SavePal(pb: PBuf; path: ARRAY OF CHAR): BOOLEAN;
VAR f: File;
    i, r, g, b: INTEGER;
    packed: CARDINAL;
BEGIN
  Lookup(f, path, TRUE);
  IF NOT Done THEN RETURN FALSE END;
  FOR i := 0 TO 255 DO
    packed := PalPacked(pb, i);
    r := UnpackR(packed);
    g := UnpackG(packed);
    b := UnpackB(packed);
    WriteIntF(f, r);
    WriteChar(f, " ");
    WriteIntF(f, g);
    WriteChar(f, " ");
    WriteIntF(f, b);
    WriteChar(f, 12C)  (* newline *)
  END;
  Close(f);
  RETURN TRUE
END SavePal;

PROCEDURE LoadPal(pb: PBuf; path: ARRAY OF CHAR): BOOLEAN;
VAR f: File;
    i, r, g, b: INTEGER;
BEGIN
  Lookup(f, path, FALSE);
  IF NOT Done THEN RETURN FALSE END;
  FOR i := 0 TO 255 DO
    IF NOT ReadIntF(f, r) THEN Close(f); RETURN i > 0 END;
    IF NOT ReadIntF(f, g) THEN Close(f); RETURN i > 0 END;
    IF NOT ReadIntF(f, b) THEN Close(f); RETURN i > 0 END;
    SetPal(pb, i, r, g, b)
  END;
  Close(f);
  RETURN TRUE
END LoadPal;

(* ================================================================
   Animation Frame System — pure M2 (state in module variables)
   ================================================================ *)

PROCEDURE FrameInit(pb: PBuf);
VAR i: INTEGER;
BEGIN
  FOR i := 0 TO MaxFrames - 1 DO
    frames[i] := NIL;
    frameTiming[i] := 100
  END;
  frames[0] := pb;
  nFrames := 1;
  currentFrame := 0
END FrameInit;

PROCEDURE FrameCount(): INTEGER;
BEGIN RETURN nFrames END FrameCount;

PROCEDURE FrameCurrent(): INTEGER;
BEGIN RETURN currentFrame END FrameCurrent;

PROCEDURE FrameNew(w, h: INTEGER): INTEGER;
VAR p: PBuf; idx: INTEGER;
BEGIN
  IF nFrames >= MaxFrames THEN RETURN -1 END;
  p := gfx_pb_create(w, h);
  IF p = NIL THEN RETURN -1 END;
  IF frames[0] # NIL THEN CopyPal(frames[0], p) END;
  gfx_pb_clear(p, 0);
  idx := nFrames;
  frames[idx] := p;
  frameTiming[idx] := 100;
  INC(nFrames);
  RETURN idx
END FrameNew;

PROCEDURE FrameDelete(idx: INTEGER);
VAR i: INTEGER;
BEGIN
  IF (idx < 0) OR (idx >= nFrames) OR (nFrames <= 1) THEN RETURN END;
  IF frames[idx] # NIL THEN gfx_pb_free(frames[idx]) END;
  FOR i := idx TO nFrames - 2 DO
    frames[i] := frames[i + 1];
    frameTiming[i] := frameTiming[i + 1]
  END;
  DEC(nFrames);
  frames[nFrames] := NIL;
  IF currentFrame >= nFrames THEN currentFrame := nFrames - 1 END
END FrameDelete;

PROCEDURE FrameSet(idx: INTEGER);
BEGIN
  IF (idx >= 0) AND (idx < nFrames) THEN currentFrame := idx END
END FrameSet;

PROCEDURE FrameGet(idx: INTEGER): PBuf;
BEGIN
  IF (idx >= 0) AND (idx < nFrames) THEN RETURN frames[idx]
  ELSE RETURN NIL END
END FrameGet;

PROCEDURE FrameGetCurrent(): PBuf;
BEGIN
  IF (currentFrame >= 0) AND (currentFrame < nFrames) THEN
    RETURN frames[currentFrame]
  ELSE RETURN NIL END
END FrameGetCurrent;

PROCEDURE FrameTiming(idx: INTEGER): INTEGER;
BEGIN
  IF (idx >= 0) AND (idx < nFrames) THEN RETURN frameTiming[idx]
  ELSE RETURN 100 END
END FrameTiming;

PROCEDURE FrameSetTiming(idx, ms: INTEGER);
BEGIN
  IF (idx >= 0) AND (idx < nFrames) THEN frameTiming[idx] := ms END
END FrameSetTiming;

PROCEDURE FrameDuplicate(idx: INTEGER): PBuf;
VAR src, p: PBuf; ni: INTEGER;
BEGIN
  IF (idx < 0) OR (idx >= nFrames) OR (nFrames >= MaxFrames) THEN RETURN NIL END;
  src := frames[idx];
  IF src = NIL THEN RETURN NIL END;
  p := gfx_pb_create(gfx_pb_width(src), gfx_pb_height(src));
  IF p = NIL THEN RETURN NIL END;
  gfx_pb_copy_pixels(src, p);
  ni := nFrames;
  frames[ni] := p;
  frameTiming[ni] := frameTiming[idx];
  INC(nFrames);
  RETURN p
END FrameDuplicate;

PROCEDURE RenderAlpha(ren: ADDRESS; tex: ADDRESS; pb: PBuf; alpha: INTEGER);
BEGIN gfx_pb_render_alpha(ren, tex, pb, alpha) END RenderAlpha;

PROCEDURE FramesToSheet(cols: INTEGER): PBuf;
VAR rows, fw, fh, f, dx, dy, x, y: INTEGER;
    sheet: PBuf;
    src: PBuf;
    srcBuf: ADDRESS;
    dstBuf: ADDRESS;
    sw, sheetW: INTEGER;
BEGIN
  IF nFrames <= 0 THEN RETURN NIL END;
  IF cols <= 0 THEN cols := nFrames END;
  rows := (nFrames + cols - 1) DIV cols;
  IF frames[0] # NIL THEN
    fw := gfx_pb_width(frames[0]); fh := gfx_pb_height(frames[0])
  ELSE fw := 64; fh := 64 END;
  sheet := gfx_pb_create(fw * cols, fh * rows);
  IF sheet = NIL THEN RETURN NIL END;
  IF frames[0] # NIL THEN CopyPal(frames[0], sheet) END;
  gfx_pb_clear(sheet, 0);
  sheetW := fw * cols;
  dstBuf := gfx_pb_pixel_ptr(sheet);
  FOR f := 0 TO nFrames - 1 DO
    src := frames[f];
    IF src # NIL THEN
      srcBuf := gfx_pb_pixel_ptr(src);
      sw := gfx_pb_width(src);
      dx := (f MOD cols) * fw;
      dy := (f DIV cols) * fh;
      FOR y := 0 TO fh - 1 DO
        IF y < gfx_pb_height(src) THEN
          FOR x := 0 TO fw - 1 DO
            IF x < sw THEN
              gfx_buf_set(dstBuf, (dy + y) * sheetW + (dx + x),
                          gfx_buf_get(srcBuf, y * sw + x))
            END
          END
        END
      END
    END
  END;
  gfx_pb_mark_dirty(sheet, 0, 0, fw * cols, fh * rows);
  RETURN sheet
END FramesToSheet;

(* ================================================================
   Advanced rendering — stays in C (SDL2)
   ================================================================ *)

PROCEDURE RenderHAM(ren: ADDRESS; tex: ADDRESS; pb: PBuf; mode: INTEGER);
BEGIN gfx_pb_render_ham(ren, tex, pb, mode) END RenderHAM;

PROCEDURE CopperGradient(ren: ADDRESS; tex: ADDRESS; pb: PBuf;
                         startLine, endLine, c1, c2: INTEGER);
VAR packed1, packed2, px, scr: CARDINAL;
    r1, g1, b1, r2, g2, b2: INTEGER;
    w, h, range, y, x, offset: INTEGER;
    tVal, tr, tg, tb: INTEGER;
    pixR, pixG, pixB: INTEGER;
BEGIN
  w := gfx_pb_width(pb); h := gfx_pb_height(pb);
  IF startLine < 0 THEN startLine := 0 END;
  IF endLine > h THEN endLine := h END;
  range := endLine - startLine;
  IF range <= 0 THEN RETURN END;

  (* Extract c1 and c2 palette colors *)
  packed1 := PalPacked(pb, c1 MOD 256);
  packed2 := PalPacked(pb, c2 MOD 256);
  r1 := UnpackR(packed1); g1 := UnpackG(packed1); b1 := UnpackB(packed1);
  r2 := UnpackR(packed2); g2 := UnpackG(packed2); b2 := UnpackB(packed2);

  (* Base render: palette -> screen-format RGBA buffer *)
  gfx_pb_pal_to_screen(pb);

  (* Tint scanlines in the copper range *)
  FOR y := startLine TO endLine - 1 DO
    tVal := (y - startLine) * 255 DIV range;
    tr := r1 + (r2 - r1) * tVal DIV 255;
    tg := g1 + (g2 - g1) * tVal DIV 255;
    tb := b1 + (b2 - b1) * tVal DIV 255;
    FOR x := 0 TO w - 1 DO
      offset := y * w + x;
      px := CARDINAL(gfx_pb_rgba_get32(pb, offset));
      (* Extract R, G, B from screen pixel (0xFFRRGGBB) *)
      pixR := INTEGER(BAND(SHR(px, 16), 0FFH));
      pixG := INTEGER(BAND(SHR(px, 8), 0FFH));
      pixB := INTEGER(BAND(px, 0FFH));
      (* Blend with copper color at 40% opacity *)
      pixR := (pixR * 60 + tr * 40) DIV 100;
      pixG := (pixG * 60 + tg * 40) DIV 100;
      pixB := (pixB * 60 + tb * 40) DIV 100;
      (* Pack back to screen format *)
      scr := BOR(BOR(BOR(0FF000000H, SHL(CARDINAL(pixR), 16)),
                     SHL(CARDINAL(pixG), 8)),
                 CARDINAL(pixB));
      gfx_pb_rgba_set32(pb, offset, INTEGER(scr))
    END
  END;

  (* Push to SDL texture *)
  gfx_pb_flush_tex(tex, pb)
END CopperGradient;

(* ================================================================
   Configuration persistence — pure M2 (Log stays in C for append)
   ================================================================ *)

PROCEDURE WriteStr(VAR f: File; s: ARRAY OF CHAR);
VAR i: INTEGER;
BEGIN
  i := 0;
  WHILE (i <= HIGH(s)) AND (s[i] # 0C) DO
    WriteChar(f, s[i]); INC(i)
  END
END WriteStr;

PROCEDURE ConfigSave(path: ARRAY OF CHAR; VAR keys, vals: ARRAY OF INTEGER;
                     count: INTEGER): BOOLEAN;
VAR f: File;
    i, n: INTEGER;
BEGIN
  Lookup(f, path, TRUE);
  IF NOT Done THEN RETURN FALSE END;
  WriteStr(f, "DPAINT_CFG 1");
  WriteChar(f, 12C);
  n := count;
  IF n > 64 THEN n := 64 END;
  FOR i := 0 TO n - 1 DO
    WriteIntF(f, keys[i]);
    WriteChar(f, " ");
    WriteIntF(f, vals[i]);
    WriteChar(f, 12C)
  END;
  Close(f);
  RETURN TRUE
END ConfigSave;

PROCEDURE ConfigLoad(path: ARRAY OF CHAR; VAR keys, vals: ARRAY OF INTEGER;
                     maxCount: INTEGER): INTEGER;
VAR f: File;
    ch: CHAR;
    cnt, ver: INTEGER;
BEGIN
  Lookup(f, path, FALSE);
  IF NOT Done THEN RETURN 0 END;
  (* Skip header line — read until newline *)
  ReadChar(f, ch);
  WHILE Done AND (ch # 12C) AND (ch # 15C) DO ReadChar(f, ch) END;
  (* Read key-value pairs *)
  cnt := 0;
  WHILE cnt < maxCount DO
    IF NOT ReadIntF(f, keys[cnt]) THEN Close(f); RETURN cnt END;
    IF NOT ReadIntF(f, vals[cnt]) THEN Close(f); RETURN cnt END;
    INC(cnt)
  END;
  Close(f);
  RETURN cnt
END ConfigLoad;

PROCEDURE Log(path, msg: ARRAY OF CHAR);
BEGIN gfx_log(ADR(path), ADR(msg)) END Log;

BEGIN
  polyN := 0;
  layerCount := 0;
  layerActive := 0;
  nFrames := 0;
  currentFrame := 0
END PixBuf.
