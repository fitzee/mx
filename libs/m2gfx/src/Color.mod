IMPLEMENTATION MODULE Color;

(* Pure Modula-2 color pack/unpack using builtin bit operations.
   RGBA8888 layout: R bits 31..24, G 23..16, B 15..8, A 7..0. *)

PROCEDURE Pack(r, g, b: INTEGER): CARDINAL;
BEGIN
  RETURN BOR(BOR(BOR(SHL(CARDINAL(r), 24),
                     SHL(CARDINAL(g), 16)),
                 SHL(CARDINAL(b), 8)),
             0FFH)
END Pack;

PROCEDURE PackAlpha(r, g, b, a: INTEGER): CARDINAL;
BEGIN
  RETURN BOR(BOR(BOR(SHL(CARDINAL(r), 24),
                     SHL(CARDINAL(g), 16)),
                 SHL(CARDINAL(b), 8)),
             CARDINAL(a))
END PackAlpha;

PROCEDURE UnpackR(c: CARDINAL): INTEGER;
BEGIN
  RETURN INTEGER(BAND(SHR(c, 24), 0FFH))
END UnpackR;

PROCEDURE UnpackG(c: CARDINAL): INTEGER;
BEGIN
  RETURN INTEGER(BAND(SHR(c, 16), 0FFH))
END UnpackG;

PROCEDURE UnpackB(c: CARDINAL): INTEGER;
BEGIN
  RETURN INTEGER(BAND(SHR(c, 8), 0FFH))
END UnpackB;

PROCEDURE Blend(base, target, pct: INTEGER): INTEGER;
BEGIN
  RETURN base + (target - base) * pct DIV 100
END Blend;

END Color.
