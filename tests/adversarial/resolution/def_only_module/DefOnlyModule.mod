MODULE DefOnlyModule;
(* Regression test: definition-only modules (no .mod) must have
   their types emitted in the C output so that other modules can
   reference them in procedure signatures and variable declarations. *)

FROM InOut IMPORT WriteString, WriteLn, WriteInt;
FROM SharedTypes IMPORT Color, Coord, Status, Red, Green, Blue,
                        StOk, StNotFound, MaxItems;
FROM Service IMPORT Init, SetColor, GetCoord;
FROM Backend IMPORT Handle;

VAR
  h: Handle;
  st: Status;
  p: Coord;
  c: Color;

BEGIN
  (* Test enum values from def-only module *)
  c := Red;
  WriteString("color="); WriteInt(ORD(c), 1); WriteLn;

  c := Blue;
  WriteString("color="); WriteInt(ORD(c), 1); WriteLn;

  (* Test constant from def-only module *)
  WriteString("max="); WriteInt(MaxItems, 1); WriteLn;

  (* Test record from def-only module *)
  p.x := 10;
  p.y := 20;
  WriteString("coord="); WriteInt(p.x, 1);
  WriteString(","); WriteInt(p.y, 1); WriteLn;

  (* Test cross-module calls using def-only types *)
  st := Init(h);
  IF st = StOk THEN
    WriteString("init OK"); WriteLn
  END;

  st := SetColor(h, Green);
  IF st = StOk THEN
    WriteString("setcolor OK"); WriteLn
  END;

  st := GetCoord(h, p);
  IF st = StOk THEN
    WriteString("getcoord OK"); WriteLn
  END;

  WriteString("all def-only module OK"); WriteLn
END DefOnlyModule.
