MODULE M2PlusRef;
(* Test Modula-2+ REF types and REFANY *)
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

TYPE
  IntRef = REF INTEGER;
  Point = RECORD
    x, y: INTEGER;
  END;
  PointRef = REF Point;

VAR
  ir: IntRef;
  pr: PointRef;

BEGIN
  WriteString("=== M2+ REF Type Test ==="); WriteLn;

  (* Allocate a REF INTEGER *)
  NEW(ir);
  ir^ := 42;
  WriteString("IntRef value: "); WriteInt(ir^, 1); WriteLn;

  (* Allocate a REF Point *)
  NEW(pr);
  pr^.x := 10;
  pr^.y := 20;
  WriteString("Point: ("); WriteInt(pr^.x, 1);
  WriteString(", "); WriteInt(pr^.y, 1);
  WriteString(")"); WriteLn;

  WriteString("Done"); WriteLn
END M2PlusRef.
