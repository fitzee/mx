# PixBuf

Indexed pixel buffer with an 8-bit palette supporting up to 256 colors. All drawing primitives operate on palette indices, not direct RGB values. The module provides a complete pixel art toolkit: drawing primitives, gradient and dither fills, region transforms, a polygon system, layers with compositing, animation frames with per-frame timing, and file I/O for BMP, PNG, DP2 (native format preserving layers/frames/palette), and raw palette files. To display a buffer on screen, call `Render` to expand palette indices to RGBA and push them to an SDL texture.

## Types

```modula2
TYPE
  PBuf   = ADDRESS;   (* opaque pixel buffer handle *)
  Region = ADDRESS;   (* saved region for undo      *)
```

`PBuf` is an opaque handle to a heap-allocated pixel buffer containing an indexed pixel array, an RGBA conversion buffer, a 256-entry palette, and dirty-tracking state. `Region` is an opaque handle to a saved rectangular snapshot of pixel data, used for undo.

---

## Buffer Management

### Create

```modula2
PROCEDURE Create(w, h: INTEGER): PBuf;
```

Allocates a new pixel buffer of dimensions `w` by `h` pixels. The pixel array is zero-initialized (all pixels set to index 0). The palette defaults to a greyscale ramp where index `i` maps to RGB `(i, i, i)`. Returns `NIL` if allocation fails. The caller must eventually call `Free` to release the buffer.

```modula2
pb := PixBuf.Create(320, 200);
```

### Free

```modula2
PROCEDURE Free(pb: PBuf);
```

Releases all memory associated with pixel buffer `pb`, including the pixel array, RGBA conversion buffer, and palette. Passing `NIL` is safe (no-op).

### Clear

```modula2
PROCEDURE Clear(pb: PBuf; idx: INTEGER);
```

Sets every pixel in the buffer to palette index `idx` (0..255). Marks the entire buffer as dirty so the next `Render` call will update the full texture.

```modula2
PixBuf.Clear(pb, 0);  (* fill with background color *)
```

### Width

```modula2
PROCEDURE Width(pb: PBuf): INTEGER;
```

Returns the width of pixel buffer `pb` in pixels. Returns 0 if `pb` is `NIL`.

### Height

```modula2
PROCEDURE Height(pb: PBuf): INTEGER;
```

Returns the height of pixel buffer `pb` in pixels. Returns 0 if `pb` is `NIL`.

---

## Palette

The palette holds 256 entries, each storing an RGB color. Internally each entry is packed as RGBA8888 (R in the most significant byte, A in the least significant byte, A always 0xFF). Drawing primitives write palette indices; colors are resolved at render time.

### SetPal

```modula2
PROCEDURE SetPal(pb: PBuf; idx, r, g, b: INTEGER);
```

Sets palette entry `idx` (0..255) to the color `(r, g, b)` where each channel is 0..255. Changing a palette entry marks the entire buffer dirty since any pixel referencing that index will change color on the next render.

```modula2
PixBuf.SetPal(pb, 0,   0,   0,   0);    (* black *)
PixBuf.SetPal(pb, 1, 255, 255, 255);    (* white *)
PixBuf.SetPal(pb, 2,  50, 100, 220);    (* blue  *)
```

### PalR

```modula2
PROCEDURE PalR(pb: PBuf; idx: INTEGER): INTEGER;
```

Returns the red channel (0..255) of palette entry `idx`. Extracts the value by unpacking the RGBA8888 packed representation.

### PalG

```modula2
PROCEDURE PalG(pb: PBuf; idx: INTEGER): INTEGER;
```

Returns the green channel (0..255) of palette entry `idx`.

### PalB

```modula2
PROCEDURE PalB(pb: PBuf; idx: INTEGER): INTEGER;
```

Returns the blue channel (0..255) of palette entry `idx`.

### PalPacked

```modula2
PROCEDURE PalPacked(pb: PBuf; idx: INTEGER): CARDINAL;
```

Returns palette entry `idx` as a packed RGBA8888 32-bit value. The layout is `0xRRGGBBFF` -- red in bits 31..24, green in bits 23..16, blue in bits 15..8, alpha in bits 7..0 (always 0xFF). Useful for direct color manipulation with the `Color` module's `UnpackR`/`UnpackG`/`UnpackB` procedures.

```modula2
packed := PixBuf.PalPacked(pb, 5);
r := Color.UnpackR(packed);
```

---

## Pixel Access

### SetPix

```modula2
PROCEDURE SetPix(pb: PBuf; x, y, idx: INTEGER);
```

Sets the pixel at coordinates `(x, y)` to palette index `idx` (0..255). Coordinates outside the buffer bounds are silently ignored (bounds-checked).

### GetPix

```modula2
PROCEDURE GetPix(pb: PBuf; x, y: INTEGER): INTEGER;
```

Returns the palette index of the pixel at `(x, y)`. Returns 0 for out-of-bounds coordinates.

```modula2
oldColor := PixBuf.GetPix(pb, 10, 20);
PixBuf.SetPix(pb, 10, 20, 5);
```

---

## Drawing Primitives

All drawing primitives take a palette index `idx` (0..255) as the drawing color. Coordinates are in pixel buffer space. Out-of-bounds pixels are clipped at the C bridge level.

### Line

```modula2
PROCEDURE Line(pb: PBuf; x1, y1, x2, y2, idx: INTEGER);
```

Draws a 1-pixel-wide line from `(x1, y1)` to `(x2, y2)` using Bresenham's algorithm (via `DrawAlgo.Line`). All pixels along the line are set to palette index `idx`.

```modula2
PixBuf.Line(pb, 0, 0, 319, 199, 1);
```

### ThickLine

```modula2
PROCEDURE ThickLine(pb: PBuf; x1, y1, x2, y2, idx, thick: INTEGER);
```

Draws a line from `(x1, y1)` to `(x2, y2)` with thickness `thick` pixels. If `thick` is 1 or less, falls back to a standard 1-pixel `Line`. For thicker lines, stamps a filled circle of radius `thick DIV 2` at each point along the Bresenham path, producing rounded endpoints and uniform width.

```modula2
PixBuf.ThickLine(pb, 10, 10, 200, 150, 3, 4);  (* 4px wide *)
```

### Rect

```modula2
PROCEDURE Rect(pb: PBuf; x, y, w, h, idx: INTEGER);
```

Draws an outlined (unfilled) rectangle with top-left corner at `(x, y)`, width `w`, and height `h`. The outline is 1 pixel wide, drawn using four line segments. The palette index `idx` sets the outline color.

### FillRect

```modula2
PROCEDURE FillRect(pb: PBuf; x, y, w, h, idx: INTEGER);
```

Fills a solid rectangle at `(x, y)` with dimensions `w` by `h` using palette index `idx`. Coordinates are clipped to buffer bounds. Uses optimized row fills internally.

```modula2
PixBuf.FillRect(pb, 40, 40, 100, 80, 2);
```

### Circle

```modula2
PROCEDURE Circle(pb: PBuf; cx, cy, radius, idx: INTEGER);
```

Draws an outlined circle centered at `(cx, cy)` with the given `radius` in pixels. Uses the Midpoint circle algorithm (via `DrawAlgo.Circle`). The outline is 1 pixel wide.

### FillCircle

```modula2
PROCEDURE FillCircle(pb: PBuf; cx, cy, radius, idx: INTEGER);
```

Draws a filled circle centered at `(cx, cy)` with the given `radius`. Uses horizontal span fills for each scanline, producing a solid disk.

```modula2
PixBuf.FillCircle(pb, 160, 100, 50, 4);
```

### Ellipse

```modula2
PROCEDURE Ellipse(pb: PBuf; cx, cy, rx, ry, idx: INTEGER);
```

Draws an outlined ellipse centered at `(cx, cy)` with horizontal radius `rx` and vertical radius `ry`. Uses the Midpoint ellipse algorithm.

### FillEllipse

```modula2
PROCEDURE FillEllipse(pb: PBuf; cx, cy, rx, ry, idx: INTEGER);
```

Draws a filled ellipse centered at `(cx, cy)` with radii `rx` and `ry`. Uses horizontal span fills for each scanline.

### Triangle

```modula2
PROCEDURE Triangle(pb: PBuf; x1, y1, x2, y2, x3, y3, idx: INTEGER);
```

Draws an outlined triangle with vertices at `(x1, y1)`, `(x2, y2)`, `(x3, y3)`. The outline is 1 pixel wide, drawn as three line segments.

### FillTriangle

```modula2
PROCEDURE FillTriangle(pb: PBuf; x1, y1, x2, y2, x3, y3, idx: INTEGER);
```

Draws a filled triangle with vertices at `(x1, y1)`, `(x2, y2)`, `(x3, y3)`. Uses scanline-based horizontal span filling.

```modula2
PixBuf.FillTriangle(pb, 160, 20, 100, 180, 220, 180, 6);
```

### FloodFill

```modula2
PROCEDURE FloodFill(pb: PBuf; x, y, idx: INTEGER);
```

Flood-fills the contiguous region containing `(x, y)` with palette index `idx`. The fill replaces all connected pixels that share the same index as the pixel at `(x, y)`. Uses a span-based scanline algorithm with an explicit stack (max depth 8192 entries). If `(x, y)` is out of bounds, or the target pixel already has index `idx`, the call is a no-op. Marks the entire buffer dirty after completion.

```modula2
PixBuf.FloodFill(pb, 50, 50, 3);  (* fill region at (50,50) with index 3 *)
```

### LinePerfect

```modula2
PROCEDURE LinePerfect(pb: PBuf; x0, y0, x1, y1, idx: INTEGER);
```

Draws a pixel-perfect line from `(x0, y0)` to `(x1, y1)` using a pure Modula-2 Bresenham implementation that sets pixels directly via `gfx_pb_set`. Unlike `Line` (which delegates to `DrawAlgo.Line` with a callback), this avoids callback overhead and is suitable for performance-critical paths. Produces identical output to `Line`.

### Bezier

```modula2
PROCEDURE Bezier(pb: PBuf; x1, y1, cx1, cy1, cx2, cy2,
                 x2, y2, idx, steps: INTEGER);
```

Draws a cubic Bezier curve from `(x1, y1)` to `(x2, y2)` with control points `(cx1, cy1)` and `(cx2, cy2)`. The curve is approximated with `steps` line segments. If `steps` is less than 4, it defaults to 32. Higher values produce smoother curves at the cost of more pixel operations.

```modula2
PixBuf.Bezier(pb, 10, 100, 80, 10, 240, 10, 310, 100, 1, 64);
```

---

## Fills

### Gradient

```modula2
PROCEDURE Gradient(pb: PBuf; x, y, w, h, c1, c2: INTEGER;
                   horiz: BOOLEAN; ncolors: INTEGER);
```

Fills the rectangle at `(x, y)` with dimensions `w` by `h` with a linear gradient interpolated between palette entries `c1` and `c2` (indices taken modulo 256). When `horiz` is `TRUE`, the gradient runs left to right; when `FALSE`, top to bottom. The RGB values of `c1` and `c2` are linearly interpolated per column (horizontal) or per row (vertical), and each interpolated color is mapped to the nearest palette entry within the first `ncolors` entries using Euclidean RGB distance. If `ncolors` is less than 1, it defaults to 32.

```modula2
PixBuf.SetPal(pb, 10, 0, 0, 128);    (* dark blue *)
PixBuf.SetPal(pb, 11, 100, 200, 255); (* light blue *)
PixBuf.Gradient(pb, 0, 0, 320, 200, 10, 11, FALSE, 32);  (* vertical sky *)
```

### GradientAngle

```modula2
PROCEDURE GradientAngle(pb: PBuf; x, y, w, h, c1, c2,
                        angleDeg, ncolors: INTEGER);
```

Fills the rectangle at `(x, y)` with dimensions `w` by `h` with a linear gradient at an arbitrary angle. `angleDeg` specifies the gradient direction in degrees (0 = left-to-right, 90 = top-to-bottom). The implementation projects each pixel position onto the gradient axis using `cos` and `sin`, normalizes the projection to the range [0.0, 1.0], then interpolates RGB between palette entries `c1` and `c2` and finds the nearest match in the first `ncolors` palette entries. Returns immediately if `w` or `h` is zero or negative. If `ncolors` is less than 1, defaults to 32.

```modula2
PixBuf.GradientAngle(pb, 0, 0, 320, 200, 1, 2, 45, 64);  (* 45-degree gradient *)
```

### PatternFill

```modula2
PROCEDURE PatternFill(pb: PBuf; x, y, w, h, fg, bg, pattern: INTEGER);
```

Fills the rectangle at `(x, y)` with dimensions `w` by `h` using an ordered dither pattern. Uses a 4x4 Bayer matrix (values 0..15). The `pattern` parameter is a threshold value clamped to 0..16: at each pixel, if the Bayer matrix value at `(row MOD 4, col MOD 4)` is less than `pattern`, the pixel is set to `fg`; otherwise `bg`. A threshold of 0 produces a solid `bg` fill; 16 produces solid `fg`; values in between produce increasingly dense stipple patterns.

```modula2
PixBuf.PatternFill(pb, 10, 10, 100, 80, 1, 0, 8);  (* 50% dither *)
```

### DitherFill

```modula2
PROCEDURE DitherFill(pb: PBuf; x, y, w, h, fg, bg,
                     matrixType, threshold: INTEGER);
```

Fills the rectangle at `(x, y)` with dimensions `w` by `h` using a Bayer dither matrix of configurable size. `matrixType` selects the matrix: 0 = 2x2 (threshold range 0..4), 1 = 4x4 (threshold range 0..16, the default), 2 = 8x8 (threshold range 0..64). The `threshold` is clamped to the valid range for the selected matrix. At each pixel, if the matrix value is less than `threshold`, the pixel is set to `fg`; otherwise `bg`. Larger matrices produce finer dither gradations.

```modula2
PixBuf.DitherFill(pb, 0, 0, 64, 64, 2, 0, 2, 32);  (* 8x8 dither, 50% *)
```

---

## Transforms

### FlipH

```modula2
PROCEDURE FlipH(pb: PBuf; x, y, w, h: INTEGER);
```

Flips the rectangular region at `(x, y)` with dimensions `w` by `h` horizontally (left-right mirror) in place. Each row within the region has its pixels reversed. Coordinates are bounds-checked per pixel.

### FlipV

```modula2
PROCEDURE FlipV(pb: PBuf; x, y, w, h: INTEGER);
```

Flips the rectangular region at `(x, y)` with dimensions `w` by `h` vertically (top-bottom mirror) in place. Rows are swapped symmetrically from top and bottom. Coordinates are bounds-checked per pixel.

```modula2
PixBuf.FlipV(pb, 0, 0, PixBuf.Width(pb), PixBuf.Height(pb));
```

### CopyRegion

```modula2
PROCEDURE CopyRegion(pb: PBuf; sx, sy, w, h, dx, dy: INTEGER);
```

Copies a rectangular region of `w` by `h` pixels from source position `(sx, sy)` to destination position `(dx, dy)` within the same buffer. Uses a temporary buffer internally, so overlapping source and destination regions are handled correctly. Out-of-bounds pixels in the source read as 0; out-of-bounds destinations are clipped. Returns immediately if `w` or `h` is zero or negative.

```modula2
PixBuf.CopyRegion(pb, 0, 0, 64, 64, 100, 50);  (* copy 64x64 block *)
```

### Rotate90

```modula2
PROCEDURE Rotate90(pb: PBuf; x, y, w, h: INTEGER);
```

Rotates the rectangular region at `(x, y)` with dimensions `w` by `h` clockwise by 90 degrees in place. The mapping is `(col, row)` to `(h-1-row, col)`. Uses a temporary buffer. The dirty region is marked as the bounding square of `max(w, h)`. Returns immediately if `w` or `h` is zero or negative.

### Rotate180

```modula2
PROCEDURE Rotate180(pb: PBuf; x, y, w, h: INTEGER);
```

Rotates the rectangular region at `(x, y)` with dimensions `w` by `h` by 180 degrees in place. The mapping is `(col, row)` to `(w-1-col, h-1-row)`. Uses a temporary buffer. Returns immediately if `w` or `h` is zero or negative.

### Rotate270

```modula2
PROCEDURE Rotate270(pb: PBuf; x, y, w, h: INTEGER);
```

Rotates the rectangular region at `(x, y)` with dimensions `w` by `h` clockwise by 270 degrees (equivalently, 90 degrees counter-clockwise) in place. The mapping is `(col, row)` to `(row, w-1-col)`. Uses a temporary buffer. Returns immediately if `w` or `h` is zero or negative.

---

## Color Utilities

### NearestColor

```modula2
PROCEDURE NearestColor(pb: PBuf; r, g, b, ncolors: INTEGER): INTEGER;
```

Searches the first `ncolors` palette entries (indices 0..`ncolors`-1) and returns the index of the entry closest to `(r, g, b)` by Euclidean distance in RGB space (sum of squared differences). Used internally by gradient fills, anti-aliasing, and PNG loading. For best results, `ncolors` should match the number of palette entries actually in use.

```modula2
idx := PixBuf.NearestColor(pb, 128, 0, 255, 32);  (* find closest purple *)
```

### ReplaceColor

```modula2
PROCEDURE ReplaceColor(pb: PBuf; oldIdx, newIdx: INTEGER);
```

Scans every pixel in the buffer and replaces all occurrences of palette index `oldIdx` with `newIdx`. Operates over the entire buffer dimensions.

```modula2
PixBuf.ReplaceColor(pb, 5, 10);  (* remap all index-5 pixels to index 10 *)
```

### AntiAlias

```modula2
PROCEDURE AntiAlias(pb: PBuf; x, y, w, h, ncolors: INTEGER);
```

Applies a 3x3 box-blur anti-aliasing pass to the rectangular region at `(x, y)` with dimensions `w` by `h`. For each interior pixel (excluding a 1-pixel border), if at least 2 of its 4 cardinal neighbors differ from the center pixel, the pixel is replaced with the nearest palette match (within the first `ncolors` entries) to the average RGB of its 3x3 neighborhood. This smooths jagged edges at color boundaries without affecting flat regions. If `ncolors` is less than 1, defaults to 32. Uses a temporary buffer snapshot so reads are not affected by writes. Returns immediately if `w` or `h` is zero or negative.

```modula2
PixBuf.AntiAlias(pb, 0, 0, 320, 200, 32);
```

---

## Polygon

The polygon system uses a global vertex buffer stored at module level. Vertices are shared across all buffers. The maximum vertex count is 256. Build a polygon by calling `PolyReset`, then `PolyAdd` for each vertex, then `PolyDraw` or `PolyFill` to render it.

### PolyReset

```modula2
PROCEDURE PolyReset;
```

Clears the global vertex buffer, setting the vertex count to 0. Must be called before building a new polygon.

### PolyAdd

```modula2
PROCEDURE PolyAdd(x, y: INTEGER);
```

Appends vertex `(x, y)` to the global vertex buffer. If the buffer is full (256 vertices), the call is silently ignored.

```modula2
PixBuf.PolyReset;
PixBuf.PolyAdd(160, 20);
PixBuf.PolyAdd(100, 180);
PixBuf.PolyAdd(220, 180);
```

### PolyCount

```modula2
PROCEDURE PolyCount(): INTEGER;
```

Returns the current number of vertices in the global vertex buffer.

### PolyX

```modula2
PROCEDURE PolyX(i: INTEGER): INTEGER;
```

Returns the X coordinate of vertex `i` (0-based). Returns 0 if `i` is out of range.

### PolyY

```modula2
PROCEDURE PolyY(i: INTEGER): INTEGER;
```

Returns the Y coordinate of vertex `i` (0-based). Returns 0 if `i` is out of range.

### PolyDraw

```modula2
PROCEDURE PolyDraw(pb: PBuf; idx: INTEGER);
```

Draws the outline of the polygon defined by the current vertex buffer onto pixel buffer `pb` using palette index `idx`. Draws line segments between consecutive vertices and closes the polygon by connecting the last vertex back to the first. Requires at least 2 vertices; does nothing if fewer.

```modula2
PixBuf.PolyDraw(pb, 1);  (* draw polygon outline in color 1 *)
```

### PolyFill

```modula2
PROCEDURE PolyFill(pb: PBuf; idx: INTEGER);
```

Fills the polygon defined by the current vertex buffer onto pixel buffer `pb` using palette index `idx`. Uses a scanline fill algorithm with edge intersection sorting. Requires at least 3 vertices; does nothing if fewer. For each scanline within the polygon's vertical extent, computes edge intersections, sorts them, and fills pixel spans between pairs.

```modula2
PixBuf.PolyReset;
PixBuf.PolyAdd(50, 50);
PixBuf.PolyAdd(150, 30);
PixBuf.PolyAdd(200, 100);
PixBuf.PolyAdd(120, 160);
PixBuf.PolyAdd(30, 120);
PixBuf.PolyFill(pb, 5);
```

---

## Text

### StampText

```modula2
PROCEDURE StampText(pb: PBuf; ren: Renderer; font: FontHandle;
                    text: ARRAY OF CHAR; x, y, idx: INTEGER);
```

Renders the string `text` using SDL2_ttf font `font` and stamps it onto the pixel buffer `pb` at position `(x, y)`. The text is first rendered to a temporary SDL surface in white using blended mode. Each pixel of the rendered text with alpha greater than 128 is written to the buffer as palette index `idx`; all other pixels are left unchanged. This converts anti-aliased TrueType text into a hard-edged single-color stamp suitable for indexed pixel art. The `ren` parameter is the SDL renderer (required by the rendering pipeline but not used for direct drawing).

```modula2
PixBuf.StampText(pb, ren, font, "HELLO", 10, 10, 1);
```

---

## Rendering

### Render

```modula2
PROCEDURE Render(ren: Renderer; tex: ADDRESS; pb: PBuf);
```

Expands the pixel buffer's palette indices to RGBA and uploads the result to SDL texture `tex`. Only the dirty sub-rectangle (pixels modified since the last render) is converted and uploaded, making repeated calls efficient when few pixels change. If no pixels are dirty, the call is a no-op. The texture must have been created at the same dimensions as the buffer using `Texture.Create`. After rendering, the dirty flag is cleared.

```modula2
PixBuf.Render(ren, tex, pb);
Texture.Draw(ren, tex, 0, 0);
Gfx.Present(ren);
```

### RenderAlpha

```modula2
PROCEDURE RenderAlpha(ren: ADDRESS; tex: ADDRESS; pb: PBuf; alpha: INTEGER);
```

Renders the pixel buffer to texture `tex` with global alpha transparency. `alpha` ranges from 0 (fully transparent) to 255 (fully opaque). Converts the entire buffer from palette indices to ARGB, sets the texture blend mode to `SDL_BLENDMODE_BLEND`, applies the alpha modulation, copies to the renderer, then restores the texture to full opacity and no blending. Useful for fading layers or ghost previews.

```modula2
PixBuf.RenderAlpha(ren, tex, pb, 128);  (* 50% transparent *)
```

### RenderHAM

```modula2
PROCEDURE RenderHAM(ren: ADDRESS; tex: ADDRESS; pb: PBuf; mode: INTEGER);
```

Renders the pixel buffer in Amiga-style Hold-And-Modify mode. `mode` is 6 for HAM6 or 8 for HAM8. In HAM mode, the top 2 bits of each pixel index encode a command: `00` = set color from palette (bottom 4 bits for HAM6, bottom 6 bits for HAM8), `01` = modify blue channel, `10` = modify red channel, `11` = modify green channel. Processing proceeds left-to-right per scanline, with RGB state held from the previous pixel. HAM6 provides 16 base palette colors with 4-bit channel modification; HAM8 provides 64 base colors with 6-bit modification. The result is uploaded to `tex` and the dirty flag is cleared.

```modula2
PixBuf.RenderHAM(ren, tex, pb, 6);  (* HAM6 rendering *)
```

### CopperGradient

```modula2
PROCEDURE CopperGradient(ren: ADDRESS; tex: ADDRESS; pb: PBuf;
                         startLine, endLine, c1, c2: INTEGER);
```

Renders the pixel buffer with an Amiga-style copper gradient overlay. First performs a standard palette-to-RGBA expansion of the entire buffer. Then, for each scanline between `startLine` and `endLine` (clamped to buffer bounds), tints the existing pixel colors by blending with a linearly interpolated gradient between palette entries `c1` and `c2` at 40% opacity (original pixel contributes 60%, copper color contributes 40%). `startLine` and `endLine` are scanline indices (0-based, exclusive end). The result is uploaded to `tex`. This simulates the Amiga copper coprocessor's per-scanline color register changes.

```modula2
PixBuf.CopperGradient(ren, tex, pb, 0, 200, 10, 11);  (* sky tint *)
```

---

## Region Save/Restore

The region system captures rectangular snapshots of pixel data for delta-based undo. A `Region` is a heap-allocated copy of a rectangular area's palette indices. Save before a destructive operation, then restore to undo it.

### Save

```modula2
PROCEDURE Save(pb: PBuf; x, y, w, h: INTEGER): Region;
```

Captures the rectangular area at `(x, y)` with dimensions `w` by `h` from pixel buffer `pb` and returns an opaque `Region` handle. The region is clipped to buffer bounds. Returns `NIL` if `pb` is `NIL` or the clipped region has zero area. The caller must eventually call `FreeSave` to release the region.

```modula2
rgn := PixBuf.Save(pb, 50, 50, 100, 80);
(* ... perform edits ... *)
PixBuf.Restore(pb, rgn, 50, 50);  (* undo *)
PixBuf.FreeSave(rgn);
```

### Restore

```modula2
PROCEDURE Restore(pb: PBuf; region: Region; x, y: INTEGER);
```

Writes the saved pixel data from `region` back into pixel buffer `pb` at position `(x, y)`. The destination coordinates can differ from the original save position, allowing region relocation. Out-of-bounds pixels are clipped. The affected area is marked dirty.

### SaveW

```modula2
PROCEDURE SaveW(region: Region): INTEGER;
```

Returns the width in pixels of the saved `region`. Returns 0 if `region` is `NIL`.

### SaveH

```modula2
PROCEDURE SaveH(region: Region): INTEGER;
```

Returns the height in pixels of the saved `region`. Returns 0 if `region` is `NIL`.

### FreeSave

```modula2
PROCEDURE FreeSave(region: Region);
```

Releases the memory allocated for `region`. Passing `NIL` is safe (no-op). Must be called for every `Region` returned by `Save` to avoid memory leaks.

---

## Layers

The layer system manages up to 16 layers stored in module-level arrays. Layer 0 is always the base layer. All drawing primitives operate on the active layer's `PBuf`. Layers can be individually shown or hidden, reordered, and composited (flattened) into a destination buffer. The palette is shared: when a new layer is added, it copies the palette from layer 0.

### LayerInit

```modula2
PROCEDURE LayerInit(pb: PBuf);
```

Initializes the layer system with `pb` as layer 0 (the base layer). Sets the layer count to 1, marks layer 0 as visible, and sets the active layer to 0. Must be called before any other layer operations.

```modula2
pb := PixBuf.Create(320, 200);
PixBuf.LayerInit(pb);
```

### LayerCount

```modula2
PROCEDURE LayerCount(): INTEGER;
```

Returns the current number of layers (1..16).

### LayerActive

```modula2
PROCEDURE LayerActive(): INTEGER;
```

Returns the index of the currently active layer (0-based).

### LayerSetActive

```modula2
PROCEDURE LayerSetActive(idx: INTEGER);
```

Sets the active layer to `idx`. Does nothing if `idx` is out of range (negative or >= layer count). Subsequent drawing operations target the newly active layer's `PBuf`.

```modula2
PixBuf.LayerSetActive(1);
drawTarget := PixBuf.LayerGetActive();
```

### LayerGet

```modula2
PROCEDURE LayerGet(idx: INTEGER): PBuf;
```

Returns the `PBuf` handle for layer `idx`. Returns `NIL` if `idx` is out of range.

### LayerGetActive

```modula2
PROCEDURE LayerGetActive(): PBuf;
```

Returns the `PBuf` handle for the currently active layer. Convenience wrapper equivalent to `LayerGet(LayerActive())`.

### LayerAdd

```modula2
PROCEDURE LayerAdd(w, h: INTEGER): INTEGER;
```

Creates a new layer with dimensions `w` by `h`, cleared to index 0, with the palette copied from layer 0. Appends it to the layer stack and marks it as visible. Returns the new layer's index, or -1 if the maximum layer count (16) has been reached or allocation fails.

```modula2
newIdx := PixBuf.LayerAdd(320, 200);
IF newIdx >= 0 THEN
  PixBuf.LayerSetActive(newIdx);
END;
```

### LayerRemove

```modula2
PROCEDURE LayerRemove(idx: INTEGER);
```

Removes and frees layer `idx`. Layer 0 (the base layer) cannot be removed -- if `idx` is 0 or out of range, the call is a no-op. Remaining layers above `idx` are shifted down to fill the gap. If the active layer index is at or beyond the new count, it is clamped to the last layer.

### LayerVisible

```modula2
PROCEDURE LayerVisible(idx: INTEGER): BOOLEAN;
```

Returns `TRUE` if layer `idx` is visible, `FALSE` if hidden or if `idx` is out of range.

### LayerSetVisible

```modula2
PROCEDURE LayerSetVisible(idx: INTEGER; vis: BOOLEAN);
```

Sets the visibility of layer `idx`. Hidden layers are skipped during `LayerFlatten`. Does nothing if `idx` is out of range.

```modula2
PixBuf.LayerSetVisible(2, FALSE);  (* hide layer 2 *)
```

### LayerMoveUp

```modula2
PROCEDURE LayerMoveUp(idx: INTEGER);
```

Swaps layer `idx` with layer `idx - 1`, moving it up (toward the base) in the stack. Does nothing if `idx` is 0 (already at bottom) or out of range. If the active layer is either of the swapped layers, the active index is updated to follow it.

### LayerMoveDown

```modula2
PROCEDURE LayerMoveDown(idx: INTEGER);
```

Swaps layer `idx` with layer `idx + 1`, moving it down (toward the top) in the stack. Does nothing if `idx` is the last layer or out of range. Implemented by calling `LayerMoveUp(idx + 1)`.

### LayerFlatten

```modula2
PROCEDURE LayerFlatten(dst: PBuf; transparentIdx: INTEGER);
```

Composites all visible layers into destination buffer `dst`. Layer 0 is copied directly (or `dst` is cleared to 0 if layer 0 is hidden). Layers 1 through `layerCount - 1` are composited in order: for each pixel in the source layer, if the pixel's index is not `transparentIdx`, it overwrites the corresponding pixel in `dst`. Hidden layers are skipped. The `dst` buffer should have the same dimensions as the layers.

```modula2
PixBuf.LayerFlatten(displayBuf, 0);  (* index 0 is transparent *)
PixBuf.Render(ren, tex, displayBuf);
```

---

## Animation Frames

The frame system manages up to 256 animation frames stored in module-level arrays. Each frame is an independent `PBuf` with its own pixel data and a per-frame timing value in milliseconds. The palette is shared: new frames copy the palette from frame 0. Frame 0 is initialized from the provided buffer.

### FrameInit

```modula2
PROCEDURE FrameInit(pb: PBuf);
```

Initializes the frame system with `pb` as frame 0. Sets the frame count to 1, the current frame to 0, and all timing slots to 100 ms. All other frame slots are set to `NIL`. Must be called before any other frame operations.

```modula2
pb := PixBuf.Create(64, 64);
PixBuf.FrameInit(pb);
```

### FrameCount

```modula2
PROCEDURE FrameCount(): INTEGER;
```

Returns the total number of frames (1..256).

### FrameCurrent

```modula2
PROCEDURE FrameCurrent(): INTEGER;
```

Returns the index of the currently selected frame (0-based).

### FrameNew

```modula2
PROCEDURE FrameNew(w, h: INTEGER): INTEGER;
```

Creates a new blank frame with dimensions `w` by `h`, cleared to index 0, with the palette copied from frame 0 and timing set to 100 ms. Appends it to the frame list. Returns the new frame's index, or -1 if the maximum frame count (256) has been reached or allocation fails.

```modula2
newFrame := PixBuf.FrameNew(64, 64);
PixBuf.FrameSet(newFrame);
```

### FrameDelete

```modula2
PROCEDURE FrameDelete(idx: INTEGER);
```

Deletes frame `idx` and frees its pixel buffer. Remaining frames above `idx` are shifted down. The last remaining frame cannot be deleted (the call is a no-op if `nFrames` is 1). If the current frame index is at or beyond the new count, it is clamped to the last frame. Does nothing if `idx` is out of range.

### FrameSet

```modula2
PROCEDURE FrameSet(idx: INTEGER);
```

Sets the current frame to `idx`. Does nothing if `idx` is out of range.

### FrameGet

```modula2
PROCEDURE FrameGet(idx: INTEGER): PBuf;
```

Returns the `PBuf` handle for frame `idx`. Returns `NIL` if `idx` is out of range.

### FrameGetCurrent

```modula2
PROCEDURE FrameGetCurrent(): PBuf;
```

Returns the `PBuf` handle for the currently selected frame. Returns `NIL` if the current frame index is somehow invalid.

### FrameTiming

```modula2
PROCEDURE FrameTiming(idx: INTEGER): INTEGER;
```

Returns the display timing for frame `idx` in milliseconds. Returns 100 (the default) if `idx` is out of range.

### FrameSetTiming

```modula2
PROCEDURE FrameSetTiming(idx, ms: INTEGER);
```

Sets the display timing for frame `idx` to `ms` milliseconds. Does nothing if `idx` is out of range. Typical values range from 16 (60 fps) to 500 (slow animation).

```modula2
PixBuf.FrameSetTiming(0, 200);   (* frame 0 displays for 200 ms *)
PixBuf.FrameSetTiming(1, 100);   (* frame 1 displays for 100 ms *)
```

### FrameDuplicate

```modula2
PROCEDURE FrameDuplicate(idx: INTEGER): PBuf;
```

Creates a deep copy of frame `idx`, including all pixel data and palette, and appends it to the frame list with the same timing value. Returns the new frame's `PBuf` handle, or `NIL` if `idx` is out of range, the source frame is `NIL`, or the maximum frame count (256) has been reached.

```modula2
copy := PixBuf.FrameDuplicate(0);
```

### FramesToSheet

```modula2
PROCEDURE FramesToSheet(cols: INTEGER): PBuf;
```

Assembles all frames into a single sprite sheet `PBuf`. Frames are laid out in a grid with `cols` columns; the number of rows is computed as `ceil(frameCount / cols)`. If `cols` is 0 or negative, it defaults to the total frame count (all frames in one row). Each cell has the dimensions of frame 0. The sheet copies frame 0's palette and is cleared to index 0. Frame pixels are copied directly into their grid positions. Returns `NIL` if there are no frames or allocation fails. The caller must `Free` the returned buffer when done.

```modula2
sheet := PixBuf.FramesToSheet(4);  (* 4 columns *)
PixBuf.SavePNG(sheet, "spritesheet.png");
PixBuf.Free(sheet);
```

---

## File I/O

### SaveBMP

```modula2
PROCEDURE SaveBMP(pb: PBuf; path: ARRAY OF CHAR): BOOLEAN;
```

Saves the pixel buffer as an 8-bit indexed BMP file at `path`. Writes a standard 14-byte BMP header, 40-byte BITMAPINFOHEADER, 1024-byte color table (256 BGRA entries), and bottom-up pixel rows padded to 4-byte alignment. Returns `TRUE` on success, `FALSE` if `pb` is `NIL` or the file cannot be created. Implemented in pure Modula-2 using `FileSystem`.

```modula2
ok := PixBuf.SaveBMP(pb, "output.bmp");
```

### SavePNG

```modula2
PROCEDURE SavePNG(pb: PBuf; path: ARRAY OF CHAR): BOOLEAN;
```

Saves the pixel buffer as a 24-bit RGB PNG file at `path`. The palette-indexed pixels are expanded to RGB before encoding using stb_image_write in the C bridge. Returns `TRUE` on success, `FALSE` on failure. Note: the PNG is not palette-indexed; palette information is lost.

```modula2
ok := PixBuf.SavePNG(pb, "output.png");
```

### LoadPNG

```modula2
PROCEDURE LoadPNG(path: ARRAY OF CHAR; ncolors: INTEGER): PBuf;
```

Loads a PNG image from `path` and quantizes it to the current default greyscale palette using the first `ncolors` entries. For each pixel, the RGB color is matched to the nearest palette entry by Euclidean distance. If `ncolors` is less than 1, defaults to 32. Returns a new `PBuf` with the image dimensions, or `NIL` if loading fails. The caller must `Free` the returned buffer. To load with a custom palette, create a buffer first, set the palette, then use the loaded buffer as a reference -- or set the palette on the returned buffer after loading.

```modula2
img := PixBuf.LoadPNG("sprite.png", 32);
```

### SaveDP2

```modula2
PROCEDURE SaveDP2(path: ARRAY OF CHAR): BOOLEAN;
```

Saves the complete project state to a DP2 file at `path`. The DP2 format is a native binary format that preserves the full palette (256 RGB entries), all layers (pixel data and visibility flags), and buffer dimensions. The file structure is: magic bytes `"DP2"`, version bytes (0, 1), 16-bit color count (256), 8-bit layer count, 32-bit width and height, 768 bytes of palette (256 * 3 RGB), then for each layer: 1 byte visibility flag followed by `w * h` bytes of pixel data. Returns `TRUE` on success, `FALSE` if there are no layers, the base layer is `NIL`, or the file cannot be created. Operates on the global layer system state.

```modula2
ok := PixBuf.SaveDP2("project.dp2");
```

### LoadDP2

```modula2
PROCEDURE LoadDP2(path: ARRAY OF CHAR): BOOLEAN;
```

Loads a DP2 file from `path`, replacing the current layer system state. Validates the magic bytes and version (must be 1). Enforces dimension limits (width and height each 1..8192). Tears down existing layers, creates a new base layer, applies the stored palette, and loads pixel data for each layer. Restores visibility flags. Sets the active layer to 0. Returns `TRUE` on success (including partial loads where some layers fail to allocate), `FALSE` on file errors or invalid format.

```modula2
IF PixBuf.LoadDP2("project.dp2") THEN
  pb := PixBuf.LayerGet(0);
END;
```

### SavePal

```modula2
PROCEDURE SavePal(pb: PBuf; path: ARRAY OF CHAR): BOOLEAN;
```

Saves the palette of buffer `pb` to a text file at `path`. Writes 256 lines, each containing three space-separated decimal integers (R G B, each 0..255) followed by a newline. Returns `TRUE` on success, `FALSE` if the file cannot be created.

```modula2
ok := PixBuf.SavePal(pb, "palette.pal");
```

### LoadPal

```modula2
PROCEDURE LoadPal(pb: PBuf; path: ARRAY OF CHAR): BOOLEAN;
```

Loads a palette from a text file at `path` into buffer `pb`. Reads up to 256 lines of `R G B` values (whitespace-separated decimal integers). Returns `TRUE` if at least one entry was read successfully, `FALSE` if the file cannot be opened or no entries were parsed. Partial reads (fewer than 256 entries) return `TRUE` with the successfully read entries applied.

```modula2
ok := PixBuf.LoadPal(pb, "palette.pal");
```

---

## Configuration

### ConfigSave

```modula2
PROCEDURE ConfigSave(path: ARRAY OF CHAR; VAR keys, vals: ARRAY OF INTEGER;
                     count: INTEGER): BOOLEAN;
```

Saves key-value configuration pairs to a text file at `path`. Writes a header line `"DPAINT_CFG 1"` followed by up to `count` lines, each containing a key-value pair as two space-separated integers. The `keys` and `vals` arrays must have at least `count` elements. Maximum 64 entries are written (if `count` exceeds 64 it is clamped). Returns `TRUE` on success, `FALSE` if the file cannot be created.

```modula2
keys[0] := 1; vals[0] := 320;   (* width *)
keys[1] := 2; vals[1] := 200;   (* height *)
ok := PixBuf.ConfigSave("config.dat", keys, vals, 2);
```

### ConfigLoad

```modula2
PROCEDURE ConfigLoad(path: ARRAY OF CHAR; VAR keys, vals: ARRAY OF INTEGER;
                     maxCount: INTEGER): INTEGER;
```

Loads key-value configuration pairs from a text file at `path`. Skips the header line, then reads up to `maxCount` key-value pairs into the `keys` and `vals` arrays. Returns the number of pairs successfully read, or 0 if the file cannot be opened.

```modula2
n := PixBuf.ConfigLoad("config.dat", keys, vals, 64);
```

### Log

```modula2
PROCEDURE Log(path, msg: ARRAY OF CHAR);
```

Appends the string `msg` followed by a newline to the file at `path`. Opens the file in append mode via the C bridge (`fopen` with `"a"`). Useful for debug logging. If the file does not exist, it is created.

```modula2
PixBuf.Log("debug.log", "Frame rendered");
```

---

## Complete Example

```modula2
MODULE PixBufDemo;

IMPORT Gfx, Canvas, Texture, PixBuf, Font, Events;

VAR
  win, ren, tex, pb, overlay: ADDRESS;
  font: ADDRESS;
  evt, frame, ticks, lastTick: INTEGER;
  running: BOOLEAN;

BEGIN
  IF Gfx.Init() AND Gfx.TTFInit() THEN
    win := Gfx.CreateWindow("PixBuf Demo", 320, 240, 1);
    ren := Gfx.CreateRenderer(win, 3);
    pb := PixBuf.Create(320, 240);
    tex := Texture.Create(ren, 320, 240);

    (* Set up a 16-color palette *)
    PixBuf.SetPal(pb,  0,   0,   0,   0);   (* black        *)
    PixBuf.SetPal(pb,  1, 255, 255, 255);   (* white        *)
    PixBuf.SetPal(pb,  2,  50, 100, 220);   (* blue         *)
    PixBuf.SetPal(pb,  3, 220,  50,  50);   (* red          *)
    PixBuf.SetPal(pb,  4,  50, 200,  50);   (* green        *)
    PixBuf.SetPal(pb,  5, 200, 200,  50);   (* yellow       *)
    PixBuf.SetPal(pb,  6, 128,   0, 200);   (* purple       *)
    PixBuf.SetPal(pb,  7, 180, 120,  60);   (* brown        *)
    PixBuf.SetPal(pb,  8,   0,  40, 100);   (* dark blue    *)
    PixBuf.SetPal(pb,  9,  40, 100, 180);   (* medium blue  *)
    PixBuf.SetPal(pb, 10, 100, 180, 240);   (* light blue   *)

    (* Draw a sky gradient background *)
    PixBuf.Gradient(pb, 0, 0, 320, 200, 8, 10, FALSE, 11);

    (* Draw ground *)
    PixBuf.FillRect(pb, 0, 200, 320, 40, 4);

    (* Draw a house *)
    PixBuf.FillRect(pb, 100, 140, 80, 60, 3);
    PixBuf.FillTriangle(pb, 90, 140, 140, 100, 190, 140, 7);
    PixBuf.FillRect(pb, 130, 170, 20, 30, 5);

    (* Draw a filled polygon star *)
    PixBuf.PolyReset;
    PixBuf.PolyAdd(260, 30);
    PixBuf.PolyAdd(270, 60);
    PixBuf.PolyAdd(300, 60);
    PixBuf.PolyAdd(275, 78);
    PixBuf.PolyAdd(285, 108);
    PixBuf.PolyAdd(260, 90);
    PixBuf.PolyAdd(235, 108);
    PixBuf.PolyAdd(245, 78);
    PixBuf.PolyAdd(220, 60);
    PixBuf.PolyAdd(250, 60);
    PixBuf.PolyFill(pb, 5);

    (* Draw a thick-line tree trunk and circle canopy *)
    PixBuf.ThickLine(pb, 50, 160, 50, 200, 7, 6);
    PixBuf.FillCircle(pb, 50, 145, 25, 4);

    (* Anti-alias the scene *)
    PixBuf.AntiAlias(pb, 0, 0, 320, 240, 11);

    (* Main loop *)
    running := TRUE;
    WHILE running DO
      evt := Events.Poll();
      WHILE evt # 0 DO
        IF evt = 1 THEN running := FALSE END;
        evt := Events.Poll();
      END;
      Canvas.SetColor(ren, 0, 0, 0, 255);
      Canvas.Clear(ren);
      PixBuf.Render(ren, tex, pb);
      Texture.Draw(ren, tex, 0, 0);
      Gfx.Present(ren);
      Gfx.Delay(16);
    END;

    PixBuf.SavePNG(pb, "demo_output.png");
    PixBuf.Free(pb);
    Texture.Destroy(tex);
    Gfx.DestroyRenderer(ren);
    Gfx.DestroyWindow(win);
    Gfx.TTFQuit;
    Gfx.Quit;
  END;
END PixBufDemo.
```
