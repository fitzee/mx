# DPaint M2+ — Complete Specification

An Amiga DeluxePaint-inspired pixel art editor written in Modula-2+ using the
m2gfx SDL2 graphics library. The program is a single-module application that
demonstrates M2+ language features (REF types, exceptions, TRY/FINALLY) while
providing a usable indexed-color painting tool.

---

## 1. Architecture

### 1.1 Indexed Pixel Buffer

All artwork is stored in an 8-bit indexed pixel buffer (`PixBuf.PBuf`). Each
pixel holds a palette index (0–255). A separate 256-entry palette maps indices
to RGB triples. Drawing tools operate on palette indices, never on raw RGB.

Every frame the buffer is converted to RGBA via palette lookup and uploaded to
an SDL streaming texture for display.

**Rationale.** Indexed color is the foundation for flood fill, eyedropper,
palette manipulation, dither patterns, gradient nearest-color matching, and BMP
export — none of which work cleanly on a direct-RGB surface.

### 1.2 Rendering Pipeline

```
User input → Tool logic → PixBuf drawing primitives → PixBuf.Render → SDL texture → Present
```

1. **Input dispatch.** `MainLoop` polls SDL events and routes to `HandleKey`,
   `HandleMouseDown`, `HandleMouseMove`, `HandleMouseUp`, or `WheelZoom`.
2. **Tool application.** Freehand tools call `ApplyFreehand`; shape tools call
   `ApplyShape`. Both push an undo region before modifying pixels.
3. **Buffer render.** `PixBuf.Render(ren, canvas, pb)` converts the indexed
   buffer to RGBA and uploads to the SDL texture `canvas`.
4. **Compositing.** `DrawFrame` blits `canvas` (zoomed or full), then overlays
   UI chrome (toolbar, palette bar, menu bar, status bar, grid, selection
   marching ants, shape preview, polygon preview, cursor, mini-map).
5. **Present.** `Present(ren)` flips the backbuffer.

### 1.3 Module Dependencies

```
DPaint.mod
  ├── Gfx        (init, window, renderer, cursor, timer)
  ├── Canvas     (SetColor, Clear, draw primitives for UI overlay)
  ├── Events     (poll, key/mouse accessors, modifiers)
  ├── Font       (TTF text for UI labels and text tool)
  ├── Texture    (create/destroy/draw SDL textures for canvas)
  ├── PixBuf     (indexed pixel buffer — all artwork operations)
  └── InOut      (console status messages)
```

All graphics go through `m2gfx`, which wraps SDL2 + SDL2_ttf via a single C
bridge file (`gfx_bridge.c`). The `PixBuf` module was added specifically for
DPaint's indexed-color workflow.

### 1.4 M2+ Language Features Used

| Feature | Usage |
|---------|-------|
| `EXCEPTION` / `RAISE` | `InitFailed` exception for SDL init errors |
| `TRY` / `EXCEPT` / `FINALLY` | Main entry wraps `InitGraphics` + `MainLoop`; `FINALLY` calls `Cleanup` |
| `REF` types + `NEW` | `UndoRef` (undo stack), `ZoomRef` (zoom stack) — heap-allocated linked lists |
| `CASE` statement | Tool dispatch in `ApplyShape`, `ToolName`, `DrawToolIcon` |

---

## 2. Window Layout

```
+-----------------------------------------------------------+
| Menu Bar (28px)     DPaint M2+  | shortcuts | Th: ████    |
+------+------------------------------------------------+---+
|      |                                                |   |
| Tool |            Canvas Area                         |   |
| Bar  |         (WW−52) × (WH−28−46−22)               |   |
| 52px |                                                |   |
|      |                                                |   |
|      |                                         [mini] |   |
+------+------------------------------------------------+---+
| Palette Bar (46px)   [ color swatches ]    [FG] [BG]      |
+-----------------------------------------------------------+
| Status Bar (22px)  Tool | X: ██ Y: ██ | ZOOM | FG: BG:   |
+-----------------------------------------------------------+
```

### 2.1 Constants

| Name | Value | Description |
|------|-------|-------------|
| `WW` | 1024 | Window width |
| `WH` | 740 | Window height |
| `TBW` | 52 | Toolbar width (left) |
| `MBARH` | 28 | Menu bar height (top) |
| `PALH` | 46 | Palette bar height (bottom) |
| `STATH` | 22 | Status bar height (bottom) |
| `NCOLORS` | 32 | Active palette size |
| `MAX_THICK` | 24 | Maximum brush/line thickness |
| `MAX_UNDO` | 200 | Undo stack depth limit |
| `NTOOLS` | 20 | Number of tools |

Canvas area = `(WW − TBW) × (WH − MBARH − PALH − STATH)` = 972 × 644.

---

## 3. Palette

32-color default palette initialized in `InitPalette`. Colors are a curated
set covering: black, white, primary/secondary colors, earth tones, pastels,
and grays. The palette is stored inside the `PixBuf` C structure as a 256-entry
RGBA table; only the first 32 entries are set by default.

| Index | Color | RGB |
|-------|-------|-----|
| 0 | Black | (0, 0, 0) |
| 1 | White | (255, 255, 255) |
| 2 | Red | (204, 51, 51) |
| 3 | Cyan | (51, 204, 204) |
| 4 | Purple | (153, 51, 204) |
| 5 | Green | (51, 170, 51) |
| 6 | Blue | (51, 51, 204) |
| 7 | Yellow | (238, 238, 51) |
| 8 | Orange | (238, 153, 51) |
| 9 | Brown | (153, 102, 51) |
| 10 | Light Red | (255, 153, 136) |
| 11 | Dark Gray | (68, 68, 68) |
| 12 | Mid Gray | (119, 119, 119) |
| 13 | Light Green | (136, 255, 136) |
| 14 | Light Blue | (136, 136, 255) |
| 15 | Light Gray | (187, 187, 187) |
| 16–31 | Extended | Earth tones, pastels, muted colors |

**Palette bar interaction.** Left-click a swatch → set foreground color.
Right-click → set background color. The current FG/BG pair is shown as
overlapping squares at the right edge of the palette bar.

---

## 4. Tools

20 tools, accessible via toolbar clicks or keyboard shortcuts.

### 4.1 Tool Index

| ID | Name | Key | Category |
|----|------|-----|----------|
| 0 | Pencil | `1` | Freehand |
| 1 | Brush | `2` | Freehand |
| 2 | Spray | `3` | Freehand |
| 3 | Line | `4` | Shape |
| 4 | Rect | `5` | Shape |
| 5 | Fill Rect | `6` | Shape |
| 6 | Circle | `7` | Shape |
| 7 | Fill Circle | `8` | Shape |
| 8 | Ellipse | `9` | Shape |
| 9 | Gradient | `g` | Shape |
| 10 | Eraser | `e` | Freehand |
| 11 | Flood Fill | `f` | Single-click |
| 12 | Eyedropper | `i` | Single-click |
| 13 | Select | — | Drag |
| 14 | Text | `t` | Single-click |
| 15 | Polygon | `p` | Multi-click |
| 16 | Pattern | — | Shape |
| 17 | Symmetry | — | Modifier |
| 18 | Lighten | — | Freehand |
| 19 | Darken | — | Freehand |

### 4.2 Freehand Tools (continuous stroke)

These tools apply on every `MOUSEMOVE` event while the button is held, drawing
segments between `(lpx, lpy)` and the current canvas position.

**Pencil** (`T_PENCIL`). Draws a thick line segment with width
`lineThick + 1`. The basic drawing tool.

**Brush** (`T_BRUSH`). Draws a thick line segment with width
`lineThick * 2 + 2`. Wider than pencil for expressive strokes.

**Spray** (`T_SPRAY`). On mouse-down, fills a circle of radius
`max(lineThick / 2, 1)`. On mouse-move, scatters `8 + lineThick * 2` random
dots within a `lineThick * 3` radius of the cursor. RNG seeded from
`Ticks()` on first click.

**Eraser** (`T_ERASER`). Draws thick line segments with width
`lineThick * 4 + 6` using the background color index.

**Lighten** (`T_LIGHTEN`). Reads the palette index under the cursor and
decrements it by 1 (moving toward index 0 = black). Draws a filled circle.

**Darken** (`T_DARKEN`). Reads the palette index under the cursor and
increments it by 1 (moving toward higher indices). Draws a filled circle.

All freehand tools push an undo region covering the bounding box of the
segment plus a `thick + 2` padding.

**Symmetry.** When `symmetryX` is active, freehand strokes are mirrored
horizontally around the canvas center. When `symmetryY` is active, strokes are
mirrored vertically. Both can be active simultaneously for 4-way symmetry.

### 4.3 Shape Tools (drag to define)

These tools record `(dx0, dy0)` on mouse-down. A translucent preview is drawn
each frame while dragging. On mouse-up, the shape is committed to the pixel
buffer.

**Line** (`T_LINE`). `PixBuf.ThickLine` from start to end with width
`lineThick`. Supports Shift-constrained 45° angles via `Constrain45`.

**Rect** (`T_RECT`). Outline rectangle. Bounding box = min/max of start/end.

**Fill Rect** (`T_FRECT`). Filled rectangle.

**Circle** (`T_CIRCLE`). Outline circle. Center = midpoint of drag box.
Radius = half the horizontal extent.

**Fill Circle** (`T_FCIRCLE`). Filled circle.

**Ellipse** (`T_ELLIPSE`). Outline ellipse. Center = midpoint. Radii = half
the horizontal and vertical extents.

**Gradient** (`T_GRADIENT`). RGB interpolation between FG and BG palette
entries. Direction is horizontal if width ≥ height, vertical otherwise.
Uses `PixBuf.Gradient` which finds nearest palette matches for each
interpolated color. `ncolors = NCOLORS`.

**Pattern** (`T_PATTERN`). 4×4 Bayer dither fill between FG and BG colors.
Threshold controlled by `lineThick` (0–16 maps to progressively denser
patterns).

### 4.4 Single-Click Tools

**Flood Fill** (`T_FLOOD`). Scanline-based flood fill from click position.
Left-click fills with FG color; right-click fills with BG. Pushes a
full-canvas undo region (conservative).

**Eyedropper** (`T_EYEDROP`). Reads the palette index at the click position.
Left-click → set FG. Right-click → set BG. During drag, continuously
updates FG.

**Text** (`T_TEXT`). Types characters into a 256-byte text buffer via keyboard.
Click on canvas to stamp the current text at that position using
`PixBuf.StampText`. Backspace deletes last character; Enter clears buffer;
Escape exits text mode. Characters accepted: ASCII 32–126.

### 4.5 Polygon Tool

**Polygon** (`T_POLYGON`). Multi-click: each click adds a vertex. A preview
line is drawn from the last vertex to the cursor. When the user clicks within
8 pixels of the first vertex and at least 3 vertices exist, the polygon
closes. Left-click close → filled polygon (`PixBuf.PolyFill`). Right-click
close → outline polygon (`PixBuf.PolyDraw`). Escape cancels the current
polygon. Maximum 256 vertices (C-side static array).

---

## 5. Selection

The **Select** tool (`T_SELECT`) defines a rectangular region by dragging.
While a selection is active:

| Action | Effect |
|--------|--------|
| `Ctrl+H` | Flip selection horizontally |
| `Ctrl+V` | Flip selection vertically |
| `Ctrl+C` | Copy selection to clipboard buffer |
| `v` | Paste clipboard buffer at selection origin |
| `Delete` (key 261) | Fill selection with BG color |
| `Escape` | Deselect |

The selection is visualized with marching ants (alternating black/white dashed
outline, phase toggles every 200ms via `Ticks()`).

The copy buffer (`selBuf`) is a `Region` (opaque saved pixel data). Paste
restores the saved region at the selection origin.

---

## 6. Undo System

Delta-based undo using a singly-linked list of `UndoRec` nodes.

### 6.1 UndoRec Structure

```modula2
UndoRef = REF UndoRec;
UndoRec = RECORD
  region: Region;     (* saved pixel data before modification *)
  rx, ry: INTEGER;    (* top-left position for restore *)
  next: UndoRef;
END;
```

### 6.2 Operations

**PushUndo(x, y, w, h).** Saves the rectangular region of the pixel buffer
before a tool modifies it. Allocates a new `UndoRef` via `NEW` and prepends
to `undoHead`. Increments `undoCount`.

**Undo.** Restores the most recent saved region to the pixel buffer, frees the
saved data, and pops the stack.

**Depth limit.** `MAX_UNDO = 200`. Old entries beyond the limit are not freed
(they leak). This is an acceptable tradeoff for simplicity in a demo app.

### 6.3 Region Sizing

Each tool calculates the minimum bounding box that covers the affected area:

- **Freehand tools:** Bounding box of `(x1,y1)–(x2,y2)` plus `thick + 2`
  padding on all sides.
- **Shape tools:** Bounding box of start/end points plus `thick` padding.
- **Flood fill:** Full canvas (conservative — actual fill area unknown a priori).
- **Selection delete:** Exact selection rectangle.

---

## 7. Zoom & Navigation

### 7.1 Zoom Stack

A linked list of `ZoomRec` nodes stores previous viewport rectangles:

```modula2
ZoomRef = REF ZoomRec;
ZoomRec = RECORD
  x, y, w, h: INTEGER;  (* viewport in canvas coordinates *)
  prev: ZoomRef;
END;
```

`PushZoom` saves the current viewport and sets a new one. `PopZoom` restores
the previous viewport.

### 7.2 Zoom Modes

**Magnify mode** (`m` key). Activates a rubber-band selection. Drag a
rectangle on the canvas; on release, the viewport zooms to that region.
Yellow outline preview while dragging.

**Mouse wheel zoom.** Scrolling up zooms in (viewport shrinks to 75%),
scrolling down zooms out (viewport grows to 133%). The zoom is centered on
the cursor position. Viewport is clamped to canvas bounds. Minimum viewport
size: 16×16.

**Unzoom** (`n` key). Pops one level from the zoom stack.

**Zoom to fit** (`0` key). Resets to full canvas view, clearing the zoom stack.

**Zoom to 1:1** (internal). Centers the view if the canvas is larger than the
display area.

### 7.3 Coordinate Conversion

Two procedures convert between screen coordinates and canvas coordinates:

- `ScreenToCanvas(sx, sy, cx, cy)` — accounts for toolbar offset and zoom.
- `CanvasToScreen(cx, cy, sx, sy)` — inverse mapping for preview rendering.
- `InCanvas(sx, sy)` — tests whether a screen point is within the canvas area.

### 7.4 Grid Overlay

When `showGrid` is `TRUE` and the view is zoomed such that `zoomW < canW/2`,
a 1-pixel grid is drawn over the canvas. Grid lines are semi-transparent
(`alpha = 60`). Lines are only drawn if the spacing is at least 4 pixels (to
avoid visual clutter at low zoom).

### 7.5 Mini-Map

When zoomed, a 130-pixel-wide mini-map is drawn in the top-right corner of
the canvas area. It shows the full canvas at reduced size with a red rectangle
indicating the current viewport. The mini-map has a semi-transparent black
background and a light border.

### 7.6 Panning

**Spacebar pan.** Hold space to enter pan mode. Drag to scroll the viewport.
Movement is proportional: `delta_canvas = delta_screen × zoomW / canW`.
Viewport is clamped to canvas bounds. Release space to exit pan mode.

---

## 8. UI Chrome

### 8.1 Amiga-Style 3D Beveling

All UI elements use a consistent Amiga-inspired color scheme:

| Name | RGB | Purpose |
|------|-----|---------|
| `UIBg` | (40, 50, 70) | Window background / canvas border |
| `UIBar` | (55, 65, 90) | Menu bar, status bar, palette bar fill |
| `UIFace` | (90, 100, 115) | Toolbar button face |
| `UIHi` | (140, 150, 170) | Highlight edge (raised bevel) |
| `UISh` | (30, 35, 50) | Shadow edge (raised bevel) |
| `UISel` | (220, 160, 50) | Selected tool highlight, title accent |
| `UITxt` | (230, 230, 230) | Text labels |

The `Bevel` procedure draws 1px highlight and shadow edges to create a 3D
raised or sunken appearance.

### 8.2 Toolbar

Left-side vertical strip, 52px wide. Contains 20 tool buttons (40×28 each),
vertically stacked with 2px spacing. Each button has:

- A 3D beveled border (raised for inactive, sunken for selected).
- A hand-drawn icon (`DrawToolIcon`) rendered with Canvas drawing primitives.
- The selected tool is tinted with `UISel` at 80 alpha.

Below the tools: a **MAG** indicator (red) when magnify mode is active, and a
**SYM** indicator (green) when symmetry is active (showing X, Y, or XY).

### 8.3 Menu Bar

Top strip, 28px high. Contains:

- "DPaint M2+" title in gold (`UISel` color).
- Keyboard shortcut reference: `M=zoom N=out []=thick Z=undo S=save F=fill I=pick X/Y=sym`.
- "ZOOM" indicator (red) when zoomed.
- Thickness indicator: `Th:` label with a proportional fill bar.

### 8.4 Status Bar

Bottom strip, 22px high. Shows:

- **Tool name** in gold.
- **Coordinate bars**: proportional fill bars for X and Y position (relative
  to canvas dimensions). Shown only when cursor is in the canvas area.
- **ZOOMED** label (red) when zoomed.
- **SEL** label (green) when a selection is active.
- **FG/BG** color swatches (14×14 filled squares).
- **Undo count** bar (max display width = 20px).

### 8.5 Palette Bar

Bottom bar above status bar, 46px high. Displays NCOLORS swatches in a
16-column × 2-row grid. Each swatch is 24×18. The FG color has a white
outline; the BG color has a gray outline. A FG/BG preview with overlapping
squares is shown at the right edge.

---

## 9. Cursor Display

Per-tool custom cursors drawn in `DrawCursor`, overlaid on the canvas:

| Tool | Cursor |
|------|--------|
| Magnify mode | Yellow crosshair with box |
| Eraser | Gray circle (radius = `lineThick * 2 + 3`) |
| Brush | Gray circle (radius = `lineThick + 1`) |
| Spray | Gray circle (radius = `lineThick * 3`) |
| Flood Fill | Cross of thick bars (bucket-like) |
| Eyedropper | White circle with dropper line |
| Text | Vertical bar (I-beam) |
| All others | Crosshair |

All cursors use alpha blending for non-intrusive overlay.

---

## 10. Shape Preview

While dragging a shape tool, a translucent preview is drawn on the canvas
overlay (not committed to the pixel buffer). The preview uses the FG color
at reduced alpha:

- **Line, Rect, Fill Rect, Circle, Fill Circle, Ellipse:** SDL-rendered
  preview in the FG color at alpha 160.
- **Gradient:** Solid FG fill at alpha 100 (actual gradient only on commit).
- **Select:** White outline at alpha 160.
- **Pattern:** FG fill at alpha 80.
- **Magnify box:** Yellow outline at alpha 200.
- **Shift constraint:** When Shift is held during drag, the endpoint is
  snapped to the nearest 45° angle from the start point.

---

## 11. File I/O

### 11.1 BMP Export

`S` key saves the pixel buffer to `dpaint_out.bmp` in the working directory.
Uses `PixBuf.SaveBMP` which writes an 8-bit indexed BMP with:

- 14-byte BMP file header.
- 40-byte BITMAPINFOHEADER.
- 1024-byte color table (256 BGRA entries from the palette).
- Bottom-up pixel rows with 4-byte row padding.

Returns `TRUE` on success. Console message confirms the save.

---

## 12. Keyboard Reference

### 12.1 Tool Selection

| Key | Action |
|-----|--------|
| `1`–`9` | Select tools 0–8 (Pencil through Ellipse) |
| `g` | Gradient tool |
| `e` | Eraser tool |
| `f` | Flood fill tool |
| `i` | Eyedropper tool |
| `t` | Text tool |
| `p` | Polygon tool (resets current polygon) |

### 12.2 View & Navigation

| Key | Action |
|-----|--------|
| `m` | Toggle magnify mode |
| `n` | Unzoom (pop zoom stack) |
| `0` | Zoom to fit (reset) |
| Space (hold) | Pan mode (drag to scroll when zoomed) |
| Mouse wheel | Zoom in/out centered on cursor |

### 12.3 Drawing Controls

| Key | Action |
|-----|--------|
| `[` | Decrease thickness (min 1) |
| `]` | Increase thickness (max 24) |
| `x` | Toggle X symmetry (horizontal mirror) |
| `y` | Toggle Y symmetry (vertical mirror) |
| `d` | Toggle grid overlay |
| `=` | Swap FG and BG colors |
| Shift (hold) | Constrain lines/shapes to 45° angles |

### 12.4 Canvas Operations

| Key | Action |
|-----|--------|
| `z` | Undo |
| `c` | Clear canvas to BG color (resets undo + zoom) |
| `s` | Save BMP to `dpaint_out.bmp` |
| Escape | Cancel polygon / deselect / exit text / quit |

### 12.5 Selection Operations (when selection active)

| Key | Action |
|-----|--------|
| `Ctrl+H` | Flip selection horizontally |
| `Ctrl+V` | Flip selection vertically |
| `Ctrl+C` | Copy selection to buffer |
| `v` | Paste buffer at selection origin |
| `Delete` | Fill selection with BG color |

### 12.6 Text Tool (when text tool active)

| Key | Action |
|-----|--------|
| ASCII 32–126 | Append character to text buffer |
| Backspace | Delete last character |
| Enter | Clear text buffer |
| Click | Stamp text at canvas position |

---

## 13. PixBuf C Backend

The pixel buffer is implemented in C (`gfx_bridge.c`) and exposed through
`GfxBridge.def` (FOR "C" FFI) → `PixBuf.def` / `PixBuf.mod` (M2 wrapper).

### 13.1 Data Structures

```c
typedef struct {
    uint8_t  *pixels;   /* indexed color buffer (w × h bytes) */
    uint32_t *rgba;     /* RGBA conversion buffer (w × h × 4 bytes) */
    int32_t   w, h;
    uint32_t  pal[256]; /* packed RGBA: (R<<24)|(G<<16)|(B<<8)|0xFF */
} PixBuf;

typedef struct {
    uint8_t *data;      /* saved pixel data */
    int32_t  w, h;
} PBRegion;
```

### 13.2 Drawing Algorithms

| Function | Algorithm |
|----------|-----------|
| `gfx_pb_line` | Bresenham's line algorithm |
| `gfx_pb_thick_line` | Bresenham with perpendicular filled circles |
| `gfx_pb_circle` | Midpoint circle algorithm |
| `gfx_pb_fill_circle` | Midpoint circle with horizontal span fill |
| `gfx_pb_ellipse` | Two-region midpoint ellipse |
| `gfx_pb_fill_ellipse` | Two-region midpoint with span fill |
| `gfx_pb_triangle` | Three edge lines (Bresenham) |
| `gfx_pb_fill_triangle` | Scanline with sorted edge interpolation |
| `gfx_pb_flood_fill` | Scanline span-based with explicit stack (no recursion) |
| `gfx_pb_gradient` | RGB linear interpolation + nearest-color (Euclidean distance) |
| `gfx_pb_pattern_fill` | 4×4 Bayer dither matrix, threshold selects FG or BG |
| `gfx_pb_stamp_text` | TTF render to SDL surface → per-pixel stamp to buffer |
| `gfx_pb_poly_fill` | Scanline with edge-table intersection sort |
| `gfx_pb_save_bmp` | 8-bit indexed BMP with BITMAPINFOHEADER |
| `gfx_pb_nearest_color` | Brute-force Euclidean RGB distance over first N entries |

### 13.3 Polygon Vertex Storage

C-side static arrays: `g_poly_xs[256]`, `g_poly_ys[256]`, `g_poly_n`. This
avoids M2-to-C array passing issues. Vertices are added one at a time via
`gfx_pb_poly_add` and read back via `gfx_pb_poly_x/y`.

---

## 14. Initialization & Cleanup

### 14.1 Startup Sequence

```
1. Initialize global variables (tool, colors, flags)
2. Create pixel buffer: PixBuf.Create(canW, canH)
3. Initialize palette: InitPalette
4. Clear to white (index 1)
5. Reset zoom
6. TRY:
   a. Init SDL2 + SDL2_ttf
   b. Create window (1024×740, centered, resizable)
   c. Create renderer (accelerated + vsync)
   d. Create canvas texture
   e. Open fonts (Helvetica fallback chain)
   f. Enter MainLoop
7. EXCEPT InitFailed: print error
8. FINALLY: Cleanup (free all resources in reverse order)
```

### 14.2 Font Fallback Chain

```
/System/Library/Fonts/Helvetica.ttc (13pt) — primary
/System/Library/Fonts/SFNSMono.ttf (13pt) — macOS fallback
/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf (13pt) — Linux fallback
```

Small font (`fontSm`) uses Helvetica at 11pt; falls back to the primary font
if unavailable.

### 14.3 Cleanup Order

```
1. Close small font (if distinct from primary)
2. Close primary font
3. Destroy canvas texture
4. Free pixel buffer
5. Destroy renderer
6. Destroy window
7. QuitFont + Quit (SDL shutdown)
```

---

## 15. Build Configuration

### 15.1 Project Manifest (`m2.toml`)

```toml
name = "dpaint"
version = "0.1.0"
entry = "src/DPaint.mod"
m2plus = true
includes = "src ../../libs/m2gfx/src"

[cc]
cflags = "-I/opt/homebrew/include"
ldflags = "-L/opt/homebrew/lib"
libs = "SDL2 SDL2_ttf"
extra-c = "../../libs/m2gfx/src/gfx_bridge.c"
```

### 15.2 Dependencies

- **SDL2** — window, renderer, events, textures.
- **SDL2_ttf** — TrueType font rendering (UI text + text tool stamp).
- **m2gfx** — M2 wrapper library (Gfx, Canvas, Events, Font, Texture, PixBuf).
- **gfx_bridge.c** — C backend implementing all SDL2 calls + pixel buffer ops.

### 15.3 Build & Run

```sh
cd examples/graphics/dpaint
mx build    # compile
mx run      # compile + execute
```

---

## 16. Implementation Status

### 16.1 Implemented

- [x] Indexed 8-bit pixel buffer (256-color)
- [x] 32-color default palette
- [x] Delta-based undo (linked list, region save/restore)
- [x] 20 tools: Pencil, Brush, Spray, Line, Rect, Fill Rect, Circle,
  Fill Circle, Ellipse, Gradient, Eraser, Flood Fill, Eyedropper, Select,
  Text, Polygon, Pattern, Symmetry, Lighten, Darken
- [x] Shift-constrained 45° angles
- [x] X/Y symmetry drawing
- [x] Mouse wheel zoom (cursor-centered)
- [x] Magnify mode (rubber-band zoom)
- [x] Zoom stack with push/pop
- [x] Spacebar pan
- [x] Grid overlay when zoomed
- [x] Mini-map when zoomed
- [x] Selection with copy/paste/flip/delete
- [x] BMP file save
- [x] Status bar (tool, coords, zoom, selection, FG/BG, undo count)
- [x] Amiga-style 3D beveled UI chrome
- [x] Per-tool cursor shapes
- [x] Shape preview while dragging
- [x] Polygon preview (vertex chain + cursor line)
- [x] Palette bar with FG/BG indicators
- [x] Toolbar with tool icons
- [x] Menu bar with shortcut reference
- [x] TRY/EXCEPT/FINALLY exception handling
- [x] REF + NEW for heap-allocated undo/zoom stacks
- [x] Nearest-color gradient matching
- [x] 4×4 Bayer pattern fill
- [x] Text stamp via TTF font
- [x] Replace color utility (C backend)
- [x] Nearest color utility (C backend)

### 16.2 Not Yet Implemented (Future Roadmap)

#### Core Architecture

- [ ] Separate render layer / CanvasBuffer module
- [ ] Dirty rectangle tracking for partial redraw
- [ ] Layer abstraction (base + overlay layers)
- [ ] Command pattern for undo/redo (bidirectional)
- [ ] Memory pool allocator for undo commands
- [ ] Tool interface abstraction (`ToolRec` with Apply/Preview/Icon)
- [ ] Plugin-style tool registry
- [ ] Serialization layer (save/load native `.dp2` format)

#### Tools

- [ ] Freeform lasso selection
- [ ] Move selected region (drag)
- [ ] Rotate selection (90°, 180°, 270°)
- [ ] Gradient with angle control (arbitrary direction)
- [ ] Airbrush with pressure simulation
- [ ] Smudge tool
- [ ] Replace color tool (global, UI-driven)
- [ ] Bezier curve tool
- [ ] Stamp brush (custom shape from selection)

#### Pixel Precision

- [ ] Pixel snapping mode
- [ ] True pixel-perfect line mode (no double stepping)
- [ ] Subpixel brush smoothing
- [ ] Anti-alias toggle
- [ ] Full dither matrix library (2×2, ordered, etc.)
- [ ] Indexed transparency color
- [ ] Onion-skin preview mode

#### UI

- [ ] Tooltips on hover
- [ ] Keyboard shortcut overlay panel
- [ ] Dockable tool panels
- [ ] Palette editor window (edit RGB per entry)
- [ ] Layer manager panel
- [ ] History panel (visual undo stack)
- [ ] Brush preview widget
- [ ] Dark/light UI theme toggle
- [ ] Animated toolbar hover effects
- [ ] Resizable canvas area (window resize handling)
- [ ] Fullscreen toggle

#### Performance

- [ ] GPU texture streaming (only upload dirty tiles)
- [ ] Tile-based rendering
- [ ] Batch draw calls
- [ ] Frame rate limiter with adaptive delay
- [ ] VSync toggle
- [ ] Hardware scaling vs nearest-neighbor toggle
- [ ] Multi-threaded brush application
- [ ] SIMD-accelerated flood fill

#### File Handling

- [ ] Native `.dp2` format (layers, palette, undo history)
- [ ] PNG export
- [ ] Indexed PNG export
- [ ] GIF export
- [ ] PNG import
- [ ] Palette import/export (`.pal`)
- [ ] Autosave
- [ ] Versioned save history
- [ ] Drag-and-drop file open

#### Advanced Features

- [ ] Animation frames (flipbook)
- [ ] Frame onion skinning
- [ ] Frame timing control
- [ ] Sprite sheet export
- [ ] HAM6/HAM8 simulation mode
- [ ] Amiga 32-color constraint mode
- [ ] Copper-style gradient band tool
- [ ] Tile map editing mode
- [ ] Brush capture from selection
- [ ] Procedural noise brush
- [ ] Scanline CRT overlay mode

#### UX

- [ ] Smooth brush stroke interpolation (Catmull-Rom / bezier)
- [ ] System cursor shapes per tool (SDL_SetCursor)
- [ ] Middle-click color swap
- [ ] Right-click quick color picker popup
- [ ] Double-click tool for options dialog
- [ ] Undo limit setting (configurable)
- [ ] Preferences dialog
- [ ] Config file persistence
- [ ] Recent files menu
- [ ] Keyboard remapping

#### Engineering

- [ ] Split into modules (CanvasBuffer, Tools, UI, History, Zoom)
- [ ] Replace magic constants with config record
- [ ] Central `AppState` record
- [ ] Logging system
- [ ] Error dialog (instead of console output)
- [ ] Assertion framework
- [ ] Unit tests for drawing primitives
- [ ] Document M2+ exception model
- [ ] Debug vs Release build profiles
