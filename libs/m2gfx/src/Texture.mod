IMPLEMENTATION MODULE Texture;

FROM SYSTEM IMPORT ADR;
FROM GfxBridge IMPORT gfx_load_bmp, gfx_load_bmp_keyed,
     gfx_create_texture, gfx_text_texture,
     gfx_destroy_texture,
     gfx_draw_texture, gfx_draw_texture_ex, gfx_draw_texture_rot,
     gfx_tex_width, gfx_tex_height,
     gfx_set_tex_alpha, gfx_set_tex_blend, gfx_set_tex_color,
     gfx_set_target, gfx_reset_target;

PROCEDURE LoadBMP(ren: Renderer; path: ARRAY OF CHAR): Tex;
BEGIN
  RETURN gfx_load_bmp(ren, ADR(path))
END LoadBMP;

PROCEDURE LoadBMPKeyed(ren: Renderer; path: ARRAY OF CHAR;
                        kr, kg, kb: INTEGER): Tex;
BEGIN
  RETURN gfx_load_bmp_keyed(ren, ADR(path), kr, kg, kb)
END LoadBMPKeyed;

PROCEDURE Create(ren: Renderer; w, h: INTEGER): Tex;
BEGIN
  RETURN gfx_create_texture(ren, w, h)
END Create;

PROCEDURE FromText(ren: Renderer; font: FontHandle;
                   text: ARRAY OF CHAR; r, g, b, a: INTEGER): Tex;
BEGIN
  RETURN gfx_text_texture(ren, font, ADR(text), r, g, b, a)
END FromText;

PROCEDURE Destroy(tex: Tex);
BEGIN
  gfx_destroy_texture(tex)
END Destroy;

PROCEDURE Draw(ren: Renderer; tex: Tex; x, y: INTEGER);
BEGIN
  gfx_draw_texture(ren, tex, x, y)
END Draw;

PROCEDURE DrawRegion(ren: Renderer; tex: Tex;
                     sx, sy, sw, sh: INTEGER;
                     dx, dy, dw, dh: INTEGER);
BEGIN
  gfx_draw_texture_ex(ren, tex, sx, sy, sw, sh, dx, dy, dw, dh)
END DrawRegion;

PROCEDURE DrawRotated(ren: Renderer; tex: Tex;
                      dx, dy, dw, dh: INTEGER;
                      angleDeg: INTEGER; flip: INTEGER);
BEGIN
  gfx_draw_texture_rot(ren, tex, dx, dy, dw, dh, angleDeg, flip)
END DrawRotated;

PROCEDURE Width(tex: Tex): INTEGER;
BEGIN
  RETURN gfx_tex_width(tex)
END Width;

PROCEDURE Height(tex: Tex): INTEGER;
BEGIN
  RETURN gfx_tex_height(tex)
END Height;

PROCEDURE SetAlpha(tex: Tex; alpha: INTEGER);
BEGIN
  gfx_set_tex_alpha(tex, alpha)
END SetAlpha;

PROCEDURE SetBlendMode(tex: Tex; mode: INTEGER);
BEGIN
  gfx_set_tex_blend(tex, mode)
END SetBlendMode;

PROCEDURE SetColorMod(tex: Tex; r, g, b: INTEGER);
BEGIN
  gfx_set_tex_color(tex, r, g, b)
END SetColorMod;

PROCEDURE SetTarget(ren: Renderer; tex: Tex);
BEGIN
  gfx_set_target(ren, tex)
END SetTarget;

PROCEDURE ResetTarget(ren: Renderer);
BEGIN
  gfx_reset_target(ren)
END ResetTarget;

END Texture.
