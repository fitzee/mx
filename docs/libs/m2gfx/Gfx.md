# Gfx

The core m2gfx module providing SDL2 window management, hardware-accelerated renderer lifecycle, display information, system clipboard access, millisecond-precision timing, and cursor control. `Gfx.Init` must be called successfully before using any other m2gfx module. On high-DPI displays (e.g., macOS Retina), the renderer automatically configures logical-to-physical scaling so all coordinates remain in logical (window) units.

## Types

```modula2
TYPE
  Window   = ADDRESS;   (* opaque SDL_Window handle *)
  Renderer = ADDRESS;   (* opaque SDL_Renderer handle *)
```

## Initialization

### Constants

- `WIN_CENTERED` (1) -- center the window on screen at creation
- `WIN_RESIZABLE` (2) -- allow user to resize the window
- `WIN_BORDERLESS` (4) -- remove window decorations (title bar, border)
- `WIN_FULLSCREEN` (8) -- create in exclusive fullscreen mode
- `WIN_HIDDEN` (16) -- create hidden; use `ShowWindow` to reveal
- `WIN_MAXIMIZED` (32) -- create maximized
- `WIN_HIGHDPI` (64) -- enable high-DPI / Retina support; renderer output may be larger than window size

These flags are combinable with `+` (e.g., `WIN_CENTERED + WIN_RESIZABLE + WIN_HIGHDPI`).

### Init

```modula2
PROCEDURE Init(): BOOLEAN;
```

Initializes SDL2 video and timer subsystems. Returns `TRUE` on success, `FALSE` if SDL2 initialization fails. Must be called before any other `Gfx`, `Canvas`, `Events`, `Font`, or `Texture` procedure.

### InitFont

```modula2
PROCEDURE InitFont(): BOOLEAN;
```

Initializes the SDL2_ttf font subsystem. Returns `TRUE` on success. Must be called before using any procedure in the `Font` module. Requires `Init` to have succeeded first.

### Quit

```modula2
PROCEDURE Quit;
```

Shuts down SDL2 and releases all associated resources, including the cached system cursor. Call after `DestroyRenderer` and `DestroyWindow`. If `InitFont` was called, call `QuitFont` before `Quit`.

### QuitFont

```modula2
PROCEDURE QuitFont;
```

Shuts down the SDL2_ttf font subsystem. Call before `Quit` if `InitFont` was used.

```modula2
(* Typical shutdown sequence *)
QuitFont;
Quit;
```

## Window Management

### CreateWindow

```modula2
PROCEDURE CreateWindow(title: ARRAY OF CHAR;
                       w, h: INTEGER;
                       flags: INTEGER): Window;
```

Creates a new window with the given `title`, logical size `w` x `h` in pixels, and creation `flags`. Returns an opaque `Window` handle, or `NIL` on failure. Unless `WIN_HIDDEN` is set, the window is shown immediately. When `WIN_CENTERED` is not set, the window position is undefined (OS-chosen).

```modula2
win := CreateWindow("My App", 1024, 768,
                    WIN_CENTERED + WIN_RESIZABLE + WIN_HIGHDPI);
```

### DestroyWindow

```modula2
PROCEDURE DestroyWindow(win: Window);
```

Destroys the window and frees its resources. The renderer associated with this window must be destroyed first via `DestroyRenderer`. Safe to call with `NIL`.

### SetTitle

```modula2
PROCEDURE SetTitle(win: Window; title: ARRAY OF CHAR);
```

Changes the title bar text of `win` to `title`. No-op if `win` is `NIL`.

```modula2
SetTitle(win, "Untitled - MyEditor");
```

### SetWindowSize

```modula2
PROCEDURE SetWindowSize(win: Window; w, h: INTEGER);
```

Resizes the window's client area to `w` x `h` logical pixels. Does not affect position. On high-DPI displays, the physical size may differ.

### GetWindowWidth

```modula2
PROCEDURE GetWindowWidth(win: Window): INTEGER;
```

Returns the current logical width of the window in pixels. Returns 0 if `win` is `NIL`.

### GetWindowHeight

```modula2
PROCEDURE GetWindowHeight(win: Window): INTEGER;
```

Returns the current logical height of the window in pixels. Returns 0 if `win` is `NIL`.

```modula2
w := GetWindowWidth(win);
h := GetWindowHeight(win);
```

### SetWindowPos

```modula2
PROCEDURE SetWindowPos(win: Window; x, y: INTEGER);
```

Moves the window so that its top-left corner is at screen coordinates (`x`, `y`).

### SetFullscreen

```modula2
PROCEDURE SetFullscreen(win: Window; mode: INTEGER);
```

Changes the fullscreen state of `win`. Use the `FULLSCREEN_*` constants for `mode`:

- `FULLSCREEN_OFF` (0) -- return to windowed mode
- `FULLSCREEN_TRUE` (1) -- exclusive fullscreen at the window's resolution
- `FULLSCREEN_DESKTOP` (2) -- fullscreen at the desktop's native resolution (recommended)

```modula2
SetFullscreen(win, FULLSCREEN_DESKTOP);
```

### ShowWindow

```modula2
PROCEDURE ShowWindow(win: Window);
```

Makes a hidden window visible. Use after creating with `WIN_HIDDEN`.

### HideWindow

```modula2
PROCEDURE HideWindow(win: Window);
```

Hides the window without destroying it. The window can be shown again with `ShowWindow`.

### RaiseWindow

```modula2
PROCEDURE RaiseWindow(win: Window);
```

Raises the window above other windows and gives it input focus.

### MinimizeWindow

```modula2
PROCEDURE MinimizeWindow(win: Window);
```

Minimizes the window to the taskbar/dock.

### MaximizeWindow

```modula2
PROCEDURE MaximizeWindow(win: Window);
```

Maximizes the window to fill the available screen area. The window must have been created with `WIN_RESIZABLE` for this to have a visible effect on most platforms.

### RestoreWindow

```modula2
PROCEDURE RestoreWindow(win: Window);
```

Restores a minimized or maximized window to its previous size and position.

### GetWindowID

```modula2
PROCEDURE GetWindowID(win: Window): INTEGER;
```

Returns the unique numeric ID assigned to the window by SDL2. This ID appears in window events and can be used to identify which window received an event in multi-window applications. Returns 0 if `win` is `NIL`.

### SetWindowMinSize

```modula2
PROCEDURE SetWindowMinSize(win: Window; w, h: INTEGER);
```

Sets the minimum allowed size for user-initiated resizing. Has no effect unless the window was created with `WIN_RESIZABLE`.

```modula2
SetWindowMinSize(win, 320, 240);
```

### SetWindowMaxSize

```modula2
PROCEDURE SetWindowMaxSize(win: Window; w, h: INTEGER);
```

Sets the maximum allowed size for user-initiated resizing. Has no effect unless the window was created with `WIN_RESIZABLE`.

## Renderer

### Constants

- `RENDER_ACCELERATED` (1) -- use GPU-accelerated rendering (recommended)
- `RENDER_VSYNC` (2) -- synchronize `Present` to the display's refresh rate; prevents tearing
- `RENDER_SOFTWARE` (4) -- force software rendering (fallback)

These flags are combinable with `+` (e.g., `RENDER_ACCELERATED + RENDER_VSYNC`).

### CreateRenderer

```modula2
PROCEDURE CreateRenderer(win: Window; flags: INTEGER): Renderer;
```

Creates a 2D renderer attached to `win` with the specified `flags`. Returns an opaque `Renderer` handle, or `NIL` on failure. On high-DPI windows (`WIN_HIGHDPI`), the renderer automatically sets a logical size matching the window dimensions so all drawing coordinates remain in logical pixels.

```modula2
ren := CreateRenderer(win, RENDER_ACCELERATED + RENDER_VSYNC);
```

### DestroyRenderer

```modula2
PROCEDURE DestroyRenderer(ren: Renderer);
```

Destroys the renderer and frees GPU resources. Must be called before `DestroyWindow`. Safe to call with `NIL`.

### Present

```modula2
PROCEDURE Present(ren: Renderer);
```

Flips the back buffer to the screen, displaying everything drawn since the last `Present` or `Clear`. Call exactly once per frame, after all drawing is complete. If `RENDER_VSYNC` was set, this call blocks until the next vertical blank.

### OutputWidth

```modula2
PROCEDURE OutputWidth(ren: Renderer): INTEGER;
```

Returns the physical output width of the renderer in pixels. On standard displays this equals the window width. On high-DPI displays (with `WIN_HIGHDPI`), this is typically 2x the logical window width. Returns 0 if `ren` is `NIL`.

### OutputHeight

```modula2
PROCEDURE OutputHeight(ren: Renderer): INTEGER;
```

Returns the physical output height of the renderer in pixels. On high-DPI displays, this is typically 2x the logical window height. Returns 0 if `ren` is `NIL`.

```modula2
(* Detect DPI scaling *)
scale := OutputWidth(ren) DIV GetWindowWidth(win);
```

## Screen Info

### ScreenWidth

```modula2
PROCEDURE ScreenWidth(): INTEGER;
```

Returns the width of the primary display in pixels. Queries `SDL_GetDisplayBounds` for display index 0. Returns 0 on failure. Does not require a window to exist.

### ScreenHeight

```modula2
PROCEDURE ScreenHeight(): INTEGER;
```

Returns the height of the primary display in pixels. Returns 0 on failure.

```modula2
(* Center a window manually *)
x := (ScreenWidth() - 800) DIV 2;
y := (ScreenHeight() - 600) DIV 2;
SetWindowPos(win, x, y);
```

### DisplayCount

```modula2
PROCEDURE DisplayCount(): INTEGER;
```

Returns the number of connected displays/monitors. Useful for multi-monitor setups.

## Clipboard

### SetClipboard

```modula2
PROCEDURE SetClipboard(text: ARRAY OF CHAR);
```

Copies `text` to the system clipboard, replacing any existing clipboard content. The string is null-terminated. No-op if `text` is empty.

### GetClipboard

```modula2
PROCEDURE GetClipboard(VAR text: ARRAY OF CHAR);
```

Reads the current system clipboard text into `text`. The result is truncated to fit `HIGH(text) + 1` characters. If the clipboard is empty or contains non-text data, `text` is set to an empty string.

```modula2
VAR buf: ARRAY [0..255] OF CHAR;
GetClipboard(buf);
```

### HasClipboard

```modula2
PROCEDURE HasClipboard(): BOOLEAN;
```

Returns `TRUE` if the system clipboard currently contains text. Use to check before calling `GetClipboard` to avoid reading an empty string.

## Timer

### Ticks

```modula2
PROCEDURE Ticks(): INTEGER;
```

Returns the number of milliseconds elapsed since `Init` was called. Wraps around after approximately 49 days (`MAX(INTEGER)`). Useful for frame timing, animation, and elapsed-time measurement.

```modula2
t0 := Ticks();
(* ... do work ... *)
elapsed := Ticks() - t0;
```

### Delay

```modula2
PROCEDURE Delay(ms: INTEGER);
```

Pauses the current thread for at least `ms` milliseconds. Values less than or equal to 0 are ignored. The actual delay may be slightly longer due to OS scheduling. When using `RENDER_VSYNC`, explicit delays are usually unnecessary since `Present` already synchronizes to the display refresh.

## Cursor

### Constants

- `CURSOR_ARROW` (0) -- default arrow cursor
- `CURSOR_IBEAM` (1) -- text insertion cursor (I-beam)
- `CURSOR_WAIT` (2) -- busy/hourglass cursor
- `CURSOR_CROSSHAIR` (3) -- precision crosshair
- `CURSOR_HAND` (4) -- pointing hand (used for clickable elements)
- `CURSOR_SIZE_NWSE` (5) -- diagonal resize (northwest-southeast)
- `CURSOR_SIZE_NESW` (6) -- diagonal resize (northeast-southwest)
- `CURSOR_SIZE_WE` (7) -- horizontal resize (west-east)
- `CURSOR_SIZE_NS` (8) -- vertical resize (north-south)
- `CURSOR_SIZE_ALL` (9) -- move/pan in all directions
- `CURSOR_NO` (10) -- not-allowed / forbidden

### SetCursor

```modula2
PROCEDURE SetCursor(cursorType: INTEGER);
```

Changes the mouse cursor to one of the `CURSOR_*` system cursor types. The previous cursor is freed automatically to avoid resource leaks. Values outside 0..10 default to `CURSOR_ARROW`.

```modula2
SetCursor(CURSOR_HAND);  (* hovering over a button *)
```

### ShowCursor

```modula2
PROCEDURE ShowCursor(show: BOOLEAN);
```

Shows the mouse cursor when `show` is `TRUE`, hides it when `FALSE`. Useful for full-screen games or custom-drawn cursors.

```modula2
ShowCursor(FALSE);  (* hide for custom cursor rendering *)
```

## Example

```modula2
MODULE GfxDemo;

FROM Gfx IMPORT Init, Quit, CreateWindow, DestroyWindow,
                 CreateRenderer, DestroyRenderer, Present, Delay,
                 Ticks, SetTitle, GetWindowWidth, GetWindowHeight,
                 WIN_CENTERED, WIN_RESIZABLE, WIN_HIGHDPI,
                 RENDER_ACCELERATED, RENDER_VSYNC;
FROM Canvas IMPORT SetColor, Clear, FillRect;
FROM Events IMPORT Poll, QUIT_EVENT;

VAR
  win: ADDRESS;
  ren: ADDRESS;
  evt: INTEGER;
  running: BOOLEAN;
  frames: INTEGER;
  t0, elapsed: INTEGER;
  title: ARRAY [0..63] OF CHAR;

BEGIN
  IF Init() THEN
    win := CreateWindow("Gfx Demo", 800, 600,
                        WIN_CENTERED + WIN_RESIZABLE + WIN_HIGHDPI);
    ren := CreateRenderer(win, RENDER_ACCELERATED + RENDER_VSYNC);
    running := TRUE;
    frames := 0;
    t0 := Ticks();

    WHILE running DO
      evt := Poll();
      WHILE evt # 0 DO
        IF evt = QUIT_EVENT THEN running := FALSE END;
        evt := Poll()
      END;

      SetColor(ren, 30, 30, 50, 255);
      Clear(ren);

      (* Draw a centered rectangle that adapts to window size *)
      SetColor(ren, 80, 140, 220, 255);
      FillRect(ren,
               GetWindowWidth(win) DIV 4,
               GetWindowHeight(win) DIV 4,
               GetWindowWidth(win) DIV 2,
               GetWindowHeight(win) DIV 2);

      Present(ren);
      INC(frames);

      elapsed := Ticks() - t0;
      IF elapsed >= 1000 THEN
        SetTitle(win, "Gfx Demo");
        frames := 0;
        t0 := Ticks()
      END
    END;

    DestroyRenderer(ren);
    DestroyWindow(win);
    Quit
  END
END GfxDemo.
```
