MODULE MTLinkTrap;
FROM InOut IMPORT WriteInt, WriteLn;
FROM MTL_A IMPORT GetValue;
FROM MTL_B IMPORT GetValue;
IMPORT MTL_A;
IMPORT MTL_B;
VAR a, b: INTEGER;
BEGIN
  (* Use qualified access to get values from each module *)
  a := MTL_A.GetValue();
  b := MTL_B.GetValue();
  WriteInt(a, 0);
  WriteLn;
  WriteInt(b, 0);
  WriteLn;
  (* Also verify the exported variables *)
  WriteInt(MTL_A.value, 0);
  WriteLn;
  WriteInt(MTL_B.value, 0);
  WriteLn
END MTLinkTrap.
