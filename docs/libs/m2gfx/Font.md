# Font

TrueType font loading, styling, rendering, and metric queries via SDL2_ttf. Fonts are loaded from `.ttf` or `.ttc` files at a specified point size and rendered as blended anti-aliased textures. All metric and size values are returned in logical (DPI-scaled) pixels. `Gfx.InitFont` must be called before any procedure in this module.

## Types

```modula2
TYPE
  FontHandle = ADDRESS;
```

Opaque handle to a loaded TrueType font. `NIL` indicates no font or a failed load.

## Constants

### Style Flags

Combinable with `+` to apply multiple styles simultaneously. Pass to `SetStyle` or compare against `GetStyle`.

| Constant | Value | Description |
|---|---|---|
| `STYLE_NORMAL` | 0 | No styling (resets all flags) |
| `STYLE_BOLD` | 1 | Bold weight |
| `STYLE_ITALIC` | 2 | Italic slant |
| `STYLE_UNDERLINE` | 4 | Underline decoration |
| `STYLE_STRIKETHROUGH` | 8 | Strikethrough decoration |

```modula2
SetStyle(fnt, STYLE_BOLD + STYLE_ITALIC);  (* bold-italic *)
```

### Hinting Modes

Control how glyph outlines are fitted to the pixel grid. Pass to `SetHinting`.

| Constant | Value | Description |
|---|---|---|
| `HINT_NORMAL` | 0 | Full hinting (default). Best for body text on LCD screens. |
| `HINT_LIGHT` | 1 | Light hinting. Preserves glyph shapes at the cost of some crispness. |
| `HINT_MONO` | 2 | Monochrome hinting. Snaps aggressively to pixel grid. Best for pixel-art style rendering or small sizes. |
| `HINT_NONE` | 3 | No hinting. Glyphs rendered at exact outline positions. May look blurry at small sizes. |

## Loading

### Open

```modula2
PROCEDURE Open(path: ARRAY OF CHAR; size: INTEGER): FontHandle;
```

Loads a TrueType font from the file at `path` at `size` points. Returns a `FontHandle` on success, or `NIL` if the file cannot be read or is not a valid font. The font is opened at physical DPI-scaled resolution internally, so text appears sharp on Retina/HiDPI displays. `Gfx.InitFont` must have been called before `Open`.

```modula2
fnt := Open("/System/Library/Fonts/Menlo.ttc", 16);
IF fnt = NIL THEN (* handle error *) END;
```

### Close

```modula2
PROCEDURE Close(font: FontHandle);
```

Frees all resources associated with `font`. After calling `Close`, the handle must not be used. Always close fonts before calling `Gfx.QuitFont`.

## Style

### SetStyle

```modula2
PROCEDURE SetStyle(font: FontHandle; style: INTEGER);
```

Sets the rendering style for `font`. Pass `STYLE_NORMAL` (0) to clear all styles, or combine flags with `+`. The style affects all subsequent `DrawText`, `DrawTextWrapped`, `TextWidth`, and `TextHeight` calls using this font. Has no effect if `font` is `NIL`.

```modula2
SetStyle(fnt, STYLE_BOLD + STYLE_UNDERLINE);
```

### GetStyle

```modula2
PROCEDURE GetStyle(font: FontHandle): INTEGER;
```

Returns the current style bitmask of `font`. Test individual flags using integer division: `GetStyle(fnt) DIV STYLE_BOLD MOD 2 = 1` tests for bold. Returns 0 if `font` is `NIL`.

### SetHinting

```modula2
PROCEDURE SetHinting(font: FontHandle; hint: INTEGER);
```

Sets the hinting mode for `font` using one of the `HINT_*` constants. Affects glyph rendering quality and alignment. `HINT_MONO` is recommended for bitmap-style or small pixel rendering. `HINT_NORMAL` is the default. Has no effect if `font` is `NIL`.

```modula2
SetHinting(fnt, HINT_MONO);  (* crisp pixel-aligned glyphs *)
```

## Rendering

### DrawText

```modula2
PROCEDURE DrawText(ren: Renderer; font: FontHandle;
                   text: ARRAY OF CHAR; x, y: INTEGER;
                   r, g, b, a: INTEGER);
```

Renders `text` at position (`x`, `y`) on the renderer `ren` using `font`. The color is specified by `r`, `g`, `b`, `a` (each 0..255). Text is rendered using blended anti-aliasing (smooth edges). The position specifies the top-left corner of the rendered text. Empty strings and `NIL` font/renderer are silently ignored.

```modula2
DrawText(ren, fnt, "Score: 42", 10, 10, 255, 255, 255, 255);
```

### DrawTextWrapped

```modula2
PROCEDURE DrawTextWrapped(ren: Renderer; font: FontHandle;
                          text: ARRAY OF CHAR;
                          x, y, wrapWidth: INTEGER;
                          r, g, b, a: INTEGER);
```

Renders `text` at position (`x`, `y`) with automatic word-wrapping at `wrapWidth` pixels. Lines that exceed `wrapWidth` are broken at word boundaries. Otherwise behaves identically to `DrawText`. `wrapWidth` is in logical pixels.

```modula2
DrawTextWrapped(ren, fnt,
  "This is a long paragraph that wraps automatically.",
  20, 50, 300, 200, 200, 200, 255);
```

## Metrics

### TextWidth

```modula2
PROCEDURE TextWidth(font: FontHandle; text: ARRAY OF CHAR): INTEGER;
```

Returns the width in logical pixels that `text` would occupy if rendered with `font` at its current style. Returns 0 if `font` is `NIL` or `text` is empty. Does not actually draw anything.

```modula2
w := TextWidth(fnt, "Hello");
x := (screenW - w) DIV 2;  (* center text horizontally *)
```

### TextHeight

```modula2
PROCEDURE TextHeight(font: FontHandle; text: ARRAY OF CHAR): INTEGER;
```

Returns the height in logical pixels that `text` would occupy if rendered with `font`. For single-line text this equals `Height`, but the value accounts for the actual rendered surface size. Returns 0 if `font` is `NIL`.

### Height

```modula2
PROCEDURE Height(font: FontHandle): INTEGER;
```

Returns the maximum pixel height of the font face (ascent + descent), independent of any specific text string. Use for computing line heights and layout. Returns 0 if `font` is `NIL`.

### Ascent

```modula2
PROCEDURE Ascent(font: FontHandle): INTEGER;
```

Returns the maximum ascent (pixels above the baseline) of the font. Always positive. Returns 0 if `font` is `NIL`.

### Descent

```modula2
PROCEDURE Descent(font: FontHandle): INTEGER;
```

Returns the maximum descent (pixels below the baseline) of the font. Typically negative or zero. Returns 0 if `font` is `NIL`.

### LineSkip

```modula2
PROCEDURE LineSkip(font: FontHandle): INTEGER;
```

Returns the recommended line spacing in pixels -- the distance to advance vertically between baselines of consecutive lines. Typically slightly larger than `Height` to provide inter-line padding. Returns 0 if `font` is `NIL`.

```modula2
y := startY;
FOR i := 0 TO lineCount - 1 DO
  DrawText(ren, fnt, lines[i], x, y, 255, 255, 255, 255);
  y := y + LineSkip(fnt);
END;
```

## Example

```modula2
MODULE FontDemo;
FROM Gfx IMPORT Init, InitFont, Quit, QuitFont,
                 CreateWindow, DestroyWindow,
                 CreateRenderer, DestroyRenderer, Present, Delay,
                 WIN_CENTERED, RENDER_ACCELERATED, RENDER_VSYNC;
FROM Canvas IMPORT SetColor, Clear;
FROM Font IMPORT FontHandle, Open, Close, SetStyle, SetHinting,
                  DrawText, DrawTextWrapped,
                  TextWidth, Height, LineSkip, Ascent, Descent,
                  STYLE_BOLD, STYLE_ITALIC, HINT_NORMAL;
FROM Events IMPORT Poll, QUIT_EVENT;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;
VAR
  win, ren: ADDRESS;
  fnt, fntSmall: FontHandle;
  evt, y, w: INTEGER;
  running: BOOLEAN;
BEGIN
  IF Init() & InitFont() THEN
    win := CreateWindow("Font Demo", 640, 480, WIN_CENTERED);
    ren := CreateRenderer(win, RENDER_ACCELERATED + RENDER_VSYNC);

    fnt := Open("/System/Library/Fonts/Menlo.ttc", 24);
    fntSmall := Open("/System/Library/Fonts/Menlo.ttc", 14);

    WriteString("Height="); WriteInt(Height(fnt), 0);
    WriteString(" Ascent="); WriteInt(Ascent(fnt), 0);
    WriteString(" Descent="); WriteInt(Descent(fnt), 0);
    WriteString(" LineSkip="); WriteInt(LineSkip(fnt), 0);
    WriteLn;

    running := TRUE;
    WHILE running DO
      evt := Poll();
      WHILE evt # 0 DO
        IF evt = QUIT_EVENT THEN running := FALSE END;
        evt := Poll();
      END;

      SetColor(ren, 20, 20, 40, 255);
      Clear(ren);

      y := 30;
      SetStyle(fnt, STYLE_BOLD);
      DrawText(ren, fnt, "Bold Title", 30, y, 255, 220, 100, 255);
      y := y + LineSkip(fnt);

      SetStyle(fnt, STYLE_ITALIC);
      DrawText(ren, fnt, "Italic Subtitle", 30, y, 180, 180, 220, 255);
      y := y + LineSkip(fnt) + 10;

      SetStyle(fntSmall, 0);
      DrawTextWrapped(ren, fntSmall,
        "This paragraph demonstrates word-wrapped text rendering. "
        + "Lines break automatically at the specified width.",
        30, y, 400, 200, 200, 200, 255);

      (* Center a label *)
      w := TextWidth(fnt, "Centered");
      SetStyle(fnt, 0);
      DrawText(ren, fnt, "Centered", (640 - w) DIV 2, 400,
               100, 255, 100, 255);

      Present(ren);
      Delay(16);
    END;

    Close(fntSmall);
    Close(fnt);
    DestroyRenderer(ren);
    DestroyWindow(win);
    QuitFont;
    Quit;
  END;
END FontDemo.
```
