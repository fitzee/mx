# Events

Event polling and input state for keyboard, mouse, window, and text input. Events are retrieved from a queue via `Poll` or `Wait`, then inspected with accessor procedures that read fields from the most recently polled event. Direct state queries (`IsKeyPressed`, `GetMouseState`) bypass the event queue entirely and sample live hardware state.

## Event Types

Constants returned by `Poll`, `Wait`, and `WaitTimeout`. Each value identifies the kind of event that was dequeued.

| Constant | Value | Description |
|---|---|---|
| `NONE` | 0 | Queue empty, no event available |
| `QUIT_EVENT` | 1 | Window close or system quit request |
| `KEYDOWN` | 2 | Key pressed |
| `KEYUP` | 3 | Key released |
| `MOUSEDOWN` | 4 | Mouse button pressed |
| `MOUSEUP` | 5 | Mouse button released |
| `MOUSEMOVE` | 6 | Mouse cursor moved |
| `MOUSEWHEEL` | 7 | Mouse wheel scrolled |
| `TEXTINPUT` | 8 | Text input character(s) entered |
| `WINDOW_EVENT` | 9 | Window state changed (see subtypes below) |
| `TEXTEDITING` | 10 | IME composition in progress |

## Window Event Subtypes

When `Poll` returns `WINDOW_EVENT`, call `WindowEvent()` to get the specific subtype.

| Constant | Value | Description |
|---|---|---|
| `WEVT_SHOWN` | 1 | Window became visible |
| `WEVT_HIDDEN` | 2 | Window became hidden |
| `WEVT_EXPOSED` | 3 | Window needs redraw (uncovered by another window) |
| `WEVT_MOVED` | 4 | Window was moved |
| `WEVT_RESIZED` | 5 | Window was resized |
| `WEVT_MINIMIZED` | 6 | Window was minimized |
| `WEVT_MAXIMIZED` | 7 | Window was maximized |
| `WEVT_RESTORED` | 8 | Window restored from minimized/maximized |
| `WEVT_ENTER` | 9 | Mouse cursor entered the window |
| `WEVT_LEAVE` | 10 | Mouse cursor left the window |
| `WEVT_FOCUS_GAINED` | 11 | Window gained keyboard focus |
| `WEVT_FOCUS_LOST` | 12 | Window lost keyboard focus |
| `WEVT_CLOSE` | 13 | Window close button pressed |

## Mouse Buttons

Values returned by `MouseButton()` and used to interpret the bitmask from `GetMouseState`.

| Constant | Value | Description |
|---|---|---|
| `BUTTON_LEFT` | 1 | Left mouse button |
| `BUTTON_MIDDLE` | 2 | Middle mouse button (wheel click) |
| `BUTTON_RIGHT` | 3 | Right mouse button |

## Modifier Flags

Bitmask values returned by `KeyMod()`. Combine with `+` to test multiple modifiers.

| Constant | Value | Description |
|---|---|---|
| `MOD_SHIFT` | 1 | Either Shift key |
| `MOD_CTRL` | 2 | Either Control key |
| `MOD_ALT` | 4 | Either Alt/Option key |
| `MOD_GUI` | 8 | Either GUI key (Cmd on macOS, Win on Windows) |

Test with bitwise AND via integer arithmetic: `IF KeyMod() DIV 2 MOD 2 = 1 THEN (* Ctrl held *) END`.

## Key Codes

Layout-dependent virtual key codes returned by `KeyCode()`. ASCII-range keys (letters, digits, punctuation) use their ASCII value directly: `a` = 97, `A` = 65, `0` = 48, etc. Non-ASCII keys use the values below.

**Editing keys:**

| Constant | Value |
|---|---|
| `KEY_BACKSPACE` | 8 |
| `KEY_TAB` | 9 |
| `KEY_RETURN` | 13 |
| `KEY_ESCAPE` | 27 |
| `KEY_SPACE` | 32 |
| `KEY_DELETE` | 127 |

**Arrow keys:**

| Constant | Value |
|---|---|
| `KEY_UP` | 256 |
| `KEY_DOWN` | 257 |
| `KEY_LEFT` | 258 |
| `KEY_RIGHT` | 259 |

**Navigation keys:**

| Constant | Value |
|---|---|
| `KEY_INSERT` | 260 |
| `KEY_HOME` | 262 |
| `KEY_END` | 263 |
| `KEY_PAGEUP` | 264 |
| `KEY_PAGEDOWN` | 265 |

**Function keys:**

| Constant | Value | | Constant | Value |
|---|---|---|---|---|
| `KEY_F1` | 282 | | `KEY_F7` | 288 |
| `KEY_F2` | 283 | | `KEY_F8` | 289 |
| `KEY_F3` | 284 | | `KEY_F9` | 290 |
| `KEY_F4` | 285 | | `KEY_F10` | 291 |
| `KEY_F5` | 286 | | `KEY_F11` | 292 |
| `KEY_F6` | 287 | | `KEY_F12` | 293 |

**Lock keys:**

| Constant | Value |
|---|---|
| `KEY_NUMLOCK` | 300 |
| `KEY_CAPSLOCK` | 301 |
| `KEY_SCROLLLOCK` | 302 |

**Modifier keys (as key codes, for detecting individual key press/release):**

| Constant | Value | | Constant | Value |
|---|---|---|---|---|
| `KEY_LSHIFT` | 304 | | `KEY_RSHIFT` | 305 |
| `KEY_LCTRL` | 306 | | `KEY_RCTRL` | 307 |
| `KEY_LALT` | 308 | | `KEY_RALT` | 309 |
| `KEY_LGUI` | 310 | | `KEY_RGUI` | 311 |

**Keypad:**

| Constant | Value |
|---|---|
| `KEY_KP_ENTER` | 271 |
| `KEY_KP_0` .. `KEY_KP_9` | 320 .. 329 |

## Scancodes

Layout-independent physical key position codes for use with `IsKeyPressed`. A QWERTY "W" and an AZERTY "Z" both produce `SCAN_W` (26) because they occupy the same physical position. Use scancodes for movement keys (WASD) that should work regardless of keyboard layout.

**Letters:**

| Constant | Value | | Constant | Value | | Constant | Value |
|---|---|---|---|---|---|---|---|
| `SCAN_A` | 4 | | `SCAN_J` | 13 | | `SCAN_S` | 22 |
| `SCAN_B` | 5 | | `SCAN_K` | 14 | | `SCAN_T` | 23 |
| `SCAN_C` | 6 | | `SCAN_L` | 15 | | `SCAN_U` | 24 |
| `SCAN_D` | 7 | | `SCAN_M` | 16 | | `SCAN_V` | 25 |
| `SCAN_E` | 8 | | `SCAN_N` | 17 | | `SCAN_W` | 26 |
| `SCAN_F` | 9 | | `SCAN_O` | 18 | | `SCAN_X` | 27 |
| `SCAN_G` | 10 | | `SCAN_P` | 19 | | `SCAN_Y` | 28 |
| `SCAN_H` | 11 | | `SCAN_Q` | 20 | | `SCAN_Z` | 29 |
| `SCAN_I` | 12 | | `SCAN_R` | 21 | | | |

**Digits:**

| Constant | Value | | Constant | Value |
|---|---|---|---|---|
| `SCAN_1` | 30 | | `SCAN_6` | 35 |
| `SCAN_2` | 31 | | `SCAN_7` | 36 |
| `SCAN_3` | 32 | | `SCAN_8` | 37 |
| `SCAN_4` | 33 | | `SCAN_9` | 38 |
| `SCAN_5` | 34 | | `SCAN_0` | 39 |

**Other:**

| Constant | Value | | Constant | Value |
|---|---|---|---|---|
| `SCAN_RETURN` | 40 | | `SCAN_TAB` | 43 |
| `SCAN_ESCAPE` | 41 | | `SCAN_SPACE` | 44 |
| `SCAN_BACKSPACE` | 42 | | | |
| `SCAN_RIGHT` | 79 | | `SCAN_LEFT` | 80 |
| `SCAN_DOWN` | 81 | | `SCAN_UP` | 82 |

## Event Polling

### Poll

```modula2
PROCEDURE Poll(): INTEGER;
```

Dequeues and returns the next event type from the event queue, or `NONE` (0) when the queue is empty. Call in a loop to drain all pending events each frame. After `Poll` returns a non-zero value, use the event accessor procedures to read the event's details.

```modula2
evt := Poll();
WHILE evt # NONE DO
  (* handle evt *)
  evt := Poll();
END;
```

### Wait

```modula2
PROCEDURE Wait(): INTEGER;
```

Blocks until an event is available, then dequeues and returns its type. Returns 0 only on error. Use for applications that do not need continuous rendering (e.g., a text editor) to avoid busy-waiting and reduce CPU usage.

### WaitTimeout

```modula2
PROCEDURE WaitTimeout(ms: INTEGER): INTEGER;
```

Blocks until an event is available or `ms` milliseconds elapse, whichever comes first. Returns the event type, or `NONE` if the timeout expired with no events. Useful for cursor blink or idle animations that need periodic updates even without input.

```modula2
evt := WaitTimeout(500);  (* wake every 500ms to blink cursor *)
```

## Event Accessors

These procedures read fields from the most recently polled/waited event. Only call them after `Poll` or `Wait` returns a non-zero value. Calling an accessor that does not match the current event type returns 0 or an empty string.

### KeyCode

```modula2
PROCEDURE KeyCode(): INTEGER;
```

Returns the layout-dependent virtual key code of the most recent `KEYDOWN` or `KEYUP` event. For ASCII keys, this is the ASCII value (e.g., 97 for `a`). For non-ASCII keys, returns the corresponding `KEY_*` constant. Layout-dependent means a French AZERTY keyboard's "A" key produces 113 (`q`), not 97 (`a`).

```modula2
IF KeyCode() = KEY_ESCAPE THEN running := FALSE END;
```

### ScanCode

```modula2
PROCEDURE ScanCode(): INTEGER;
```

Returns the layout-independent physical scancode of the most recent `KEYDOWN` or `KEYUP` event. The scancode identifies the physical key position regardless of keyboard layout. Use `SCAN_*` constants to compare. Prefer scancodes over key codes for WASD-style movement bindings.

### KeyMod

```modula2
PROCEDURE KeyMod(): INTEGER;
```

Returns the modifier key bitmask active during the most recent `KEYDOWN` or `KEYUP` event. Test individual modifiers using integer division and modulo: `KeyMod() DIV MOD_CTRL MOD 2 = 1` tests for Ctrl. Multiple modifiers are combined: Ctrl+Shift yields `MOD_CTRL + MOD_SHIFT` = 3.

```modula2
IF (KeyMod() DIV MOD_CTRL MOD 2 = 1) & (KeyCode() = ORD("s")) THEN
  Save();
END;
```

### MouseX

```modula2
PROCEDURE MouseX(): INTEGER;
```

Returns the x coordinate (in window-relative pixels) of the most recent `MOUSEDOWN`, `MOUSEUP`, or `MOUSEMOVE` event. Returns 0 for other event types.

### MouseY

```modula2
PROCEDURE MouseY(): INTEGER;
```

Returns the y coordinate (in window-relative pixels) of the most recent `MOUSEDOWN`, `MOUSEUP`, or `MOUSEMOVE` event. Returns 0 for other event types.

### MouseButton

```modula2
PROCEDURE MouseButton(): INTEGER;
```

Returns the button number of the most recent `MOUSEDOWN` or `MOUSEUP` event. Compare against `BUTTON_LEFT` (1), `BUTTON_MIDDLE` (2), or `BUTTON_RIGHT` (3).

```modula2
IF (evt = MOUSEDOWN) & (MouseButton() = BUTTON_LEFT) THEN
  StartDrag(MouseX(), MouseY());
END;
```

### WheelX

```modula2
PROCEDURE WheelX(): INTEGER;
```

Returns the horizontal scroll amount of the most recent `MOUSEWHEEL` event. Positive values scroll right, negative values scroll left. Most mice only produce vertical scroll events.

### WheelY

```modula2
PROCEDURE WheelY(): INTEGER;
```

Returns the vertical scroll amount of the most recent `MOUSEWHEEL` event. Positive values scroll up (away from user), negative values scroll down (toward user).

```modula2
IF evt = MOUSEWHEEL THEN
  scrollOffset := scrollOffset - WheelY() * 20;
END;
```

### WindowID

```modula2
PROCEDURE WindowID(): INTEGER;
```

Returns the window ID associated with the most recent event. In multi-window applications, use this to determine which window the event targets. Compare against the value from `Gfx.GetWindowID`. Returns 0 for event types that have no associated window (e.g., `QUIT_EVENT`).

### WindowEvent

```modula2
PROCEDURE WindowEvent(): INTEGER;
```

Returns the window event subtype of the most recent `WINDOW_EVENT`. Compare against `WEVT_*` constants. Returns 0 if the current event is not a `WINDOW_EVENT`.

```modula2
IF (evt = WINDOW_EVENT) & (WindowEvent() = WEVT_RESIZED) THEN
  newW := Gfx.GetWindowWidth(win);
  newH := Gfx.GetWindowHeight(win);
END;
```

### TextInput

```modula2
PROCEDURE TextInput(VAR s: ARRAY OF CHAR);
```

Copies the UTF-8 text from the most recent `TEXTINPUT` event into `s`. The string is null-terminated and truncated to fit `s`. If the current event is not `TEXTINPUT`, `s` is set to an empty string. Call `StartTextInput` before expecting `TEXTINPUT` events.

### TextInputLen

```modula2
PROCEDURE TextInputLen(): INTEGER;
```

Returns the byte length of the text from the most recent `TEXTINPUT` event, not including the null terminator. Returns 0 if the current event is not `TEXTINPUT`. Useful for checking whether the input will fit in a buffer before calling `TextInput`.

## Text Input

SDL text input mode must be explicitly enabled to receive `TEXTINPUT` events. When active, the system IME is engaged, which is necessary for composed characters and international input.

### StartTextInput

```modula2
PROCEDURE StartTextInput;
```

Enables text input mode. While active, keystroke events that produce text also generate `TEXTINPUT` events with the composed characters. Call this when a text field gains focus.

### StopTextInput

```modula2
PROCEDURE StopTextInput;
```

Disables text input mode. `TEXTINPUT` events will no longer be generated. Call this when a text field loses focus or the application switches to a non-text mode.

### IsTextInputActive

```modula2
PROCEDURE IsTextInputActive(): BOOLEAN;
```

Returns `TRUE` if text input mode is currently enabled, `FALSE` otherwise.

```modula2
IF NOT IsTextInputActive() THEN StartTextInput END;
```

## Keyboard State

### IsKeyPressed

```modula2
PROCEDURE IsKeyPressed(scancode: INTEGER): BOOLEAN;
```

Queries the current live keyboard state and returns `TRUE` if the key at the given physical `scancode` is currently held down. This reads hardware state directly and does not depend on the event queue -- it reflects the key state at the moment of the call, not at the time an event was generated. Use `SCAN_*` constants for the `scancode` parameter.

```modula2
IF IsKeyPressed(SCAN_W) THEN playerY := playerY - speed END;
IF IsKeyPressed(SCAN_A) THEN playerX := playerX - speed END;
IF IsKeyPressed(SCAN_S) THEN playerY := playerY + speed END;
IF IsKeyPressed(SCAN_D) THEN playerX := playerX + speed END;
```

## Mouse State

### GetMouseState

```modula2
PROCEDURE GetMouseState(VAR x, y: INTEGER): INTEGER;
```

Queries the current mouse position relative to the focused window, storing coordinates in `x` and `y`. Returns a bitmask of currently pressed buttons (bit 0 = left, bit 2 = middle, bit 3 = right as per SDL). This reads live hardware state, not the event queue.

```modula2
buttons := GetMouseState(mx, my);
```

### GetMouseGlobal

```modula2
PROCEDURE GetMouseGlobal(VAR x, y: INTEGER): INTEGER;
```

Queries the current mouse position in global screen coordinates, storing them in `x` and `y`. Returns the same button bitmask as `GetMouseState`. Useful for dragging operations that extend beyond the window boundary.

### WarpMouse

```modula2
PROCEDURE WarpMouse(win: ADDRESS; x, y: INTEGER);
```

Moves the mouse cursor to position (`x`, `y`) within the window `win`. Coordinates are in window-relative pixels. The warp generates a `MOUSEMOVE` event. Pass a valid `Gfx.Window` handle as `win`.

```modula2
WarpMouse(win, 320, 240);  (* center cursor in a 640x480 window *)
```

## Example

```modula2
MODULE EventsDemo;
FROM Gfx IMPORT Init, Quit, CreateWindow, DestroyWindow,
                 CreateRenderer, DestroyRenderer, Present, Delay,
                 WIN_CENTERED, RENDER_ACCELERATED, RENDER_VSYNC;
FROM Canvas IMPORT SetColor, Clear;
FROM Font IMPORT FontHandle, Open, Close, DrawText;
FROM Events IMPORT Poll, NONE, QUIT_EVENT, KEYDOWN, KEYUP,
                   MOUSEDOWN, MOUSEMOVE, MOUSEWHEEL, WINDOW_EVENT,
                   KeyCode, KeyMod, ScanCode, MouseX, MouseY,
                   MouseButton, WheelY, WindowEvent, IsKeyPressed,
                   GetMouseState, KEY_ESCAPE, KEY_SPACE,
                   MOD_SHIFT, BUTTON_LEFT, WEVT_RESIZED,
                   SCAN_W, SCAN_A, SCAN_S, SCAN_D;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;
VAR
  win, ren: ADDRESS;
  fnt: FontHandle;
  evt, mx, my, btn: INTEGER;
  px, py, scrollY: INTEGER;
  running: BOOLEAN;
BEGIN
  IF Init() THEN
    win := CreateWindow("Events Demo", 640, 480, WIN_CENTERED);
    ren := CreateRenderer(win, RENDER_ACCELERATED + RENDER_VSYNC);
    px := 320; py := 240; scrollY := 0;
    running := TRUE;
    WHILE running DO
      (* Drain event queue *)
      evt := Poll();
      WHILE evt # NONE DO
        IF evt = QUIT_EVENT THEN
          running := FALSE;
        ELSIF evt = KEYDOWN THEN
          IF KeyCode() = KEY_ESCAPE THEN running := FALSE END;
          WriteString("Key down scancode=");
          WriteInt(ScanCode(), 0); WriteLn;
        ELSIF evt = MOUSEDOWN THEN
          IF MouseButton() = BUTTON_LEFT THEN
            WriteString("Left click at ");
            WriteInt(MouseX(), 0); WriteString(",");
            WriteInt(MouseY(), 0); WriteLn;
          END;
        ELSIF evt = MOUSEWHEEL THEN
          scrollY := scrollY + WheelY();
        ELSIF evt = WINDOW_EVENT THEN
          IF WindowEvent() = WEVT_RESIZED THEN
            WriteString("Window resized"); WriteLn;
          END;
        END;
        evt := Poll();
      END;

      (* Continuous movement via live keyboard state *)
      IF IsKeyPressed(SCAN_W) THEN py := py - 3 END;
      IF IsKeyPressed(SCAN_S) THEN py := py + 3 END;
      IF IsKeyPressed(SCAN_A) THEN px := px - 3 END;
      IF IsKeyPressed(SCAN_D) THEN px := px + 3 END;

      SetColor(ren, 30, 30, 50, 255);
      Clear(ren);
      SetColor(ren, 200, 80, 80, 255);
      Present(ren);
      Delay(16);
    END;
    DestroyRenderer(ren);
    DestroyWindow(win);
    Quit;
  END;
END EventsDemo.
```
