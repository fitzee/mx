/*
 * gfx_bridge.c — SDL2/SDL2_ttf graphics bridge for the m2c Modula-2 compiler.
 *
 * Provides a flat C API wrapping SDL2 and SDL2_ttf so that Modula-2 programs
 * can call graphics routines via FFI.  All integer arguments use int32_t;
 * all opaque handles are passed as void*.
 *
 * Compile with:
 *   cc -c gfx_bridge.c $(sdl2-config --cflags) -I/path/to/SDL2_ttf
 * Link with:
 *   -lSDL2 -lSDL2_ttf
 */

#include <SDL2/SDL.h>
#include <SDL2/SDL_ttf.h>
#include <stdint.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <math.h>

#define STB_IMAGE_IMPLEMENTATION
#define STBI_ONLY_PNG
#include "stb_image.h"

#define STB_IMAGE_WRITE_IMPLEMENTATION
#include "stb_image_write.h"

/* ========================================================================
 * Global state
 * ======================================================================== */

/* Last polled/waited event — shared by all gfx_event_* accessors. */
static SDL_Event g_event;

/* Cached system cursor so we can free the previous one on change. */
static SDL_Cursor *g_cursor = NULL;

/* DPI scale factor for high-DPI / Retina displays (1 on normal, 2 on Retina). */
static int g_dpi_scale = 1;

/* ========================================================================
 * Internal helpers
 * ======================================================================== */

/* Map our custom event-type enum from an SDL event. */
static int32_t map_event_type(const SDL_Event *ev)
{
    switch (ev->type) {
    case SDL_QUIT:             return 1;
    case SDL_KEYDOWN:          return 2;
    case SDL_KEYUP:            return 3;
    case SDL_MOUSEBUTTONDOWN:  return 4;
    case SDL_MOUSEBUTTONUP:    return 5;
    case SDL_MOUSEMOTION:      return 6;
    case SDL_MOUSEWHEEL:       return 7;
    case SDL_TEXTINPUT:        return 8;
    case SDL_WINDOWEVENT:      return 9;
    case SDL_TEXTEDITING:      return 10;
    default:                   return 0;
    }
}

/* Map our custom blend-mode integer to SDL_BlendMode. */
static SDL_BlendMode map_blend(int32_t mode)
{
    switch (mode) {
    case 0:  return SDL_BLENDMODE_NONE;
    case 1:  return SDL_BLENDMODE_BLEND;
    case 2:  return SDL_BLENDMODE_ADD;
    case 4:  return SDL_BLENDMODE_MOD;
    default: return SDL_BLENDMODE_NONE;
    }
}

/* ========================================================================
 * Init / Quit
 * ======================================================================== */

int32_t gfx_init(void)
{
    if (SDL_Init(SDL_INIT_VIDEO | SDL_INIT_TIMER) < 0)
        return 0;
    return 1;
}

void gfx_quit(void)
{
    if (g_cursor) {
        SDL_FreeCursor(g_cursor);
        g_cursor = NULL;
    }
    SDL_Quit();
}

int32_t gfx_ttf_init(void)
{
    if (TTF_Init() < 0)
        return 0;
    return 1;
}

void gfx_ttf_quit(void)
{
    TTF_Quit();
}

/* ========================================================================
 * Window
 * ======================================================================== */

void *gfx_create_window(const char *title, int32_t w, int32_t h, int32_t flags)
{
    int pos_x, pos_y;
    Uint32 sdl_flags = 0;

    /* Position: centered if bit 0 set, else undefined. */
    if (flags & 1) {
        pos_x = SDL_WINDOWPOS_CENTERED;
        pos_y = SDL_WINDOWPOS_CENTERED;
    } else {
        pos_x = SDL_WINDOWPOS_UNDEFINED;
        pos_y = SDL_WINDOWPOS_UNDEFINED;
    }

    /* Map our flag bits to SDL window flags. */
    if (flags & 2)  sdl_flags |= SDL_WINDOW_RESIZABLE;
    if (flags & 4)  sdl_flags |= SDL_WINDOW_BORDERLESS;
    if (flags & 8)  sdl_flags |= SDL_WINDOW_FULLSCREEN;
    if (flags & 16) sdl_flags |= SDL_WINDOW_HIDDEN;
    if (flags & 32) sdl_flags |= SDL_WINDOW_MAXIMIZED;
    if (flags & 64) sdl_flags |= SDL_WINDOW_ALLOW_HIGHDPI;

    /* Default to SHOWN unless HIDDEN was requested. */
    if (!(flags & 16))
        sdl_flags |= SDL_WINDOW_SHOWN;

    return (void *)SDL_CreateWindow(title, pos_x, pos_y, w, h, sdl_flags);
}

void gfx_destroy_window(void *win)
{
    if (win) SDL_DestroyWindow((SDL_Window *)win);
}

void gfx_set_title(void *win, const char *title)
{
    if (win) SDL_SetWindowTitle((SDL_Window *)win, title);
}

void gfx_set_win_size(void *win, int32_t w, int32_t h)
{
    if (win) SDL_SetWindowSize((SDL_Window *)win, w, h);
}

int32_t gfx_get_win_width(void *win)
{
    int w = 0, h = 0;
    if (win) SDL_GetWindowSize((SDL_Window *)win, &w, &h);
    return (int32_t)w;
}

int32_t gfx_get_win_height(void *win)
{
    int w = 0, h = 0;
    if (win) SDL_GetWindowSize((SDL_Window *)win, &w, &h);
    return (int32_t)h;
}

void gfx_set_win_pos(void *win, int32_t x, int32_t y)
{
    if (win) SDL_SetWindowPosition((SDL_Window *)win, x, y);
}

void gfx_set_fullscreen(void *win, int32_t mode)
{
    if (!win) return;
    Uint32 f = 0;
    switch (mode) {
    case 1: f = SDL_WINDOW_FULLSCREEN;         break;
    case 2: f = SDL_WINDOW_FULLSCREEN_DESKTOP; break;
    default: f = 0; break;
    }
    SDL_SetWindowFullscreen((SDL_Window *)win, f);
}

void gfx_show_win(void *win)
{
    if (win) SDL_ShowWindow((SDL_Window *)win);
}

void gfx_hide_win(void *win)
{
    if (win) SDL_HideWindow((SDL_Window *)win);
}

void gfx_raise_win(void *win)
{
    if (win) SDL_RaiseWindow((SDL_Window *)win);
}

void gfx_minimize_win(void *win)
{
    if (win) SDL_MinimizeWindow((SDL_Window *)win);
}

void gfx_maximize_win(void *win)
{
    if (win) SDL_MaximizeWindow((SDL_Window *)win);
}

void gfx_restore_win(void *win)
{
    if (win) SDL_RestoreWindow((SDL_Window *)win);
}

int32_t gfx_get_win_id(void *win)
{
    if (!win) return 0;
    return (int32_t)SDL_GetWindowID((SDL_Window *)win);
}

void gfx_set_win_min_size(void *win, int32_t w, int32_t h)
{
    if (win) SDL_SetWindowMinimumSize((SDL_Window *)win, w, h);
}

void gfx_set_win_max_size(void *win, int32_t w, int32_t h)
{
    if (win) SDL_SetWindowMaximumSize((SDL_Window *)win, w, h);
}

/* ========================================================================
 * Renderer
 * ======================================================================== */

void *gfx_create_renderer(void *win, int32_t flags)
{
    Uint32 sdl_flags = 0;
    if (flags & 1) sdl_flags |= SDL_RENDERER_ACCELERATED;
    if (flags & 2) sdl_flags |= SDL_RENDERER_PRESENTVSYNC;
    if (flags & 4) sdl_flags |= SDL_RENDERER_SOFTWARE;
    SDL_Renderer *ren = SDL_CreateRenderer((SDL_Window *)win, -1, sdl_flags);
    if (ren && win) {
        /* If the window has high-DPI, compute the scale factor and set logical
           size so all drawing code uses the same coordinates while SDL renders
           at native physical resolution. */
        int ww = 0, wh = 0, ow = 0, oh = 0;
        SDL_GetWindowSize((SDL_Window *)win, &ww, &wh);
        SDL_GetRendererOutputSize(ren, &ow, &oh);
        if (ww > 0 && ow > ww) {
            g_dpi_scale = ow / ww;
            SDL_RenderSetLogicalSize(ren, ww, wh);
        } else {
            g_dpi_scale = 1;
        }
    }
    return (void *)ren;
}

void gfx_update_logical_size(void *ren, void *win)
{
    if (!ren || !win) return;
    if (g_dpi_scale > 1) {
        int ww = 0, wh = 0;
        SDL_GetWindowSize((SDL_Window *)win, &ww, &wh);
        if (ww > 0 && wh > 0) {
            SDL_RenderSetLogicalSize((SDL_Renderer *)ren, ww, wh);
        }
    }
}

void gfx_destroy_renderer(void *ren)
{
    if (ren) SDL_DestroyRenderer((SDL_Renderer *)ren);
}

void gfx_present(void *ren)
{
    if (ren) SDL_RenderPresent((SDL_Renderer *)ren);
}

int32_t gfx_output_width(void *ren)
{
    int w = 0, h = 0;
    if (ren) SDL_GetRendererOutputSize((SDL_Renderer *)ren, &w, &h);
    return (int32_t)w;
}

int32_t gfx_output_height(void *ren)
{
    int w = 0, h = 0;
    if (ren) SDL_GetRendererOutputSize((SDL_Renderer *)ren, &w, &h);
    return (int32_t)h;
}

/* ========================================================================
 * Color and Clear
 * ======================================================================== */

void gfx_set_color(void *ren, int32_t r, int32_t g, int32_t b, int32_t a)
{
    if (ren)
        SDL_SetRenderDrawColor((SDL_Renderer *)ren,
                               (Uint8)r, (Uint8)g, (Uint8)b, (Uint8)a);
}

void gfx_get_color(void *ren, int32_t *r, int32_t *g, int32_t *b, int32_t *a)
{
    Uint8 rr = 0, gg = 0, bb = 0, aa = 0;
    if (ren)
        SDL_GetRenderDrawColor((SDL_Renderer *)ren, &rr, &gg, &bb, &aa);
    if (r) *r = (int32_t)rr;
    if (g) *g = (int32_t)gg;
    if (b) *b = (int32_t)bb;
    if (a) *a = (int32_t)aa;
}

void gfx_clear(void *ren)
{
    if (ren) SDL_RenderClear((SDL_Renderer *)ren);
}

/* ========================================================================
 * Drawing Primitives
 * ======================================================================== */

void gfx_draw_point(void *ren, int32_t x, int32_t y)
{
    if (ren) SDL_RenderDrawPoint((SDL_Renderer *)ren, x, y);
}

void gfx_draw_line(void *ren, int32_t x1, int32_t y1, int32_t x2, int32_t y2)
{
    if (ren) SDL_RenderDrawLine((SDL_Renderer *)ren, x1, y1, x2, y2);
}

void gfx_draw_rect(void *ren, int32_t x, int32_t y, int32_t w, int32_t h)
{
    if (!ren) return;
    SDL_Rect rect = { x, y, w, h };
    SDL_RenderDrawRect((SDL_Renderer *)ren, &rect);
}

void gfx_fill_rect(void *ren, int32_t x, int32_t y, int32_t w, int32_t h)
{
    if (!ren) return;
    SDL_Rect rect = { x, y, w, h };
    SDL_RenderFillRect((SDL_Renderer *)ren, &rect);
}

/* ========================================================================
 * Clipping
 * ======================================================================== */

void gfx_set_clip(void *ren, int32_t x, int32_t y, int32_t w, int32_t h)
{
    if (!ren) return;
    SDL_Rect rect = { x, y, w, h };
    SDL_RenderSetClipRect((SDL_Renderer *)ren, &rect);
}

void gfx_clear_clip(void *ren)
{
    if (ren) SDL_RenderSetClipRect((SDL_Renderer *)ren, NULL);
}

int32_t gfx_get_clip_x(void *ren)
{
    SDL_Rect rect = {0, 0, 0, 0};
    if (ren) SDL_RenderGetClipRect((SDL_Renderer *)ren, &rect);
    return rect.x;
}

int32_t gfx_get_clip_y(void *ren)
{
    SDL_Rect rect = {0, 0, 0, 0};
    if (ren) SDL_RenderGetClipRect((SDL_Renderer *)ren, &rect);
    return rect.y;
}

int32_t gfx_get_clip_w(void *ren)
{
    SDL_Rect rect = {0, 0, 0, 0};
    if (ren) SDL_RenderGetClipRect((SDL_Renderer *)ren, &rect);
    return rect.w;
}

int32_t gfx_get_clip_h(void *ren)
{
    SDL_Rect rect = {0, 0, 0, 0};
    if (ren) SDL_RenderGetClipRect((SDL_Renderer *)ren, &rect);
    return rect.h;
}

/* ========================================================================
 * Blend Mode
 * ======================================================================== */

void gfx_set_blend(void *ren, int32_t mode)
{
    if (ren) SDL_SetRenderDrawBlendMode((SDL_Renderer *)ren, map_blend(mode));
}

/* ========================================================================
 * Viewport
 * ======================================================================== */

void gfx_set_viewport(void *ren, int32_t x, int32_t y, int32_t w, int32_t h)
{
    if (!ren) return;
    SDL_Rect rect = { x, y, w, h };
    SDL_RenderSetViewport((SDL_Renderer *)ren, &rect);
}

void gfx_reset_viewport(void *ren)
{
    if (ren) SDL_RenderSetViewport((SDL_Renderer *)ren, NULL);
}

/* ========================================================================
 * Events
 * ======================================================================== */

int32_t gfx_poll_event(void)
{
    if (SDL_PollEvent(&g_event))
        return map_event_type(&g_event);
    return 0;
}

int32_t gfx_wait_event(void)
{
    if (SDL_WaitEvent(&g_event))
        return map_event_type(&g_event);
    return 0;
}

int32_t gfx_wait_event_timeout(int32_t ms)
{
    if (SDL_WaitEventTimeout(&g_event, ms))
        return map_event_type(&g_event);
    return 0;
}

/* Map an SDL keycode to our simplified key enum. */
int32_t gfx_event_key(void)
{
    SDL_Keycode k = g_event.key.keysym.sym;

    /* ASCII range: return as-is. */
    if (k >= 0 && k <= 127)
        return (int32_t)k;

    switch (k) {
    /* Arrow keys */
    case SDLK_UP:          return 256;
    case SDLK_DOWN:        return 257;
    case SDLK_LEFT:        return 258;
    case SDLK_RIGHT:       return 259;

    /* Navigation */
    case SDLK_INSERT:      return 260;
    case SDLK_HOME:        return 262;
    case SDLK_END:         return 263;
    case SDLK_PAGEUP:      return 264;
    case SDLK_PAGEDOWN:    return 265;

    /* Function keys */
    case SDLK_F1:          return 282;
    case SDLK_F2:          return 283;
    case SDLK_F3:          return 284;
    case SDLK_F4:          return 285;
    case SDLK_F5:          return 286;
    case SDLK_F6:          return 287;
    case SDLK_F7:          return 288;
    case SDLK_F8:          return 289;
    case SDLK_F9:          return 290;
    case SDLK_F10:         return 291;
    case SDLK_F11:         return 292;
    case SDLK_F12:         return 293;

    /* Lock keys */
    case SDLK_NUMLOCKCLEAR: return 300;
    case SDLK_CAPSLOCK:     return 301;
    case SDLK_SCROLLLOCK:   return 302;

    /* Modifier keys */
    case SDLK_LSHIFT:      return 304;
    case SDLK_RSHIFT:      return 305;
    case SDLK_LCTRL:       return 306;
    case SDLK_RCTRL:       return 307;
    case SDLK_LALT:        return 308;
    case SDLK_RALT:        return 309;
    case SDLK_LGUI:        return 310;
    case SDLK_RGUI:        return 311;

    /* Misc */
    case SDLK_PRINTSCREEN: return 316;
    case SDLK_PAUSE:       return 317;

    /* Keypad */
    case SDLK_KP_ENTER:   return 271;
    case SDLK_KP_0:       return 320;
    case SDLK_KP_1:       return 321;
    case SDLK_KP_2:       return 322;
    case SDLK_KP_3:       return 323;
    case SDLK_KP_4:       return 324;
    case SDLK_KP_5:       return 325;
    case SDLK_KP_6:       return 326;
    case SDLK_KP_7:       return 327;
    case SDLK_KP_8:       return 328;
    case SDLK_KP_9:       return 329;

    default:
        return (int32_t)(k & 0x1FF);
    }
}

int32_t gfx_event_scancode(void)
{
    return (int32_t)g_event.key.keysym.scancode;
}

int32_t gfx_event_key_repeat(void)
{
    return (int32_t)g_event.key.repeat;
}

int32_t gfx_event_mod(void)
{
    /* Use SDL_GetModState() instead of g_event.key.keysym.mod so that
       modifier state is correct for ALL event types, not just keyboard.
       Reading the key union member during a mouse event returns garbage
       (e.g. the mouse Y coordinate aliased over keysym.mod). */
    Uint16 m = SDL_GetModState();
    int32_t out = 0;
    if (m & KMOD_SHIFT) out |= 1;
    if (m & KMOD_CTRL)  out |= 2;
    if (m & KMOD_ALT)   out |= 4;
    if (m & KMOD_GUI)   out |= 8;
    return out;
}

int32_t gfx_event_mouse_x(void)
{
    if (g_event.type == SDL_MOUSEBUTTONDOWN || g_event.type == SDL_MOUSEBUTTONUP)
        return (int32_t)g_event.button.x;
    if (g_event.type == SDL_MOUSEMOTION)
        return (int32_t)g_event.motion.x;
    return 0;
}

int32_t gfx_event_mouse_y(void)
{
    if (g_event.type == SDL_MOUSEBUTTONDOWN || g_event.type == SDL_MOUSEBUTTONUP)
        return (int32_t)g_event.button.y;
    if (g_event.type == SDL_MOUSEMOTION)
        return (int32_t)g_event.motion.y;
    return 0;
}

int32_t gfx_event_mouse_btn(void)
{
    return (int32_t)g_event.button.button;
}

int32_t gfx_event_wheel_x(void)
{
    return (int32_t)g_event.wheel.x;
}

int32_t gfx_event_wheel_y(void)
{
    return (int32_t)g_event.wheel.y;
}

int32_t gfx_event_win_id(void)
{
    /* The windowID field is at the same offset for window, key, mouse events. */
    switch (g_event.type) {
    case SDL_WINDOWEVENT:
        return (int32_t)g_event.window.windowID;
    case SDL_KEYDOWN:
    case SDL_KEYUP:
        return (int32_t)g_event.key.windowID;
    case SDL_MOUSEBUTTONDOWN:
    case SDL_MOUSEBUTTONUP:
        return (int32_t)g_event.button.windowID;
    case SDL_MOUSEMOTION:
        return (int32_t)g_event.motion.windowID;
    case SDL_MOUSEWHEEL:
        return (int32_t)g_event.wheel.windowID;
    case SDL_TEXTINPUT:
        return (int32_t)g_event.text.windowID;
    case SDL_TEXTEDITING:
        return (int32_t)g_event.edit.windowID;
    default:
        return 0;
    }
}

int32_t gfx_event_win_event(void)
{
    if (g_event.type != SDL_WINDOWEVENT)
        return 0;

    switch (g_event.window.event) {
    case SDL_WINDOWEVENT_SHOWN:        return 1;
    case SDL_WINDOWEVENT_HIDDEN:       return 2;
    case SDL_WINDOWEVENT_EXPOSED:      return 3;
    case SDL_WINDOWEVENT_MOVED:        return 4;
    case SDL_WINDOWEVENT_RESIZED:      return 5;
    case SDL_WINDOWEVENT_SIZE_CHANGED: return 5;
    case SDL_WINDOWEVENT_MINIMIZED:    return 6;
    case SDL_WINDOWEVENT_MAXIMIZED:    return 7;
    case SDL_WINDOWEVENT_RESTORED:     return 8;
    case SDL_WINDOWEVENT_ENTER:        return 9;
    case SDL_WINDOWEVENT_LEAVE:        return 10;
    case SDL_WINDOWEVENT_FOCUS_GAINED: return 11;
    case SDL_WINDOWEVENT_FOCUS_LOST:   return 12;
    case SDL_WINDOWEVENT_CLOSE:        return 13;
    default:                           return 0;
    }
}

void gfx_event_text(char *buf, int32_t buflen)
{
    if (!buf || buflen <= 0) return;
    if (g_event.type == SDL_TEXTINPUT) {
        strncpy(buf, g_event.text.text, (size_t)(buflen - 1));
        buf[buflen - 1] = '\0';
    } else {
        buf[0] = '\0';
    }
}

int32_t gfx_event_text_len(void)
{
    if (g_event.type == SDL_TEXTINPUT)
        return (int32_t)strlen(g_event.text.text);
    return 0;
}

void gfx_start_text(void)
{
    SDL_StartTextInput();
}

void gfx_stop_text(void)
{
    SDL_StopTextInput();
}

int32_t gfx_is_text_active(void)
{
    return SDL_IsTextInputActive() ? 1 : 0;
}

int32_t gfx_key_state(int32_t scancode)
{
    const Uint8 *state = SDL_GetKeyboardState(NULL);
    if (!state) return 0;
    return (int32_t)state[scancode];
}

int32_t gfx_mouse_state(int32_t *x, int32_t *y)
{
    int ix = 0, iy = 0;
    Uint32 buttons = SDL_GetMouseState(&ix, &iy);
    if (x) *x = (int32_t)ix;
    if (y) *y = (int32_t)iy;
    return (int32_t)buttons;
}

int32_t gfx_mouse_global(int32_t *x, int32_t *y)
{
    int ix = 0, iy = 0;
    Uint32 buttons = SDL_GetGlobalMouseState(&ix, &iy);
    if (x) *x = (int32_t)ix;
    if (y) *y = (int32_t)iy;
    return (int32_t)buttons;
}

void gfx_warp_mouse(void *win, int32_t x, int32_t y)
{
    if (win) SDL_WarpMouseInWindow((SDL_Window *)win, x, y);
}

/* ========================================================================
 * Font (SDL2_ttf)
 * ======================================================================== */

void *gfx_open_font(const char *path, int32_t size)
{
    /* Open at physical DPI size so text textures are native resolution. */
    return (void *)TTF_OpenFont(path, size * g_dpi_scale);
}

void *gfx_open_font_physical(const char *path, int32_t physical_size)
{
    /* Open at exact physical pixel size — no DPI scaling applied. */
    return (void *)TTF_OpenFont(path, physical_size);
}

int32_t gfx_dpi_scale(void)
{
    return (int32_t)g_dpi_scale;
}

void gfx_close_font(void *font)
{
    if (font) TTF_CloseFont((TTF_Font *)font);
}

void gfx_font_style(void *font, int32_t style)
{
    if (!font) return;

    int sdl_style = TTF_STYLE_NORMAL;
    if (style & 1) sdl_style |= TTF_STYLE_BOLD;
    if (style & 2) sdl_style |= TTF_STYLE_ITALIC;
    if (style & 4) sdl_style |= TTF_STYLE_UNDERLINE;
    if (style & 8) sdl_style |= TTF_STYLE_STRIKETHROUGH;

    TTF_SetFontStyle((TTF_Font *)font, sdl_style);
}

int32_t gfx_font_get_style(void *font)
{
    if (!font) return 0;

    int sdl_style = TTF_GetFontStyle((TTF_Font *)font);
    int32_t out = 0;
    if (sdl_style & TTF_STYLE_BOLD)          out |= 1;
    if (sdl_style & TTF_STYLE_ITALIC)        out |= 2;
    if (sdl_style & TTF_STYLE_UNDERLINE)     out |= 4;
    if (sdl_style & TTF_STYLE_STRIKETHROUGH) out |= 8;
    return out;
}

void gfx_draw_text(void *ren, void *font, const char *text,
                    int32_t x, int32_t y,
                    int32_t r, int32_t g, int32_t b, int32_t a)
{
    if (!ren || !font || !text || !*text) return;

    SDL_Color color = { (Uint8)r, (Uint8)g, (Uint8)b, (Uint8)a };
    SDL_Surface *surf = TTF_RenderUTF8_Blended((TTF_Font *)font, text, color);
    if (!surf) return;

    SDL_Texture *tex = SDL_CreateTextureFromSurface((SDL_Renderer *)ren, surf);
    if (!tex) {
        SDL_FreeSurface(surf);
        return;
    }

    /* Destination rect in logical coords — font is rendered at DPI scale,
       so divide surface dimensions to get logical size. */
    SDL_Rect dst = { x, y, surf->w / g_dpi_scale, surf->h / g_dpi_scale };
    SDL_RenderCopy((SDL_Renderer *)ren, tex, NULL, &dst);

    SDL_DestroyTexture(tex);
    SDL_FreeSurface(surf);
}

void gfx_draw_text_wrapped(void *ren, void *font, const char *text,
                            int32_t x, int32_t y, int32_t wrapWidth,
                            int32_t r, int32_t g, int32_t b, int32_t a)
{
    if (!ren || !font || !text || !*text) return;

    SDL_Color color = { (Uint8)r, (Uint8)g, (Uint8)b, (Uint8)a };
    SDL_Surface *surf = TTF_RenderUTF8_Blended_Wrapped(
        (TTF_Font *)font, text, color, (Uint32)(wrapWidth * g_dpi_scale));
    if (!surf) return;

    SDL_Texture *tex = SDL_CreateTextureFromSurface((SDL_Renderer *)ren, surf);
    if (!tex) {
        SDL_FreeSurface(surf);
        return;
    }

    SDL_Rect dst = { x, y, surf->w / g_dpi_scale, surf->h / g_dpi_scale };
    SDL_RenderCopy((SDL_Renderer *)ren, tex, NULL, &dst);

    SDL_DestroyTexture(tex);
    SDL_FreeSurface(surf);
}

int32_t gfx_text_width(void *font, const char *text)
{
    int w = 0, h = 0;
    if (font && text)
        TTF_SizeUTF8((TTF_Font *)font, text, &w, &h);
    return (int32_t)(w / g_dpi_scale);
}

int32_t gfx_text_height(void *font, const char *text)
{
    int w = 0, h = 0;
    if (font && text)
        TTF_SizeUTF8((TTF_Font *)font, text, &w, &h);
    return (int32_t)(h / g_dpi_scale);
}

int32_t gfx_font_height(void *font)
{
    if (!font) return 0;
    return (int32_t)(TTF_FontHeight((TTF_Font *)font) / g_dpi_scale);
}

int32_t gfx_font_ascent(void *font)
{
    if (!font) return 0;
    return (int32_t)(TTF_FontAscent((TTF_Font *)font) / g_dpi_scale);
}

int32_t gfx_font_descent(void *font)
{
    if (!font) return 0;
    return (int32_t)(TTF_FontDescent((TTF_Font *)font) / g_dpi_scale);
}

int32_t gfx_font_line_skip(void *font)
{
    if (!font) return 0;
    return (int32_t)(TTF_FontLineSkip((TTF_Font *)font) / g_dpi_scale);
}

void gfx_font_set_hinting(void *font, int32_t hint)
{
    if (!font) return;
    int sdl_hint;
    switch (hint) {
    case 0:  sdl_hint = TTF_HINTING_NORMAL; break;
    case 1:  sdl_hint = TTF_HINTING_LIGHT;  break;
    case 2:  sdl_hint = TTF_HINTING_MONO;   break;
    case 3:  sdl_hint = TTF_HINTING_NONE;   break;
    default: sdl_hint = TTF_HINTING_NORMAL;  break;
    }
    TTF_SetFontHinting((TTF_Font *)font, sdl_hint);
}

/* ========================================================================
 * Texture
 * ======================================================================== */

void *gfx_load_bmp(void *ren, const char *path)
{
    if (!ren || !path) return NULL;

    SDL_Surface *surf = SDL_LoadBMP(path);
    if (!surf) return NULL;

    /* Use nearest-neighbor scaling for pixel art */
    SDL_SetHint(SDL_HINT_RENDER_SCALE_QUALITY, "0");
    SDL_Texture *tex = SDL_CreateTextureFromSurface((SDL_Renderer *)ren, surf);
    SDL_SetHint(SDL_HINT_RENDER_SCALE_QUALITY, "1");
    SDL_FreeSurface(surf);
    return (void *)tex;
}

void *gfx_load_bmp_keyed(void *ren, const char *path,
                         int32_t kr, int32_t kg, int32_t kb)
{
    if (!ren || !path) return NULL;

    SDL_Surface *surf = SDL_LoadBMP(path);
    if (!surf) return NULL;

    SDL_SetColorKey(surf, SDL_TRUE,
                    SDL_MapRGB(surf->format, (Uint8)kr, (Uint8)kg, (Uint8)kb));
    /* Use nearest-neighbor scaling for pixel art / bitmap fonts */
    SDL_SetHint(SDL_HINT_RENDER_SCALE_QUALITY, "0");
    SDL_Texture *tex = SDL_CreateTextureFromSurface((SDL_Renderer *)ren, surf);
    SDL_SetHint(SDL_HINT_RENDER_SCALE_QUALITY, "1");
    SDL_FreeSurface(surf);
    return (void *)tex;
}

void *gfx_create_texture(void *ren, int32_t w, int32_t h)
{
    if (!ren) return NULL;
    return (void *)SDL_CreateTexture(
        (SDL_Renderer *)ren,
        SDL_PIXELFORMAT_RGBA8888,
        SDL_TEXTUREACCESS_TARGET,
        w, h);
}

void *gfx_text_texture(void *ren, void *font, const char *text,
                        int32_t r, int32_t g, int32_t b, int32_t a)
{
    if (!ren || !font || !text || !*text) return NULL;

    SDL_Color color = { (Uint8)r, (Uint8)g, (Uint8)b, (Uint8)a };
    SDL_Surface *surf = TTF_RenderUTF8_Blended((TTF_Font *)font, text, color);
    if (!surf) return NULL;

    SDL_Texture *tex = SDL_CreateTextureFromSurface((SDL_Renderer *)ren, surf);
    SDL_FreeSurface(surf);
    return (void *)tex;
}

void gfx_destroy_texture(void *tex)
{
    if (tex) SDL_DestroyTexture((SDL_Texture *)tex);
}

void gfx_draw_texture(void *ren, void *tex, int32_t x, int32_t y)
{
    if (!ren || !tex) return;

    int w = 0, h = 0;
    SDL_QueryTexture((SDL_Texture *)tex, NULL, NULL, &w, &h);

    SDL_Rect dst = { x, y, w, h };
    SDL_RenderCopy((SDL_Renderer *)ren, (SDL_Texture *)tex, NULL, &dst);
}

void gfx_draw_texture_ex(void *ren, void *tex,
                          int32_t sx, int32_t sy, int32_t sw, int32_t sh,
                          int32_t dx, int32_t dy, int32_t dw, int32_t dh)
{
    if (!ren || !tex) return;

    SDL_Rect src = { sx, sy, sw, sh };
    SDL_Rect dst = { dx, dy, dw, dh };
    SDL_RenderCopy((SDL_Renderer *)ren, (SDL_Texture *)tex, &src, &dst);
}

void gfx_draw_texture_rot(void *ren, void *tex,
                           int32_t dx, int32_t dy, int32_t dw, int32_t dh,
                           int32_t angleDeg, int32_t flip)
{
    if (!ren || !tex) return;

    SDL_Rect dst = { dx, dy, dw, dh };
    SDL_RendererFlip sdl_flip = SDL_FLIP_NONE;
    switch (flip) {
    case 1: sdl_flip = SDL_FLIP_HORIZONTAL; break;
    case 2: sdl_flip = SDL_FLIP_VERTICAL;   break;
    case 3: sdl_flip = (SDL_RendererFlip)(SDL_FLIP_HORIZONTAL | SDL_FLIP_VERTICAL); break;
    default: break;
    }

    SDL_RenderCopyEx((SDL_Renderer *)ren, (SDL_Texture *)tex,
                     NULL, &dst, (double)angleDeg, NULL, sdl_flip);
}

int32_t gfx_tex_width(void *tex)
{
    int w = 0;
    if (tex) SDL_QueryTexture((SDL_Texture *)tex, NULL, NULL, &w, NULL);
    return (int32_t)w;
}

int32_t gfx_tex_height(void *tex)
{
    int h = 0;
    if (tex) SDL_QueryTexture((SDL_Texture *)tex, NULL, NULL, NULL, &h);
    return (int32_t)h;
}

void gfx_set_tex_alpha(void *tex, int32_t alpha)
{
    if (tex) SDL_SetTextureAlphaMod((SDL_Texture *)tex, (Uint8)alpha);
}

void gfx_set_tex_blend(void *tex, int32_t mode)
{
    if (tex) SDL_SetTextureBlendMode((SDL_Texture *)tex, map_blend(mode));
}

void gfx_set_tex_color(void *tex, int32_t r, int32_t g, int32_t b)
{
    if (tex)
        SDL_SetTextureColorMod((SDL_Texture *)tex,
                               (Uint8)r, (Uint8)g, (Uint8)b);
}

void gfx_set_target(void *ren, void *tex)
{
    if (ren)
        SDL_SetRenderTarget((SDL_Renderer *)ren, (SDL_Texture *)tex);
}

void gfx_reset_target(void *ren)
{
    if (ren)
        SDL_SetRenderTarget((SDL_Renderer *)ren, NULL);
}

/* ========================================================================
 * Screen
 * ======================================================================== */

int32_t gfx_screen_width(void)
{
    SDL_Rect rect;
    if (SDL_GetDisplayBounds(0, &rect) == 0)
        return (int32_t)rect.w;
    return 0;
}

int32_t gfx_screen_height(void)
{
    SDL_Rect rect;
    if (SDL_GetDisplayBounds(0, &rect) == 0)
        return (int32_t)rect.h;
    return 0;
}

int32_t gfx_display_count(void)
{
    return (int32_t)SDL_GetNumVideoDisplays();
}

/* ========================================================================
 * Clipboard
 * ======================================================================== */

void gfx_set_clipboard(const char *text)
{
    if (text) SDL_SetClipboardText(text);
}

void gfx_get_clipboard(char *buf, int32_t buflen)
{
    if (!buf || buflen <= 0) return;

    char *text = SDL_GetClipboardText();
    if (text) {
        strncpy(buf, text, (size_t)(buflen - 1));
        buf[buflen - 1] = '\0';
        SDL_free(text);
    } else {
        buf[0] = '\0';
    }
}

int32_t gfx_has_clipboard(void)
{
    return SDL_HasClipboardText() ? 1 : 0;
}

/* ========================================================================
 * Timer
 * ======================================================================== */

int32_t gfx_ticks(void)
{
    return (int32_t)SDL_GetTicks();
}

void gfx_delay(int32_t ms)
{
    if (ms > 0) SDL_Delay((Uint32)ms);
}

/* ========================================================================
 * Cursor
 * ======================================================================== */

void gfx_set_cursor(int32_t type)
{
    SDL_SystemCursor id;
    switch (type) {
    case 0:  id = SDL_SYSTEM_CURSOR_ARROW;     break;
    case 1:  id = SDL_SYSTEM_CURSOR_IBEAM;     break;
    case 2:  id = SDL_SYSTEM_CURSOR_WAIT;      break;
    case 3:  id = SDL_SYSTEM_CURSOR_CROSSHAIR; break;
    case 4:  id = SDL_SYSTEM_CURSOR_HAND;      break;
    case 5:  id = SDL_SYSTEM_CURSOR_SIZENWSE;  break;
    case 6:  id = SDL_SYSTEM_CURSOR_SIZENESW;  break;
    case 7:  id = SDL_SYSTEM_CURSOR_SIZEWE;    break;
    case 8:  id = SDL_SYSTEM_CURSOR_SIZENS;    break;
    case 9:  id = SDL_SYSTEM_CURSOR_SIZEALL;   break;
    case 10: id = SDL_SYSTEM_CURSOR_NO;        break;
    case 11: id = SDL_SYSTEM_CURSOR_WAITARROW; break;
    default: id = SDL_SYSTEM_CURSOR_ARROW;     break;
    }

    SDL_Cursor *new_cursor = SDL_CreateSystemCursor(id);
    if (new_cursor) {
        SDL_SetCursor(new_cursor);
        /* Free the previously cached cursor to avoid leaks. */
        if (g_cursor)
            SDL_FreeCursor(g_cursor);
        g_cursor = new_cursor;
    }
}

void gfx_show_cursor(int32_t show)
{
    SDL_ShowCursor(show ? SDL_ENABLE : SDL_DISABLE);
}

/* ========================================================================
 * Indexed Pixel Buffer (8-bit palette, up to 256 colors)
 *
 * Provides a software pixel buffer where each pixel stores a palette index.
 * All drawing primitives operate on the buffer.  A render function converts
 * the indexed data to RGBA and pushes it to an SDL texture for display.
 * Region save/restore enables delta-based undo.
 * ======================================================================== */

typedef struct {
    uint8_t  *pixels;       /* indexed color buffer (w * h bytes)         */
    uint32_t *rgba;         /* RGBA conversion buffer for SDL_UpdateTexture */
    int32_t   w, h;
    uint32_t  pal[256];     /* packed RGBA per palette entry               */
    int32_t   dirty;        /* 1 if any pixels changed since last render   */
    int32_t   dx, dy, dw, dh; /* dirty sub-rectangle                      */
} PixBuf;

/* Saved rectangular region for undo. */
typedef struct {
    uint8_t *data;
    int32_t  w, h;
} PBRegion;

/* --- Internal helpers -------------------------------------------------- */

static inline void pb_pack(uint32_t *out, uint8_t r, uint8_t g, uint8_t b)
{
    /* Match SDL_PIXELFORMAT_RGBA8888: R in MSB, A in LSB. */
    *out = ((uint32_t)r << 24) | ((uint32_t)g << 16)
         | ((uint32_t)b << 8)  | 0xFFu;
}

static inline void pb_unpack(uint32_t c, uint8_t *r, uint8_t *g, uint8_t *b)
{
    *r = (uint8_t)(c >> 24);
    *g = (uint8_t)((c >> 16) & 0xFF);
    *b = (uint8_t)((c >> 8)  & 0xFF);
}

static inline int pb_ok(PixBuf *p, int32_t x, int32_t y)
{
    return x >= 0 && x < p->w && y >= 0 && y < p->h;
}

static inline void pb_mark_dirty(PixBuf *p, int32_t x, int32_t y)
{
    if (!p->dirty) {
        p->dirty = 1;
        p->dx = x; p->dy = y; p->dw = 1; p->dh = 1;
    } else {
        int32_t x2 = p->dx + p->dw;
        int32_t y2 = p->dy + p->dh;
        if (x < p->dx) p->dx = x;
        if (y < p->dy) p->dy = y;
        if (x + 1 > x2) x2 = x + 1;
        if (y + 1 > y2) y2 = y + 1;
        p->dw = x2 - p->dx;
        p->dh = y2 - p->dy;
    }
}

static inline void pb_mark_dirty_rect(PixBuf *p, int32_t rx, int32_t ry,
                                       int32_t rw, int32_t rh)
{
    if (rw <= 0 || rh <= 0) return;
    if (!p->dirty) {
        p->dirty = 1;
        p->dx = rx; p->dy = ry; p->dw = rw; p->dh = rh;
    } else {
        int32_t x2 = p->dx + p->dw;
        int32_t y2 = p->dy + p->dh;
        if (rx < p->dx) p->dx = rx;
        if (ry < p->dy) p->dy = ry;
        if (rx + rw > x2) x2 = rx + rw;
        if (ry + rh > y2) y2 = ry + rh;
        p->dw = x2 - p->dx;
        p->dh = y2 - p->dy;
    }
}

static inline void pb_put(PixBuf *p, int32_t x, int32_t y, uint8_t ci)
{
    if (pb_ok(p, x, y)) {
        p->pixels[y * p->w + x] = ci;
        pb_mark_dirty(p, x, y);
    }
}

static inline uint8_t pb_peek(PixBuf *p, int32_t x, int32_t y)
{
    return pb_ok(p, x, y) ? p->pixels[y * p->w + x] : 0;
}

/* --- Management -------------------------------------------------------- */

void *gfx_pb_create(int32_t w, int32_t h)
{
    PixBuf *p = (PixBuf *)calloc(1, sizeof(PixBuf));
    if (!p) return NULL;
    p->w = w;  p->h = h;
    p->pixels = (uint8_t  *)calloc((size_t)(w * h), 1);
    p->rgba   = (uint32_t *)calloc((size_t)(w * h), sizeof(uint32_t));
    if (!p->pixels || !p->rgba) {
        free(p->pixels); free(p->rgba); free(p);
        return NULL;
    }
    for (int i = 0; i < 256; i++)
        pb_pack(&p->pal[i], (uint8_t)i, (uint8_t)i, (uint8_t)i);
    pb_mark_dirty_rect(p, 0, 0, w, h);
    return p;
}

void gfx_pb_free(void *pb)
{
    PixBuf *p = (PixBuf *)pb;
    if (!p) return;
    free(p->pixels); free(p->rgba); free(p);
}

void gfx_pb_clear(void *pb, int32_t idx)
{
    PixBuf *p = (PixBuf *)pb;
    if (!p) return;
    memset(p->pixels, (uint8_t)idx, (size_t)(p->w * p->h));
    pb_mark_dirty_rect(p, 0, 0, p->w, p->h);
}

int32_t gfx_pb_width(void *pb)  { PixBuf *p=(PixBuf*)pb; return p?p->w:0; }
int32_t gfx_pb_height(void *pb) { PixBuf *p=(PixBuf*)pb; return p?p->h:0; }

/* --- Palette ----------------------------------------------------------- */

void gfx_pb_set_pal(void *pb, int32_t idx, int32_t r, int32_t g, int32_t b)
{
    PixBuf *p = (PixBuf *)pb;
    if (p && idx >= 0 && idx <= 255) {
        pb_pack(&p->pal[idx], (uint8_t)r, (uint8_t)g, (uint8_t)b);
        pb_mark_dirty_rect(p, 0, 0, p->w, p->h);
    }
}

void gfx_pb_set_pal_alpha(void *pb, int32_t idx, int32_t r, int32_t g, int32_t b, int32_t a)
{
    PixBuf *p = (PixBuf *)pb;
    if (p && idx >= 0 && idx <= 255) {
        p->pal[idx] = ((uint32_t)(uint8_t)r << 24) | ((uint32_t)(uint8_t)g << 16)
                    | ((uint32_t)(uint8_t)b << 8) | (uint32_t)(uint8_t)a;
        pb_mark_dirty_rect(p, 0, 0, p->w, p->h);
    }
}

int32_t gfx_pb_pal_packed(void *pb, int32_t idx)
{
    PixBuf *p = (PixBuf *)pb;
    return (p && idx >= 0 && idx <= 255) ? (int32_t)p->pal[idx] : 0;
}

/* --- Pixel access ------------------------------------------------------ */

void gfx_pb_set(void *pb, int32_t x, int32_t y, int32_t idx)
{
    PixBuf *p = (PixBuf *)pb;
    if (p) pb_put(p, x, y, (uint8_t)idx);
}

int32_t gfx_pb_get(void *pb, int32_t x, int32_t y)
{
    PixBuf *p = (PixBuf *)pb;
    return p ? (int32_t)pb_peek(p, x, y) : 0;
}

/* --- Infrastructure helpers for M2 algorithms -------------------------- */

void gfx_pb_fill_row(void *pb, int32_t x, int32_t y, int32_t w, int32_t idx)
{
    PixBuf *p = (PixBuf *)pb;
    if (!p) return;
    int32_t x0 = x < 0 ? 0 : x;
    int32_t x1 = (x + w > p->w) ? p->w : x + w;
    if (y < 0 || y >= p->h || x0 >= x1) return;
    memset(p->pixels + y * p->w + x0, (uint8_t)idx, (size_t)(x1 - x0));
    pb_mark_dirty_rect(p, x0, y, x1 - x0, 1);
}

void gfx_pb_mark_dirty(void *pb, int32_t x, int32_t y, int32_t w, int32_t h)
{
    PixBuf *p = (PixBuf *)pb;
    if (p) pb_mark_dirty_rect(p, x, y, w, h);
}

int32_t gfx_pb_total(void *pb)
{
    PixBuf *p = (PixBuf *)pb;
    return p ? p->w * p->h : 0;
}

void *gfx_alloc(int32_t bytes)
{
    if (bytes <= 0) return NULL;
    return calloc(1, (size_t)bytes);
}

void gfx_dealloc(void *ptr)
{
    free(ptr);
}

int32_t gfx_buf_get(void *buf, int32_t offset)
{
    if (!buf || offset < 0) return 0;
    return (int32_t)((uint8_t *)buf)[offset];
}

void gfx_buf_set(void *buf, int32_t offset, int32_t val)
{
    if (!buf || offset < 0) return;
    ((uint8_t *)buf)[offset] = (uint8_t)val;
}

void *gfx_pb_pixel_ptr(void *pb)
{
    PixBuf *p = (PixBuf *)pb;
    return p ? p->pixels : NULL;
}

void gfx_pb_composite(void *dst, void *src, int32_t transparent_idx)
{
    PixBuf *d = (PixBuf *)dst;
    PixBuf *s = (PixBuf *)src;
    if (!d || !s) return;
    int32_t sz = s->w * s->h;
    int32_t dsz = d->w * d->h;
    if (sz > dsz) sz = dsz;
    uint8_t ti = (uint8_t)transparent_idx;
    for (int32_t i = 0; i < sz; i++) {
        if (s->pixels[i] != ti)
            d->pixels[i] = s->pixels[i];
    }
    pb_mark_dirty_rect(d, 0, 0, d->w, d->h);
}

void gfx_pb_copy_pixels(void *src, void *dst)
{
    PixBuf *s = (PixBuf *)src;
    PixBuf *d = (PixBuf *)dst;
    if (!s || !d) return;
    int32_t sz = s->w * s->h;
    int32_t dsz = d->w * d->h;
    if (sz > dsz) sz = dsz;
    memcpy(d->pixels, s->pixels, (size_t)sz);
    memcpy(d->pal, s->pal, sizeof(d->pal));
    pb_mark_dirty_rect(d, 0, 0, d->w, d->h);
}

/* --- Nearest-color lookup (used by load_png) --------------------------- */

static int32_t pb_nearest(PixBuf *p, uint8_t r, uint8_t g, uint8_t b, int n)
{
    int32_t best = 0, bestd = INT32_MAX;
    for (int32_t i = 0; i < n; i++) {
        uint8_t pr, pg, pb_;
        pb_unpack(p->pal[i], &pr, &pg, &pb_);
        int32_t dr=(int32_t)r-pr, dg=(int32_t)g-pg, db=(int32_t)b-pb_;
        int32_t d = dr*dr + dg*dg + db*db;
        if (d < bestd) { bestd = d; best = i; }
    }
    return best;
}

/* --- Text stamp -------------------------------------------------------- */

void gfx_pb_stamp_text(void *pb, void *ren, void *font,
                        const char *text, int32_t x, int32_t y, int32_t idx)
{
    PixBuf *p = (PixBuf *)pb;
    if (!p || !font || !text || !*text) return;

    SDL_Color white = { 255, 255, 255, 255 };
    SDL_Surface *surf = TTF_RenderUTF8_Blended((TTF_Font *)font, text, white);
    if (!surf) return;

    SDL_LockSurface(surf);
    for (int row = 0; row < surf->h; row++)
        for (int col = 0; col < surf->w; col++) {
            Uint8 *pp = (Uint8 *)surf->pixels + row * surf->pitch
                      + col * surf->format->BytesPerPixel;
            Uint32 px;
            memcpy(&px, pp, surf->format->BytesPerPixel);
            Uint8 r, g, b, a;
            SDL_GetRGBA(px, surf->format, &r, &g, &b, &a);
            if (a > 128)
                pb_put(p, x + col, y + row, (uint8_t)idx);
        }
    SDL_UnlockSurface(surf);
    SDL_FreeSurface(surf);
}

/* --- Render to SDL texture --------------------------------------------- */

void gfx_pb_render(void *ren, void *tex, void *pb)
{
    PixBuf *p = (PixBuf *)pb;
    if (!ren || !tex || !p) return;
    if (!p->dirty) return;  /* nothing changed since last render */

    /* Clamp dirty rect to buffer bounds */
    int32_t x0 = p->dx < 0 ? 0 : p->dx;
    int32_t y0 = p->dy < 0 ? 0 : p->dy;
    int32_t x1 = p->dx + p->dw; if (x1 > p->w) x1 = p->w;
    int32_t y1 = p->dy + p->dh; if (y1 > p->h) y1 = p->h;
    int32_t dw = x1 - x0, dh = y1 - y0;

    if (dw <= 0 || dh <= 0) { p->dirty = 0; return; }

    /* Convert only dirty sub-rectangle from indexed to RGBA */
    for (int32_t row = y0; row < y1; row++) {
        int32_t off = row * p->w + x0;
        for (int32_t col = 0; col < dw; col++)
            p->rgba[off + col] = p->pal[p->pixels[off + col]];
    }

    /* Upload only the dirty sub-rectangle */
    SDL_Rect r;
    r.x = x0; r.y = y0; r.w = dw; r.h = dh;
    SDL_UpdateTexture((SDL_Texture *)tex, &r,
                      p->rgba + y0 * p->w + x0,
                      p->w * (int)sizeof(uint32_t));
    p->dirty = 0;
}

/* --- Region save / restore (for delta-based undo) ---------------------- */

void *gfx_pb_save(void *pb, int32_t x, int32_t y, int32_t w, int32_t h)
{
    PixBuf *p = (PixBuf *)pb;
    if (!p) return NULL;
    if (x < 0) { w += x; x = 0; }
    if (y < 0) { h += y; y = 0; }
    if (x + w > p->w) w = p->w - x;
    if (y + h > p->h) h = p->h - y;
    if (w <= 0 || h <= 0) return NULL;

    PBRegion *r = (PBRegion *)malloc(sizeof(PBRegion));
    if (!r) return NULL;
    r->w = w;  r->h = h;
    r->data = (uint8_t *)malloc((size_t)(w * h));
    if (!r->data) { free(r); return NULL; }
    for (int32_t row = 0; row < h; row++)
        memcpy(r->data + row * w, p->pixels + (y + row) * p->w + x, (size_t)w);
    return r;
}

void gfx_pb_restore(void *pb, void *region, int32_t x, int32_t y)
{
    PixBuf *p = (PixBuf *)pb;
    PBRegion *r = (PBRegion *)region;
    if (!p || !r) return;
    for (int32_t row = 0; row < r->h; row++) {
        int32_t dy = y + row;
        if (dy < 0 || dy >= p->h) continue;
        int32_t dx = x < 0 ? 0 : x;
        int32_t sw = r->w, sx = 0;
        if (x < 0) { sx = -x; sw += x; }
        if (dx + sw > p->w) sw = p->w - dx;
        if (sw > 0)
            memcpy(p->pixels + dy*p->w + dx, r->data + row*r->w + sx, (size_t)sw);
    }
    pb_mark_dirty_rect(p, x < 0 ? 0 : x, y < 0 ? 0 : y, r->w, r->h);
}

int32_t gfx_pb_save_w(void *region)
{
    PBRegion *r = (PBRegion *)region;
    return r ? r->w : 0;
}

int32_t gfx_pb_save_h(void *region)
{
    PBRegion *r = (PBRegion *)region;
    return r ? r->h : 0;
}

void gfx_pb_free_save(void *region)
{
    PBRegion *r = (PBRegion *)region;
    if (!r) return;
    free(r->data); free(r);
}

/* SaveBMP — migrated to Modula-2 (PixBuf module) */

/* Layer system — migrated to Modula-2 (PixBuf module) */

/* ========================================================================
 * File I/O — PNG, .dp2, palette
 * ======================================================================== */

int32_t gfx_pb_save_png(void *pb, const char *path)
{
    PixBuf *p = (PixBuf *)pb;
    if (!p || !path) return 0;
    int32_t w = p->w, h = p->h;
    uint8_t *rgb = (uint8_t *)malloc((size_t)(w * h * 3));
    if (!rgb) return 0;
    for (int32_t i = 0; i < w * h; i++) {
        uint8_t r, g, b;
        pb_unpack(p->pal[p->pixels[i]], &r, &g, &b);
        rgb[i*3+0] = r; rgb[i*3+1] = g; rgb[i*3+2] = b;
    }
    int ok = stbi_write_png(path, w, h, 3, rgb, w * 3);
    free(rgb);
    return ok ? 1 : 0;
}

void *gfx_pb_load_png(const char *path, int32_t ncolors)
{
    int w, h, comp;
    uint8_t *data = stbi_load(path, &w, &h, &comp, 3);
    if (!data) return NULL;
    if (ncolors < 1) ncolors = 32;
    PixBuf *p = (PixBuf *)gfx_pb_create(w, h);
    if (!p) { stbi_image_free(data); return NULL; }
    for (int32_t i = 0; i < w * h; i++) {
        uint8_t r = data[i*3+0], g = data[i*3+1], b = data[i*3+2];
        p->pixels[i] = (uint8_t)pb_nearest(p, r, g, b, ncolors);
    }
    stbi_image_free(data);
    pb_mark_dirty_rect(p, 0, 0, p->w, p->h);
    return p;
}

/* Load PNG with a pre-defined palette (no auto-quantization). */
void *gfx_pb_load_png_pal(const char *path, void *palPb, int32_t ncolors)
{
    int w, h, comp;
    uint8_t *data = stbi_load(path, &w, &h, &comp, 3);
    if (!data) return NULL;
    if (ncolors < 1) ncolors = 32;
    PixBuf *p = (PixBuf *)gfx_pb_create(w, h);
    if (!p) { stbi_image_free(data); return NULL; }
    /* Copy palette from palPb */
    PixBuf *pp = (PixBuf *)palPb;
    if (pp) {
        for (int i = 0; i < 256 && i < ncolors; i++)
            p->pal[i] = pp->pal[i];
    }
    for (int32_t i = 0; i < w * h; i++) {
        uint8_t r = data[i*3+0], g = data[i*3+1], b = data[i*3+2];
        p->pixels[i] = (uint8_t)pb_nearest(p, r, g, b, ncolors);
    }
    stbi_image_free(data);
    pb_mark_dirty_rect(p, 0, 0, p->w, p->h);
    return p;
}

/* DP2 save/load — migrated to Modula-2 (PixBuf module) */

/* Frame system — migrated to Modula-2 (PixBuf module) */

/* RenderAlpha — stays in C (SDL texture operations) */
void gfx_pb_render_alpha(void *ren, void *tex, void *pb, int32_t alpha) {
    PixBuf *p = (PixBuf *)pb;
    SDL_Texture *t = (SDL_Texture *)tex;
    if (!p || !t) return;
    SDL_SetTextureBlendMode(t, SDL_BLENDMODE_BLEND);
    SDL_SetTextureAlphaMod(t, (uint8_t)alpha);
    uint32_t *argb = (uint32_t *)malloc((size_t)p->w * p->h * 4);
    if (!argb) return;
    for (int i = 0; i < p->w * p->h; i++) {
        uint32_t c = p->pal[p->pixels[i]];
        uint8_t r = (c >> 24) & 0xFF;
        uint8_t g = (c >> 16) & 0xFF;
        uint8_t b = (c >> 8)  & 0xFF;
        argb[i] = 0xFF000000u | ((uint32_t)r << 16) |
                  ((uint32_t)g << 8) | (uint32_t)b;
    }
    SDL_UpdateTexture(t, NULL, argb, p->w * 4);
    free(argb);
    SDL_RenderCopy((SDL_Renderer *)ren, t, NULL, NULL);
    SDL_SetTextureAlphaMod(t, 255);
    SDL_SetTextureBlendMode(t, SDL_BLENDMODE_NONE);
}

/* ═══════════════════════════════════════════════════════════════════
   HAM (Hold-And-Modify) Rendering
   ═══════════════════════════════════════════════════════════════════ */

/* Render pixel buffer in HAM6 or HAM8 mode.
   In HAM mode, the top 2 bits of each pixel index encode the modify command:
     00 = set color from palette (bottom 4/6 bits = index)
     01 = modify blue channel
     10 = modify red channel
     11 = modify green channel
   mode: 6 = HAM6 (16 base colors, 4-bit modify)
         8 = HAM8 (64 base colors, 6-bit modify) */
void gfx_pb_render_ham(void *ren, void *tex, void *pb, int32_t mode)
{
    PixBuf *p = (PixBuf *)pb;
    SDL_Texture *t = (SDL_Texture *)tex;
    if (!p || !t) return;

    int base_bits = (mode == 8) ? 6 : 4;
    int mod_shift = (mode == 8) ? 2 : 4;

    for (int y = 0; y < p->h; y++) {
        uint8_t r = 0, g = 0, b = 0;
        for (int x = 0; x < p->w; x++) {
            uint8_t ci = p->pixels[y * p->w + x];
            int cmd = (ci >> base_bits) & 3;
            int val = ci & ((1 << base_bits) - 1);
            if (cmd == 0) {
                /* Set from palette */
                uint32_t c = p->pal[val];
                r = (c >> 24) & 0xFF;
                g = (c >> 16) & 0xFF;
                b = (c >> 8)  & 0xFF;
            } else if (cmd == 1) {
                b = (uint8_t)(val << mod_shift);
            } else if (cmd == 2) {
                r = (uint8_t)(val << mod_shift);
            } else {
                g = (uint8_t)(val << mod_shift);
            }
            p->rgba[y * p->w + x] = 0xFF000000u |
                ((uint32_t)r << 16) | ((uint32_t)g << 8) | (uint32_t)b;
        }
    }
    SDL_UpdateTexture(t, NULL, p->rgba, p->w * 4);
    p->dirty = 0;
}

/* --- RGBA buffer helpers (for M2-side post-processing) -------------- */

void gfx_pb_pal_to_screen(void *pb)
{
    PixBuf *p = (PixBuf *)pb;
    if (!p) return;
    for (int y = 0; y < p->h; y++) {
        for (int x = 0; x < p->w; x++) {
            uint32_t c = p->pal[p->pixels[y * p->w + x]];
            p->rgba[y * p->w + x] = ((c >> 24) << 16)
                | (((c >> 16) & 0xFF) << 8) | ((c >> 8) & 0xFF)
                | 0xFF000000u;
        }
    }
}

int32_t gfx_pb_rgba_get32(void *pb, int32_t offset)
{
    PixBuf *p = (PixBuf *)pb;
    return (p && offset >= 0 && offset < p->w * p->h)
        ? (int32_t)p->rgba[offset] : 0;
}

void gfx_pb_rgba_set32(void *pb, int32_t offset, int32_t val)
{
    PixBuf *p = (PixBuf *)pb;
    if (p && offset >= 0 && offset < p->w * p->h)
        p->rgba[offset] = (uint32_t)val;
}

void gfx_pb_flush_tex(void *tex, void *pb)
{
    PixBuf *p = (PixBuf *)pb;
    SDL_Texture *t = (SDL_Texture *)tex;
    if (!p || !t) return;
    SDL_UpdateTexture(t, NULL, p->rgba, p->w * 4);
    p->dirty = 0;
}

/* ConfigSave/ConfigLoad — migrated to Modula-2 (PixBuf module) */

void gfx_log(const char *path, const char *msg)
{
    if (!path || !msg) return;
    FILE *f = fopen(path, "a");
    if (!f) return;
    fprintf(f, "%s\n", msg);
    fclose(f);
}
