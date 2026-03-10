MODULE GfxDemo;

(* m2gfx demo — draws shapes, responds to keyboard and mouse *)

FROM Gfx IMPORT Init, InitFont, Quit, QuitFont,
     CreateWindow, DestroyWindow, CreateRenderer, DestroyRenderer,
     Present, Ticks, Delay, WIN_CENTERED, WIN_RESIZABLE,
     RENDER_ACCELERATED, RENDER_VSYNC;
FROM Canvas IMPORT SetColor, Clear, DrawRect, FillRect,
     DrawRoundRect, FillRoundRect,
     DrawLine, DrawThickLine, DrawPoint,
     DrawCircle, FillCircle,
     DrawEllipse, FillEllipse,
     DrawTriangle, FillTriangle,
     DrawArc, DrawBezier, SetBlendMode, BLEND_ALPHA;
FROM Events IMPORT Poll, KeyCode, MouseX, MouseY, MouseButton,
     QUIT_EVENT, KEYDOWN, MOUSEDOWN, MOUSEMOVE,
     KEY_ESCAPE, KEY_SPACE, BUTTON_LEFT;
FROM Font IMPORT Open, Close, DrawText, TextWidth, Height,
     FontHandle;
FROM InOut IMPORT WriteString, WriteLn;

CONST
  WIDTH  = 800;
  HEIGHT = 600;

VAR
  win: ADDRESS;
  ren: ADDRESS;
  font: FontHandle;
  running: BOOLEAN;
  event: INTEGER;
  mx, my: INTEGER;
  frame: INTEGER;
  t: INTEGER;

PROCEDURE DrawScene;
VAR y, i: INTEGER;
    pulse: INTEGER;
BEGIN
  (* Dark background *)
  SetColor(ren, 20, 20, 30, 255);
  Clear(ren);

  SetBlendMode(ren, BLEND_ALPHA);

  (* Title text *)
  IF font # NIL THEN
    DrawText(ren, font, "m2gfx Demo", 20, 15, 255, 255, 255, 255);
    DrawText(ren, font, "ESC=quit  SPACE=toggle", 20, 45, 160, 160, 160, 255)
  END;

  (* Filled rectangles *)
  SetColor(ren, 200, 50, 50, 255);
  FillRect(ren, 40, 100, 120, 80);
  SetColor(ren, 50, 200, 50, 255);
  FillRect(ren, 180, 100, 120, 80);
  SetColor(ren, 50, 50, 200, 255);
  FillRect(ren, 320, 100, 120, 80);

  (* Outlined rectangles *)
  SetColor(ren, 255, 255, 0, 255);
  DrawRect(ren, 40, 100, 120, 80);
  DrawRect(ren, 180, 100, 120, 80);
  DrawRect(ren, 320, 100, 120, 80);

  (* Rounded rectangles *)
  SetColor(ren, 200, 100, 255, 200);
  FillRoundRect(ren, 480, 100, 140, 80, 15);
  SetColor(ren, 255, 200, 100, 255);
  DrawRoundRect(ren, 480, 100, 140, 80, 15);

  (* Circles *)
  SetColor(ren, 0, 200, 200, 255);
  FillCircle(ren, 100, 280, 50);
  SetColor(ren, 255, 255, 255, 255);
  DrawCircle(ren, 100, 280, 50);

  SetColor(ren, 200, 0, 200, 180);
  FillCircle(ren, 250, 280, 40);
  SetColor(ren, 255, 255, 255, 255);
  DrawCircle(ren, 250, 280, 40);

  (* Ellipses *)
  SetColor(ren, 100, 200, 50, 200);
  FillEllipse(ren, 420, 280, 80, 40);
  SetColor(ren, 255, 255, 255, 255);
  DrawEllipse(ren, 420, 280, 80, 40);

  (* Triangles *)
  SetColor(ren, 255, 100, 0, 200);
  FillTriangle(ren, 600, 230, 700, 330, 550, 330);
  SetColor(ren, 255, 255, 255, 255);
  DrawTriangle(ren, 600, 230, 700, 330, 550, 330);

  (* Lines *)
  y := 380;
  FOR i := 0 TO 7 DO
    SetColor(ren, 50 + i * 25, 200 - i * 20, 100 + i * 15, 255);
    DrawLine(ren, 40, y, 200, y + 100 - i * 12);
  END;

  (* Thick lines *)
  SetColor(ren, 255, 200, 50, 255);
  DrawThickLine(ren, 250, 380, 400, 480, 4);
  SetColor(ren, 50, 200, 255, 255);
  DrawThickLine(ren, 280, 380, 430, 480, 2);

  (* Arc *)
  SetColor(ren, 255, 150, 200, 255);
  DrawArc(ren, 550, 430, 60, 0, 270);

  (* Bezier curve *)
  SetColor(ren, 100, 255, 200, 255);
  DrawBezier(ren, 650, 380, 700, 500, 750, 350, 780, 480, 32);

  (* Mouse cursor indicator *)
  SetColor(ren, 255, 255, 255, 120);
  DrawCircle(ren, mx, my, 15);
  SetColor(ren, 255, 255, 255, 60);
  FillCircle(ren, mx, my, 12);

  (* Grid of points *)
  SetColor(ren, 100, 100, 100, 255);
  FOR i := 0 TO 19 DO
    FOR y := 0 TO 5 DO
      DrawPoint(ren, 40 + i * 8, 550 + y * 8)
    END
  END;

  Present(ren)
END DrawScene;

BEGIN
  mx := 0; my := 0;
  frame := 0;

  IF NOT Init() THEN
    WriteString("Failed to init SDL2"); WriteLn;
    HALT
  END;

  IF NOT InitFont() THEN
    WriteString("Failed to init SDL2_ttf"); WriteLn;
    Quit;
    HALT
  END;

  win := CreateWindow("m2gfx Demo", WIDTH, HEIGHT,
                       WIN_CENTERED + WIN_RESIZABLE);
  IF win = NIL THEN
    WriteString("Failed to create window"); WriteLn;
    QuitFont; Quit;
    HALT
  END;

  ren := CreateRenderer(win, RENDER_ACCELERATED + RENDER_VSYNC);
  IF ren = NIL THEN
    WriteString("Failed to create renderer"); WriteLn;
    DestroyWindow(win); QuitFont; Quit;
    HALT
  END;

  (* Try to load a system font *)
  font := Open("/System/Library/Fonts/Helvetica.ttc", 20);
  IF font = NIL THEN
    font := Open("/System/Library/Fonts/SFNSMono.ttf", 20)
  END;
  IF font = NIL THEN
    font := Open("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", 20)
  END;

  running := TRUE;
  WHILE running DO
    event := Poll();
    WHILE event # 0 DO
      IF event = QUIT_EVENT THEN
        running := FALSE
      ELSIF event = KEYDOWN THEN
        IF KeyCode() = KEY_ESCAPE THEN
          running := FALSE
        END
      ELSIF event = MOUSEMOVE THEN
        mx := MouseX();
        my := MouseY()
      END;
      event := Poll()
    END;

    DrawScene;
    INC(frame);
    Delay(1)
  END;

  IF font # NIL THEN Close(font) END;
  DestroyRenderer(ren);
  DestroyWindow(win);
  QuitFont;
  Quit
END GfxDemo.
