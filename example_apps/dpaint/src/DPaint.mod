MODULE DPaint;

(* ═══════════════════════════════════════════════════════════════════════
   DPaint — Amiga DeluxePaint-style pixel art editor
   ═══════════════════════════════════════════════════════════════════════
   Modula-2+ features:
     • EXCEPTION / RAISE
     • TRY / EXCEPT / FINALLY
     • REF types & NEW  (undo stack, zoom stack)

   Architecture:
     • 8-bit indexed pixel buffer (256-color palette)
     • Delta-based undo (saves affected region before each operation)
     • All drawing tools operate on the pixel buffer
     • Buffer rendered to SDL texture each frame

   Controls — see DrawMenuBar for on-screen help.
   ═══════════════════════════════════════════════════════════════════════ *)

FROM Gfx IMPORT Init, InitFont, Quit, QuitFont,
     CreateWindow, DestroyWindow, CreateRenderer, DestroyRenderer,
     Present, Delay, Ticks,
     WIN_CENTERED, WIN_RESIZABLE, WIN_HIGHDPI, RENDER_ACCELERATED, RENDER_VSYNC,
     FULLSCREEN_OFF, FULLSCREEN_DESKTOP,
     CURSOR_CROSSHAIR, CURSOR_ARROW, CURSOR_HAND, CURSOR_IBEAM,
     SetCursor, SetFullscreen, GetWindowWidth, GetWindowHeight;
FROM Canvas IMPORT SetColor, Clear, DrawRect, FillRect,
     DrawLine, DrawThickLine, DrawPoint,
     DrawCircle, FillCircle,
     DrawEllipse, FillEllipse,
     SetBlendMode, BLEND_ALPHA, BLEND_NONE;
FROM Events IMPORT Poll, KeyCode, KeyMod, MouseX, MouseY, MouseButton,
     QUIT_EVENT, KEYDOWN, MOUSEDOWN, MOUSEUP, MOUSEMOVE, MOUSEWHEEL,
     WINDOW_EVENT, WindowEvent, WEVT_RESIZED,
     WheelY,
     KEY_ESCAPE, BUTTON_LEFT, BUTTON_RIGHT, BUTTON_MIDDLE;
FROM Font IMPORT Open, Close, DrawText, FontHandle, TextWidth;
FROM Texture IMPORT Create, Destroy, Draw, DrawRegion,
     SetTarget, ResetTarget, Tex;
FROM PixBuf IMPORT PBuf, Region;
FROM SYSTEM IMPORT ADR;
FROM InOut IMPORT WriteString, WriteLn;

(* ── M2+ Exceptions ─────────────────────────────────────────── *)

EXCEPTION
  InitFailed;

(* ═══════════════════════════════════════════════════════════════
   Constants
   ═══════════════════════════════════════════════════════════════ *)

CONST
  WW = 1024; WH = 740;
  TBW   = 52;          (* toolbar width *)
  PALH  = 46;          (* palette bar height *)
  MBARH = 28;          (* menu bar height *)
  STATH = 22;          (* status bar height *)

  (* Menu IDs *)
  MNU_FILE = 0;  MNU_EDIT = 1;  MNU_TOOLS = 2;
  MNU_VIEW = 3;  MNU_SET  = 4;  MNU_HELP  = 5;
  NMENUS   = 6;
  MENULH   = 20;       (* item line height *)
  MENUSEPH = 10;       (* separator height *)
  MENUPAD  = 24;       (* left padding for label text *)
  MENUSHPAD = 16;      (* gap between label and shortcut *)

  (* Tool IDs *)
  T_PENCIL   = 0;  T_BRUSH    = 1;  T_SPRAY    = 2;
  T_LINE     = 3;  T_RECT     = 4;  T_FRECT    = 5;
  T_CIRCLE   = 6;  T_FCIRCLE  = 7;  T_ELLIPSE  = 8;
  T_GRADIENT = 9;  T_ERASER   = 10; T_FLOOD    = 11;
  T_EYEDROP  = 12; T_SELECT   = 13; T_TEXT     = 14;
  T_POLYGON  = 15; T_PATTERN  = 16; T_SYMM     = 17;
  T_LIGHTEN  = 18; T_DARKEN   = 19;
  T_BEZIER   = 20; T_AIRBRUSH = 21; T_SMUDGE   = 22;
  T_STAMP    = 23; T_REPLACE  = 24; T_GRADANG  = 25;
  T_MOVE     = 26;
  NTOOLS     = 27;

  NCOLORS   = 32;      (* active palette size *)
  MAX_THICK = 24;

  MAX_UNDO = 200;
  AUTOSAVE_MS = 60000;  (* autosave every 60 seconds *)
  STATUS_DURATION = 3000;  (* status message visible for 3 seconds *)

(* ═══════════════════════════════════════════════════════════════
   Types
   ═══════════════════════════════════════════════════════════════ *)

TYPE
  (* Undo entry — saves region before modification *)
  UndoRef = REF UndoRec;
  UndoRec = RECORD
    region: Region;     (* saved pixel data *)
    rx, ry: INTEGER;    (* position to restore *)
    layerIdx: INTEGER;  (* which layer this undo applies to *)
    next: UndoRef;
  END;

  (* Zoom stack *)
  ZoomRef = REF ZoomRec;
  ZoomRec = RECORD
    x, y, w, h: INTEGER;
    prev: ZoomRef;
  END;

(* ═══════════════════════════════════════════════════════════════
   Globals
   ═══════════════════════════════════════════════════════════════ *)

VAR
  win: ADDRESS;  ren: ADDRESS;
  font: FontHandle;  fontSm: FontHandle;
  canvas: ADDRESS;           (* SDL texture for display *)
  pb: PBuf;                  (* indexed pixel buffer *)
  canW, canH: INTEGER;

  running: BOOLEAN;
  curTool: INTEGER;
  fgIdx, bgIdx: INTEGER;
  lineThick: INTEGER;

  dragging: BOOLEAN;
  dx0, dy0: INTEGER;        (* drag start — canvas coords *)
  lpx, lpy: INTEGER;        (* last freehand point *)
  mx, my: INTEGER;           (* mouse screen coords *)

  undoHead: UndoRef;
  undoCount: INTEGER;
  redoHead: UndoRef;
  redoCount: INTEGER;

  (* Zoom *)
  zoomed: BOOLEAN;
  magnifyMode: BOOLEAN;
  zoomX, zoomY, zoomW, zoomH: INTEGER;
  zoomStack: ZoomRef;

  (* Selection *)
  hasSelection: BOOLEAN;
  selX, selY, selW, selH: INTEGER;
  selBuf: Region;            (* copy buffer *)

  (* Symmetry *)
  symmetryX, symmetryY: BOOLEAN;

  (* Grid *)
  showGrid: BOOLEAN;

  (* Pan *)
  panning: BOOLEAN;
  panStartX, panStartY: INTEGER;

  (* Polygon tool *)
  polyActive: BOOLEAN;

  (* Text tool *)
  textBuf: ARRAY [0..255] OF CHAR;
  textLen: INTEGER;
  textInputMode: BOOLEAN;

  rngState: INTEGER;

  (* Shift-constrained mode *)
  shiftDown: BOOLEAN;

  (* Bezier tool state — flat arrays for X and Y *)
  bezX: ARRAY [0..3] OF INTEGER;
  bezY: ARRAY [0..3] OF INTEGER;
  bezCount: INTEGER;

  (* Move selection state *)
  moveOldX, moveOldY: INTEGER;
  moveBuf: Region;

  (* Lasso state *)
  lassoActive: BOOLEAN;
  lassoCount: INTEGER;

  (* Pixel-perfect line mode *)
  pixelPerfect: BOOLEAN;

  (* Transparency *)
  transparentIdx: INTEGER;
  showTransparency: BOOLEAN;

  (* Layer panel *)
  showLayerPanel: BOOLEAN;
  displayBuf: PBuf;  (* flattened composite for rendering *)

  (* Autosave *)
  lastAutoSave: INTEGER;  (* last autosave tick *)
  dirty: BOOLEAN;         (* TRUE if canvas modified since last save *)

  (* Status message *)
  statusMsg: ARRAY [0..63] OF CHAR;
  statusTick: INTEGER;  (* when status message was set *)

  (* Tooltips *)
  hoverTool: INTEGER;      (* tool index under mouse, -1 if none *)
  hoverStart: INTEGER;     (* tick when hover began *)
  showTooltip: BOOLEAN;

  (* Theme *)
  darkTheme: BOOLEAN;
  thBgR, thBgG, thBgB: INTEGER;      (* panel background *)
  thBarR, thBarG, thBarB: INTEGER;    (* bar background *)
  thTxtR, thTxtG, thTxtB: INTEGER;   (* text color *)
  thHiR, thHiG, thHiB: INTEGER;      (* highlight *)
  thShR, thShG, thShB: INTEGER;      (* shadow *)
  thSelR, thSelG, thSelB: INTEGER;    (* selected accent *)

  (* Overlay panels *)
  showShortcuts: BOOLEAN;
  showPalEdit: BOOLEAN;
  palEditIdx: INTEGER;     (* which palette entry is being edited *)
  palEditR, palEditG, palEditB: INTEGER;
  showHistory: BOOLEAN;

  (* Fullscreen *)
  isFullscreen: BOOLEAN;

  (* Brush preview *)
  showBrushPreview: BOOLEAN;

  (* Window dimensions (may change on resize) *)
  winW, winH: INTEGER;

  (* Animation *)
  showFrameStrip: BOOLEAN;
  onionSkin: BOOLEAN;
  playingAnim: BOOLEAN;
  playTick: INTEGER;  (* last frame advance tick *)

  (* Preferences dialog *)
  showPrefs: BOOLEAN;

  (* Advanced features *)
  tileMode: BOOLEAN;
  tileW, tileH: INTEGER;
  brushBuf: Region;    (* captured brush stamp *)
  noiseBrush: BOOLEAN;
  showCRT: BOOLEAN;
  hamMode: INTEGER;    (* 0=off, 6=HAM6, 8=HAM8 *)
  copperEnabled: BOOLEAN;

  (* Menu state *)
  menuOpen: INTEGER;       (* -1 = closed, 0..5 = open menu index *)
  menuHover: INTEGER;      (* -1 = no highlight, 0+ = hovered item *)
  menuTitleX: ARRAY [0..5] OF INTEGER;
  menuTitleW: ARRAY [0..5] OF INTEGER;

(* ═══════════════════════════════════════════════════════════════
   Utilities
   ═══════════════════════════════════════════════════════════════ *)

PROCEDURE MinI(a, b: INTEGER): INTEGER;
BEGIN IF a < b THEN RETURN a ELSE RETURN b END END MinI;

PROCEDURE MaxI(a, b: INTEGER): INTEGER;
BEGIN IF a > b THEN RETURN a ELSE RETURN b END END MaxI;

PROCEDURE AbsI(a: INTEGER): INTEGER;
BEGIN IF a < 0 THEN RETURN -a ELSE RETURN a END END AbsI;

PROCEDURE Rand(): INTEGER;
BEGIN
  rngState := (rngState * 1103515 + 12345) MOD 65536;
  RETURN rngState
END Rand;

PROCEDURE RandRange(lo, hi: INTEGER): INTEGER;
BEGIN
  IF hi <= lo THEN RETURN lo END;
  RETURN lo + Rand() MOD (hi - lo + 1)
END RandRange;

PROCEDURE SetStatus(s: ARRAY OF CHAR);
VAR i: INTEGER;
BEGIN
  i := 0;
  WHILE (i < HIGH(s)) AND (i < 63) AND (s[i] # 0C) DO
    statusMsg[i] := s[i];
    INC(i)
  END;
  statusMsg[i] := 0C;
  statusTick := Ticks()
END SetStatus;

PROCEDURE ApplyTheme;
BEGIN
  IF darkTheme THEN
    (* Amiga-inspired dark theme — original default *)
    thBgR := 40;  thBgG := 50;  thBgB := 70;
    thBarR := 55; thBarG := 65; thBarB := 90;
    thTxtR := 230; thTxtG := 230; thTxtB := 230;
    thHiR := 140;  thHiG := 150;  thHiB := 170;
    thShR := 30;  thShG := 35;  thShB := 50;
    thSelR := 220; thSelG := 160; thSelB := 50
  ELSE
    (* Light theme *)
    thBgR := 180; thBgG := 180; thBgB := 185;
    thBarR := 200; thBarG := 200; thBarB := 205;
    thTxtR := 20;  thTxtG := 20;  thTxtB := 25;
    thHiR := 235; thHiG := 235; thHiB := 240;
    thShR := 120; thShG := 120; thShB := 130;
    thSelR := 60;  thSelG := 100; thSelB := 200
  END
END ApplyTheme;

(* Config keys *)
CONST
  CK_THEME   = 1;  CK_FGIDX   = 2;  CK_BGIDX   = 3;
  CK_THICK   = 4;  CK_TOOL    = 5;  CK_GRID    = 6;
  CK_PXPERF  = 7;  CK_TILEW   = 8;  CK_TILEH   = 9;
  CK_SHOWBP  = 10; CK_SHOWCRT = 11;
  NCFG = 11;

PROCEDURE SaveConfig;
VAR keys: ARRAY [0..15] OF INTEGER;
    vals: ARRAY [0..15] OF INTEGER;
BEGIN
  keys[0] := CK_THEME;   vals[0] := ORD(darkTheme);
  keys[1] := CK_FGIDX;   vals[1] := fgIdx;
  keys[2] := CK_BGIDX;   vals[2] := bgIdx;
  keys[3] := CK_THICK;   vals[3] := lineThick;
  keys[4] := CK_TOOL;    vals[4] := curTool;
  keys[5] := CK_GRID;    vals[5] := ORD(showGrid);
  keys[6] := CK_PXPERF;  vals[6] := ORD(pixelPerfect);
  keys[7] := CK_TILEW;   vals[7] := tileW;
  keys[8] := CK_TILEH;   vals[8] := tileH;
  keys[9] := CK_SHOWBP;  vals[9] := ORD(showBrushPreview);
  keys[10] := CK_SHOWCRT; vals[10] := ORD(showCRT);
  IF PixBuf.ConfigSave("dpaint.cfg", keys, vals, NCFG) THEN
    SetStatus("Config saved")
  END
END SaveConfig;

PROCEDURE LoadConfig;
VAR keys: ARRAY [0..15] OF INTEGER;
    vals: ARRAY [0..15] OF INTEGER;
    n, i: INTEGER;
BEGIN
  n := PixBuf.ConfigLoad("dpaint.cfg", keys, vals, 16);
  FOR i := 0 TO n - 1 DO
    IF keys[i] = CK_THEME THEN
      darkTheme := vals[i] # 0;  ApplyTheme
    ELSIF keys[i] = CK_FGIDX THEN fgIdx := vals[i]
    ELSIF keys[i] = CK_BGIDX THEN bgIdx := vals[i]
    ELSIF keys[i] = CK_THICK THEN lineThick := vals[i]
    ELSIF keys[i] = CK_TOOL THEN curTool := vals[i]
    ELSIF keys[i] = CK_GRID THEN showGrid := vals[i] # 0
    ELSIF keys[i] = CK_PXPERF THEN pixelPerfect := vals[i] # 0
    ELSIF keys[i] = CK_TILEW THEN tileW := vals[i]
    ELSIF keys[i] = CK_TILEH THEN tileH := vals[i]
    ELSIF keys[i] = CK_SHOWBP THEN showBrushPreview := vals[i] # 0
    ELSIF keys[i] = CK_SHOWCRT THEN showCRT := vals[i] # 0
    END
  END;
  IF n > 0 THEN SetStatus("Config loaded") END
END LoadConfig;

PROCEDURE FgR(): INTEGER;
BEGIN RETURN PixBuf.PalR(pb, fgIdx) END FgR;
PROCEDURE FgG(): INTEGER;
BEGIN RETURN PixBuf.PalG(pb, fgIdx) END FgG;
PROCEDURE FgB(): INTEGER;
BEGIN RETURN PixBuf.PalB(pb, fgIdx) END FgB;
PROCEDURE BgR(): INTEGER;
BEGIN RETURN PixBuf.PalR(pb, bgIdx) END BgR;
PROCEDURE BgG(): INTEGER;
BEGIN RETURN PixBuf.PalG(pb, bgIdx) END BgG;
PROCEDURE BgB(): INTEGER;
BEGIN RETURN PixBuf.PalB(pb, bgIdx) END BgB;

(* ── Coordinate conversion ─────────────────────────────────── *)

PROCEDURE ScreenToCanvas(sx, sy: INTEGER; VAR cx, cy: INTEGER);
BEGIN
  IF zoomed THEN
    cx := zoomX + (sx - TBW) * zoomW DIV canW;
    cy := zoomY + (sy - MBARH) * zoomH DIV canH
  ELSE
    cx := sx - TBW;
    cy := sy - MBARH
  END
END ScreenToCanvas;

PROCEDURE CanvasToScreen(cx, cy: INTEGER; VAR sx, sy: INTEGER);
BEGIN
  IF zoomed THEN
    sx := (cx - zoomX) * canW DIV zoomW + TBW;
    sy := (cy - zoomY) * canH DIV zoomH + MBARH
  ELSE
    sx := cx + TBW;
    sy := cy + MBARH
  END
END CanvasToScreen;

PROCEDURE InCanvas(sx, sy: INTEGER): BOOLEAN;
BEGIN
  RETURN (sx >= TBW) AND (sx < TBW + canW)
     AND (sy >= MBARH) AND (sy < MBARH + canH)
END InCanvas;

(* Constrain to 45-degree angles *)
PROCEDURE Constrain45(x0, y0: INTEGER; VAR x1, y1: INTEGER);
VAR dx, dy, adx, ady: INTEGER;
BEGIN
  dx := x1 - x0;  dy := y1 - y0;
  adx := AbsI(dx);  ady := AbsI(dy);
  IF adx > ady * 2 THEN
    y1 := y0     (* horizontal *)
  ELSIF ady > adx * 2 THEN
    x1 := x0     (* vertical *)
  ELSE
    (* 45 degrees — use the larger extent *)
    IF adx > ady THEN
      IF dy < 0 THEN y1 := y0 - adx ELSE y1 := y0 + adx END
    ELSE
      IF dx < 0 THEN x1 := x0 - ady ELSE x1 := x0 + ady END
    END
  END
END Constrain45;

(* ═══════════════════════════════════════════════════════════════
   Palette
   ═══════════════════════════════════════════════════════════════ *)

PROCEDURE InitPalette;
BEGIN
  PixBuf.SetPal(pb,  0,   0,   0,   0);  PixBuf.SetPal(pb,  1, 255, 255, 255);
  PixBuf.SetPal(pb,  2, 204,  51,  51);  PixBuf.SetPal(pb,  3,  51, 204, 204);
  PixBuf.SetPal(pb,  4, 153,  51, 204);  PixBuf.SetPal(pb,  5,  51, 170,  51);
  PixBuf.SetPal(pb,  6,  51,  51, 204);  PixBuf.SetPal(pb,  7, 238, 238,  51);
  PixBuf.SetPal(pb,  8, 238, 153,  51);  PixBuf.SetPal(pb,  9, 153, 102,  51);
  PixBuf.SetPal(pb, 10, 255, 153, 136);  PixBuf.SetPal(pb, 11,  68,  68,  68);
  PixBuf.SetPal(pb, 12, 119, 119, 119);  PixBuf.SetPal(pb, 13, 136, 255, 136);
  PixBuf.SetPal(pb, 14, 136, 136, 255);  PixBuf.SetPal(pb, 15, 187, 187, 187);
  PixBuf.SetPal(pb, 16,  34,  34,  85);  PixBuf.SetPal(pb, 17,  85, 153,  85);
  PixBuf.SetPal(pb, 18, 170,  85,  85);  PixBuf.SetPal(pb, 19, 204, 170, 102);
  PixBuf.SetPal(pb, 20,  85, 170, 204);  PixBuf.SetPal(pb, 21, 204, 102, 170);
  PixBuf.SetPal(pb, 22, 170, 204,  85);  PixBuf.SetPal(pb, 23, 102,  68, 153);
  PixBuf.SetPal(pb, 24, 255, 204,  68);  PixBuf.SetPal(pb, 25,  68, 153, 136);
  PixBuf.SetPal(pb, 26, 221, 136,  68);  PixBuf.SetPal(pb, 27, 136, 204, 170);
  PixBuf.SetPal(pb, 28, 187, 136, 204);  PixBuf.SetPal(pb, 29, 238, 204, 170);
  PixBuf.SetPal(pb, 30,  51, 119,  68);  PixBuf.SetPal(pb, 31, 170,  51,  51)
END InitPalette;

(* ═══════════════════════════════════════════════════════════════
   SDL Init / Cleanup — TRY / EXCEPT / FINALLY
   ═══════════════════════════════════════════════════════════════ *)

PROCEDURE InitGraphics;
BEGIN
  IF NOT Init() THEN RAISE InitFailed END;
  IF NOT InitFont() THEN Quit; RAISE InitFailed END;
  win := CreateWindow("DPaint M2+", WW, WH,
                       WIN_CENTERED + WIN_RESIZABLE + WIN_HIGHDPI);
  IF win = NIL THEN QuitFont; Quit; RAISE InitFailed END;
  ren := CreateRenderer(win, RENDER_ACCELERATED + RENDER_VSYNC);
  IF ren = NIL THEN
    DestroyWindow(win); QuitFont; Quit; RAISE InitFailed
  END;

  canW := WW - TBW;
  canH := WH - MBARH - PALH - STATH;
  canvas := Create(ren, canW, canH);
  IF canvas = NIL THEN
    DestroyRenderer(ren); DestroyWindow(win);
    QuitFont; Quit; RAISE InitFailed
  END;

  (* White canvas *)
  SetTarget(ren, canvas);
  SetColor(ren, 255, 255, 255, 255);
  Clear(ren);
  ResetTarget(ren);

  font := Open("/System/Library/Fonts/Helvetica.ttc", 13);
  IF font = NIL THEN
    font := Open("/System/Library/Fonts/SFNSMono.ttf", 13)
  END;
  IF font = NIL THEN
    font := Open("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", 13)
  END;
  fontSm := Open("/System/Library/Fonts/Helvetica.ttc", 11);
  IF fontSm = NIL THEN fontSm := font END;
  InitMenuPositions
END InitGraphics;

PROCEDURE Cleanup;
BEGIN
  IF (fontSm # NIL) AND (fontSm # font) THEN Close(fontSm) END;
  IF font   # NIL THEN Close(font)          END;
  IF canvas # NIL THEN Destroy(canvas)      END;
  IF displayBuf # NIL THEN PixBuf.Free(displayBuf) END;
  IF pb     # NIL THEN PixBuf.Free(pb)      END;
  IF ren    # NIL THEN DestroyRenderer(ren) END;
  IF win    # NIL THEN DestroyWindow(win)   END;
  QuitFont; Quit
END Cleanup;

(* ═══════════════════════════════════════════════════════════════
   Zoom stack — REF + NEW
   ═══════════════════════════════════════════════════════════════ *)

PROCEDURE PushZoom(x, y, w, h: INTEGER);
VAR node: ZoomRef;
BEGIN
  NEW(node);
  node^.x := zoomX;  node^.y := zoomY;
  node^.w := zoomW;  node^.h := zoomH;
  node^.prev := zoomStack;
  zoomStack := node;
  zoomX := x;  zoomY := y;  zoomW := w;  zoomH := h;
  zoomed := TRUE
END PushZoom;

PROCEDURE PopZoom;
BEGIN
  IF zoomStack # NIL THEN
    zoomX := zoomStack^.x;  zoomY := zoomStack^.y;
    zoomW := zoomStack^.w;  zoomH := zoomStack^.h;
    zoomStack := zoomStack^.prev;
    zoomed := (zoomW < canW) OR (zoomH < canH)
  ELSE
    zoomed := FALSE;
    zoomX := 0; zoomY := 0; zoomW := canW; zoomH := canH
  END
END PopZoom;

PROCEDURE ResetZoom;
BEGIN
  zoomed := FALSE;  magnifyMode := FALSE;
  zoomX := 0; zoomY := 0; zoomW := canW; zoomH := canH;
  zoomStack := NIL
END ResetZoom;

PROCEDURE ZoomToFit;
BEGIN
  ResetZoom
END ZoomToFit;

PROCEDURE ZoomTo1to1;
BEGIN
  IF canW <= PixBuf.Width(pb) THEN
    (* center the view *)
    zoomX := (PixBuf.Width(pb) - canW) DIV 2;
    zoomY := (PixBuf.Height(pb) - canH) DIV 2;
    zoomW := canW;  zoomH := canH;
    zoomed := FALSE
  END
END ZoomTo1to1;

PROCEDURE WheelZoom(dir, sx, sy: INTEGER);
VAR cx, cy, nw, nh: INTEGER;
BEGIN
  ScreenToCanvas(sx, sy, cx, cy);
  IF dir > 0 THEN
    (* zoom in *)
    nw := zoomW * 3 DIV 4;  nh := zoomH * 3 DIV 4;
    IF nw < 16 THEN nw := 16 END;
    IF nh < 16 THEN nh := 16 END
  ELSE
    (* zoom out *)
    nw := zoomW * 4 DIV 3;  nh := zoomH * 4 DIV 3;
    IF nw > PixBuf.Width(pb) THEN nw := PixBuf.Width(pb) END;
    IF nh > PixBuf.Height(pb) THEN nh := PixBuf.Height(pb) END
  END;
  (* Center on cursor *)
  zoomX := cx - nw DIV 2;
  zoomY := cy - nh DIV 2;
  zoomW := nw;  zoomH := nh;
  (* Clamp *)
  IF zoomX < 0 THEN zoomX := 0 END;
  IF zoomY < 0 THEN zoomY := 0 END;
  IF zoomX + zoomW > PixBuf.Width(pb) THEN
    zoomX := PixBuf.Width(pb) - zoomW END;
  IF zoomY + zoomH > PixBuf.Height(pb) THEN
    zoomY := PixBuf.Height(pb) - zoomH END;
  zoomed := (zoomW < PixBuf.Width(pb)) OR (zoomH < PixBuf.Height(pb))
END WheelZoom;

(* ═══════════════════════════════════════════════════════════════
   Undo — delta-based (saves affected pixel regions)
   ═══════════════════════════════════════════════════════════════ *)

PROCEDURE ClearRedo;
VAR tmp: UndoRef;
BEGIN
  WHILE redoHead # NIL DO
    tmp := redoHead;
    PixBuf.FreeSave(tmp^.region);
    redoHead := tmp^.next
  END;
  redoCount := 0
END ClearRedo;

PROCEDURE TrimUndo;
VAR node: UndoRef;
    i: INTEGER;
BEGIN
  IF undoCount <= MAX_UNDO THEN RETURN END;
  node := undoHead;
  i := 1;
  WHILE (node # NIL) AND (i < MAX_UNDO) DO
    node := node^.next;
    INC(i)
  END;
  (* node is now the last entry to keep; free everything after *)
  IF node # NIL THEN
    WHILE node^.next # NIL DO
      PixBuf.FreeSave(node^.next^.region);
      node^.next := node^.next^.next
    END
  END;
  undoCount := MAX_UNDO
END TrimUndo;

PROCEDURE PushUndo(x, y, w, h: INTEGER);
VAR node: UndoRef;
    reg: Region;
BEGIN
  reg := PixBuf.Save(pb, x, y, w, h);
  IF reg = NIL THEN RETURN END;
  ClearRedo;
  NEW(node);
  node^.region := reg;
  node^.rx := x;  node^.ry := y;
  node^.layerIdx := PixBuf.LayerActive();
  node^.next := undoHead;
  undoHead := node;
  INC(undoCount);
  TrimUndo;
  dirty := TRUE
END PushUndo;

PROCEDURE Undo;
VAR cur: UndoRef;
    reg: Region;
    targetPb: PBuf;
BEGIN
  IF undoHead = NIL THEN RETURN END;
  (* Switch to the layer this undo applies to *)
  targetPb := PixBuf.LayerGet(undoHead^.layerIdx);
  IF targetPb = NIL THEN targetPb := pb END;
  (* Save current state to redo before restoring *)
  reg := PixBuf.Save(targetPb, undoHead^.rx, undoHead^.ry,
                     PixBuf.SaveW(undoHead^.region),
                     PixBuf.SaveH(undoHead^.region));
  IF reg # NIL THEN
    NEW(cur);
    cur^.region := reg;
    cur^.rx := undoHead^.rx;  cur^.ry := undoHead^.ry;
    cur^.layerIdx := undoHead^.layerIdx;
    cur^.next := redoHead;
    redoHead := cur;
    INC(redoCount)
  END;
  PixBuf.Restore(targetPb, undoHead^.region, undoHead^.rx, undoHead^.ry);
  PixBuf.FreeSave(undoHead^.region);
  undoHead := undoHead^.next;
  DEC(undoCount)
END Undo;

PROCEDURE Redo;
VAR cur: UndoRef;
    reg: Region;
    targetPb: PBuf;
BEGIN
  IF redoHead = NIL THEN RETURN END;
  targetPb := PixBuf.LayerGet(redoHead^.layerIdx);
  IF targetPb = NIL THEN targetPb := pb END;
  (* Save current state to undo before re-applying *)
  reg := PixBuf.Save(targetPb, redoHead^.rx, redoHead^.ry,
                     PixBuf.SaveW(redoHead^.region),
                     PixBuf.SaveH(redoHead^.region));
  IF reg # NIL THEN
    NEW(cur);
    cur^.region := reg;
    cur^.rx := redoHead^.rx;  cur^.ry := redoHead^.ry;
    cur^.layerIdx := redoHead^.layerIdx;
    cur^.next := undoHead;
    undoHead := cur;
    INC(undoCount)
  END;
  PixBuf.Restore(targetPb, redoHead^.region, redoHead^.rx, redoHead^.ry);
  PixBuf.FreeSave(redoHead^.region);
  redoHead := redoHead^.next;
  DEC(redoCount)
END Redo;

(* ═══════════════════════════════════════════════════════════════
   Drawing on pixel buffer
   ═══════════════════════════════════════════════════════════════ *)

PROCEDURE ApplyFreehand(x1, y1, x2, y2, ci, thick: INTEGER);
VAR bx, bt, bw, bh, rr: INTEGER;
BEGIN
  rr := thick + 2;
  bx := MinI(x1, x2) - rr;  bt := MinI(y1, y2) - rr;
  bw := AbsI(x2 - x1) + rr * 2 + 1;
  bh := AbsI(y2 - y1) + rr * 2 + 1;
  PushUndo(bx, bt, bw, bh);

  IF curTool = T_BRUSH THEN
    PixBuf.ThickLine(pb, x1, y1, x2, y2, ci, thick * 2 + 2)
  ELSIF curTool = T_ERASER THEN
    PixBuf.ThickLine(pb, x1, y1, x2, y2, ci, thick * 4 + 6)
  ELSIF curTool = T_SPRAY THEN
    IF noiseBrush THEN
      (* Noise brush — scatter random pixels in radius *)
      PixBuf.SetPix(pb, x1 + RandRange(-thick, thick),
                    y1 + RandRange(-thick, thick), ci);
      PixBuf.SetPix(pb, x1 + RandRange(-thick, thick),
                    y1 + RandRange(-thick, thick), ci);
      PixBuf.SetPix(pb, x1 + RandRange(-thick, thick),
                    y1 + RandRange(-thick, thick), ci)
    ELSE
      PixBuf.FillCircle(pb, x1, y1, MaxI(thick DIV 2, 1), ci)
    END
  ELSIF curTool = T_LIGHTEN THEN
    PixBuf.FillCircle(pb, x1, y1, thick, ci)
  ELSIF curTool = T_DARKEN THEN
    PixBuf.FillCircle(pb, x1, y1, thick, ci)
  ELSIF pixelPerfect AND (thick <= 1) THEN
    PixBuf.LinePerfect(pb, x1, y1, x2, y2, ci)
  ELSE
    PixBuf.ThickLine(pb, x1, y1, x2, y2, ci, thick + 1)
  END;

  (* Mirror drawing *)
  IF symmetryX THEN
    PixBuf.ThickLine(pb, PixBuf.Width(pb) - 1 - x1, y1,
                     PixBuf.Width(pb) - 1 - x2, y2, ci, thick + 1)
  END;
  IF symmetryY THEN
    PixBuf.ThickLine(pb, x1, PixBuf.Height(pb) - 1 - y1,
                     x2, PixBuf.Height(pb) - 1 - y2, ci, thick + 1)
  END
END ApplyFreehand;

PROCEDURE ApplyShape(tool, x1, y1, x2, y2, ci, thick: INTEGER);
VAR cx, cy, rx, ry, bx, bt, bw, bh: INTEGER;
BEGIN
  bx := MinI(x1, x2) - thick;  bt := MinI(y1, y2) - thick;
  bw := AbsI(x2 - x1) + thick * 2 + 2;
  bh := AbsI(y2 - y1) + thick * 2 + 2;
  PushUndo(bx, bt, bw, bh);

  CASE tool OF
    T_LINE:
      PixBuf.ThickLine(pb, x1, y1, x2, y2, ci, thick) |
    T_RECT:
      PixBuf.Rect(pb, MinI(x1,x2), MinI(y1,y2),
                  AbsI(x2-x1), AbsI(y2-y1), ci) |
    T_FRECT:
      PixBuf.FillRect(pb, MinI(x1,x2), MinI(y1,y2),
                      AbsI(x2-x1), AbsI(y2-y1), ci) |
    T_CIRCLE:
      cx := (x1+x2) DIV 2;  cy := (y1+y2) DIV 2;
      PixBuf.Circle(pb, cx, cy, AbsI(x2-x1) DIV 2, ci) |
    T_FCIRCLE:
      cx := (x1+x2) DIV 2;  cy := (y1+y2) DIV 2;
      PixBuf.FillCircle(pb, cx, cy, AbsI(x2-x1) DIV 2, ci) |
    T_ELLIPSE:
      cx := (x1+x2) DIV 2;  cy := (y1+y2) DIV 2;
      rx := AbsI(x2-x1) DIV 2;  ry := AbsI(y2-y1) DIV 2;
      PixBuf.Ellipse(pb, cx, cy, rx, ry, ci) |
    T_GRADIENT:
      PixBuf.Gradient(pb, MinI(x1,x2), MinI(y1,y2),
                      AbsI(x2-x1), AbsI(y2-y1),
                      fgIdx, bgIdx, (AbsI(x2-x1) >= AbsI(y2-y1)),
                      NCOLORS) |
    T_PATTERN:
      PixBuf.PatternFill(pb, MinI(x1,x2), MinI(y1,y2),
                         AbsI(x2-x1), AbsI(y2-y1),
                         fgIdx, bgIdx, lineThick)
  ELSE
  END
END ApplyShape;

(* ═══════════════════════════════════════════════════════════════
   UI Chrome — Amiga-style 3D beveled
   ═══════════════════════════════════════════════════════════════ *)

(* UI colors now driven by theme variables th* *)

PROCEDURE Bevel(x, y, w, h: INTEGER; raised: BOOLEAN);
VAR hr, hg, hb, sr, sg, sb: INTEGER;
BEGIN
  IF raised THEN
    hr := thHiR; hg := thHiG; hb := thHiB;
    sr := thShR; sg := thShG; sb := thShB
  ELSE
    sr := thHiR; sg := thHiG; sb := thHiB;
    hr := thShR; hg := thShG; hb := thShB
  END;
  SetColor(ren, hr, hg, hb, 255);
  DrawLine(ren, x, y, x+w-1, y);
  DrawLine(ren, x, y, x, y+h-1);
  SetColor(ren, sr, sg, sb, 255);
  DrawLine(ren, x+w-1, y, x+w-1, y+h-1);
  DrawLine(ren, x, y+h-1, x+w-1, y+h-1)
END Bevel;

(* ── Tool names for status bar ─────────────────────────────── *)

PROCEDURE ToolName(t: INTEGER; VAR name: ARRAY OF CHAR);
BEGIN
  CASE t OF
    T_PENCIL:   name := "Pencil" |
    T_BRUSH:    name := "Brush" |
    T_SPRAY:    name := "Spray" |
    T_LINE:     name := "Line" |
    T_RECT:     name := "Rect" |
    T_FRECT:    name := "Fill Rect" |
    T_CIRCLE:   name := "Circle" |
    T_FCIRCLE:  name := "Fill Circ" |
    T_ELLIPSE:  name := "Ellipse" |
    T_GRADIENT: name := "Gradient" |
    T_ERASER:   name := "Eraser" |
    T_FLOOD:    name := "Flood" |
    T_EYEDROP:  name := "Eyedrop" |
    T_SELECT:   name := "Select" |
    T_TEXT:     name := "Text" |
    T_POLYGON:  name := "Polygon" |
    T_PATTERN:  name := "Pattern" |
    T_SYMM:     name := "Symmetry" |
    T_LIGHTEN:  name := "Lighten" |
    T_DARKEN:   name := "Darken" |
    T_BEZIER:   name := "Bezier" |
    T_AIRBRUSH: name := "Airbrush" |
    T_SMUDGE:   name := "Smudge" |
    T_STAMP:    name := "Stamp" |
    T_REPLACE:  name := "Replace" |
    T_GRADANG:  name := "AngGrad" |
    T_MOVE:     name := "Move"
  ELSE
    name := "???"
  END
END ToolName;

(* ─── Menu system data & layout ─────────────────────────────── *)

PROCEDURE StrCopy(src: ARRAY OF CHAR; VAR dst: ARRAY OF CHAR);
VAR i: INTEGER;
BEGIN
  i := 0;
  WHILE (i < HIGH(src)) AND (i < HIGH(dst)) AND (src[i] # 0C) DO
    dst[i] := src[i];
    INC(i)
  END;
  dst[i] := 0C
END StrCopy;

PROCEDURE MenuItemCount(menu: INTEGER): INTEGER;
BEGIN
  CASE menu OF
    MNU_FILE:  RETURN 11 |
    MNU_EDIT:  RETURN 13 |
    MNU_TOOLS: RETURN 31 |
    MNU_VIEW:  RETURN 15 |
    MNU_SET:   RETURN  9 |
    MNU_HELP:  RETURN  2
  ELSE RETURN 0
  END
END MenuItemCount;

PROCEDURE MenuIsSep(menu, item: INTEGER): BOOLEAN;
BEGIN
  CASE menu OF
    MNU_FILE:  RETURN (item = 2) OR (item = 7) OR (item = 9) |
    MNU_EDIT:  RETURN (item = 2) OR (item = 5) OR (item = 9) |
    MNU_TOOLS: RETURN (item = 3) OR (item = 10) OR (item = 17) OR (item = 23) |
    MNU_VIEW:  RETURN (item = 3) OR (item = 8) OR (item = 11) |
    MNU_SET:   RETURN (item = 4) OR (item = 7)
  ELSE RETURN FALSE
  END
END MenuIsSep;

PROCEDURE MenuIsToggle(menu, item: INTEGER): BOOLEAN;
BEGIN
  CASE menu OF
    MNU_VIEW:
      RETURN (item = 4) OR (item = 5) OR (item = 6) OR (item = 7)
          OR (item = 9) OR (item = 10) OR (item = 12) OR (item = 13)
          OR (item = 14) |
    MNU_SET:
      RETURN (item = 1) OR (item = 2) OR (item = 3)
          OR (item = 5) OR (item = 6)
  ELSE RETURN FALSE
  END
END MenuIsToggle;

PROCEDURE MenuItemChecked(menu, item: INTEGER): BOOLEAN;
BEGIN
  CASE menu OF
    MNU_VIEW:
      CASE item OF
        4:  RETURN showGrid |
        5:  RETURN symmetryX |
        6:  RETURN symmetryY |
        7:  RETURN pixelPerfect |
        9:  RETURN tileMode |
        10: RETURN isFullscreen |
        12: RETURN showLayerPanel |
        13: RETURN showHistory |
        14: RETURN showFrameStrip
      ELSE RETURN FALSE
      END |
    MNU_SET:
      CASE item OF
        1: RETURN showCRT |
        2: RETURN hamMode > 0 |
        3: RETURN copperEnabled |
        5: RETURN noiseBrush |
        6: RETURN onionSkin
      ELSE RETURN FALSE
      END
  ELSE RETURN FALSE
  END
END MenuItemChecked;

PROCEDURE MenuItemLabel(menu, item: INTEGER; VAR s: ARRAY OF CHAR);
BEGIN
  CASE menu OF
    MNU_FILE:
      CASE item OF
        0: StrCopy("New", s) |
        1: StrCopy("Open", s) |
        3: StrCopy("Save Project", s) |
        4: StrCopy("Save BMP", s) |
        5: StrCopy("Export PNG", s) |
        6: StrCopy("Save Palette", s) |
        8: StrCopy("Save Config", s) |
        10: StrCopy("Quit", s)
      ELSE s[0] := 0C
      END |
    MNU_EDIT:
      CASE item OF
        0: StrCopy("Undo", s) |
        1: StrCopy("Redo", s) |
        3: StrCopy("Clear Canvas", s) |
        4: StrCopy("Swap Colors", s) |
        6: StrCopy("Copy", s) |
        7: StrCopy("Paste", s) |
        8: StrCopy("Delete", s) |
        10: StrCopy("Flip H", s) |
        11: StrCopy("Flip V", s) |
        12: StrCopy("Rotate 90", s)
      ELSE s[0] := 0C
      END |
    MNU_TOOLS:
      CASE item OF
        0: StrCopy("Pencil", s) |
        1: StrCopy("Brush", s) |
        2: StrCopy("Spray", s) |
        4: StrCopy("Line", s) |
        5: StrCopy("Rect", s) |
        6: StrCopy("Fill Rect", s) |
        7: StrCopy("Circle", s) |
        8: StrCopy("Fill Circ", s) |
        9: StrCopy("Ellipse", s) |
        11: StrCopy("Gradient", s) |
        12: StrCopy("Eraser", s) |
        13: StrCopy("Flood Fill", s) |
        14: StrCopy("Eyedropper", s) |
        15: StrCopy("Select", s) |
        16: StrCopy("Text", s) |
        18: StrCopy("Polygon", s) |
        19: StrCopy("Pattern", s) |
        20: StrCopy("Bezier", s) |
        21: StrCopy("Airbrush", s) |
        22: StrCopy("Smudge", s) |
        24: StrCopy("Lighten", s) |
        25: StrCopy("Darken", s) |
        26: StrCopy("Stamp", s) |
        27: StrCopy("Replace", s) |
        28: StrCopy("Ang Grad", s) |
        29: StrCopy("Move", s) |
        30: StrCopy("Symmetry", s)
      ELSE s[0] := 0C
      END |
    MNU_VIEW:
      CASE item OF
        0: StrCopy("Zoom Fit", s) |
        1: StrCopy("Zoom In", s) |
        2: StrCopy("Zoom Out", s) |
        4: StrCopy("Grid", s) |
        5: StrCopy("X Symmetry", s) |
        6: StrCopy("Y Symmetry", s) |
        7: StrCopy("Pixel Perfect", s) |
        9: StrCopy("Tile Mode", s) |
        10: StrCopy("Fullscreen", s) |
        12: StrCopy("Layers", s) |
        13: StrCopy("History", s) |
        14: StrCopy("Frame Strip", s)
      ELSE s[0] := 0C
      END |
    MNU_SET:
      CASE item OF
        0: StrCopy("Theme", s) |
        1: StrCopy("CRT Scanlines", s) |
        2: StrCopy("HAM Mode", s) |
        3: StrCopy("Copper", s) |
        5: StrCopy("Noise Brush", s) |
        6: StrCopy("Onion Skin", s) |
        8: StrCopy("Preferences", s)
      ELSE s[0] := 0C
      END |
    MNU_HELP:
      CASE item OF
        0: StrCopy("Shortcuts", s) |
        1: StrCopy("About", s)
      ELSE s[0] := 0C
      END
  ELSE s[0] := 0C
  END
END MenuItemLabel;

PROCEDURE MenuItemShortcut(menu, item: INTEGER; VAR s: ARRAY OF CHAR);
BEGIN
  CASE menu OF
    MNU_FILE:
      CASE item OF
        1: StrCopy("Ctrl+O", s) |
        3: StrCopy("Ctrl+S", s) |
        4: StrCopy("s", s) |
        5: StrCopy("Shift+S", s) |
        6: StrCopy("Ctrl+P", s) |
        8: StrCopy("Ctrl+K", s) |
        10: StrCopy("Esc", s)
      ELSE s[0] := 0C
      END |
    MNU_EDIT:
      CASE item OF
        0: StrCopy("z", s) |
        1: StrCopy("r", s) |
        3: StrCopy("c", s) |
        4: StrCopy("=", s) |
        6: StrCopy("Ctrl+C", s) |
        7: StrCopy("v", s) |
        8: StrCopy("Del", s) |
        10: StrCopy("Ctrl+H", s) |
        11: StrCopy("Ctrl+V", s) |
        12: StrCopy("Ctrl+R", s)
      ELSE s[0] := 0C
      END |
    MNU_TOOLS:
      CASE item OF
        0: StrCopy("1", s) |
        1: StrCopy("2", s) |
        2: StrCopy("3", s) |
        4: StrCopy("4", s) |
        5: StrCopy("5", s) |
        6: StrCopy("6", s) |
        7: StrCopy("7", s) |
        8: StrCopy("8", s) |
        9: StrCopy("9", s) |
        11: StrCopy("g", s) |
        12: StrCopy("e", s) |
        13: StrCopy("f", s) |
        14: StrCopy("i", s) |
        16: StrCopy("t", s) |
        18: StrCopy("p", s)
      ELSE s[0] := 0C
      END |
    MNU_VIEW:
      CASE item OF
        0: StrCopy("0", s) |
        1: StrCopy("m", s) |
        2: StrCopy("n", s) |
        4: StrCopy("d", s) |
        5: StrCopy("x", s) |
        6: StrCopy("y", s) |
        7: StrCopy("w", s) |
        9: StrCopy("F2", s) |
        10: StrCopy("F11", s) |
        12: StrCopy("l", s) |
        13: StrCopy("h", s) |
        14: StrCopy("a", s)
      ELSE s[0] := 0C
      END |
    MNU_SET:
      CASE item OF
        0: StrCopy("Ctrl+T", s) |
        1: StrCopy("F5", s) |
        2: StrCopy("F3", s) |
        3: StrCopy("F4", s) |
        5: StrCopy("Shift+B", s) |
        6: StrCopy("o", s) |
        8: StrCopy("F6", s)
      ELSE s[0] := 0C
      END |
    MNU_HELP:
      CASE item OF
        0: StrCopy("?", s)
      ELSE s[0] := 0C
      END
  ELSE s[0] := 0C
  END
END MenuItemShortcut;

PROCEDURE MenuToolId(item: INTEGER): INTEGER;
BEGIN
  CASE item OF
    0: RETURN T_PENCIL |
    1: RETURN T_BRUSH |
    2: RETURN T_SPRAY |
    4: RETURN T_LINE |
    5: RETURN T_RECT |
    6: RETURN T_FRECT |
    7: RETURN T_CIRCLE |
    8: RETURN T_FCIRCLE |
    9: RETURN T_ELLIPSE |
    11: RETURN T_GRADIENT |
    12: RETURN T_ERASER |
    13: RETURN T_FLOOD |
    14: RETURN T_EYEDROP |
    15: RETURN T_SELECT |
    16: RETURN T_TEXT |
    18: RETURN T_POLYGON |
    19: RETURN T_PATTERN |
    20: RETURN T_BEZIER |
    21: RETURN T_AIRBRUSH |
    22: RETURN T_SMUDGE |
    24: RETURN T_LIGHTEN |
    25: RETURN T_DARKEN |
    26: RETURN T_STAMP |
    27: RETURN T_REPLACE |
    28: RETURN T_GRADANG |
    29: RETURN T_MOVE |
    30: RETURN T_SYMM
  ELSE RETURN T_PENCIL
  END
END MenuToolId;

PROCEDURE InitMenuPositions;
VAR i, x, tw: INTEGER;
    title: ARRAY [0..15] OF CHAR;
BEGIN
  x := 8;
  FOR i := 0 TO NMENUS - 1 DO
    CASE i OF
      MNU_FILE:  StrCopy("File", title) |
      MNU_EDIT:  StrCopy("Edit", title) |
      MNU_TOOLS: StrCopy("Tools", title) |
      MNU_VIEW:  StrCopy("View", title) |
      MNU_SET:   StrCopy("Settings", title) |
      MNU_HELP:  StrCopy("Help", title)
    ELSE StrCopy("?", title)
    END;
    menuTitleX[i] := x;
    IF fontSm # NIL THEN
      tw := TextWidth(fontSm, title)
    ELSE
      tw := 40
    END;
    menuTitleW[i] := tw + 16;
    x := x + tw + 16
  END
END InitMenuPositions;

PROCEDURE MenuDropdownWidth(menu: INTEGER): INTEGER;
VAR i, n, w, lw, sw, maxW: INTEGER;
    lbl: ARRAY [0..31] OF CHAR;
    sc:  ARRAY [0..15] OF CHAR;
BEGIN
  n := MenuItemCount(menu);
  maxW := 80;
  FOR i := 0 TO n - 1 DO
    IF NOT MenuIsSep(menu, i) THEN
      MenuItemLabel(menu, i, lbl);
      MenuItemShortcut(menu, i, sc);
      IF fontSm # NIL THEN
        lw := TextWidth(fontSm, lbl);
        sw := TextWidth(fontSm, sc)
      ELSE
        lw := 60;  sw := 20
      END;
      w := MENUPAD + lw + MENUSHPAD + sw + 16;
      IF w > maxW THEN maxW := w END
    END
  END;
  RETURN maxW
END MenuDropdownWidth;

PROCEDURE MenuDropdownHeight(menu: INTEGER): INTEGER;
VAR i, n, h: INTEGER;
BEGIN
  n := MenuItemCount(menu);
  h := 4;
  FOR i := 0 TO n - 1 DO
    IF MenuIsSep(menu, i) THEN
      h := h + MENUSEPH
    ELSE
      h := h + MENULH
    END
  END;
  RETURN h + 4
END MenuDropdownHeight;

PROCEDURE MenuItemYOffset(menu, item: INTEGER): INTEGER;
VAR i, y: INTEGER;
BEGIN
  y := 2;
  FOR i := 0 TO item - 1 DO
    IF MenuIsSep(menu, i) THEN
      y := y + MENUSEPH
    ELSE
      y := y + MENULH
    END
  END;
  RETURN y
END MenuItemYOffset;

PROCEDURE MenuItemAtY(menu, localY: INTEGER): INTEGER;
VAR i, n, y: INTEGER;
BEGIN
  n := MenuItemCount(menu);
  y := 2;
  FOR i := 0 TO n - 1 DO
    IF MenuIsSep(menu, i) THEN
      IF (localY >= y) AND (localY < y + MENUSEPH) THEN RETURN -1 END;
      y := y + MENUSEPH
    ELSE
      IF (localY >= y) AND (localY < y + MENULH) THEN RETURN i END;
      y := y + MENULH
    END
  END;
  RETURN -1
END MenuItemAtY;

PROCEDURE DrawToolIcon(x, y, tool: INTEGER);
VAR cx, cy: INTEGER;
BEGIN
  cx := x + 17;  cy := y + 14;
  SetColor(ren, thTxtR, thTxtG, thTxtB, 255);
  CASE tool OF
    T_PENCIL:
      DrawLine(ren, cx-6, cy+6, cx+2, cy-2);
      FillCircle(ren, cx+3, cy-3, 2) |
    T_BRUSH:
      FillCircle(ren, cx, cy, 6) |
    T_SPRAY:
      DrawPoint(ren, cx-3, cy-4); DrawPoint(ren, cx+2, cy-5);
      DrawPoint(ren, cx+5, cy-1); DrawPoint(ren, cx-4, cy+2);
      DrawPoint(ren, cx+1, cy+4); DrawPoint(ren, cx, cy-2);
      FillCircle(ren, cx, cy, 2) |
    T_LINE:
      DrawThickLine(ren, cx-8, cy+5, cx+8, cy-5, 2) |
    T_RECT:
      DrawRect(ren, cx-8, cy-5, 16, 10) |
    T_FRECT:
      FillRect(ren, cx-8, cy-5, 16, 10) |
    T_CIRCLE:
      DrawCircle(ren, cx, cy, 7) |
    T_FCIRCLE:
      FillCircle(ren, cx, cy, 7) |
    T_ELLIPSE:
      DrawEllipse(ren, cx, cy, 9, 5) |
    T_GRADIENT:
      SetColor(ren, 220, 220, 255, 255);
      FillRect(ren, cx-8, cy-5, 5, 10);
      SetColor(ren, 140, 140, 200, 255);
      FillRect(ren, cx-3, cy-5, 6, 10);
      SetColor(ren, 60, 60, 140, 255);
      FillRect(ren, cx+3, cy-5, 5, 10) |
    T_ERASER:
      FillRect(ren, cx-5, cy-3, 10, 6);
      SetColor(ren, thShR, thShG, thShB, 255);
      DrawRect(ren, cx-5, cy-3, 10, 6) |
    T_FLOOD:
      (* bucket icon *)
      SetColor(ren, thTxtR, thTxtG, thTxtB, 255);
      FillRect(ren, cx-4, cy-2, 8, 7);
      DrawLine(ren, cx-4, cy-3, cx+3, cy-3);
      DrawLine(ren, cx+5, cy-5, cx+5, cy+2) |
    T_EYEDROP:
      DrawLine(ren, cx-4, cy+4, cx+2, cy-2);
      FillCircle(ren, cx+3, cy-3, 2) |
    T_SELECT:
      SetColor(ren, thTxtR, thTxtG, thTxtB, 200);
      DrawRect(ren, cx-7, cy-5, 14, 10) |
    T_TEXT:
      IF font # NIL THEN
        DrawText(ren, font, "A", cx-4, cy-6, thTxtR, thTxtG, thTxtB, 255)
      END |
    T_POLYGON:
      DrawLine(ren, cx-6, cy+4, cx+6, cy+4);
      DrawLine(ren, cx+6, cy+4, cx+3, cy-5);
      DrawLine(ren, cx+3, cy-5, cx-3, cy-5);
      DrawLine(ren, cx-3, cy-5, cx-6, cy+4) |
    T_PATTERN:
      (* checkerboard *)
      FillRect(ren, cx-6, cy-5, 4, 4);
      FillRect(ren, cx-2, cy-1, 4, 4);
      FillRect(ren, cx+2, cy-5, 4, 4);
      FillRect(ren, cx-6, cy+1, 4, 4) |
    T_SYMM:
      DrawLine(ren, cx, cy-7, cx, cy+7);
      DrawLine(ren, cx-5, cy-3, cx-5, cy+3);
      DrawLine(ren, cx+5, cy-3, cx+5, cy+3) |
    T_LIGHTEN:
      SetColor(ren, 255, 255, 200, 255);
      FillCircle(ren, cx, cy, 5) |
    T_DARKEN:
      SetColor(ren, 60, 60, 80, 255);
      FillCircle(ren, cx, cy, 5) |
    T_BEZIER:
      DrawLine(ren, cx-8, cy+4, cx-2, cy-6);
      DrawLine(ren, cx-2, cy-6, cx+4, cy-2);
      DrawLine(ren, cx+4, cy-2, cx+8, cy+4) |
    T_AIRBRUSH:
      DrawPoint(ren, cx-4, cy-3); DrawPoint(ren, cx+3, cy-4);
      DrawPoint(ren, cx+6, cy); DrawPoint(ren, cx-5, cy+3);
      DrawPoint(ren, cx+2, cy+5); DrawPoint(ren, cx-1, cy-1);
      DrawPoint(ren, cx+1, cy+2); DrawPoint(ren, cx-3, cy+1);
      FillCircle(ren, cx, cy, 3) |
    T_SMUDGE:
      SetColor(ren, 180, 160, 140, 255);
      FillCircle(ren, cx, cy, 5);
      SetColor(ren, thTxtR, thTxtG, thTxtB, 255);
      DrawLine(ren, cx-2, cy, cx+6, cy-4) |
    T_STAMP:
      DrawRect(ren, cx-6, cy-5, 12, 10);
      DrawLine(ren, cx, cy-8, cx, cy-5);
      DrawLine(ren, cx-3, cy-8, cx+3, cy-8) |
    T_REPLACE:
      SetColor(ren, 200, 80, 80, 255);
      FillCircle(ren, cx-3, cy, 4);
      SetColor(ren, 80, 200, 80, 255);
      FillCircle(ren, cx+3, cy, 4);
      SetColor(ren, thTxtR, thTxtG, thTxtB, 255);
      DrawLine(ren, cx-1, cy-2, cx+1, cy+2) |
    T_GRADANG:
      SetColor(ren, 220, 220, 255, 255);
      FillRect(ren, cx-7, cy-5, 4, 10);
      SetColor(ren, 140, 140, 200, 255);
      FillRect(ren, cx-3, cy-5, 4, 10);
      SetColor(ren, 60, 60, 140, 255);
      FillRect(ren, cx+1, cy-5, 4, 10);
      SetColor(ren, thTxtR, thTxtG, thTxtB, 255);
      DrawLine(ren, cx+5, cy-5, cx+7, cy+5) |
    T_MOVE:
      DrawLine(ren, cx, cy-7, cx, cy+7);
      DrawLine(ren, cx-7, cy, cx+7, cy);
      DrawLine(ren, cx, cy-7, cx-3, cy-4);
      DrawLine(ren, cx, cy-7, cx+3, cy-4);
      DrawLine(ren, cx+7, cy, cx+4, cy-3);
      DrawLine(ren, cx+7, cy, cx+4, cy+3)
  ELSE
  END
END DrawToolIcon;

PROCEDURE DrawToolbar;
VAR i, bx, bty, bw, bh: INTEGER;
BEGIN
  bw := 40;  bh := 28;
  bx := (TBW - bw) DIV 2;

  SetColor(ren, thHiR, thHiG, thHiB, 255);
  FillRect(ren, 0, MBARH, TBW, WH - MBARH - PALH - STATH);

  FOR i := 0 TO NTOOLS - 1 DO
    bty := MBARH + 2 + i * (bh + 2);
    IF bty + bh > WH - PALH - STATH THEN (* skip if off screen *) ELSE
      IF i = curTool THEN
        SetColor(ren, thSelR, thSelG, thSelB, 80);
        FillRect(ren, bx, bty, bw, bh);
        Bevel(bx, bty, bw, bh, FALSE)
      ELSE
        SetColor(ren, thBarR, thBarG, thBarB, 255);
        FillRect(ren, bx, bty, bw, bh);
        Bevel(bx, bty, bw, bh, TRUE)
      END;
      DrawToolIcon(bx + 2, bty + 1, i)
    END
  END;

  (* Magnify mode indicator *)
  IF magnifyMode THEN
    SetColor(ren, 255, 80, 80, 255);
    FillRect(ren, 4, WH - PALH - STATH - 22, TBW - 8, 18);
    IF font # NIL THEN
      DrawText(ren, font, "MAG", 8, WH - PALH - STATH - 21,
               255, 255, 255, 255)
    END
  END;
  (* Symmetry indicators *)
  IF symmetryX OR symmetryY THEN
    SetColor(ren, 80, 200, 80, 255);
    FillRect(ren, 4, WH - PALH - STATH - 42, TBW - 8, 18);
    IF font # NIL THEN
      IF symmetryX AND symmetryY THEN
        DrawText(ren, font, "SYM XY", 6, WH - PALH - STATH - 41,
                 255, 255, 255, 255)
      ELSIF symmetryX THEN
        DrawText(ren, font, "SYM X", 8, WH - PALH - STATH - 41,
                 255, 255, 255, 255)
      ELSE
        DrawText(ren, font, "SYM Y", 8, WH - PALH - STATH - 41,
                 255, 255, 255, 255)
      END
    END
  END;

  SetColor(ren, thShR, thShG, thShB, 255);
  DrawLine(ren, TBW-1, MBARH, TBW-1, WH - PALH - STATH)
END DrawToolbar;

PROCEDURE DrawPaletteBar;
VAR i, x, y, cw, ch, row, col: INTEGER;
BEGIN
  y := WH - PALH - STATH;
  SetColor(ren, thBarR, thBarG, thBarB, 255);
  FillRect(ren, 0, y, WW, PALH);
  SetColor(ren, thShR, thShG, thShB, 255);
  DrawLine(ren, 0, y, WW, y);
  SetColor(ren, thHiR, thHiG, thHiB, 255);
  DrawLine(ren, 0, y+1, WW, y+1);

  cw := 24;  ch := 18;
  FOR i := 0 TO NCOLORS - 1 DO
    row := i DIV 16;  col := i MOD 16;
    x := TBW + 8 + col * (cw + 2);
    SetColor(ren, PixBuf.PalR(pb, i), PixBuf.PalG(pb, i),
             PixBuf.PalB(pb, i), 255);
    FillRect(ren, x, y + 4 + row * (ch + 2), cw, ch);
    IF i = fgIdx THEN
      SetColor(ren, 255, 255, 255, 255);
      DrawRect(ren, x-2, y + 2 + row*(ch+2), cw+4, ch+4)
    ELSIF i = bgIdx THEN
      SetColor(ren, 200, 200, 200, 160);
      DrawRect(ren, x-1, y + 3 + row*(ch+2), cw+2, ch+2)
    END
  END;
  (* FG/BG preview *)
  x := WW - 60;
  SetColor(ren, BgR(), BgG(), BgB(), 255);
  FillRect(ren, x + 12, y + 8, 24, 24);
  Bevel(x + 12, y + 8, 24, 24, FALSE);
  SetColor(ren, FgR(), FgG(), FgB(), 255);
  FillRect(ren, x, y + 4, 24, 24);
  Bevel(x, y + 4, 24, 24, TRUE)
END DrawPaletteBar;

PROCEDURE DrawMenuBar;
VAR i: INTEGER;
    title: ARRAY [0..15] OF CHAR;
BEGIN
  SetColor(ren, thBarR, thBarG, thBarB, 255);
  FillRect(ren, 0, 0, WW, MBARH);
  SetColor(ren, thHiR, thHiG, thHiB, 255);
  DrawLine(ren, 0, 0, WW, 0);
  SetColor(ren, thShR, thShG, thShB, 255);
  DrawLine(ren, 0, MBARH-1, WW, MBARH-1);

  IF fontSm # NIL THEN
    FOR i := 0 TO NMENUS - 1 DO
      CASE i OF
        MNU_FILE:  StrCopy("File", title) |
        MNU_EDIT:  StrCopy("Edit", title) |
        MNU_TOOLS: StrCopy("Tools", title) |
        MNU_VIEW:  StrCopy("View", title) |
        MNU_SET:   StrCopy("Settings", title) |
        MNU_HELP:  StrCopy("Help", title)
      ELSE StrCopy("?", title)
      END;
      IF i = menuOpen THEN
        SetBlendMode(ren, BLEND_ALPHA);
        SetColor(ren, thSelR, thSelG, thSelB, 80);
        FillRect(ren, menuTitleX[i] - 4, 1, menuTitleW[i], MBARH - 2);
        SetBlendMode(ren, BLEND_NONE);
        DrawText(ren, fontSm, title, menuTitleX[i], 7,
                 thSelR, thSelG, thSelB, 255)
      ELSE
        DrawText(ren, fontSm, title, menuTitleX[i], 7,
                 thTxtR, thTxtG, thTxtB, 255)
      END
    END;

    IF zoomed THEN
      DrawText(ren, font, "ZOOM", WW - 200, 6, 255, 100, 100, 255)
    END;

    (* Thickness bar *)
    DrawText(ren, fontSm, "Th:", WW - 130, 7, thTxtR, thTxtG, thTxtB, 255);
    SetColor(ren, thSelR, thSelG, thSelB, 255);
    FillRect(ren, WW - 100, 9, lineThick * 3, 12);
    SetColor(ren, thShR, thShG, thShB, 255);
    DrawRect(ren, WW - 100, 9, MAX_THICK * 3, 12)
  END
END DrawMenuBar;

PROCEDURE DrawDropdownMenu;
VAR i, n, dx, dy, dw, dh, iy, lw: INTEGER;
    lbl: ARRAY [0..31] OF CHAR;
    sc:  ARRAY [0..15] OF CHAR;
BEGIN
  IF menuOpen < 0 THEN RETURN END;
  n := MenuItemCount(menuOpen);
  dx := menuTitleX[menuOpen] - 4;
  dy := MBARH;
  dw := MenuDropdownWidth(menuOpen);
  dh := MenuDropdownHeight(menuOpen);

  (* Shadow *)
  SetBlendMode(ren, BLEND_ALPHA);
  SetColor(ren, 0, 0, 0, 60);
  FillRect(ren, dx + 3, dy + 3, dw, dh);

  (* Background *)
  SetBlendMode(ren, BLEND_NONE);
  SetColor(ren, thBarR, thBarG, thBarB, 255);
  FillRect(ren, dx, dy, dw, dh);
  Bevel(dx, dy, dw, dh, TRUE);

  IF fontSm = NIL THEN RETURN END;

  FOR i := 0 TO n - 1 DO
    iy := dy + MenuItemYOffset(menuOpen, i);
    IF MenuIsSep(menuOpen, i) THEN
      SetColor(ren, thShR, thShG, thShB, 255);
      DrawLine(ren, dx + 4, iy + MENUSEPH DIV 2,
               dx + dw - 4, iy + MENUSEPH DIV 2)
    ELSE
      (* Hover highlight *)
      IF i = menuHover THEN
        SetBlendMode(ren, BLEND_ALPHA);
        SetColor(ren, thSelR, thSelG, thSelB, 80);
        FillRect(ren, dx + 2, iy, dw - 4, MENULH);
        SetBlendMode(ren, BLEND_NONE)
      END;
      (* Toggle checkmark *)
      IF MenuIsToggle(menuOpen, i) AND MenuItemChecked(menuOpen, i) THEN
        SetColor(ren, thSelR, thSelG, thSelB, 255);
        FillRect(ren, dx + 6, iy + 7, 6, 6)
      END;
      (* Label *)
      MenuItemLabel(menuOpen, i, lbl);
      DrawText(ren, fontSm, lbl, dx + MENUPAD, iy + 3,
               thTxtR, thTxtG, thTxtB, 255);
      (* Shortcut — right aligned, dimmer *)
      MenuItemShortcut(menuOpen, i, sc);
      IF sc[0] # 0C THEN
        lw := TextWidth(fontSm, sc);
        DrawText(ren, fontSm, sc, dx + dw - lw - 8, iy + 3,
                 thHiR, thHiG, thHiB, 255)
      END
    END
  END
END DrawDropdownMenu;

PROCEDURE DrawStatusBar;
VAR y, cx, cy: INTEGER;
    name: ARRAY [0..31] OF CHAR;
BEGIN
  y := WH - STATH;
  SetColor(ren, thBarR, thBarG, thBarB, 255);
  FillRect(ren, 0, y, WW, STATH);
  SetColor(ren, thShR, thShG, thShB, 255);
  DrawLine(ren, 0, y, WW, y);
  SetColor(ren, thHiR, thHiG, thHiB, 255);
  DrawLine(ren, 0, y+1, WW, y+1);

  IF fontSm = NIL THEN RETURN END;

  (* Tool name *)
  ToolName(curTool, name);
  DrawText(ren, fontSm, name, 8, y + 4, thSelR, thSelG, thSelB, 255);

  (* Coordinates *)
  IF InCanvas(mx, my) THEN
    ScreenToCanvas(mx, my, cx, cy);
    (* Show as numbers using simple approach *)
    DrawText(ren, fontSm, "X:", 120, y + 4, thTxtR, thTxtG, thTxtB, 255);
    DrawText(ren, fontSm, "Y:", 200, y + 4, thTxtR, thTxtG, thTxtB, 255);
    (* Draw coordinate bars proportional to position *)
    SetColor(ren, 100, 150, 200, 255);
    IF PixBuf.Width(pb) > 0 THEN
      FillRect(ren, 136, y + 6, cx * 50 DIV PixBuf.Width(pb), 10)
    END;
    IF PixBuf.Height(pb) > 0 THEN
      FillRect(ren, 216, y + 6, cy * 50 DIV PixBuf.Height(pb), 10)
    END
  END;

  (* Zoom indicator *)
  IF zoomed THEN
    DrawText(ren, fontSm, "ZOOMED", 300, y + 4, 255, 100, 100, 255)
  END;

  (* Selection info *)
  IF hasSelection THEN
    DrawText(ren, fontSm, "SEL", 380, y + 4, 100, 255, 100, 255)
  END;

  (* Pixel-perfect indicator *)
  IF pixelPerfect THEN
    DrawText(ren, fontSm, "PX", 420, y + 4, 200, 200, 100, 255)
  END;

  (* Status message — shown temporarily *)
  IF (statusMsg[0] # 0C) AND (Ticks() - statusTick < STATUS_DURATION) THEN
    DrawText(ren, fontSm, statusMsg, 460, y + 4, 255, 220, 100, 255)
  END;

  (* Dirty indicator *)
  IF dirty THEN
    DrawText(ren, fontSm, "*", WW - 170, y + 4, 255, 100, 100, 255)
  END;

  (* FG/BG index *)
  DrawText(ren, fontSm, "FG:", WW - 160, y + 4, thTxtR, thTxtG, thTxtB, 255);
  SetColor(ren, FgR(), FgG(), FgB(), 255);
  FillRect(ren, WW - 140, y + 4, 14, 14);

  DrawText(ren, fontSm, "BG:", WW - 110, y + 4, thTxtR, thTxtG, thTxtB, 255);
  SetColor(ren, BgR(), BgG(), BgB(), 255);
  FillRect(ren, WW - 90, y + 4, 14, 14);

  (* Undo/Redo count *)
  DrawText(ren, fontSm, "U:", WW - 65, y + 4, thTxtR, thTxtG, thTxtB, 200);
  SetColor(ren, 150, 150, 180, 255);
  IF undoCount > 0 THEN
    FillRect(ren, WW - 52, y + 6, MinI(undoCount, 15), 10)
  END;
  DrawText(ren, fontSm, "R:", WW - 33, y + 4, thTxtR, thTxtG, thTxtB, 200);
  SetColor(ren, 180, 150, 150, 255);
  IF redoCount > 0 THEN
    FillRect(ren, WW - 20, y + 6, MinI(redoCount, 15), 10)
  END
END DrawStatusBar;

PROCEDURE DrawMiniMap;
VAR mmW, mmH, mmX, mmY: INTEGER;
    rx, ry, rw, rh: INTEGER;
BEGIN
  IF NOT zoomed THEN RETURN END;
  mmW := 130;  mmH := mmW * canH DIV canW;
  mmX := TBW + canW - mmW - 8;
  mmY := MBARH + 8;

  SetColor(ren, 0, 0, 0, 160);
  SetBlendMode(ren, BLEND_ALPHA);
  FillRect(ren, mmX - 2, mmY - 2, mmW + 4, mmH + 4);
  DrawRegion(ren, canvas, 0, 0, PixBuf.Width(pb), PixBuf.Height(pb),
             mmX, mmY, mmW, mmH);

  rx := mmX + zoomX * mmW DIV PixBuf.Width(pb);
  ry := mmY + zoomY * mmH DIV PixBuf.Height(pb);
  rw := zoomW * mmW DIV PixBuf.Width(pb);
  rh := zoomH * mmH DIV PixBuf.Height(pb);
  IF rw < 2 THEN rw := 2 END;
  IF rh < 2 THEN rh := 2 END;
  SetColor(ren, 255, 80, 80, 255);
  DrawRect(ren, rx, ry, rw, rh);
  SetColor(ren, 255, 255, 255, 200);
  DrawRect(ren, rx-1, ry-1, rw+2, rh+2);
  SetColor(ren, thHiR, thHiG, thHiB, 255);
  DrawRect(ren, mmX - 2, mmY - 2, mmW + 4, mmH + 4)
END DrawMiniMap;

PROCEDURE DrawGrid;
VAR stepX, stepY, sx, sy, cx, cy, gx, gy: INTEGER;
BEGIN
  IF NOT showGrid THEN RETURN END;
  IF NOT zoomed THEN RETURN END;
  IF zoomW > canW DIV 2 THEN RETURN END;  (* only show when zoomed enough *)

  SetBlendMode(ren, BLEND_ALPHA);
  SetColor(ren, 100, 100, 120, 60);

  stepX := canW DIV zoomW;
  stepY := canH DIV zoomH;
  IF stepX < 4 THEN RETURN END;

  (* Vertical grid lines *)
  FOR gx := 0 TO zoomW DO
    sx := TBW + gx * canW DIV zoomW;
    IF (sx >= TBW) AND (sx < TBW + canW) THEN
      DrawLine(ren, sx, MBARH, sx, MBARH + canH)
    END
  END;
  (* Horizontal grid lines *)
  FOR gy := 0 TO zoomH DO
    sy := MBARH + gy * canH DIV zoomH;
    IF (sy >= MBARH) AND (sy < MBARH + canH) THEN
      DrawLine(ren, TBW, sy, TBW + canW, sy)
    END
  END
END DrawGrid;

PROCEDURE DrawSelection;
VAR sx1, sy1, sx2, sy2: INTEGER;
    phase: INTEGER;
BEGIN
  IF NOT hasSelection THEN RETURN END;
  CanvasToScreen(selX, selY, sx1, sy1);
  CanvasToScreen(selX + selW, selY + selH, sx2, sy2);

  SetBlendMode(ren, BLEND_ALPHA);
  phase := (Ticks() DIV 200) MOD 2;
  IF phase = 0 THEN
    SetColor(ren, 255, 255, 255, 200)
  ELSE
    SetColor(ren, 0, 0, 0, 200)
  END;
  DrawRect(ren, sx1, sy1, sx2 - sx1, sy2 - sy1)
END DrawSelection;

PROCEDURE DrawPreview;
VAR cx1, cy1, cx2, cy2, sx1, sy1, sx2, sy2, i, n: INTEGER;
BEGIN
  IF NOT dragging THEN RETURN END;
  IF magnifyMode THEN
    SetColor(ren, 255, 255, 80, 200);
    SetBlendMode(ren, BLEND_ALPHA);
    DrawRect(ren, MinI(dx0 + TBW, mx), MinI(dy0 + MBARH, my),
             AbsI(mx - dx0 - TBW), AbsI(my - dy0 - MBARH));
    RETURN
  END;

  IF (curTool <= T_SPRAY) OR (curTool = T_ERASER)
  OR (curTool = T_FLOOD) OR (curTool = T_EYEDROP)
  OR (curTool = T_LIGHTEN) OR (curTool = T_DARKEN)
  OR (curTool = T_AIRBRUSH) OR (curTool = T_SMUDGE)
  OR (curTool = T_STAMP) OR (curTool = T_REPLACE)
  OR (curTool = T_BEZIER) OR (curTool = T_MOVE) THEN RETURN END;

  ScreenToCanvas(mx, my, cx2, cy2);
  cx1 := dx0;  cy1 := dy0;
  IF shiftDown THEN Constrain45(cx1, cy1, cx2, cy2) END;

  CanvasToScreen(cx1, cy1, sx1, sy1);
  CanvasToScreen(cx2, cy2, sx2, sy2);

  SetBlendMode(ren, BLEND_ALPHA);

  IF (curTool = T_GRADIENT) OR (curTool = T_GRADANG) THEN
    SetColor(ren, FgR(), FgG(), FgB(), 100);
    FillRect(ren, MinI(sx1,sx2), MinI(sy1,sy2),
             AbsI(sx2-sx1), AbsI(sy2-sy1));
    RETURN
  END;

  IF curTool = T_SELECT THEN
    SetColor(ren, 255, 255, 255, 160);
    DrawRect(ren, MinI(sx1,sx2), MinI(sy1,sy2),
             AbsI(sx2-sx1), AbsI(sy2-sy1));
    RETURN
  END;

  IF curTool = T_PATTERN THEN
    SetColor(ren, FgR(), FgG(), FgB(), 80);
    FillRect(ren, MinI(sx1,sx2), MinI(sy1,sy2),
             AbsI(sx2-sx1), AbsI(sy2-sy1));
    RETURN
  END;

  SetColor(ren, FgR(), FgG(), FgB(), 160);
  CASE curTool OF
    T_LINE:
      DrawThickLine(ren, sx1, sy1, sx2, sy2, lineThick) |
    T_RECT:
      DrawRect(ren, MinI(sx1,sx2), MinI(sy1,sy2),
               AbsI(sx2-sx1), AbsI(sy2-sy1)) |
    T_FRECT:
      FillRect(ren, MinI(sx1,sx2), MinI(sy1,sy2),
               AbsI(sx2-sx1), AbsI(sy2-sy1)) |
    T_CIRCLE:
      DrawCircle(ren, (sx1+sx2) DIV 2, (sy1+sy2) DIV 2,
                 AbsI(sx2-sx1) DIV 2) |
    T_FCIRCLE:
      FillCircle(ren, (sx1+sx2) DIV 2, (sy1+sy2) DIV 2,
                 AbsI(sx2-sx1) DIV 2) |
    T_ELLIPSE:
      DrawEllipse(ren, (sx1+sx2) DIV 2, (sy1+sy2) DIV 2,
                  AbsI(sx2-sx1) DIV 2, AbsI(sy2-sy1) DIV 2)
  ELSE
  END
END DrawPreview;

PROCEDURE DrawPolygonPreview;
VAR i, n, sx, sy, sx2, sy2: INTEGER;
BEGIN
  IF NOT polyActive THEN RETURN END;
  n := PixBuf.PolyCount();
  IF n < 1 THEN RETURN END;
  SetBlendMode(ren, BLEND_ALPHA);
  SetColor(ren, FgR(), FgG(), FgB(), 200);
  FOR i := 0 TO n - 2 DO
    CanvasToScreen(PixBuf.PolyX(i), PixBuf.PolyY(i), sx, sy);
    CanvasToScreen(PixBuf.PolyX(i+1), PixBuf.PolyY(i+1), sx2, sy2);
    DrawLine(ren, sx, sy, sx2, sy2)
  END;
  (* Line from last vertex to cursor *)
  IF InCanvas(mx, my) THEN
    CanvasToScreen(PixBuf.PolyX(n-1), PixBuf.PolyY(n-1), sx, sy);
    DrawLine(ren, sx, sy, mx, my)
  END
END DrawPolygonPreview;

PROCEDURE DrawCursor;
BEGIN
  IF NOT InCanvas(mx, my) THEN RETURN END;
  SetBlendMode(ren, BLEND_ALPHA);
  IF magnifyMode THEN
    SetColor(ren, 255, 255, 80, 200);
    DrawRect(ren, mx - 6, my - 6, 12, 12);
    DrawLine(ren, mx, my - 10, mx, my + 10);
    DrawLine(ren, mx - 10, my, mx + 10, my)
  ELSIF curTool = T_ERASER THEN
    SetColor(ren, 200, 200, 200, 100);
    DrawCircle(ren, mx, my, lineThick * 2 + 3)
  ELSIF curTool = T_BRUSH THEN
    SetColor(ren, 200, 200, 200, 100);
    DrawCircle(ren, mx, my, lineThick + 1)
  ELSIF curTool = T_SPRAY THEN
    SetColor(ren, 200, 200, 200, 80);
    DrawCircle(ren, mx, my, lineThick * 3)
  ELSIF curTool = T_FLOOD THEN
    SetColor(ren, FgR(), FgG(), FgB(), 150);
    FillRect(ren, mx - 2, my - 8, 4, 16);
    FillRect(ren, mx - 8, my - 2, 16, 4)
  ELSIF curTool = T_EYEDROP THEN
    SetColor(ren, 255, 255, 255, 180);
    DrawCircle(ren, mx, my, 6);
    DrawLine(ren, mx, my + 6, mx, my + 12)
  ELSIF curTool = T_TEXT THEN
    SetColor(ren, FgR(), FgG(), FgB(), 180);
    DrawLine(ren, mx, my - 8, mx, my + 8)
  ELSIF curTool = T_AIRBRUSH THEN
    SetColor(ren, FgR(), FgG(), FgB(), 60);
    FillCircle(ren, mx, my, lineThick * 2);
    SetColor(ren, 200, 200, 200, 100);
    DrawCircle(ren, mx, my, lineThick * 2)
  ELSIF curTool = T_SMUDGE THEN
    SetColor(ren, 200, 180, 160, 100);
    FillCircle(ren, mx, my, lineThick + 2)
  ELSIF curTool = T_BEZIER THEN
    SetColor(ren, FgR(), FgG(), FgB(), 200);
    FillCircle(ren, mx, my, 3)
  ELSIF curTool = T_MOVE THEN
    SetColor(ren, 200, 200, 200, 150);
    DrawLine(ren, mx, my - 12, mx, my + 12);
    DrawLine(ren, mx - 12, my, mx + 12, my)
  ELSE
    SetColor(ren, 200, 200, 200, 100);
    DrawLine(ren, mx - 10, my, mx + 10, my);
    DrawLine(ren, mx, my - 10, mx, my + 10)
  END
END DrawCursor;

CONST
  LPANW = 140;  (* layer panel width *)

PROCEDURE DrawLayerPanel;
VAR i, n, py, lh, px: INTEGER;
    vis: BOOLEAN;
BEGIN
  IF NOT showLayerPanel THEN RETURN END;
  n := PixBuf.LayerCount();
  px := WW - LPANW;
  lh := 24;

  SetBlendMode(ren, BLEND_ALPHA);
  SetColor(ren, 30, 35, 50, 220);
  FillRect(ren, px, MBARH, LPANW, canH);
  SetColor(ren, thHiR, thHiG, thHiB, 255);
  DrawLine(ren, px, MBARH, px, MBARH + canH);

  IF font = NIL THEN RETURN END;
  DrawText(ren, fontSm, "Layers", px + 4, MBARH + 4,
           thSelR, thSelG, thSelB, 255);

  FOR i := 0 TO n - 1 DO
    py := MBARH + 24 + i * (lh + 2);
    IF py + lh > MBARH + canH THEN RETURN END;

    (* Highlight active layer *)
    IF i = PixBuf.LayerActive() THEN
      SetColor(ren, thSelR, thSelG, thSelB, 60);
      FillRect(ren, px + 2, py, LPANW - 4, lh);
      Bevel(px + 2, py, LPANW - 4, lh, FALSE)
    ELSE
      SetColor(ren, thHiR, thHiG, thHiB, 180);
      FillRect(ren, px + 2, py, LPANW - 4, lh);
      Bevel(px + 2, py, LPANW - 4, lh, TRUE)
    END;

    (* Visibility indicator *)
    vis := PixBuf.LayerVisible(i);
    IF vis THEN
      SetColor(ren, 100, 255, 100, 255)
    ELSE
      SetColor(ren, 100, 100, 100, 255)
    END;
    FillCircle(ren, px + 14, py + lh DIV 2, 4);

    (* Layer name/number *)
    IF i = 0 THEN
      DrawText(ren, fontSm, "BG", px + 24, py + 4,
               thTxtR, thTxtG, thTxtB, 255)
    ELSE
      DrawText(ren, fontSm, "Layer", px + 24, py + 4,
               thTxtR, thTxtG, thTxtB, 255)
    END
  END;

  (* Add layer button *)
  py := MBARH + 24 + n * (lh + 2);
  IF py + lh < MBARH + canH THEN
    SetColor(ren, thHiR, thHiG, thHiB, 200);
    FillRect(ren, px + 2, py, LPANW - 4, lh);
    Bevel(px + 2, py, LPANW - 4, lh, TRUE);
    DrawText(ren, fontSm, "+ New Layer", px + 24, py + 4,
             thTxtR, thTxtG, thTxtB, 255)
  END
END DrawLayerPanel;

PROCEDURE DrawTooltip;
VAR name: ARRAY [0..31] OF CHAR;
    tx, ty, tw: INTEGER;
BEGIN
  IF (NOT showTooltip) OR (hoverTool < 0) THEN RETURN END;
  IF fontSm = NIL THEN RETURN END;
  ToolName(hoverTool, name);
  tw := TextWidth(fontSm, name);
  tx := TBW + 4;
  ty := MBARH + hoverTool * (28 + 2) + 4;
  SetBlendMode(ren, BLEND_ALPHA);
  SetColor(ren, 20, 20, 30, 220);
  FillRect(ren, tx - 2, ty - 1, tw + 8, 18);
  DrawText(ren, fontSm, name, tx + 2, ty + 1, 255, 255, 200, 255);
  SetBlendMode(ren, BLEND_NONE)
END DrawTooltip;

PROCEDURE DrawShortcutOverlay;
VAR ox, oy, lh, col: INTEGER;
BEGIN
  IF NOT showShortcuts THEN RETURN END;
  IF fontSm = NIL THEN RETURN END;
  SetBlendMode(ren, BLEND_ALPHA);
  SetColor(ren, 10, 10, 20, 200);
  ox := TBW + 20;  oy := MBARH + 20;
  FillRect(ren, ox, oy, 500, 420);
  SetBlendMode(ren, BLEND_NONE);
  SetColor(ren, thHiR, thHiG, thHiB, 255);
  DrawRect(ren, ox, oy, 500, 420);
  lh := 16;  col := ox + 10;
  DrawText(ren, fontSm, "=== Keyboard Shortcuts ===", col, oy + 4, 255, 220, 100, 255);
  DrawText(ren, fontSm, "1-9: Select tool    0: Zoom fit", col, oy+4+lh, 200, 200, 220, 255);
  DrawText(ren, fontSm, "[/]: Brush size     =: Swap FG/BG", col, oy+4+lh*2, 200, 200, 220, 255);
  DrawText(ren, fontSm, "e: Eyedropper  t: Text  p: Polygon", col, oy+4+lh*3, 200, 200, 220, 255);
  DrawText(ren, fontSm, "f: Flood fill  m: Magnify  g: Gradient", col, oy+4+lh*4, 200, 200, 220, 255);
  DrawText(ren, fontSm, "z: Undo        r: Redo", col, oy+4+lh*5, 200, 200, 220, 255);
  DrawText(ren, fontSm, "x/y: Symmetry  d: Grid    w: Px-perfect", col, oy+4+lh*6, 200, 200, 220, 255);
  DrawText(ren, fontSm, "l: Layers      h: History ?/: Shortcuts", col, oy+4+lh*7, 200, 200, 220, 255);
  DrawText(ren, fontSm, "Space: Pan     Scroll: Zoom", col, oy+4+lh*8, 200, 200, 220, 255);
  DrawText(ren, fontSm, "Ctrl+S: Save .dp2  Shift+S: Export PNG", col, oy+4+lh*9, 200, 200, 220, 255);
  DrawText(ren, fontSm, "Ctrl+O: Load .dp2  Ctrl+P: Save palette", col, oy+4+lh*10, 200, 200, 220, 255);
  DrawText(ren, fontSm, "s: Save BMP        Ctrl+T: Toggle theme", col, oy+4+lh*11, 200, 200, 220, 255);
  DrawText(ren, fontSm, "F11: Fullscreen    Escape: Cancel/Quit", col, oy+4+lh*12, 200, 200, 220, 255);
  DrawText(ren, fontSm, "--- Selection ---", col, oy+4+lh*14, 255, 200, 100, 255);
  DrawText(ren, fontSm, "Ctrl+C: Copy  v: Paste  Del: Fill BG", col, oy+4+lh*15, 200, 200, 220, 255);
  DrawText(ren, fontSm, "Ctrl+H: Flip H  Ctrl+V: Flip V", col, oy+4+lh*16, 200, 200, 220, 255);
  DrawText(ren, fontSm, "Ctrl+R: Rotate 90", col, oy+4+lh*17, 200, 200, 220, 255);
  DrawText(ren, fontSm, "--- Mouse ---", col, oy+4+lh*19, 255, 200, 100, 255);
  DrawText(ren, fontSm, "Middle: Swap FG/BG  Alt+Right: Pick BG", col, oy+4+lh*20, 200, 200, 220, 255);
  DrawText(ren, fontSm, "Press ? to close", col, oy+4+lh*22, 150, 150, 160, 255)
END DrawShortcutOverlay;

PROCEDURE DrawHistoryPanel;
VAR px, py, lh, i: INTEGER;
    node: UndoRef;
BEGIN
  IF NOT showHistory THEN RETURN END;
  IF fontSm = NIL THEN RETURN END;
  px := WW - 200;  py := MBARH + 10;  lh := 16;
  SetBlendMode(ren, BLEND_ALPHA);
  SetColor(ren, 15, 15, 25, 210);
  FillRect(ren, px, py, 190, 300);
  SetBlendMode(ren, BLEND_NONE);
  SetColor(ren, thHiR, thHiG, thHiB, 255);
  DrawRect(ren, px, py, 190, 300);
  DrawText(ren, fontSm, "History", px + 8, py + 4, 255, 220, 100, 255);
  node := undoHead;  i := 0;
  WHILE (node # NIL) AND (i < 16) DO
    DrawText(ren, fontSm, "Undo step", px + 8, py + 24 + i * lh,
             180, 180, 200, 255);
    node := node^.next;
    INC(i)
  END;
  IF i = 0 THEN
    DrawText(ren, fontSm, "(no history)", px + 8, py + 24, 120, 120, 140, 255)
  END
END DrawHistoryPanel;

PROCEDURE DrawBrushPreview;
VAR bpx, bpy, br: INTEGER;
BEGIN
  IF NOT showBrushPreview THEN RETURN END;
  IF (curTool > T_SPRAY) AND (curTool # T_ERASER) AND
     (curTool # T_AIRBRUSH) THEN RETURN END;
  bpx := TBW DIV 2;
  bpy := WH - PALH - STATH - 30;
  br := lineThick DIV 2;
  IF br < 1 THEN br := 1 END;
  SetBlendMode(ren, BLEND_ALPHA);
  SetColor(ren, 0, 0, 0, 100);
  FillCircle(ren, bpx, bpy, br + 2);
  SetColor(ren, FgR(), FgG(), FgB(), 200);
  FillCircle(ren, bpx, bpy, br);
  SetBlendMode(ren, BLEND_NONE)
END DrawBrushPreview;

PROCEDURE DrawFrameStrip;
VAR fx, fy, fw, fh, i, nc, cur: INTEGER;
BEGIN
  IF NOT showFrameStrip THEN RETURN END;
  IF fontSm = NIL THEN RETURN END;
  nc := PixBuf.FrameCount();
  cur := PixBuf.FrameCurrent();
  fh := 36;  fw := 44;
  fy := WH - PALH - STATH - fh - 4;
  fx := TBW + 4;
  (* Background *)
  SetBlendMode(ren, BLEND_ALPHA);
  SetColor(ren, 15, 15, 25, 200);
  FillRect(ren, fx - 2, fy - 2, MinI(nc + 2, 16) * (fw + 2) + 8, fh + 8);
  SetBlendMode(ren, BLEND_NONE);
  FOR i := 0 TO nc - 1 DO
    IF i >= 15 THEN (* only show first 15 frames inline *)
      DrawText(ren, fontSm, "...", fx + i * (fw + 2), fy + 10,
               thTxtR, thTxtG, thTxtB, 255);
      (* skip rest *)
      i := nc  (* break *)
    ELSE
      IF i = cur THEN
        SetColor(ren, thSelR, thSelG, thSelB, 255);
        DrawRect(ren, fx + i * (fw + 2) - 1, fy - 1, fw + 2, fh + 2)
      END;
      SetColor(ren, thShR, thShG, thShB, 255);
      FillRect(ren, fx + i * (fw + 2), fy, fw, fh);
      (* Frame number *)
      DrawText(ren, fontSm, "#", fx + i * (fw + 2) + 2, fy + 2,
               thTxtR, thTxtG, thTxtB, 200)
    END
  END;
  (* + button *)
  i := MinI(nc, 15);
  SetColor(ren, thHiR, thHiG, thHiB, 255);
  FillRect(ren, fx + i * (fw + 2), fy, fw, fh);
  DrawText(ren, fontSm, "+", fx + i * (fw + 2) + fw DIV 2 - 4, fy + 8,
           thTxtR, thTxtG, thTxtB, 255);
  (* Labels *)
  IF playingAnim THEN
    DrawText(ren, fontSm, "PLAY", fx, fy - 14, 100, 255, 100, 255)
  END;
  IF onionSkin THEN
    DrawText(ren, fontSm, "ONION", fx + 50, fy - 14, 255, 200, 100, 255)
  END
END DrawFrameStrip;

PROCEDURE DrawPaletteEditor;
VAR px, py, sw, i, val: INTEGER;
BEGIN
  IF NOT showPalEdit THEN RETURN END;
  IF fontSm = NIL THEN RETURN END;
  px := 120;  py := WH - PALH - STATH - 90;
  SetBlendMode(ren, BLEND_ALPHA);
  SetColor(ren, 20, 20, 35, 230);
  FillRect(ren, px, py, 260, 80);
  SetBlendMode(ren, BLEND_NONE);
  SetColor(ren, thHiR, thHiG, thHiB, 255);
  DrawRect(ren, px, py, 260, 80);
  (* Current color swatch *)
  SetColor(ren, palEditR, palEditG, palEditB, 255);
  FillRect(ren, px + 4, py + 4, 40, 40);
  (* R slider *)
  DrawText(ren, fontSm, "R", px + 52, py + 4, 255, 100, 100, 255);
  sw := palEditR * 180 DIV 255;
  SetColor(ren, 255, 60, 60, 255);
  FillRect(ren, px + 68, py + 6, sw, 10);
  SetColor(ren, 80, 80, 80, 255);
  FillRect(ren, px + 68 + sw, py + 6, 180 - sw, 10);
  (* G slider *)
  DrawText(ren, fontSm, "G", px + 52, py + 22, 100, 255, 100, 255);
  sw := palEditG * 180 DIV 255;
  SetColor(ren, 60, 255, 60, 255);
  FillRect(ren, px + 68, py + 24, sw, 10);
  SetColor(ren, 80, 80, 80, 255);
  FillRect(ren, px + 68 + sw, py + 24, 180 - sw, 10);
  (* B slider *)
  DrawText(ren, fontSm, "B", px + 52, py + 40, 100, 100, 255, 255);
  sw := palEditB * 180 DIV 255;
  SetColor(ren, 60, 60, 255, 255);
  FillRect(ren, px + 68, py + 42, sw, 10);
  SetColor(ren, 80, 80, 80, 255);
  FillRect(ren, px + 68 + sw, py + 42, 180 - sw, 10);
  DrawText(ren, fontSm, "Click slider to adjust / Esc to close",
           px + 4, py + 62, 150, 150, 160, 255)
END DrawPaletteEditor;

PROCEDURE DrawCRTOverlay;
VAR yy: INTEGER;
BEGIN
  IF NOT showCRT THEN RETURN END;
  SetBlendMode(ren, BLEND_ALPHA);
  SetColor(ren, 0, 0, 0, 40);
  yy := MBARH + 1;
  WHILE yy < MBARH + canH DO
    DrawLine(ren, TBW, yy, TBW + canW, yy);
    INC(yy, 2)
  END;
  SetBlendMode(ren, BLEND_NONE)
END DrawCRTOverlay;

PROCEDURE DrawTileGrid;
VAR gx, gy, tw, th, ox, oy: INTEGER;
BEGIN
  IF NOT tileMode THEN RETURN END;
  tw := tileW;  th := tileH;
  IF tw < 4 THEN tw := 16 END;
  IF th < 4 THEN th := 16 END;
  SetBlendMode(ren, BLEND_ALPHA);
  SetColor(ren, 255, 200, 0, 60);
  IF zoomed THEN
    ox := TBW;  oy := MBARH;
    gx := (tw - (zoomX MOD tw)) MOD tw;
    WHILE gx < zoomW DO
      DrawLine(ren, ox + gx * canW DIV zoomW, oy,
               ox + gx * canW DIV zoomW, oy + canH);
      INC(gx, tw)
    END;
    gy := (th - (zoomY MOD th)) MOD th;
    WHILE gy < zoomH DO
      DrawLine(ren, ox, oy + gy * canH DIV zoomH,
               ox + canW, oy + gy * canH DIV zoomH);
      INC(gy, th)
    END
  ELSE
    gx := TBW;
    WHILE gx < TBW + canW DO
      DrawLine(ren, gx, MBARH, gx, MBARH + canH);
      INC(gx, tw)
    END;
    gy := MBARH;
    WHILE gy < MBARH + canH DO
      DrawLine(ren, TBW, gy, TBW + canW, gy);
      INC(gy, th)
    END
  END;
  SetBlendMode(ren, BLEND_NONE)
END DrawTileGrid;

PROCEDURE DrawPrefsPanel;
VAR ox, oy, lh: INTEGER;
BEGIN
  IF NOT showPrefs THEN RETURN END;
  IF fontSm = NIL THEN RETURN END;
  ox := TBW + 50;  oy := MBARH + 30;  lh := 18;
  SetBlendMode(ren, BLEND_ALPHA);
  SetColor(ren, 15, 15, 30, 220);
  FillRect(ren, ox, oy, 400, 280);
  SetBlendMode(ren, BLEND_NONE);
  SetColor(ren, thHiR, thHiG, thHiB, 255);
  DrawRect(ren, ox, oy, 400, 280);
  DrawText(ren, fontSm, "=== Preferences ===", ox + 10, oy + 6, 255, 220, 100, 255);
  DrawText(ren, fontSm, "Ctrl+T: Toggle theme", ox + 10, oy + 6 + lh,
           thTxtR, thTxtG, thTxtB, 255);
  DrawText(ren, fontSm, "Ctrl+K: Save config", ox + 10, oy + 6 + lh*2,
           thTxtR, thTxtG, thTxtB, 255);
  DrawText(ren, fontSm, "F2: Tile mode   F3: HAM mode", ox + 10, oy + 6 + lh*3,
           thTxtR, thTxtG, thTxtB, 255);
  DrawText(ren, fontSm, "F4: Copper      F5: CRT overlay", ox + 10, oy + 6 + lh*4,
           thTxtR, thTxtG, thTxtB, 255);
  DrawText(ren, fontSm, "F11: Fullscreen", ox + 10, oy + 6 + lh*5,
           thTxtR, thTxtG, thTxtB, 255);
  DrawText(ren, fontSm, "--- Current Settings ---", ox + 10, oy + 6 + lh*7,
           255, 200, 100, 255);
  IF darkTheme THEN
    DrawText(ren, fontSm, "Theme: Dark", ox + 10, oy + 6 + lh*8, thTxtR, thTxtG, thTxtB, 255)
  ELSE
    DrawText(ren, fontSm, "Theme: Light", ox + 10, oy + 6 + lh*8, thTxtR, thTxtG, thTxtB, 255)
  END;
  IF tileMode THEN
    DrawText(ren, fontSm, "Tile: ON", ox + 10, oy + 6 + lh*9, 100, 255, 100, 255)
  ELSE
    DrawText(ren, fontSm, "Tile: OFF", ox + 10, oy + 6 + lh*9, 200, 200, 200, 255)
  END;
  IF showCRT THEN
    DrawText(ren, fontSm, "CRT: ON", ox + 10, oy + 6 + lh*10, 100, 255, 100, 255)
  ELSE
    DrawText(ren, fontSm, "CRT: OFF", ox + 10, oy + 6 + lh*10, 200, 200, 200, 255)
  END;
  IF pixelPerfect THEN
    DrawText(ren, fontSm, "Pixel-perfect: ON", ox + 10, oy + 6 + lh*11, 100, 255, 100, 255)
  ELSE
    DrawText(ren, fontSm, "Pixel-perfect: OFF", ox + 10, oy + 6 + lh*11, 200, 200, 200, 255)
  END;
  DrawText(ren, fontSm, "Press Esc to close", ox + 10, oy + 6 + lh*13, 150, 150, 160, 255)
END DrawPrefsPanel;

PROCEDURE DrawFrame;
BEGIN
  SetBlendMode(ren, BLEND_NONE);
  SetColor(ren, thBgR, thBgG, thBgB, 255);
  Clear(ren);

  (* Onion skin — render prev frame at low alpha *)
  IF onionSkin AND showFrameStrip AND (PixBuf.FrameCount() > 1) THEN
    IF PixBuf.FrameCurrent() > 0 THEN
      PixBuf.Render(ren, canvas,
                     PixBuf.FrameGet(PixBuf.FrameCurrent() - 1));
      IF zoomed THEN
        DrawRegion(ren, canvas, zoomX, zoomY, zoomW, zoomH,
                   TBW, MBARH, canW, canH)
      ELSE
        Draw(ren, canvas, TBW, MBARH)
      END;
      SetBlendMode(ren, BLEND_ALPHA);
      SetColor(ren, thBgR, thBgG, thBgB, 160);
      FillRect(ren, TBW, MBARH, canW, canH);
      SetBlendMode(ren, BLEND_NONE)
    END
  END;

  (* Flatten layers for display, then render *)
  IF PixBuf.LayerCount() > 1 THEN
    PixBuf.LayerFlatten(displayBuf, 0);
    IF hamMode > 0 THEN
      PixBuf.RenderHAM(ren, canvas, displayBuf, hamMode)
    ELSIF copperEnabled THEN
      PixBuf.CopperGradient(ren, canvas, displayBuf, 0,
                             PixBuf.Height(displayBuf), fgIdx, bgIdx)
    ELSE
      PixBuf.Render(ren, canvas, displayBuf)
    END
  ELSE
    IF hamMode > 0 THEN
      PixBuf.RenderHAM(ren, canvas, pb, hamMode)
    ELSIF copperEnabled THEN
      PixBuf.CopperGradient(ren, canvas, pb, 0,
                             PixBuf.Height(pb), fgIdx, bgIdx)
    ELSE
      PixBuf.Render(ren, canvas, pb)
    END
  END;

  (* Blit canvas — zoomed or full *)
  IF zoomed THEN
    DrawRegion(ren, canvas, zoomX, zoomY, zoomW, zoomH,
               TBW, MBARH, canW, canH)
  ELSE
    Draw(ren, canvas, TBW, MBARH)
  END;

  SetColor(ren, thShR, thShG, thShB, 255);
  DrawRect(ren, TBW - 1, MBARH - 1, canW + 2, canH + 2);

  DrawGrid;
  DrawTileGrid;
  DrawCRTOverlay;
  DrawSelection;
  DrawPreview;
  DrawPolygonPreview;
  DrawCursor;
  DrawMiniMap;

  DrawToolbar;
  DrawPaletteBar;
  DrawMenuBar;
  DrawStatusBar;
  DrawLayerPanel;
  DrawBrushPreview;
  DrawFrameStrip;
  DrawTooltip;
  DrawPaletteEditor;
  DrawHistoryPanel;
  DrawPrefsPanel;
  DrawShortcutOverlay;
  DrawDropdownMenu;

  Present(ren)
END DrawFrame;

(* ═══════════════════════════════════════════════════════════════
   Input
   ═══════════════════════════════════════════════════════════════ *)

(* ─── Menu input handling ────────────────────────────────────── *)

PROCEDURE MenuHitTitle(sx: INTEGER): INTEGER;
VAR i: INTEGER;
BEGIN
  FOR i := 0 TO NMENUS - 1 DO
    IF (sx >= menuTitleX[i] - 4) AND (sx < menuTitleX[i] - 4 + menuTitleW[i]) THEN
      RETURN i
    END
  END;
  RETURN -1
END MenuHitTitle;

PROCEDURE ExecuteMenuItem(menu, item: INTEGER);
VAR tid: INTEGER;
BEGIN
  CASE menu OF
    MNU_FILE:
      CASE item OF
        0: (* New *)
          PushUndo(0, 0, PixBuf.Width(pb), PixBuf.Height(pb));
          PixBuf.Clear(pb, bgIdx);
          undoHead := NIL;  undoCount := 0;
          ClearRedo;
          hasSelection := FALSE;
          ResetZoom;
          SetStatus("New canvas") |
        1: (* Open *)
          IF PixBuf.LoadDP2("dpaint_out.dp2") THEN
            SetStatus("Loaded dpaint_out.dp2");
            pb := PixBuf.LayerGetActive();
            undoHead := NIL;  undoCount := 0;
            ClearRedo;
            hasSelection := FALSE;
            dirty := FALSE;
            ResetZoom
          ELSE
            SetStatus("Load failed!")
          END |
        3: (* Save Project *)
          IF PixBuf.SaveDP2("dpaint_out.dp2") THEN
            SetStatus("Saved dpaint_out.dp2");
            dirty := FALSE
          ELSE
            SetStatus("Save failed!")
          END |
        4: (* Save BMP *)
          IF PixBuf.SaveBMP(pb, "dpaint_out.bmp") THEN
            SetStatus("Saved BMP")
          ELSE
            SetStatus("BMP save failed!")
          END |
        5: (* Export PNG *)
          IF PixBuf.SavePNG(pb, "dpaint_out.png") THEN
            SetStatus("Exported PNG")
          ELSE
            SetStatus("PNG export failed!")
          END |
        6: (* Save Palette *)
          IF PixBuf.SavePal(pb, "dpaint_out.pal") THEN
            SetStatus("Palette saved")
          ELSE
            SetStatus("Palette save failed!")
          END |
        8: (* Save Config *)
          SaveConfig;
          SetStatus("Config saved") |
        10: (* Quit *)
          running := FALSE
      ELSE (* skip *)
      END |
    MNU_EDIT:
      CASE item OF
        0: Undo |
        1: Redo |
        3: (* Clear *)
          PushUndo(0, 0, PixBuf.Width(pb), PixBuf.Height(pb));
          PixBuf.Clear(pb, bgIdx);
          undoHead := NIL;  undoCount := 0;
          ClearRedo;
          hasSelection := FALSE;
          ResetZoom |
        4: (* Swap colors *)
          tid := fgIdx;  fgIdx := bgIdx;  bgIdx := tid |
        6: (* Copy *)
          IF hasSelection THEN
            IF selBuf # NIL THEN PixBuf.FreeSave(selBuf) END;
            selBuf := PixBuf.Save(pb, selX, selY, selW, selH)
          END |
        7: (* Paste *)
          IF hasSelection AND (selBuf # NIL) THEN
            PushUndo(selX, selY, PixBuf.SaveW(selBuf), PixBuf.SaveH(selBuf));
            PixBuf.Restore(pb, selBuf, selX, selY)
          END |
        8: (* Delete *)
          IF hasSelection THEN
            PushUndo(selX, selY, selW, selH);
            PixBuf.FillRect(pb, selX, selY, selW, selH, bgIdx);
            hasSelection := FALSE
          END |
        10: (* Flip H *)
          IF hasSelection THEN
            PushUndo(selX, selY, selW, selH);
            PixBuf.FlipH(pb, selX, selY, selW, selH)
          END |
        11: (* Flip V *)
          IF hasSelection THEN
            PushUndo(selX, selY, selW, selH);
            PixBuf.FlipV(pb, selX, selY, selW, selH)
          END |
        12: (* Rotate 90 *)
          IF hasSelection THEN
            PushUndo(selX, selY, selW, selH);
            PixBuf.Rotate90(pb, selX, selY, selW, selH)
          END
      ELSE (* skip *)
      END |
    MNU_TOOLS:
      tid := MenuToolId(item);
      curTool := tid;
      magnifyMode := FALSE;
      polyActive := FALSE;
      PixBuf.PolyReset |
    MNU_VIEW:
      CASE item OF
        0: ZoomToFit |
        1: magnifyMode := NOT magnifyMode |
        2: PopZoom; magnifyMode := FALSE |
        4: showGrid := NOT showGrid |
        5: symmetryX := NOT symmetryX |
        6: symmetryY := NOT symmetryY |
        7: pixelPerfect := NOT pixelPerfect |
        9: tileMode := NOT tileMode;
           IF tileMode THEN SetStatus("Tile mode ON")
           ELSE SetStatus("Tile mode OFF") END |
        10: isFullscreen := NOT isFullscreen;
            IF isFullscreen THEN SetFullscreen(win, FULLSCREEN_DESKTOP)
            ELSE SetFullscreen(win, FULLSCREEN_OFF) END |
        12: showLayerPanel := NOT showLayerPanel |
        13: showHistory := NOT showHistory |
        14: showFrameStrip := NOT showFrameStrip
      ELSE (* skip *)
      END |
    MNU_SET:
      CASE item OF
        0: darkTheme := NOT darkTheme;
           ApplyTheme;
           IF darkTheme THEN SetStatus("Dark theme")
           ELSE SetStatus("Light theme") END |
        1: showCRT := NOT showCRT;
           IF showCRT THEN SetStatus("CRT scanlines ON")
           ELSE SetStatus("CRT OFF") END |
        2: IF hamMode = 0 THEN hamMode := 6; SetStatus("HAM6 display")
           ELSIF hamMode = 6 THEN hamMode := 8; SetStatus("HAM8 display")
           ELSE hamMode := 0; SetStatus("HAM off") END |
        3: copperEnabled := NOT copperEnabled;
           IF copperEnabled THEN SetStatus("Copper gradient ON")
           ELSE SetStatus("Copper OFF") END |
        5: noiseBrush := NOT noiseBrush;
           IF noiseBrush THEN SetStatus("Noise brush ON")
           ELSE SetStatus("Noise brush OFF") END |
        6: onionSkin := NOT onionSkin |
        8: showPrefs := NOT showPrefs
      ELSE (* skip *)
      END |
    MNU_HELP:
      CASE item OF
        0: showShortcuts := NOT showShortcuts |
        1: SetStatus("DPaint M2+ v1.0")
      ELSE (* skip *)
      END
  ELSE (* skip *)
  END
END ExecuteMenuItem;

PROCEDURE HandleMenuClick(sx, sy: INTEGER): BOOLEAN;
VAR hit, item, dx, dy, dw, dh: INTEGER;
BEGIN
  (* Check title bar click *)
  IF sy < MBARH THEN
    hit := MenuHitTitle(sx);
    IF hit >= 0 THEN
      IF hit = menuOpen THEN
        menuOpen := -1;
        menuHover := -1
      ELSE
        menuOpen := hit;
        menuHover := -1
      END;
      RETURN TRUE
    END
  END;
  (* Check dropdown area click *)
  IF menuOpen >= 0 THEN
    dx := menuTitleX[menuOpen] - 4;
    dy := MBARH;
    dw := MenuDropdownWidth(menuOpen);
    dh := MenuDropdownHeight(menuOpen);
    IF (sx >= dx) AND (sx < dx + dw) AND
       (sy >= dy) AND (sy < dy + dh) THEN
      item := MenuItemAtY(menuOpen, sy - dy);
      IF (item >= 0) AND (NOT MenuIsSep(menuOpen, item)) THEN
        ExecuteMenuItem(menuOpen, item);
        menuOpen := -1;
        menuHover := -1
      END;
      RETURN TRUE
    END;
    (* Click outside — close *)
    menuOpen := -1;
    menuHover := -1;
    RETURN TRUE
  END;
  RETURN FALSE
END HandleMenuClick;

PROCEDURE HandleMenuMove(sx, sy: INTEGER);
VAR hit, dx, dy, dw, dh: INTEGER;
BEGIN
  (* Hover over title bar — switch menus *)
  IF sy < MBARH THEN
    hit := MenuHitTitle(sx);
    IF (hit >= 0) AND (hit # menuOpen) THEN
      menuOpen := hit;
      menuHover := -1
    END;
    RETURN
  END;
  (* Hover over dropdown *)
  IF menuOpen >= 0 THEN
    dx := menuTitleX[menuOpen] - 4;
    dy := MBARH;
    dw := MenuDropdownWidth(menuOpen);
    dh := MenuDropdownHeight(menuOpen);
    IF (sx >= dx) AND (sx < dx + dw) AND
       (sy >= dy) AND (sy < dy + dh) THEN
      menuHover := MenuItemAtY(menuOpen, sy - dy)
    ELSE
      menuHover := -1
    END
  END
END HandleMenuMove;

PROCEDURE HandleToolbarClick(sy: INTEGER);
VAR i, bty, bh: INTEGER;
BEGIN
  bh := 28;
  FOR i := 0 TO NTOOLS - 1 DO
    bty := MBARH + 2 + i * (bh + 2);
    IF (sy >= bty) AND (sy < bty + bh) THEN
      curTool := i;
      magnifyMode := FALSE;
      polyActive := FALSE;
      PixBuf.PolyReset;
      RETURN
    END
  END
END HandleToolbarClick;

PROCEDURE HandlePaletteClick(sx, sy, btn: INTEGER);
VAR i, row, col, x, y, cw, ch, mods: INTEGER;
BEGIN
  mods := KeyMod();
  (* Check palette editor slider clicks *)
  IF showPalEdit THEN
    x := 120;  y := WH - PALH - STATH - 90;
    IF (sx >= x + 68) AND (sx < x + 248) AND
       (sy >= y + 4) AND (sy < y + 56) THEN
      i := (sx - x - 68) * 255 DIV 180;
      IF i < 0 THEN i := 0 END;
      IF i > 255 THEN i := 255 END;
      IF (sy >= y + 4) AND (sy < y + 18) THEN
        palEditR := i
      ELSIF (sy >= y + 22) AND (sy < y + 36) THEN
        palEditG := i
      ELSIF (sy >= y + 40) AND (sy < y + 56) THEN
        palEditB := i
      END;
      PixBuf.SetPal(pb, palEditIdx, palEditR, palEditG, palEditB);
      RETURN
    END
  END;
  cw := 24;  ch := 18;  y := WH - PALH - STATH;
  FOR i := 0 TO NCOLORS - 1 DO
    row := i DIV 16;  col := i MOD 16;
    x := TBW + 8 + col * (cw + 2);
    IF (sx >= x) AND (sx < x + cw) AND
       (sy >= y + 4 + row*(ch+2)) AND (sy < y + 4 + row*(ch+2) + ch) THEN
      IF (mods DIV 256) MOD 2 = 1 THEN
        (* Alt+click — open palette editor *)
        palEditIdx := i;
        palEditR := PixBuf.PalR(pb, i);
        palEditG := PixBuf.PalG(pb, i);
        palEditB := PixBuf.PalB(pb, i);
        showPalEdit := TRUE
      ELSIF btn = BUTTON_LEFT THEN
        fgIdx := i
      ELSE
        bgIdx := i
      END;
      RETURN
    END
  END
END HandlePaletteClick;

PROCEDURE HandleMouseDown(btn: INTEGER);
VAR cx, cy, ci, i: INTEGER;
BEGIN
  shiftDown := (KeyMod() MOD 2) = 1;

  (* Menu interaction *)
  IF (menuOpen >= 0) OR (my < MBARH) THEN
    IF HandleMenuClick(mx, my) THEN RETURN END
  END;

  (* Toolbar click *)
  IF (mx < TBW) AND (my >= MBARH) THEN
    HandleToolbarClick(my);
    RETURN
  END;
  (* Frame strip click *)
  IF showFrameStrip THEN
    ci := WH - PALH - STATH - 40;
    IF (my >= ci) AND (my < ci + 36) AND (mx >= TBW + 4) THEN
      i := (mx - TBW - 4) DIV 46;
      IF (i >= 0) AND (i < PixBuf.FrameCount()) THEN
        PixBuf.FrameSet(i);
        pb := PixBuf.FrameGetCurrent();
        PixBuf.LayerInit(pb);
        RETURN
      ELSIF i = PixBuf.FrameCount() THEN
        (* + button — add new frame *)
        IF PixBuf.FrameNew(PixBuf.Width(pb), PixBuf.Height(pb)) >= 0 THEN
          PixBuf.FrameSet(PixBuf.FrameCount() - 1);
          pb := PixBuf.FrameGetCurrent();
          PixBuf.LayerInit(pb);
          SetStatus("Added frame")
        END;
        RETURN
      END
    END
  END;
  (* Palette click *)
  IF my >= WH - PALH - STATH THEN
    HandlePaletteClick(mx, my, btn);
    RETURN
  END;
  (* Status bar — ignore *)
  IF my >= WH - STATH THEN RETURN END;

  (* Layer panel clicks *)
  IF showLayerPanel AND (mx >= WW - LPANW) AND (my >= MBARH) AND
     (my < MBARH + canH) THEN
    ci := (my - MBARH - 24) DIV 26;
    IF ci >= 0 THEN
      IF ci < PixBuf.LayerCount() THEN
        IF mx < WW - LPANW + 20 THEN
          (* Toggle visibility *)
          PixBuf.LayerSetVisible(ci, NOT PixBuf.LayerVisible(ci))
        ELSE
          (* Select layer *)
          PixBuf.LayerSetActive(ci);
          pb := PixBuf.LayerGetActive()
        END
      ELSIF ci = PixBuf.LayerCount() THEN
        (* Add new layer button *)
        i := PixBuf.LayerAdd(PixBuf.Width(pb), PixBuf.Height(pb));
        IF i >= 0 THEN
          PixBuf.LayerSetActive(i);
          pb := PixBuf.LayerGetActive()
        END
      END
    END;
    RETURN
  END;

  IF NOT InCanvas(mx, my) THEN RETURN END;

  ScreenToCanvas(mx, my, cx, cy);

  (* Middle-click — swap FG/BG *)
  IF btn = BUTTON_MIDDLE THEN
    i := fgIdx;  fgIdx := bgIdx;  bgIdx := i;
    RETURN
  END;

  (* Alt+right-click — pick into BG *)
  IF (btn = BUTTON_RIGHT) AND ((KeyMod() DIV 256) MOD 2 = 1) THEN
    bgIdx := PixBuf.GetPix(pb, cx, cy);
    RETURN
  END;

  (* Spacebar pan check — handled in HandleKey *)
  IF panning THEN
    panStartX := mx;  panStartY := my;
    RETURN
  END;

  (* Magnify mode — rubber band zoom *)
  IF magnifyMode THEN
    dragging := TRUE;
    dx0 := cx;  dy0 := cy;
    RETURN
  END;

  (* Eyedropper *)
  IF curTool = T_EYEDROP THEN
    ci := PixBuf.GetPix(pb, cx, cy);
    IF btn = BUTTON_LEFT THEN fgIdx := ci ELSE bgIdx := ci END;
    RETURN
  END;

  (* Flood fill *)
  IF curTool = T_FLOOD THEN
    ci := fgIdx;
    IF btn = BUTTON_RIGHT THEN ci := bgIdx END;
    PushUndo(0, 0, PixBuf.Width(pb), PixBuf.Height(pb));
    PixBuf.FloodFill(pb, cx, cy, ci);
    RETURN
  END;

  (* Replace color — click picks target, replaces all with FG *)
  IF curTool = T_REPLACE THEN
    ci := PixBuf.GetPix(pb, cx, cy);
    IF ci # fgIdx THEN
      PushUndo(0, 0, PixBuf.Width(pb), PixBuf.Height(pb));
      PixBuf.ReplaceColor(pb, ci, fgIdx)
    END;
    RETURN
  END;

  (* Stamp — paste selection buffer at click *)
  IF curTool = T_STAMP THEN
    IF selBuf # NIL THEN
      PushUndo(cx, cy, PixBuf.SaveW(selBuf), PixBuf.SaveH(selBuf));
      PixBuf.Restore(pb, selBuf, cx, cy)
    END;
    RETURN
  END;

  (* Bezier — 4-click tool *)
  IF curTool = T_BEZIER THEN
    bezX[bezCount] := cx;
    bezY[bezCount] := cy;
    INC(bezCount);
    IF bezCount >= 4 THEN
      PushUndo(0, 0, PixBuf.Width(pb), PixBuf.Height(pb));
      PixBuf.Bezier(pb, bezX[0], bezY[0],
                    bezX[1], bezY[1],
                    bezX[2], bezY[2],
                    bezX[3], bezY[3], fgIdx, 64);
      bezCount := 0
    END;
    RETURN
  END;

  (* Text tool *)
  IF curTool = T_TEXT THEN
    IF textLen > 0 THEN
      PushUndo(cx, cy, textLen * 10, 20);
      PixBuf.StampText(pb, ren, font, textBuf, cx, cy, fgIdx)
    END;
    RETURN
  END;

  (* Polygon tool *)
  IF curTool = T_POLYGON THEN
    IF NOT polyActive THEN
      PixBuf.PolyReset;
      polyActive := TRUE
    END;
    PixBuf.PolyAdd(cx, cy);
    (* Double-click to close: if last point near first *)
    IF PixBuf.PolyCount() >= 3 THEN
      IF (AbsI(cx - PixBuf.PolyX(0)) < 8)
      AND (AbsI(cy - PixBuf.PolyY(0)) < 8) THEN
        PushUndo(0, 0, PixBuf.Width(pb), PixBuf.Height(pb));
        IF btn = BUTTON_LEFT THEN
          PixBuf.PolyFill(pb, fgIdx)
        ELSE
          PixBuf.PolyDraw(pb, fgIdx)
        END;
        polyActive := FALSE;
        PixBuf.PolyReset
      END
    END;
    RETURN
  END;

  (* Selection tool *)
  IF curTool = T_SELECT THEN
    dragging := TRUE;
    dx0 := cx;  dy0 := cy;
    RETURN
  END;

  (* Move selection tool *)
  IF curTool = T_MOVE THEN
    IF hasSelection THEN
      dragging := TRUE;
      dx0 := cx;  dy0 := cy;
      moveOldX := selX;  moveOldY := selY;
      (* Save and clear the selection area *)
      IF moveBuf # NIL THEN PixBuf.FreeSave(moveBuf) END;
      moveBuf := PixBuf.Save(pb, selX, selY, selW, selH);
      PushUndo(selX, selY, selW, selH);
      PixBuf.FillRect(pb, selX, selY, selW, selH, bgIdx)
    END;
    RETURN
  END;

  (* Shape tools — start drag *)
  dragging := TRUE;
  dx0 := cx;  dy0 := cy;
  lpx := cx;  lpy := cy;

  (* Freehand tools — immediate first dot *)
  IF (curTool = T_PENCIL) OR (curTool = T_BRUSH) THEN
    ci := fgIdx;
    IF btn = BUTTON_RIGHT THEN ci := bgIdx END;
    ApplyFreehand(cx, cy, cx, cy, ci, lineThick)
  ELSIF curTool = T_ERASER THEN
    ApplyFreehand(cx, cy, cx, cy, bgIdx, lineThick)
  ELSIF curTool = T_SPRAY THEN
    rngState := Ticks();
    ci := fgIdx;
    IF btn = BUTTON_RIGHT THEN ci := bgIdx END;
    ApplyFreehand(cx, cy, cx, cy, ci, lineThick)
  ELSIF curTool = T_AIRBRUSH THEN
    rngState := Ticks();
    ci := fgIdx;
    IF btn = BUTTON_RIGHT THEN ci := bgIdx END;
    FOR i := 1 TO 3 + lineThick DO
      ApplyFreehand(cx + RandRange(-lineThick * 2, lineThick * 2),
                    cy + RandRange(-lineThick * 2, lineThick * 2),
                    cx + RandRange(-lineThick * 2, lineThick * 2),
                    cy + RandRange(-lineThick * 2, lineThick * 2),
                    ci, 1)
    END
  ELSIF (curTool = T_LIGHTEN) OR (curTool = T_DARKEN) THEN
    (* Lighten/darken at pixel *)
    ci := PixBuf.GetPix(pb, cx, cy);
    IF curTool = T_LIGHTEN THEN
      IF ci > 0 THEN ci := ci - 1 END
    ELSE
      IF ci < NCOLORS - 1 THEN ci := ci + 1 END
    END;
    ApplyFreehand(cx, cy, cx, cy, ci, lineThick)
  END
END HandleMouseDown;

PROCEDURE UpdateTooltip;
VAR toolIdx: INTEGER;
BEGIN
  IF (mx < TBW) AND (my >= MBARH) THEN
    toolIdx := (my - MBARH - 2) DIV 30;
    IF (toolIdx >= 0) AND (toolIdx < NTOOLS) THEN
      IF toolIdx # hoverTool THEN
        hoverTool := toolIdx;
        hoverStart := Ticks();
        showTooltip := FALSE
      ELSIF (NOT showTooltip) AND (Ticks() - hoverStart > 500) THEN
        showTooltip := TRUE
      END;
      RETURN
    END
  END;
  hoverTool := -1;
  showTooltip := FALSE
END UpdateTooltip;

PROCEDURE UpdateCursor;
BEGIN
  IF InCanvas(mx, my) THEN
    IF (curTool = T_TEXT) THEN
      SetCursor(CURSOR_IBEAM)
    ELSIF (curTool = T_EYEDROP) THEN
      SetCursor(CURSOR_CROSSHAIR)
    ELSIF (curTool = T_MOVE) OR panning THEN
      SetCursor(CURSOR_HAND)
    ELSE
      SetCursor(CURSOR_CROSSHAIR)
    END
  ELSIF (mx < TBW) OR (my >= WH - PALH - STATH) THEN
    SetCursor(CURSOR_ARROW)
  ELSE
    SetCursor(CURSOR_ARROW)
  END
END UpdateCursor;

PROCEDURE HandleMouseMove;
VAR cx, cy, ci, sx, sy, i: INTEGER;
BEGIN
  IF menuOpen >= 0 THEN
    HandleMenuMove(mx, my);
    RETURN
  END;
  shiftDown := (KeyMod() MOD 2) = 1;
  UpdateTooltip;
  UpdateCursor;

  (* Pan *)
  IF panning AND dragging THEN
    IF zoomed THEN
      zoomX := zoomX - (mx - panStartX) * zoomW DIV canW;
      zoomY := zoomY - (my - panStartY) * zoomH DIV canH;
      IF zoomX < 0 THEN zoomX := 0 END;
      IF zoomY < 0 THEN zoomY := 0 END;
      IF zoomX + zoomW > PixBuf.Width(pb) THEN
        zoomX := PixBuf.Width(pb) - zoomW END;
      IF zoomY + zoomH > PixBuf.Height(pb) THEN
        zoomY := PixBuf.Height(pb) - zoomH END;
      panStartX := mx;  panStartY := my
    END;
    RETURN
  END;

  IF NOT dragging THEN RETURN END;
  IF NOT InCanvas(mx, my) THEN RETURN END;
  IF magnifyMode THEN RETURN END;

  ScreenToCanvas(mx, my, cx, cy);
  ci := fgIdx;

  IF (curTool = T_PENCIL) OR (curTool = T_BRUSH) THEN
    ApplyFreehand(lpx, lpy, cx, cy, ci, lineThick);
    lpx := cx;  lpy := cy
  ELSIF curTool = T_ERASER THEN
    ApplyFreehand(lpx, lpy, cx, cy, bgIdx, lineThick);
    lpx := cx;  lpy := cy
  ELSIF curTool = T_SPRAY THEN
    FOR i := 1 TO 8 + lineThick * 2 DO
      sx := cx + RandRange(-lineThick * 3, lineThick * 3);
      sy := cy + RandRange(-lineThick * 3, lineThick * 3);
      ApplyFreehand(sx, sy, sx, sy, ci, lineThick)
    END
  ELSIF curTool = T_AIRBRUSH THEN
    (* Airbrush accumulates more density the longer held *)
    FOR i := 1 TO 5 + lineThick * 2 DO
      sx := cx + RandRange(-lineThick * 2, lineThick * 2);
      sy := cy + RandRange(-lineThick * 2, lineThick * 2);
      ApplyFreehand(sx, sy, sx, sy, ci, 1)
    END
  ELSIF curTool = T_SMUDGE THEN
    (* Smudge: read pixel at old pos, write at new pos *)
    ci := PixBuf.GetPix(pb, lpx, lpy);
    ApplyFreehand(lpx, lpy, cx, cy, ci, lineThick);
    lpx := cx;  lpy := cy
  ELSIF curTool = T_MOVE THEN
    (* Move selection content *)
    IF hasSelection AND (moveBuf # NIL) THEN
      selX := moveOldX + (cx - dx0);
      selY := moveOldY + (cy - dy0)
    END
  ELSIF (curTool = T_LIGHTEN) OR (curTool = T_DARKEN) THEN
    ci := PixBuf.GetPix(pb, cx, cy);
    IF curTool = T_LIGHTEN THEN
      IF ci > 0 THEN ci := ci - 1 END
    ELSE
      IF ci < NCOLORS - 1 THEN ci := ci + 1 END
    END;
    ApplyFreehand(lpx, lpy, cx, cy, ci, lineThick);
    lpx := cx;  lpy := cy
  ELSIF curTool = T_EYEDROP THEN
    ci := PixBuf.GetPix(pb, cx, cy);
    fgIdx := ci
  END
END HandleMouseMove;

PROCEDURE HandleMouseUp;
VAR cx, cy, zx, zy, zw, zh, ci: INTEGER;
BEGIN
  IF NOT dragging THEN RETURN END;
  dragging := FALSE;

  IF panning THEN RETURN END;

  IF NOT InCanvas(mx, my) THEN RETURN END;
  ScreenToCanvas(mx, my, cx, cy);

  (* Magnify mode — commit zoom *)
  IF magnifyMode THEN
    zx := MinI(dx0, cx);  zy := MinI(dy0, cy);
    zw := AbsI(cx - dx0);  zh := AbsI(cy - dy0);
    IF (zw > 4) AND (zh > 4) THEN
      PushZoom(zx, zy, zw, zh)
    END;
    magnifyMode := FALSE;
    RETURN
  END;

  IF shiftDown THEN Constrain45(dx0, dy0, cx, cy) END;

  (* Selection tool — commit selection *)
  IF curTool = T_SELECT THEN
    selX := MinI(dx0, cx);  selY := MinI(dy0, cy);
    selW := AbsI(cx - dx0);  selH := AbsI(cy - dy0);
    hasSelection := (selW > 0) AND (selH > 0);
    RETURN
  END;

  (* Move selection — commit *)
  IF curTool = T_MOVE THEN
    IF hasSelection AND (moveBuf # NIL) THEN
      PushUndo(selX, selY, selW, selH);
      PixBuf.Restore(pb, moveBuf, selX, selY);
      PixBuf.FreeSave(moveBuf);
      moveBuf := NIL
    END;
    RETURN
  END;

  (* Angled gradient — drag angle determines direction *)
  IF curTool = T_GRADANG THEN
    IF hasSelection THEN
      (* Compute angle from drag vector *)
      ci := 0;
      IF (AbsI(cx - dx0) > 2) OR (AbsI(cy - dy0) > 2) THEN
        (* Approximate angle: atan2 in degrees via integer math *)
        IF AbsI(cx - dx0) >= AbsI(cy - dy0) THEN
          IF cx > dx0 THEN ci := 0 ELSE ci := 180 END
        ELSE
          IF cy > dy0 THEN ci := 90 ELSE ci := 270 END
        END
      END;
      PushUndo(selX, selY, selW, selH);
      PixBuf.GradientAngle(pb, selX, selY, selW, selH,
                           fgIdx, bgIdx, ci, NCOLORS)
    END;
    RETURN
  END;

  (* Shape tools — commit *)
  ci := fgIdx;
  IF (curTool >= T_LINE) AND (curTool <= T_PATTERN) THEN
    ApplyShape(curTool, dx0, dy0, cx, cy, ci, lineThick)
  END
END HandleMouseUp;

PROCEDURE HandleKey(key: INTEGER);
VAR mods: INTEGER;
BEGIN
  mods := KeyMod();

  IF key = KEY_ESCAPE THEN
    IF menuOpen >= 0 THEN
      menuOpen := -1; menuHover := -1
    ELSIF showShortcuts THEN
      showShortcuts := FALSE
    ELSIF showPrefs THEN
      showPrefs := FALSE
    ELSIF showPalEdit THEN
      showPalEdit := FALSE
    ELSIF showHistory THEN
      showHistory := FALSE
    ELSIF polyActive THEN
      polyActive := FALSE;
      PixBuf.PolyReset
    ELSIF hasSelection THEN
      hasSelection := FALSE
    ELSIF textInputMode THEN
      textInputMode := FALSE
    ELSE
      running := FALSE
    END
  ELSIF key = 122 THEN       (* z — undo *)
    Undo
  ELSIF key = 114 THEN       (* r — redo *)
    Redo
  ELSIF key = 99 THEN        (* c — clear *)
    PushUndo(0, 0, PixBuf.Width(pb), PixBuf.Height(pb));
    PixBuf.Clear(pb, bgIdx);
    undoHead := NIL;  undoCount := 0;
    ClearRedo;
    hasSelection := FALSE;
    ResetZoom
  ELSIF key = 109 THEN       (* m — magnify *)
    magnifyMode := NOT magnifyMode
  ELSIF key = 110 THEN       (* n *)
    IF showFrameStrip AND ((mods DIV 4) MOD 2 = 1) THEN
      (* Shift+N — new blank frame *)
      IF PixBuf.FrameNew(PixBuf.Width(pb), PixBuf.Height(pb)) >= 0 THEN
        PixBuf.FrameSet(PixBuf.FrameCount() - 1);
        pb := PixBuf.FrameGetCurrent();
        PixBuf.LayerInit(pb);
        SetStatus("New blank frame")
      END
    ELSIF showFrameStrip AND ((mods DIV 2) MOD 2 = 1) THEN
      (* Ctrl+N — duplicate current frame *)
      IF PixBuf.FrameDuplicate(PixBuf.FrameCurrent()) # NIL THEN
        PixBuf.FrameSet(PixBuf.FrameCount() - 1);
        pb := PixBuf.FrameGetCurrent();
        PixBuf.LayerInit(pb);
        SetStatus("Frame duplicated")
      END
    ELSE
      PopZoom;
      magnifyMode := FALSE
    END
  ELSIF key = 103 THEN       (* g — gradient *)
    curTool := T_GRADIENT;  magnifyMode := FALSE
  ELSIF key = 101 THEN       (* e — eraser *)
    curTool := T_ERASER;  magnifyMode := FALSE
  ELSIF key = 102 THEN       (* f — flood fill *)
    curTool := T_FLOOD;  magnifyMode := FALSE
  ELSIF key = 105 THEN       (* i — eyedropper *)
    curTool := T_EYEDROP;  magnifyMode := FALSE
  ELSIF key = 116 THEN       (* t — text *)
    curTool := T_TEXT;  magnifyMode := FALSE
  ELSIF key = 112 THEN       (* p *)
    IF (mods DIV 2) MOD 2 = 1 THEN
      (* Ctrl+P — save palette *)
      IF PixBuf.SavePal(pb, "dpaint_out.pal") THEN
        SetStatus("Palette saved")
      ELSE
        SetStatus("Palette save failed!")
      END
    ELSE
      curTool := T_POLYGON;  magnifyMode := FALSE;
      polyActive := FALSE;  PixBuf.PolyReset
    END
  ELSIF key = 120 THEN       (* x — toggle X symmetry *)
    symmetryX := NOT symmetryX
  ELSIF key = 121 THEN       (* y — toggle Y symmetry *)
    symmetryY := NOT symmetryY
  ELSIF key = 100 THEN       (* d — toggle grid *)
    showGrid := NOT showGrid
  ELSIF key = 119 THEN       (* w — pixel-perfect toggle *)
    pixelPerfect := NOT pixelPerfect
  ELSIF key = 108 THEN       (* l — toggle layer panel *)
    showLayerPanel := NOT showLayerPanel
  ELSIF key = 115 THEN       (* s — save *)
    IF (mods DIV 2) MOD 2 = 1 THEN
      (* Ctrl+S — save .dp2 project *)
      IF PixBuf.SaveDP2("dpaint_out.dp2") THEN
        SetStatus("Saved dpaint_out.dp2");
        dirty := FALSE
      ELSE
        SetStatus("Save failed!")
      END
    ELSIF (mods DIV 4) MOD 2 = 1 THEN
      (* Shift+S — export PNG *)
      IF PixBuf.SavePNG(pb, "dpaint_out.png") THEN
        SetStatus("Exported PNG")
      ELSE
        SetStatus("PNG export failed!")
      END
    ELSE
      (* s alone — save BMP for backwards compat *)
      IF PixBuf.SaveBMP(pb, "dpaint_out.bmp") THEN
        SetStatus("Saved BMP")
      ELSE
        SetStatus("BMP save failed!")
      END
    END
  ELSIF (key = 111) AND ((mods DIV 2) MOD 2 = 1) THEN  (* Ctrl+O — load .dp2 *)
    IF PixBuf.LoadDP2("dpaint_out.dp2") THEN
      SetStatus("Loaded dpaint_out.dp2");
      pb := PixBuf.LayerGetActive();
      undoHead := NIL;  undoCount := 0;
      ClearRedo;
      hasSelection := FALSE;
      dirty := FALSE;
      ResetZoom
    ELSE
      SetStatus("Load failed!")
    END
  ELSIF key = 91 THEN        (* [ — thinner *)
    IF lineThick > 1 THEN DEC(lineThick) END
  ELSIF key = 93 THEN        (* ] — thicker *)
    IF lineThick < MAX_THICK THEN INC(lineThick) END
  ELSIF key = 32 THEN        (* space — pan mode *)
    panning := TRUE;
    panStartX := mx;  panStartY := my;
    dragging := TRUE
  ELSIF key = 48 THEN        (* 0 — zoom to fit *)
    ZoomToFit
  ELSIF key = 61 THEN        (* = — swap FG/BG *)
    key := fgIdx;  fgIdx := bgIdx;  bgIdx := key
  ELSIF (key >= 49) AND (key <= 57) THEN
    curTool := key - 49;
    magnifyMode := FALSE;
    polyActive := FALSE;
    PixBuf.PolyReset

  (* ─── UI overlay toggles ─── *)
  ELSIF key = 47 THEN         (* ? (slash) — shortcut overlay *)
    showShortcuts := NOT showShortcuts
  ELSIF key = 104 THEN        (* h *)
    IF (mods DIV 2) MOD 2 = 0 THEN  (* plain h — history panel *)
      showHistory := NOT showHistory
    END
  ELSIF (key = 116) AND ((mods DIV 2) MOD 2 = 1) THEN  (* Ctrl+T — toggle theme *)
    darkTheme := NOT darkTheme;
    ApplyTheme;
    IF darkTheme THEN SetStatus("Dark theme")
    ELSE SetStatus("Light theme") END
  ELSIF key = 1073741892 THEN  (* F11 — fullscreen toggle *)
    isFullscreen := NOT isFullscreen;
    IF isFullscreen THEN
      SetFullscreen(win, FULLSCREEN_DESKTOP)
    ELSE
      SetFullscreen(win, FULLSCREEN_OFF)
    END

  (* ─── Animation controls ─── *)
  ELSIF key = 97 THEN          (* a — toggle frame strip *)
    showFrameStrip := NOT showFrameStrip
  ELSIF key = 111 THEN         (* o *)
    IF (mods DIV 2) MOD 2 = 0 THEN  (* plain o — onion skin *)
      onionSkin := NOT onionSkin
    END
  ELSIF key = 1073741903 THEN  (* Right arrow — next frame *)
    IF showFrameStrip AND (PixBuf.FrameCurrent() < PixBuf.FrameCount() - 1) THEN
      PixBuf.FrameSet(PixBuf.FrameCurrent() + 1);
      pb := PixBuf.FrameGetCurrent();
      PixBuf.LayerInit(pb)
    END
  ELSIF key = 1073741904 THEN  (* Left arrow — prev frame *)
    IF showFrameStrip AND (PixBuf.FrameCurrent() > 0) THEN
      PixBuf.FrameSet(PixBuf.FrameCurrent() - 1);
      pb := PixBuf.FrameGetCurrent();
      PixBuf.LayerInit(pb)
    END
  ELSIF (key = 1073741882) AND showFrameStrip THEN  (* F1 — playback toggle *)
    playingAnim := NOT playingAnim;
    playTick := Ticks()

  (* ─── Advanced feature toggles ─── *)
  ELSIF (key = 1073741883) THEN  (* F2 — toggle tile mode *)
    tileMode := NOT tileMode;
    IF tileMode THEN SetStatus("Tile mode ON")
    ELSE SetStatus("Tile mode OFF") END
  ELSIF (key = 1073741884) THEN  (* F3 — cycle HAM mode *)
    IF hamMode = 0 THEN hamMode := 6; SetStatus("HAM6 display")
    ELSIF hamMode = 6 THEN hamMode := 8; SetStatus("HAM8 display")
    ELSE hamMode := 0; SetStatus("HAM off") END
  ELSIF (key = 1073741885) THEN  (* F4 — copper gradient toggle *)
    copperEnabled := NOT copperEnabled;
    IF copperEnabled THEN SetStatus("Copper gradient ON")
    ELSE SetStatus("Copper OFF") END
  ELSIF (key = 1073741886) THEN  (* F5 — CRT overlay toggle *)
    showCRT := NOT showCRT;
    IF showCRT THEN SetStatus("CRT scanlines ON")
    ELSE SetStatus("CRT OFF") END
  ELSIF (key = 1073741887) THEN  (* F6 — preferences *)
    showPrefs := NOT showPrefs
  ELSIF key = 98 THEN            (* b *)
    IF (mods DIV 4) MOD 2 = 1 THEN
      (* Shift+B — toggle noise brush *)
      noiseBrush := NOT noiseBrush;
      IF noiseBrush THEN SetStatus("Noise brush ON")
      ELSE SetStatus("Noise brush OFF") END
    ELSIF hasSelection THEN
      IF brushBuf # NIL THEN PixBuf.FreeSave(brushBuf) END;
      brushBuf := PixBuf.Save(pb, selX, selY, selW, selH);
      SetStatus("Brush captured")
    END
  ELSIF (key = 107) AND ((mods DIV 2) MOD 2 = 1) THEN  (* Ctrl+K — save config *)
    SaveConfig
  END;

  (* Selection operations *)
  IF hasSelection THEN
    IF (key = 104) AND ((mods DIV 2) MOD 2 = 1) THEN  (* Ctrl+H — flip H *)
      PushUndo(selX, selY, selW, selH);
      PixBuf.FlipH(pb, selX, selY, selW, selH)
    ELSIF (key = 118) AND ((mods DIV 2) MOD 2 = 1) THEN (* Ctrl+V — flip V *)
      PushUndo(selX, selY, selW, selH);
      PixBuf.FlipV(pb, selX, selY, selW, selH)
    ELSIF (key = 99) AND ((mods DIV 2) MOD 2 = 1) THEN  (* Ctrl+C — copy *)
      IF selBuf # NIL THEN PixBuf.FreeSave(selBuf) END;
      selBuf := PixBuf.Save(pb, selX, selY, selW, selH)
    ELSIF (key = 118) AND ((mods DIV 2) MOD 2 = 0) THEN (* v — paste *)
      IF selBuf # NIL THEN
        PushUndo(selX, selY, PixBuf.SaveW(selBuf), PixBuf.SaveH(selBuf));
        PixBuf.Restore(pb, selBuf, selX, selY)
      END
    ELSIF (key = 114) AND ((mods DIV 2) MOD 2 = 1) THEN  (* Ctrl+R — rotate 90 *)
      PushUndo(selX, selY, selW, selH);
      PixBuf.Rotate90(pb, selX, selY, selW, selH)
    ELSIF key = 261 THEN    (* Delete — fill selection with BG *)
      PushUndo(selX, selY, selW, selH);
      PixBuf.FillRect(pb, selX, selY, selW, selH, bgIdx);
      hasSelection := FALSE
    END
  END;

  (* Text input — type characters into textBuf *)
  IF curTool = T_TEXT THEN
    IF (key >= 32) AND (key <= 126) AND (textLen < 255) THEN
      textBuf[textLen] := CHR(key);
      INC(textLen);
      textBuf[textLen] := 0C
    ELSIF key = 8 THEN  (* backspace *)
      IF textLen > 0 THEN
        DEC(textLen);
        textBuf[textLen] := 0C
      END
    ELSIF key = 13 THEN  (* enter — reset text *)
      textLen := 0;
      textBuf[0] := 0C
    END
  END
END HandleKey;

PROCEDURE HandleKeyUp(key: INTEGER);
BEGIN
  IF key = 32 THEN   (* space released — end pan *)
    panning := FALSE;
    dragging := FALSE
  END
END HandleKeyUp;

(* ═══════════════════════════════════════════════════════════════
   Main Loop
   ═══════════════════════════════════════════════════════════════ *)

CONST
  FPS_LIMIT  = 60;
  FRAME_MS   = 1000 DIV FPS_LIMIT;

PROCEDURE MainLoop;
VAR ev, btn, wy: INTEGER;
    frameStart, elapsed: INTEGER;
BEGIN
  running := TRUE;
  WHILE running DO
    frameStart := Ticks();
    ev := Poll();
    WHILE ev # 0 DO
      IF ev = QUIT_EVENT THEN
        running := FALSE
      ELSIF ev = KEYDOWN THEN
        HandleKey(KeyCode())
      ELSIF ev = 3 THEN      (* KEYUP *)
        HandleKeyUp(KeyCode())
      ELSIF ev = MOUSEDOWN THEN
        mx := MouseX();  my := MouseY();
        HandleMouseDown(MouseButton())
      ELSIF ev = MOUSEUP THEN
        mx := MouseX();  my := MouseY();
        HandleMouseUp
      ELSIF ev = MOUSEMOVE THEN
        mx := MouseX();  my := MouseY();
        HandleMouseMove
      ELSIF ev = MOUSEWHEEL THEN
        wy := WheelY();
        IF InCanvas(mx, my) THEN
          WheelZoom(wy, mx, my)
        END
      ELSIF ev = WINDOW_EVENT THEN
        IF WindowEvent() = WEVT_RESIZED THEN
          winW := GetWindowWidth(win);
          winH := GetWindowHeight(win)
        END
      END;
      ev := Poll()
    END;
    (* Animation playback *)
    IF playingAnim AND showFrameStrip AND (PixBuf.FrameCount() > 1) THEN
      IF Ticks() - playTick >= PixBuf.FrameTiming(PixBuf.FrameCurrent()) THEN
        IF PixBuf.FrameCurrent() >= PixBuf.FrameCount() - 1 THEN
          PixBuf.FrameSet(0)
        ELSE
          PixBuf.FrameSet(PixBuf.FrameCurrent() + 1)
        END;
        pb := PixBuf.FrameGetCurrent();
        playTick := Ticks()
      END
    END;
    (* Autosave *)
    IF dirty AND (Ticks() - lastAutoSave > AUTOSAVE_MS) THEN
      IF PixBuf.SaveDP2("dpaint_autosave.dp2") THEN
        SetStatus("Autosaved")
      END;
      lastAutoSave := Ticks();
      dirty := FALSE
    END;
    DrawFrame;
    elapsed := Ticks() - frameStart;
    IF elapsed < FRAME_MS THEN
      Delay(FRAME_MS - elapsed)
    END
  END
END MainLoop;

(* ═══════════════════════════════════════════════════════════════
   Entry — TRY / EXCEPT / FINALLY
   ═══════════════════════════════════════════════════════════════ *)

BEGIN
  curTool := T_PENCIL;
  fgIdx := 0;  bgIdx := 1;
  lineThick := 2;
  dragging := FALSE;
  magnifyMode := FALSE;
  panning := FALSE;
  polyActive := FALSE;
  hasSelection := FALSE;
  selBuf := NIL;
  symmetryX := FALSE;  symmetryY := FALSE;
  showGrid := FALSE;
  textLen := 0;  textBuf[0] := 0C;
  textInputMode := FALSE;
  shiftDown := FALSE;
  undoHead := NIL;  undoCount := 0;
  redoHead := NIL;  redoCount := 0;
  bezCount := 0;
  moveBuf := NIL;
  lassoActive := FALSE;
  lassoCount := 0;
  pixelPerfect := FALSE;
  transparentIdx := -1;
  showTransparency := FALSE;
  showLayerPanel := FALSE;
  zoomStack := NIL;
  mx := 0;  my := 0;
  rngState := 12345;
  lastAutoSave := 0;
  dirty := FALSE;
  statusMsg[0] := 0C;
  statusTick := 0;
  hoverTool := -1;
  hoverStart := 0;
  showTooltip := FALSE;
  darkTheme := TRUE;
  ApplyTheme;
  showShortcuts := FALSE;
  showPalEdit := FALSE;
  palEditIdx := 0;
  palEditR := 0;  palEditG := 0;  palEditB := 0;
  showHistory := FALSE;
  isFullscreen := FALSE;
  showBrushPreview := TRUE;
  winW := WW;  winH := WH;
  showPrefs := FALSE;
  showFrameStrip := FALSE;
  onionSkin := FALSE;
  playingAnim := FALSE;
  playTick := 0;
  tileMode := FALSE;
  tileW := 16;  tileH := 16;
  brushBuf := NIL;
  noiseBrush := FALSE;
  showCRT := FALSE;
  hamMode := 0;
  copperEnabled := FALSE;
  menuOpen := -1;
  menuHover := -1;

  canW := WW - TBW;
  canH := WH - MBARH - PALH - STATH;

  pb := PixBuf.Create(canW, canH);
  IF pb = NIL THEN
    WriteString("Error: cannot allocate pixel buffer."); WriteLn;
    HALT
  END;
  InitPalette;
  PixBuf.Clear(pb, 1);   (* clear to white (index 1) *)
  PixBuf.LayerInit(pb);
  PixBuf.FrameInit(pb);
  LoadConfig;  (* restore preferences from dpaint.cfg if it exists *)

  displayBuf := PixBuf.Create(canW, canH);
  IF displayBuf = NIL THEN
    WriteString("Error: cannot allocate display buffer."); WriteLn;
    HALT
  END;

  ResetZoom;

  TRY
    InitGraphics;
    MainLoop
  EXCEPT InitFailed DO
    WriteString("Error: failed to initialise SDL2 graphics."); WriteLn
  FINALLY
    SaveConfig;
    Cleanup
  END
END DPaint.
