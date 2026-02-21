# Color

Pure Modula-2 color utilities for the RGBA8888 packed pixel format. All operations use the compiler builtins `SHL`, `SHR`, `BAND`, and `BOR` for bit manipulation -- no SDL or platform dependency. Colors are stored as `CARDINAL` values with the bit layout: R in bits 31..24, G in bits 23..16, B in bits 15..8, A in bits 7..0 (most significant byte is red, least significant is alpha).

## Bit Layout

```
  31      24 23      16 15       8 7        0
 +----------+----------+----------+----------+
 |    R     |    G     |    B     |    A     |
 +----------+----------+----------+----------+
```

Each channel occupies 8 bits with values in the range 0..255.

## Packing

### Pack

```modula2
PROCEDURE Pack(r, g, b: INTEGER): CARDINAL;
```

Packs three color channels into an RGBA8888 `CARDINAL` with alpha set to `0FFH` (fully opaque). Each of `r`, `g`, `b` should be in the range 0..255; values outside this range produce undefined high bits. The result is `SHL(r,24) BOR SHL(g,16) BOR SHL(b,8) BOR 0FFH`.

```modula2
red := Pack(255, 0, 0);    (* 0xFF0000FF *)
white := Pack(255, 255, 255); (* 0xFFFFFFFF *)
```

### PackAlpha

```modula2
PROCEDURE PackAlpha(r, g, b, a: INTEGER): CARDINAL;
```

Packs four channels into an RGBA8888 `CARDINAL`. Identical to `Pack` except the alpha channel is set to `a` instead of `0FFH`. All parameters should be in the range 0..255.

```modula2
semiTransparent := PackAlpha(0, 128, 255, 128); (* 0x0080FF80 *)
```

## Unpacking

### UnpackR

```modula2
PROCEDURE UnpackR(c: CARDINAL): INTEGER;
```

Extracts the red channel (bits 31..24) from an RGBA8888 color, returning an `INTEGER` in the range 0..255. Computed as `BAND(SHR(c, 24), 0FFH)`.

### UnpackG

```modula2
PROCEDURE UnpackG(c: CARDINAL): INTEGER;
```

Extracts the green channel (bits 23..16) from an RGBA8888 color, returning an `INTEGER` in the range 0..255. Computed as `BAND(SHR(c, 16), 0FFH)`.

### UnpackB

```modula2
PROCEDURE UnpackB(c: CARDINAL): INTEGER;
```

Extracts the blue channel (bits 15..8) from an RGBA8888 color, returning an `INTEGER` in the range 0..255. Computed as `BAND(SHR(c, 8), 0FFH)`.

### Round-trip

Pack and Unpack are inverses for channel values in 0..255:

```modula2
c := Pack(100, 150, 200);
r := UnpackR(c);  (* 100 *)
g := UnpackG(c);  (* 150 *)
b := UnpackB(c);  (* 200 *)
```

Note: there is no `UnpackA` procedure. To extract the alpha channel, use `BAND(c, 0FFH)`.

## Blending

### Blend

```modula2
PROCEDURE Blend(base, target, pct: INTEGER): INTEGER;
```

Integer linear interpolation between two values. Returns `base + (target - base) * pct DIV 100`. `pct` = 0 returns `base`; `pct` = 100 returns `target`. Intended for per-channel blending -- call once per R, G, B component. Values of `pct` outside 0..100 extrapolate linearly (no clamping).

```modula2
(* Blend 25% from red toward blue *)
r := Blend(255, 0, 25);    (* 191 *)
g := Blend(0, 0, 25);      (*   0 *)
b := Blend(0, 255, 25);    (*  63 *)
mixed := Pack(r, g, b);
```

## Complete Example

Blend two colors and print the resulting channel values.

```modula2
MODULE ColorDemo;

FROM Color IMPORT Pack, UnpackR, UnpackG, UnpackB, Blend;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

VAR
  sky, grass, mixed: CARDINAL;
  r, g, b, pct: INTEGER;

BEGIN
  sky   := Pack(135, 206, 235);  (* light blue *)
  grass := Pack(34, 139, 34);    (* forest green *)
  pct   := 50;                   (* 50% blend *)

  r := Blend(UnpackR(sky), UnpackR(grass), pct);
  g := Blend(UnpackG(sky), UnpackG(grass), pct);
  b := Blend(UnpackB(sky), UnpackB(grass), pct);
  mixed := Pack(r, g, b);

  WriteString("R="); WriteInt(UnpackR(mixed), 0);
  WriteString(" G="); WriteInt(UnpackG(mixed), 0);
  WriteString(" B="); WriteInt(UnpackB(mixed), 0);
  WriteLn
END ColorDemo.
```
