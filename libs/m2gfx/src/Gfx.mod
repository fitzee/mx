IMPLEMENTATION MODULE Gfx;

FROM SYSTEM IMPORT ADR;
FROM GfxBridge IMPORT gfx_init, gfx_quit, gfx_ttf_init, gfx_ttf_quit,
     gfx_create_window, gfx_destroy_window, gfx_set_title,
     gfx_set_win_size, gfx_get_win_width, gfx_get_win_height,
     gfx_set_win_pos, gfx_set_fullscreen,
     gfx_show_win, gfx_hide_win, gfx_raise_win,
     gfx_minimize_win, gfx_maximize_win, gfx_restore_win,
     gfx_get_win_id, gfx_set_win_min_size, gfx_set_win_max_size,
     gfx_create_renderer, gfx_destroy_renderer, gfx_present,
     gfx_output_width, gfx_output_height,
     gfx_screen_width, gfx_screen_height, gfx_display_count,
     gfx_set_clipboard, gfx_get_clipboard, gfx_has_clipboard,
     gfx_ticks, gfx_delay,
     gfx_set_cursor, gfx_show_cursor;

PROCEDURE Init(): BOOLEAN;
BEGIN
  RETURN gfx_init() # 0
END Init;

PROCEDURE InitFont(): BOOLEAN;
BEGIN
  RETURN gfx_ttf_init() # 0
END InitFont;

PROCEDURE Quit;
BEGIN
  gfx_quit
END Quit;

PROCEDURE QuitFont;
BEGIN
  gfx_ttf_quit
END QuitFont;

PROCEDURE CreateWindow(title: ARRAY OF CHAR; w, h: INTEGER;
                       flags: INTEGER): Window;
BEGIN
  RETURN gfx_create_window(ADR(title), w, h, flags)
END CreateWindow;

PROCEDURE DestroyWindow(win: Window);
BEGIN
  gfx_destroy_window(win)
END DestroyWindow;

PROCEDURE SetTitle(win: Window; title: ARRAY OF CHAR);
BEGIN
  gfx_set_title(win, ADR(title))
END SetTitle;

PROCEDURE SetWindowSize(win: Window; w, h: INTEGER);
BEGIN
  gfx_set_win_size(win, w, h)
END SetWindowSize;

PROCEDURE GetWindowWidth(win: Window): INTEGER;
BEGIN
  RETURN gfx_get_win_width(win)
END GetWindowWidth;

PROCEDURE GetWindowHeight(win: Window): INTEGER;
BEGIN
  RETURN gfx_get_win_height(win)
END GetWindowHeight;

PROCEDURE SetWindowPos(win: Window; x, y: INTEGER);
BEGIN
  gfx_set_win_pos(win, x, y)
END SetWindowPos;

PROCEDURE SetFullscreen(win: Window; mode: INTEGER);
BEGIN
  gfx_set_fullscreen(win, mode)
END SetFullscreen;

PROCEDURE ShowWindow(win: Window);
BEGIN
  gfx_show_win(win)
END ShowWindow;

PROCEDURE HideWindow(win: Window);
BEGIN
  gfx_hide_win(win)
END HideWindow;

PROCEDURE RaiseWindow(win: Window);
BEGIN
  gfx_raise_win(win)
END RaiseWindow;

PROCEDURE MinimizeWindow(win: Window);
BEGIN
  gfx_minimize_win(win)
END MinimizeWindow;

PROCEDURE MaximizeWindow(win: Window);
BEGIN
  gfx_maximize_win(win)
END MaximizeWindow;

PROCEDURE RestoreWindow(win: Window);
BEGIN
  gfx_restore_win(win)
END RestoreWindow;

PROCEDURE GetWindowID(win: Window): INTEGER;
BEGIN
  RETURN gfx_get_win_id(win)
END GetWindowID;

PROCEDURE SetWindowMinSize(win: Window; w, h: INTEGER);
BEGIN
  gfx_set_win_min_size(win, w, h)
END SetWindowMinSize;

PROCEDURE SetWindowMaxSize(win: Window; w, h: INTEGER);
BEGIN
  gfx_set_win_max_size(win, w, h)
END SetWindowMaxSize;

PROCEDURE CreateRenderer(win: Window; flags: INTEGER): Renderer;
BEGIN
  RETURN gfx_create_renderer(win, flags)
END CreateRenderer;

PROCEDURE DestroyRenderer(ren: Renderer);
BEGIN
  gfx_destroy_renderer(ren)
END DestroyRenderer;

PROCEDURE Present(ren: Renderer);
BEGIN
  gfx_present(ren)
END Present;

PROCEDURE OutputWidth(ren: Renderer): INTEGER;
BEGIN
  RETURN gfx_output_width(ren)
END OutputWidth;

PROCEDURE OutputHeight(ren: Renderer): INTEGER;
BEGIN
  RETURN gfx_output_height(ren)
END OutputHeight;

PROCEDURE ScreenWidth(): INTEGER;
BEGIN
  RETURN gfx_screen_width()
END ScreenWidth;

PROCEDURE ScreenHeight(): INTEGER;
BEGIN
  RETURN gfx_screen_height()
END ScreenHeight;

PROCEDURE DisplayCount(): INTEGER;
BEGIN
  RETURN gfx_display_count()
END DisplayCount;

PROCEDURE SetClipboard(text: ARRAY OF CHAR);
BEGIN
  gfx_set_clipboard(ADR(text))
END SetClipboard;

PROCEDURE GetClipboard(VAR text: ARRAY OF CHAR);
BEGIN
  gfx_get_clipboard(ADR(text), HIGH(text) + 1)
END GetClipboard;

PROCEDURE HasClipboard(): BOOLEAN;
BEGIN
  RETURN gfx_has_clipboard() # 0
END HasClipboard;

PROCEDURE Ticks(): INTEGER;
BEGIN
  RETURN gfx_ticks()
END Ticks;

PROCEDURE Delay(ms: INTEGER);
BEGIN
  gfx_delay(ms)
END Delay;

PROCEDURE SetCursor(cursorType: INTEGER);
BEGIN
  gfx_set_cursor(cursorType)
END SetCursor;

PROCEDURE ShowCursor(show: BOOLEAN);
BEGIN
  IF show THEN
    gfx_show_cursor(1)
  ELSE
    gfx_show_cursor(0)
  END
END ShowCursor;

END Gfx.
