MODULE RecordFields;
FROM InOut IMPORT WriteInt, WriteString, WriteLn;
FROM RecLib IMPORT Point, Info, MakePoint, SumPoint, InitInfo, InfoTag;

VAR
  p: Point;
  inf: Info;
  s, t: INTEGER;
BEGIN
  MakePoint(3, 7, p);
  s := SumPoint(p);
  WriteInt(s, 0); WriteLn;        (* 10 *)

  WriteInt(p.x, 0); WriteLn;     (* 3 *)
  WriteInt(p.y, 0); WriteLn;     (* 7 *)

  InitInfo(inf, 42, 99);
  t := InfoTag(inf);
  WriteInt(t, 0); WriteLn;       (* 42 *)
  WriteInt(inf.val, 0); WriteLn;  (* 99 *)

  IF inf.flag THEN
    WriteString("T"); WriteLn    (* T *)
  ELSE
    WriteString("F"); WriteLn
  END;

  (* Verify char field *)
  IF inf.ch = "A" THEN
    WriteString("A"); WriteLn    (* A *)
  END
END RecordFields.
