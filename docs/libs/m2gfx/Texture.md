# Texture

Hardware-accelerated texture management for GPU-resident images. Textures are created from BMP files, rendered text, or blank RGBA surfaces. They can be drawn to the screen with optional scaling, rotation, and flipping. Blank textures created with `Create` can serve as off-screen render targets via `SetTarget`. All textures must be destroyed before their parent renderer.

## Types

```modula2
TYPE
  Tex = ADDRESS;
```

Opaque handle to a GPU texture. `NIL` indicates no texture or a failed creation/load.

## Constants

### Flip Modes

Values for the `flip` parameter of `DrawRotated`. Combinable: `FLIP_BOTH` = `FLIP_HORIZONTAL` + `FLIP_VERTICAL`.

| Constant | Value | Description |
|---|---|---|
| `FLIP_NONE` | 0 | No flipping |
| `FLIP_HORIZONTAL` | 1 | Mirror along the vertical axis (left-right) |
| `FLIP_VERTICAL` | 2 | Mirror along the horizontal axis (top-bottom) |
| `FLIP_BOTH` | 3 | Mirror both axes (equivalent to 180-degree rotation) |

## Creation & Loading

### LoadBMP

```modula2
PROCEDURE LoadBMP(ren: Renderer; path: ARRAY OF CHAR): Tex;
```

Loads a BMP image from `path` and uploads it to the GPU as a texture associated with renderer `ren`. Returns the texture handle on success, or `NIL` if the file cannot be read or is not a valid BMP. The texture dimensions match the image dimensions. The caller must eventually call `Destroy` to free the texture.

```modula2
sprite := LoadBMP(ren, "assets/player.bmp");
IF sprite = NIL THEN (* handle missing asset *) END;
```

### Create

```modula2
PROCEDURE Create(ren: Renderer; w, h: INTEGER): Tex;
```

Allocates a blank RGBA texture of size `w` by `h` pixels on the GPU. The texture is created with `SDL_TEXTUREACCESS_TARGET`, making it usable as a render target via `SetTarget`. Initial contents are undefined. Returns `NIL` if allocation fails. The caller must eventually call `Destroy` to free the texture.

```modula2
offscreen := Create(ren, 256, 256);
SetTarget(ren, offscreen);
(* draw to offscreen texture *)
ResetTarget(ren);
```

### FromText

```modula2
PROCEDURE FromText(ren: Renderer; font: FontHandle;
                   text: ARRAY OF CHAR;
                   r, g, b, a: INTEGER): Tex;
```

Renders `text` using `font` with color (`r`, `g`, `b`, `a`) and uploads the result as a GPU texture. Returns `NIL` if `font` is `NIL`, `text` is empty, or texture creation fails. Unlike `Font.DrawText`, the resulting texture can be drawn repeatedly without re-rendering the text each frame. Requires `Gfx.InitFont` to have been called. Color components are 0..255.

```modula2
label := FromText(ren, fnt, "Game Over", 255, 0, 0, 255);
(* draw label many times per frame without re-rendering text *)
Draw(ren, label, 200, 300);
```

### Destroy

```modula2
PROCEDURE Destroy(tex: Tex);
```

Frees the GPU resources associated with `tex`. After calling `Destroy`, the handle must not be used. Passing `NIL` is safe and has no effect. Destroy all textures before destroying the renderer that owns them.

## Drawing

### Draw

```modula2
PROCEDURE Draw(ren: Renderer; tex: Tex; x, y: INTEGER);
```

Draws `tex` at position (`x`, `y`) on renderer `ren` at its native size. The position specifies the top-left corner. The texture is drawn at its full width and height with no scaling. Has no effect if `ren` or `tex` is `NIL`.

```modula2
Draw(ren, sprite, playerX, playerY);
```

### DrawRegion

```modula2
PROCEDURE DrawRegion(ren: Renderer; tex: Tex;
                     sx, sy, sw, sh: INTEGER;
                     dx, dy, dw, dh: INTEGER);
```

Draws a rectangular sub-region of `tex` onto the renderer. The source rectangle (`sx`, `sy`, `sw`, `sh`) selects pixels within the texture; the destination rectangle (`dx`, `dy`, `dw`, `dh`) specifies where and at what size to draw on screen. If the source and destination sizes differ, the region is scaled. Use for sprite sheets, tile maps, or any case where only part of a texture should be displayed.

```modula2
(* Draw the 3rd 32x32 tile from a horizontal sprite sheet *)
DrawRegion(ren, tileset,
           64, 0, 32, 32,     (* source: 3rd tile *)
           screenX, screenY, 32, 32);  (* dest: unscaled *)
```

### DrawRotated

```modula2
PROCEDURE DrawRotated(ren: Renderer; tex: Tex;
                      dx, dy, dw, dh: INTEGER;
                      angleDeg: INTEGER; flip: INTEGER);
```

Draws `tex` into the destination rectangle (`dx`, `dy`, `dw`, `dh`) rotated by `angleDeg` degrees clockwise around its center. The `flip` parameter applies mirroring using the `FLIP_*` constants. The entire texture is drawn (no source sub-region). Rotation and flip are applied after scaling to the destination rectangle.

```modula2
(* Rotate a compass needle 45 degrees *)
DrawRotated(ren, needle, 280, 200, 80, 80, 45, FLIP_NONE);

(* Draw a mirrored enemy facing left *)
DrawRotated(ren, enemyTex, ex, ey, 64, 64, 0, FLIP_HORIZONTAL);
```

## Properties

### Width

```modula2
PROCEDURE Width(tex: Tex): INTEGER;
```

Returns the width of `tex` in pixels. Returns 0 if `tex` is `NIL`.

### Height

```modula2
PROCEDURE Height(tex: Tex): INTEGER;
```

Returns the height of `tex` in pixels. Returns 0 if `tex` is `NIL`.

```modula2
w := Width(sprite);
h := Height(sprite);
Draw(ren, sprite, (640 - w) DIV 2, (480 - h) DIV 2);  (* center *)
```

## Blending & Color Mod

### SetAlpha

```modula2
PROCEDURE SetAlpha(tex: Tex; alpha: INTEGER);
```

Sets the alpha (opacity) modulation for `tex`. Range is 0 (fully transparent) to 255 (fully opaque). Requires the texture's blend mode to be set to alpha blending (see `SetBlendMode`) for the alpha to have visible effect. Has no effect if `tex` is `NIL`.

```modula2
SetBlendMode(fadeTexture, 1);  (* BLEND_ALPHA *)
SetAlpha(fadeTexture, 128);    (* 50% transparent *)
```

### SetBlendMode

```modula2
PROCEDURE SetBlendMode(tex: Tex; mode: INTEGER);
```

Sets the blend mode used when drawing `tex`. Blend mode values:

| Value | Mode | Description |
|---|---|---|
| 0 | None | No blending, overwrites destination |
| 1 | Alpha | Standard alpha blending: `src * srcA + dst * (1 - srcA)` |
| 2 | Additive | Adds source to destination: `src * srcA + dst` |
| 4 | Modulate | Multiplies source and destination: `src * dst` |

Has no effect if `tex` is `NIL`. Default for newly created textures is no blending.

```modula2
SetBlendMode(glow, 2);  (* additive blending for glow effects *)
```

### SetColorMod

```modula2
PROCEDURE SetColorMod(tex: Tex; r, g, b: INTEGER);
```

Multiplies each pixel's color channels by (`r`, `g`, `b`) / 255 when the texture is drawn. Each component is 0..255. Set to (255, 255, 255) for no modification. Use for tinting: (255, 0, 0) renders the texture in shades of red. Has no effect if `tex` is `NIL`.

```modula2
SetColorMod(sprite, 255, 100, 100);  (* red tint for damage flash *)
Draw(ren, sprite, px, py);
SetColorMod(sprite, 255, 255, 255);  (* restore original colors *)
```

## Render Targets

### SetTarget

```modula2
PROCEDURE SetTarget(ren: Renderer; tex: Tex);
```

Redirects all subsequent drawing operations on `ren` to render into `tex` instead of the screen. The texture must have been created with `Create` (which uses `SDL_TEXTUREACCESS_TARGET`). While a render target is active, `Canvas.Clear`, `Canvas.FillRect`, `Font.DrawText`, and all other draw calls write to the texture. Call `ResetTarget` to resume drawing to the screen. Has no effect if `ren` is `NIL`.

```modula2
SetTarget(ren, offscreen);
SetColor(ren, 0, 0, 0, 255);
Clear(ren);
FillCircle(ren, 128, 128, 50);
ResetTarget(ren);
(* now draw the composited offscreen texture to screen *)
Draw(ren, offscreen, 0, 0);
```

### ResetTarget

```modula2
PROCEDURE ResetTarget(ren: Renderer);
```

Restores drawing on `ren` to the default target (the screen / back buffer). Call after finishing off-screen rendering with `SetTarget`. Has no effect if `ren` is `NIL` or no render target was set.

## Example

```modula2
MODULE TextureDemo;
FROM Gfx IMPORT Init, InitFont, Quit, QuitFont,
                 CreateWindow, DestroyWindow,
                 CreateRenderer, DestroyRenderer, Present, Delay,
                 WIN_CENTERED, RENDER_ACCELERATED, RENDER_VSYNC;
FROM Canvas IMPORT SetColor, Clear, FillRect, FillCircle;
FROM Font IMPORT FontHandle, Open, Close;
FROM Texture IMPORT Tex, LoadBMP, Create, FromText, Destroy,
                     Draw, DrawRegion, DrawRotated, SetTarget,
                     ResetTarget, SetAlpha, SetBlendMode,
                     SetColorMod, Width, Height,
                     FLIP_NONE, FLIP_HORIZONTAL;
FROM Events IMPORT Poll, QUIT_EVENT;
VAR
  win, ren: ADDRESS;
  fnt: FontHandle;
  bg, label, canvas: Tex;
  evt, angle, w, h: INTEGER;
  running: BOOLEAN;
BEGIN
  IF Init() & InitFont() THEN
    win := CreateWindow("Texture Demo", 640, 480, WIN_CENTERED);
    ren := CreateRenderer(win, RENDER_ACCELERATED + RENDER_VSYNC);
    fnt := Open("/System/Library/Fonts/Menlo.ttc", 20);

    (* Load a BMP sprite *)
    bg := LoadBMP(ren, "assets/background.bmp");

    (* Pre-render text into a reusable texture *)
    label := FromText(ren, fnt, "Hello Textures!", 255, 255, 0, 255);

    (* Create an off-screen render target *)
    canvas := Create(ren, 200, 200);
    SetTarget(ren, canvas);
    SetColor(ren, 40, 40, 80, 255);
    Clear(ren);
    FillCircle(ren, 100, 100, 60);
    ResetTarget(ren);

    angle := 0;
    running := TRUE;
    WHILE running DO
      evt := Poll();
      WHILE evt # 0 DO
        IF evt = QUIT_EVENT THEN running := FALSE END;
        evt := Poll();
      END;

      SetColor(ren, 0, 0, 0, 255);
      Clear(ren);

      (* Draw background stretched to window *)
      IF bg # NIL THEN
        DrawRegion(ren, bg, 0, 0, Width(bg), Height(bg),
                   0, 0, 640, 480);
      END;

      (* Draw rotating off-screen canvas *)
      angle := (angle + 1) MOD 360;
      DrawRotated(ren, canvas, 220, 140, 200, 200,
                  angle, FLIP_NONE);

      (* Draw text label with alpha fade *)
      IF label # NIL THEN
        SetBlendMode(label, 1);
        SetAlpha(label, 200);
        w := Width(label);
        Draw(ren, label, (640 - w) DIV 2, 420);
      END;

      Present(ren);
      Delay(16);
    END;

    Destroy(canvas);
    Destroy(label);
    IF bg # NIL THEN Destroy(bg) END;
    Close(fnt);
    DestroyRenderer(ren);
    DestroyWindow(win);
    QuitFont;
    Quit;
  END;
END TextureDemo.
```
