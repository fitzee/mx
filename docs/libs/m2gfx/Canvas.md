# Canvas

Provides 2D drawing primitives on the SDL renderer. All shapes are drawn using the current color established by `SetColor`. Outline procedures (`Draw*`) render 1-pixel-wide strokes; fill procedures (`Fill*`) render solid interiors. Circle and ellipse algorithms use midpoint/Bresenham rasterization; triangle fill uses scanline interpolation; rounded rectangles use corner-arc decomposition. Requires a valid `Gfx.Renderer` obtained from `Gfx.CreateRenderer`.

## Color

### SetColor

```modula2
PROCEDURE SetColor(ren: Renderer; r, g, b, a: INTEGER);
```

Sets the current drawing color for all subsequent draw and fill operations on `ren`. Each component is in the range 0..255: `r` (red), `g` (green), `b` (blue), `a` (alpha, where 255 is fully opaque and 0 is fully transparent). Alpha blending only takes effect if `SetBlendMode` has been called with `BLEND_ALPHA`.

```modula2
SetColor(ren, 255, 0, 0, 255);   (* opaque red *)
SetColor(ren, 0, 0, 0, 128);     (* 50% transparent black *)
```

### GetColor

```modula2
PROCEDURE GetColor(ren: Renderer; VAR r, g, b, a: INTEGER);
```

Retrieves the current drawing color of `ren` into the four `VAR` parameters. Each value will be in 0..255. Useful for saving and restoring color state around helper routines.

```modula2
VAR oldR, oldG, oldB, oldA: INTEGER;
GetColor(ren, oldR, oldG, oldB, oldA);
SetColor(ren, 255, 255, 0, 255);
FillRect(ren, 0, 0, 100, 100);
SetColor(ren, oldR, oldG, oldB, oldA);
```

### Clear

```modula2
PROCEDURE Clear(ren: Renderer);
```

Fills the entire render target (or viewport, if set) with the current color. Typically called at the start of each frame before drawing.

```modula2
SetColor(ren, 0, 0, 0, 255);
Clear(ren);  (* black background *)
```

## Rectangles

### DrawRect

```modula2
PROCEDURE DrawRect(ren: Renderer; x, y, w, h: INTEGER);
```

Draws a 1-pixel-wide rectangle outline with its top-left corner at (`x`, `y`) and dimensions `w` x `h` pixels.

### FillRect

```modula2
PROCEDURE FillRect(ren: Renderer; x, y, w, h: INTEGER);
```

Draws a filled rectangle with its top-left corner at (`x`, `y`) and dimensions `w` x `h` pixels. This is the fastest fill primitive as it maps directly to `SDL_RenderFillRect`.

```modula2
SetColor(ren, 50, 100, 200, 255);
FillRect(ren, 10, 10, 200, 100);     (* solid blue rectangle *)
SetColor(ren, 255, 255, 255, 255);
DrawRect(ren, 10, 10, 200, 100);     (* white border *)
```

### DrawRoundRect

```modula2
PROCEDURE DrawRoundRect(ren: Renderer; x, y, w, h, radius: INTEGER);
```

Draws a rectangle outline with rounded corners. `radius` specifies the corner arc radius in pixels; it is clamped to at most half the smaller dimension (`w DIV 2` or `h DIV 2`). Negative `radius` values are treated as 0 (sharp corners). The corners are rendered using Bresenham's circle algorithm, connected by straight edge segments.

```modula2
DrawRoundRect(ren, 50, 50, 300, 200, 12);
```

### FillRoundRect

```modula2
PROCEDURE FillRoundRect(ren: Renderer; x, y, w, h, radius: INTEGER);
```

Draws a filled rectangle with rounded corners. The `radius` is clamped the same way as `DrawRoundRect`. Internally decomposes into three filled rectangles (body + top/bottom strips) plus four filled corner arcs.

```modula2
SetColor(ren, 60, 60, 80, 255);
FillRoundRect(ren, 20, 20, 260, 160, 16);
```

## Lines & Points

### DrawLine

```modula2
PROCEDURE DrawLine(ren: Renderer; x1, y1, x2, y2: INTEGER);
```

Draws a 1-pixel-wide line from (`x1`, `y1`) to (`x2`, `y2`) using the current color. Maps directly to `SDL_RenderDrawLine`.

### DrawThickLine

```modula2
PROCEDURE DrawThickLine(ren: Renderer; x1, y1, x2, y2, thickness: INTEGER);
```

Draws a line from (`x1`, `y1`) to (`x2`, `y2`) with the specified `thickness` in pixels. A `thickness` of 1 falls back to `DrawLine`. A `thickness` of 0 or less draws nothing. For degenerate lines (zero length), a filled circle of diameter `thickness` is drawn at the point. The thick line is rendered as two filled triangles forming a quad perpendicular to the line direction.

```modula2
SetColor(ren, 255, 200, 0, 255);
DrawThickLine(ren, 10, 300, 400, 100, 4);
```

### DrawPoint

```modula2
PROCEDURE DrawPoint(ren: Renderer; x, y: INTEGER);
```

Draws a single pixel at (`x`, `y`) using the current color.

## Circles & Ellipses

### DrawCircle

```modula2
PROCEDURE DrawCircle(ren: Renderer; cx, cy, radius: INTEGER);
```

Draws a circle outline centered at (`cx`, `cy`) with the given `radius` in pixels. Uses the midpoint circle algorithm (Bresenham) for pixel-perfect rasterization with no floating-point math. Negative `radius` draws nothing.

```modula2
SetColor(ren, 0, 255, 0, 255);
DrawCircle(ren, 320, 240, 100);
```

### FillCircle

```modula2
PROCEDURE FillCircle(ren: Renderer; cx, cy, radius: INTEGER);
```

Draws a filled circle centered at (`cx`, `cy`) with the given `radius`. Uses horizontal scanline fills for each row of the midpoint circle, ensuring no gaps. Negative `radius` draws nothing.

### DrawEllipse

```modula2
PROCEDURE DrawEllipse(ren: Renderer; cx, cy, rx, ry: INTEGER);
```

Draws an ellipse outline centered at (`cx`, `cy`) with horizontal radius `rx` and vertical radius `ry`. Uses the two-region midpoint ellipse algorithm. Negative radii draw nothing. When `rx` equals `ry`, this is equivalent to `DrawCircle`.

### FillEllipse

```modula2
PROCEDURE FillEllipse(ren: Renderer; cx, cy, rx, ry: INTEGER);
```

Draws a filled ellipse centered at (`cx`, `cy`) with horizontal radius `rx` and vertical radius `ry`. Uses scanline fills across both regions of the midpoint ellipse algorithm, with duplicate-row elimination. Negative radii draw nothing.

```modula2
SetColor(ren, 200, 100, 50, 255);
FillEllipse(ren, 400, 300, 120, 60);   (* wide oval *)
DrawEllipse(ren, 400, 300, 120, 60);   (* outline on top *)
```

## Triangles

### DrawTriangle

```modula2
PROCEDURE DrawTriangle(ren: Renderer; x1, y1, x2, y2, x3, y3: INTEGER);
```

Draws a triangle outline connecting the three vertices (`x1`, `y1`), (`x2`, `y2`), (`x3`, `y3`) with three 1-pixel lines.

### FillTriangle

```modula2
PROCEDURE FillTriangle(ren: Renderer; x1, y1, x2, y2, x3, y3: INTEGER);
```

Draws a filled triangle with vertices (`x1`, `y1`), (`x2`, `y2`), (`x3`, `y3`). Uses scanline rasterization: vertices are sorted by Y, then each scanline is interpolated between the long edge and the appropriate short edge. Degenerate triangles (all vertices collinear) produce a single horizontal line.

```modula2
SetColor(ren, 255, 100, 100, 255);
FillTriangle(ren, 300, 50, 200, 250, 400, 250);
SetColor(ren, 255, 255, 255, 255);
DrawTriangle(ren, 300, 50, 200, 250, 400, 250);
```

## Arcs & Curves

### DrawArc

```modula2
PROCEDURE DrawArc(ren: Renderer; cx, cy, radius, startDeg, endDeg: INTEGER);
```

Draws a circular arc centered at (`cx`, `cy`) with the given `radius`, sweeping from `startDeg` to `endDeg` in degrees. Angles use standard math convention: 0 is rightward (+X), and positive values go counter-clockwise. Negative angles are normalized to 0..359. If `endDeg` <= `startDeg` after normalization, the arc wraps through 360 degrees. The arc is rendered as a series of connected line segments; the segment count is proportional to `2 * PI * radius` (minimum 36 segments). Draws nothing if `radius` <= 0.

```modula2
SetColor(ren, 255, 255, 0, 255);
DrawArc(ren, 200, 200, 80, 0, 90);     (* quarter arc, bottom-right *)
DrawArc(ren, 200, 200, 80, 45, 315);   (* 270-degree arc *)
```

### DrawBezier

```modula2
PROCEDURE DrawBezier(ren: Renderer;
                     x1, y1, cx1, cy1, cx2, cy2, x2, y2,
                     steps: INTEGER);
```

Draws a cubic Bezier curve from (`x1`, `y1`) to (`x2`, `y2`) with control points (`cx1`, `cy1`) and (`cx2`, `cy2`). The `steps` parameter controls smoothness: higher values produce smoother curves at the cost of more line segments. Typical values are 20..100. A `steps` value less than 1 draws nothing. The curve is evaluated using the standard cubic Bernstein polynomial and rendered as connected line segments.

```modula2
SetColor(ren, 100, 200, 255, 255);
DrawBezier(ren, 50, 300, 150, 50, 350, 50, 450, 300, 50);
```

## Clipping

### SetClip

```modula2
PROCEDURE SetClip(ren: Renderer; x, y, w, h: INTEGER);
```

Restricts all subsequent drawing to the rectangle defined by top-left (`x`, `y`) and dimensions `w` x `h`. Pixels outside this rectangle are discarded. Only one clip rectangle is active at a time; calling `SetClip` again replaces the previous clip.

```modula2
SetClip(ren, 100, 100, 200, 200);
FillCircle(ren, 200, 200, 150);   (* clipped to 200x200 box *)
ClearClip(ren);
```

### ClearClip

```modula2
PROCEDURE ClearClip(ren: Renderer);
```

Removes the active clip rectangle, restoring drawing to the full render target. Must be called to undo `SetClip`.

### GetClipX

```modula2
PROCEDURE GetClipX(ren: Renderer): INTEGER;
```

Returns the X coordinate of the current clip rectangle's top-left corner. Returns 0 if no clip is set.

### GetClipY

```modula2
PROCEDURE GetClipY(ren: Renderer): INTEGER;
```

Returns the Y coordinate of the current clip rectangle's top-left corner. Returns 0 if no clip is set.

### GetClipW

```modula2
PROCEDURE GetClipW(ren: Renderer): INTEGER;
```

Returns the width of the current clip rectangle. Returns 0 if no clip is set.

### GetClipH

```modula2
PROCEDURE GetClipH(ren: Renderer): INTEGER;
```

Returns the height of the current clip rectangle. Returns 0 if no clip is set.

```modula2
(* Save and restore clip state *)
savedX := GetClipX(ren);
savedY := GetClipY(ren);
savedW := GetClipW(ren);
savedH := GetClipH(ren);
SetClip(ren, 0, 0, 100, 100);
(* ... draw ... *)
IF savedW > 0 THEN
  SetClip(ren, savedX, savedY, savedW, savedH)
ELSE
  ClearClip(ren)
END;
```

## Blend Mode

### Constants

- `BLEND_NONE` (0) -- no blending; source pixels overwrite destination (default)
- `BLEND_ALPHA` (1) -- standard alpha blending: `dst = src * srcA + dst * (1 - srcA)`
- `BLEND_ADD` (2) -- additive blending: `dst = src * srcA + dst`; produces glow/light effects
- `BLEND_MOD` (4) -- modulate: `dst = src * dst`; darkens; useful for shadows and tinting

### SetBlendMode

```modula2
PROCEDURE SetBlendMode(ren: Renderer; mode: INTEGER);
```

Sets the blend mode for all subsequent drawing operations on `ren`. Use the `BLEND_*` constants. `BLEND_ALPHA` must be enabled for the alpha component of `SetColor` to produce transparency. Invalid values default to `BLEND_NONE`.

```modula2
SetBlendMode(ren, BLEND_ALPHA);
SetColor(ren, 0, 0, 0, 128);          (* 50% transparent black *)
FillRect(ren, 50, 50, 200, 200);      (* semi-transparent overlay *)
SetBlendMode(ren, BLEND_NONE);        (* restore opaque drawing *)
```

## Viewport

### SetViewport

```modula2
PROCEDURE SetViewport(ren: Renderer; x, y, w, h: INTEGER);
```

Sets the drawing viewport to the sub-rectangle at (`x`, `y`) with dimensions `w` x `h`. All subsequent drawing coordinates are relative to this viewport's top-left corner, and `Clear` only fills the viewport area. Useful for split-screen layouts or UI panels.

```modula2
(* Left panel *)
SetViewport(ren, 0, 0, 400, 600);
SetColor(ren, 20, 20, 40, 255);
Clear(ren);

(* Right panel *)
SetViewport(ren, 400, 0, 400, 600);
SetColor(ren, 40, 20, 20, 255);
Clear(ren);

ResetViewport(ren);
```

### ResetViewport

```modula2
PROCEDURE ResetViewport(ren: Renderer);
```

Resets the viewport to the full render target. Call after `SetViewport` to restore normal full-window drawing.

## Example

```modula2
MODULE CanvasDemo;

FROM Gfx IMPORT Init, Quit, CreateWindow, DestroyWindow,
                 CreateRenderer, DestroyRenderer, Present, Delay,
                 WIN_CENTERED, WIN_HIGHDPI,
                 RENDER_ACCELERATED, RENDER_VSYNC;
FROM Canvas IMPORT SetColor, Clear, SetBlendMode,
                   FillRect, DrawRect, FillRoundRect,
                   DrawLine, DrawThickLine, DrawPoint,
                   FillCircle, DrawCircle,
                   FillEllipse, DrawEllipse,
                   FillTriangle, DrawTriangle,
                   DrawArc, DrawBezier,
                   SetClip, ClearClip,
                   BLEND_ALPHA, BLEND_NONE;
FROM Events IMPORT Poll, QUIT_EVENT;

VAR
  win, ren: ADDRESS;
  evt: INTEGER;
  running: BOOLEAN;

BEGIN
  IF Init() THEN
    win := CreateWindow("Canvas Demo", 800, 600,
                        WIN_CENTERED + WIN_HIGHDPI);
    ren := CreateRenderer(win, RENDER_ACCELERATED + RENDER_VSYNC);
    running := TRUE;

    WHILE running DO
      evt := Poll();
      WHILE evt # 0 DO
        IF evt = QUIT_EVENT THEN running := FALSE END;
        evt := Poll()
      END;

      (* Clear to dark background *)
      SetColor(ren, 20, 20, 30, 255);
      Clear(ren);

      (* Filled rounded rectangle with border *)
      SetColor(ren, 50, 80, 140, 255);
      FillRoundRect(ren, 30, 30, 220, 140, 12);
      SetColor(ren, 100, 160, 255, 255);
      DrawRect(ren, 30, 30, 220, 140);

      (* Filled circle *)
      SetColor(ren, 200, 60, 60, 255);
      FillCircle(ren, 400, 100, 60);
      SetColor(ren, 255, 120, 120, 255);
      DrawCircle(ren, 400, 100, 60);

      (* Filled triangle *)
      SetColor(ren, 60, 180, 80, 255);
      FillTriangle(ren, 550, 30, 500, 160, 700, 160);
      SetColor(ren, 120, 240, 140, 255);
      DrawTriangle(ren, 550, 30, 500, 160, 700, 160);

      (* Thick line *)
      SetColor(ren, 255, 200, 50, 255);
      DrawThickLine(ren, 30, 250, 300, 350, 3);

      (* Ellipse *)
      SetColor(ren, 180, 100, 220, 255);
      FillEllipse(ren, 500, 300, 100, 50);

      (* Arc *)
      SetColor(ren, 255, 255, 100, 255);
      DrawArc(ren, 200, 450, 80, 30, 270);

      (* Bezier curve *)
      SetColor(ren, 100, 220, 255, 255);
      DrawBezier(ren, 400, 400, 450, 250, 650, 550, 700, 400, 40);

      (* Semi-transparent overlay using alpha blending *)
      SetBlendMode(ren, BLEND_ALPHA);
      SetColor(ren, 0, 0, 0, 100);
      FillRect(ren, 350, 200, 200, 200);
      SetBlendMode(ren, BLEND_NONE);

      Present(ren)
    END;

    DestroyRenderer(ren);
    DestroyWindow(win);
    Quit
  END
END CanvasDemo.
```
