MODULE AmbiguousEnum;
(* Tests last-import-wins: FROM AE_A IMPORT GetA; FROM AE_B IMPORT GetA;
   The second import should shadow the first. GetA() should return 2 (from AE_B). *)
FROM InOut IMPORT WriteInt, WriteLn;
FROM AE_A IMPORT GetA;
FROM AE_B IMPORT GetA;
VAR r: INTEGER;
BEGIN
  r := GetA();
  WriteInt(r, 0); WriteLn
END AmbiguousEnum.
