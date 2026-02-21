# m2gfx

SDL2-based graphics library for Modula-2 programs. Provides window
management, 2D drawing, event handling, font rendering, textures,
and an indexed pixel buffer with layer and animation support.

m2gfx wraps SDL2 and SDL2\_ttf through a C bridge layer. On macOS,
install the dependencies via Homebrew:

```
brew install sdl2 sdl2_ttf
```

## Modules

| Module | Description |
|---|---|
| [Gfx](Gfx.md) | Window management, renderer, screen info, clipboard, timer, cursor |
| [Canvas](Canvas.md) | 2D drawing primitives on the SDL renderer |
| [Events](Events.md) | Event polling and input handling (keyboard, mouse, window) |
| [Font](Font.md) | TrueType font loading and text rendering via SDL2\_ttf |
| [Texture](Texture.md) | Hardware-accelerated texture management |
| [PixBuf](PixBuf.md) | Indexed pixel buffer with 8-bit palette, layers, and animation frames |
| [Color](Color.md) | Pure Modula-2 color utilities for RGBA8888 packed format |
| [DrawAlgo](DrawAlgo.md) | Shared drawing algorithms parameterized by output callbacks |

GfxBridge is an internal module that provides the raw C FFI bindings
to SDL2. It is not intended for direct use.

## Manifest Configuration

Add the m2gfx source directory to `includes=`, link its C bridge via
`extra-c=`, and provide SDL2 compiler/linker flags in `m2.toml`:

```ini
includes=src ../../libs/m2gfx/src

[cc]
cflags=-I/opt/homebrew/include
ldflags=-L/opt/homebrew/lib
libs=SDL2 SDL2_ttf
extra-c=../../libs/m2gfx/src/gfx_bridge.c
```

Adjust the relative paths to match your project layout.
