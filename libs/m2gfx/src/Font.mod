IMPLEMENTATION MODULE Font;

FROM SYSTEM IMPORT ADR;
FROM GfxBridge IMPORT gfx_open_font, gfx_close_font,
     gfx_font_style, gfx_font_get_style, gfx_font_set_hinting,
     gfx_draw_text, gfx_draw_text_wrapped,
     gfx_text_width, gfx_text_height,
     gfx_font_height, gfx_font_ascent, gfx_font_descent,
     gfx_font_line_skip;

PROCEDURE Open(path: ARRAY OF CHAR; size: INTEGER): FontHandle;
BEGIN
  RETURN gfx_open_font(ADR(path), size)
END Open;

PROCEDURE Close(font: FontHandle);
BEGIN
  gfx_close_font(font)
END Close;

PROCEDURE SetStyle(font: FontHandle; style: INTEGER);
BEGIN
  gfx_font_style(font, style)
END SetStyle;

PROCEDURE GetStyle(font: FontHandle): INTEGER;
BEGIN
  RETURN gfx_font_get_style(font)
END GetStyle;

PROCEDURE SetHinting(font: FontHandle; hint: INTEGER);
BEGIN
  gfx_font_set_hinting(font, hint)
END SetHinting;

PROCEDURE DrawText(ren: Renderer; font: FontHandle;
                   text: ARRAY OF CHAR; x, y: INTEGER;
                   r, g, b, a: INTEGER);
BEGIN
  gfx_draw_text(ren, font, ADR(text), x, y, r, g, b, a)
END DrawText;

PROCEDURE DrawTextWrapped(ren: Renderer; font: FontHandle;
                          text: ARRAY OF CHAR; x, y, wrapWidth: INTEGER;
                          r, g, b, a: INTEGER);
BEGIN
  gfx_draw_text_wrapped(ren, font, ADR(text), x, y, wrapWidth, r, g, b, a)
END DrawTextWrapped;

PROCEDURE TextWidth(font: FontHandle; text: ARRAY OF CHAR): INTEGER;
BEGIN
  RETURN gfx_text_width(font, ADR(text))
END TextWidth;

PROCEDURE TextHeight(font: FontHandle; text: ARRAY OF CHAR): INTEGER;
BEGIN
  RETURN gfx_text_height(font, ADR(text))
END TextHeight;

PROCEDURE Height(font: FontHandle): INTEGER;
BEGIN
  RETURN gfx_font_height(font)
END Height;

PROCEDURE Ascent(font: FontHandle): INTEGER;
BEGIN
  RETURN gfx_font_ascent(font)
END Ascent;

PROCEDURE Descent(font: FontHandle): INTEGER;
BEGIN
  RETURN gfx_font_descent(font)
END Descent;

PROCEDURE LineSkip(font: FontHandle): INTEGER;
BEGIN
  RETURN gfx_font_line_skip(font)
END LineSkip;

END Font.
