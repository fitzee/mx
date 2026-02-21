# DrawAlgo

Backend-independent drawing algorithms parameterized by output callbacks. Every procedure takes an opaque context pointer and one or more callback procedures, making `DrawAlgo` usable with any rendering backend -- SDL renderer, software pixel buffer, or custom rasterizer. Used internally by `Canvas` (SDL) and `PixBuf` (indexed pixel buffer). All coordinates are in integer pixel space with no clipping; the callbacks are responsible for bounds checking.

## Types

### Ctx

```modula2
TYPE Ctx = ADDRESS;
```

Opaque context pointer passed through to every callback. Typically the address of a renderer or pixel buffer struct.

### PointFn

```modula2
TYPE PointFn = PROCEDURE(Ctx, INTEGER, INTEGER);
```

Callback that plots a single pixel. Parameters are `(ctx, x, y)`. The callback should draw one pixel at coordinates `(x, y)` using whatever color or state is associated with `ctx`.

### HLineFn

```modula2
TYPE HLineFn = PROCEDURE(Ctx, INTEGER, INTEGER, INTEGER);
```

Callback that draws a horizontal line span. Parameters are `(ctx, x1, x2, y)` where `x1 <= x2`. The callback should fill all pixels from `x1` to `x2` inclusive on row `y`. Used by fill algorithms to emit scanlines efficiently.

### LineFn

```modula2
TYPE LineFn = PROCEDURE(Ctx, INTEGER, INTEGER, INTEGER, INTEGER);
```

Callback that draws a line segment. Parameters are `(ctx, x1, y1, x2, y2)`. The callback should draw a line from `(x1, y1)` to `(x2, y2)`. Used by `Triangle` (outline) and `Bezier` (piecewise linear approximation).

## Lines

### Line

```modula2
PROCEDURE Line(ctx: Ctx; pt: PointFn;
               x1, y1, x2, y2: INTEGER);
```

Draws a line from `(x1, y1)` to `(x2, y2)` using Bresenham's line algorithm. Emits one `pt` callback per pixel, including both endpoints. Handles all octants (steep, shallow, negative slopes). The line is exactly one pixel wide.

```modula2
Line(ctx, MyPlotPixel, 0, 0, 100, 50);
```

## Circles

### Circle

```modula2
PROCEDURE Circle(ctx: Ctx; pt: PointFn;
                 cx, cy, radius: INTEGER);
```

Draws a circle outline centered at `(cx, cy)` with the given `radius` using the midpoint circle algorithm. Exploits 8-fold symmetry, emitting 8 `pt` calls per iteration. If `radius` is negative, returns immediately without drawing. A `radius` of 0 plots a single pixel at the center.

```modula2
Circle(ctx, MyPlotPixel, 160, 120, 50);
```

### FillCircle

```modula2
PROCEDURE FillCircle(ctx: Ctx; hl: HLineFn;
                     cx, cy, radius: INTEGER);
```

Draws a filled circle centered at `(cx, cy)` with the given `radius`. Uses the midpoint circle algorithm with horizontal line spans instead of individual pixels: emits 4 `hl` calls per iteration (two pairs of symmetric scanlines). If `radius` is negative, returns immediately. Some scanlines may be emitted more than once at octant boundaries; the callback should tolerate overdraw.

```modula2
FillCircle(ctx, MyHLine, 160, 120, 50);
```

## Ellipses

### Ellipse

```modula2
PROCEDURE Ellipse(ctx: Ctx; pt: PointFn;
                  cx, cy, rx, ry: INTEGER);
```

Draws an ellipse outline centered at `(cx, cy)` with horizontal radius `rx` and vertical radius `ry` using the two-region midpoint ellipse algorithm. Region 1 iterates while the slope magnitude is less than 1 (near the horizontal axis); region 2 covers the rest (near the vertical axis). Emits 4 `pt` calls per step (4-fold symmetry). If either `rx` or `ry` is negative, returns immediately. Uses `LONGREAL` arithmetic internally for the decision variable to avoid integer overflow on large radii.

```modula2
Ellipse(ctx, MyPlotPixel, 160, 120, 80, 40);
```

### FillEllipse

```modula2
PROCEDURE FillEllipse(ctx: Ctx; hl: HLineFn;
                      cx, cy, rx, ry: INTEGER);
```

Draws a filled ellipse centered at `(cx, cy)` with radii `rx` and `ry`. Uses the same two-region midpoint algorithm as `Ellipse` but emits horizontal spans via `hl` instead of individual pixels. Tracks `lastY` in region 1 to avoid redundant scanlines on the same row. In region 2, every step decrements `y`, so each scanline is emitted exactly once. If either radius is negative, returns immediately.

```modula2
FillEllipse(ctx, MyHLine, 160, 120, 80, 40);
```

## Triangles

### Triangle

```modula2
PROCEDURE Triangle(ctx: Ctx; ln: LineFn;
                   x1, y1, x2, y2, x3, y3: INTEGER);
```

Draws a triangle outline by invoking `ln` three times for the edges `(x1,y1)-(x2,y2)`, `(x2,y2)-(x3,y3)`, and `(x3,y3)-(x1,y1)`. Vertex order does not matter. The actual line rasterization is delegated entirely to the `ln` callback.

```modula2
Triangle(ctx, MyDrawLine, 10, 10, 100, 10, 55, 80);
```

### FillTriangle

```modula2
PROCEDURE FillTriangle(ctx: Ctx; hl: HLineFn;
                       x1, y1, x2, y2, x3, y3: INTEGER);
```

Draws a filled triangle using scanline rasterization. Vertices are sorted by ascending Y coordinate internally. For each scanline from the topmost to the bottommost vertex, two edge intersection X coordinates are computed via linear interpolation (`LFLOAT`/`TRUNC`), then a horizontal span is emitted via `hl`. Handles the degenerate case where all three vertices share the same Y (draws a single horizontal line from the leftmost to the rightmost X). The upper half (top vertex to middle vertex) and lower half (middle vertex to bottom vertex) use different edge pairs for the second intersection.

```modula2
FillTriangle(ctx, MyHLine, 10, 10, 100, 10, 55, 80);
```

## Curves

### Bezier

```modula2
PROCEDURE Bezier(ctx: Ctx; ln: LineFn;
                 x1, y1, cx1, cy1, cx2, cy2, x2, y2,
                 steps: INTEGER);
```

Draws a cubic Bezier curve from `(x1, y1)` to `(x2, y2)` with control points `(cx1, cy1)` and `(cx2, cy2)`. The curve is approximated as `steps` line segments; each segment is drawn via the `ln` callback. The parametric position is evaluated using the standard cubic Bernstein polynomial: `B(t) = (1-t)^3 * P0 + 3*(1-t)^2*t * P1 + 3*(1-t)*t^2 * P2 + t^3 * P3`, with `t` sampled uniformly at `1/steps` intervals. Intermediate coordinates are computed in `REAL` and rounded to the nearest integer via `TRUNC(v + 0.5)`. If `steps` is less than 1, nothing is drawn. Higher values of `steps` produce smoother curves; 16..32 is typical for screen-resolution output.

```modula2
Bezier(ctx, MyDrawLine, 10, 200, 50, 10, 150, 10, 190, 200, 24);
```

## Complete Example

Draw a filled circle and a Bezier curve into a hypothetical pixel buffer using custom callbacks.

```modula2
MODULE DrawDemo;

FROM SYSTEM IMPORT ADDRESS, ADR;
FROM DrawAlgo IMPORT Ctx, PointFn, HLineFn, LineFn,
                     Line, FillCircle, Bezier;

CONST
  W = 320;
  H = 240;

VAR
  pixels: ARRAY [0..W*H-1] OF CARDINAL;

PROCEDURE PlotPixel(ctx: Ctx; x, y: INTEGER);
BEGIN
  IF (x >= 0) AND (x < W) AND (y >= 0) AND (y < H) THEN
    pixels[y * W + x] := 0FFFFFFFFH
  END
END PlotPixel;

PROCEDURE HLine(ctx: Ctx; x1, x2, y: INTEGER);
VAR x: INTEGER;
BEGIN
  IF (y < 0) OR (y >= H) THEN RETURN END;
  IF x1 < 0 THEN x1 := 0 END;
  IF x2 >= W THEN x2 := W - 1 END;
  FOR x := x1 TO x2 DO
    pixels[y * W + x] := 0FF8800FFH
  END
END HLine;

PROCEDURE DrawLine(ctx: Ctx; x1, y1, x2, y2: INTEGER);
BEGIN
  Line(ctx, PlotPixel, x1, y1, x2, y2)
END DrawLine;

VAR ctx: Ctx;

BEGIN
  ctx := NIL;
  FillCircle(ctx, HLine, 160, 120, 60);
  Bezier(ctx, DrawLine, 10, 200, 60, 20, 260, 20, 310, 200, 24)
END DrawDemo.
```
