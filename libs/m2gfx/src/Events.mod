IMPLEMENTATION MODULE Events;

FROM SYSTEM IMPORT ADR;
FROM GfxBridge IMPORT gfx_poll_event, gfx_wait_event, gfx_wait_event_timeout,
     gfx_event_key, gfx_event_scancode, gfx_event_key_repeat, gfx_event_mod,
     gfx_event_mouse_x, gfx_event_mouse_y, gfx_event_mouse_btn,
     gfx_event_wheel_x, gfx_event_wheel_y,
     gfx_event_win_id, gfx_event_win_event,
     gfx_event_text, gfx_event_text_len,
     gfx_start_text, gfx_stop_text, gfx_is_text_active,
     gfx_key_state, gfx_mouse_state, gfx_mouse_global, gfx_warp_mouse;

PROCEDURE Poll(): INTEGER;
BEGIN
  RETURN gfx_poll_event()
END Poll;

PROCEDURE Wait(): INTEGER;
BEGIN
  RETURN gfx_wait_event()
END Wait;

PROCEDURE WaitTimeout(ms: INTEGER): INTEGER;
BEGIN
  RETURN gfx_wait_event_timeout(ms)
END WaitTimeout;

PROCEDURE KeyCode(): INTEGER;
BEGIN
  RETURN gfx_event_key()
END KeyCode;

PROCEDURE ScanCode(): INTEGER;
BEGIN
  RETURN gfx_event_scancode()
END ScanCode;

PROCEDURE KeyRepeat(): BOOLEAN;
BEGIN
  RETURN gfx_event_key_repeat() # 0
END KeyRepeat;

PROCEDURE KeyMod(): INTEGER;
BEGIN
  RETURN gfx_event_mod()
END KeyMod;

PROCEDURE MouseX(): INTEGER;
BEGIN
  RETURN gfx_event_mouse_x()
END MouseX;

PROCEDURE MouseY(): INTEGER;
BEGIN
  RETURN gfx_event_mouse_y()
END MouseY;

PROCEDURE MouseButton(): INTEGER;
BEGIN
  RETURN gfx_event_mouse_btn()
END MouseButton;

PROCEDURE WheelX(): INTEGER;
BEGIN
  RETURN gfx_event_wheel_x()
END WheelX;

PROCEDURE WheelY(): INTEGER;
BEGIN
  RETURN gfx_event_wheel_y()
END WheelY;

PROCEDURE WindowID(): INTEGER;
BEGIN
  RETURN gfx_event_win_id()
END WindowID;

PROCEDURE WindowEvent(): INTEGER;
BEGIN
  RETURN gfx_event_win_event()
END WindowEvent;

PROCEDURE TextInput(VAR s: ARRAY OF CHAR);
BEGIN
  gfx_event_text(ADR(s), HIGH(s) + 1)
END TextInput;

PROCEDURE TextInputLen(): INTEGER;
BEGIN
  RETURN gfx_event_text_len()
END TextInputLen;

PROCEDURE StartTextInput;
BEGIN
  gfx_start_text
END StartTextInput;

PROCEDURE StopTextInput;
BEGIN
  gfx_stop_text
END StopTextInput;

PROCEDURE IsTextInputActive(): BOOLEAN;
BEGIN
  RETURN gfx_is_text_active() # 0
END IsTextInputActive;

PROCEDURE IsKeyPressed(scancode: INTEGER): BOOLEAN;
BEGIN
  RETURN gfx_key_state(scancode) # 0
END IsKeyPressed;

PROCEDURE GetMouseState(VAR x, y: INTEGER): INTEGER;
BEGIN
  RETURN gfx_mouse_state(x, y)
END GetMouseState;

PROCEDURE GetMouseGlobal(VAR x, y: INTEGER): INTEGER;
BEGIN
  RETURN gfx_mouse_global(x, y)
END GetMouseGlobal;

PROCEDURE WarpMouse(win: ADDRESS; x, y: INTEGER);
BEGIN
  gfx_warp_mouse(win, x, y)
END WarpMouse;

END Events.
