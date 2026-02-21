MODULE QualifiedAccess;
FROM InOut IMPORT WriteInt, WriteLn;
IMPORT QualA;
IMPORT QualB;
VAR r: INTEGER;
BEGIN
  r := QualA.Add(3, 4);
  WriteInt(r, 0); WriteLn;   (* 7 *)
  r := QualB.Add(3, 4);
  WriteInt(r, 0); WriteLn;   (* 12 *)
  r := QualA.Name();
  WriteInt(r, 0); WriteLn;   (* 1 *)
  r := QualB.Name();
  WriteInt(r, 0); WriteLn    (* 2 *)
END QualifiedAccess.
