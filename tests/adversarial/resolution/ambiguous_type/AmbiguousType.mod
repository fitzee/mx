MODULE AmbiguousType;
(* Tests last-import-wins for types+procs:
   FROM AT_A IMPORT Tag, GetTag; FROM AT_B IMPORT Tag, GetTag;
   The second import should shadow. GetTag() should return 2 (from AT_B). *)
FROM InOut IMPORT WriteInt, WriteLn;
FROM AT_A IMPORT Tag, GetTag;
FROM AT_B IMPORT Tag, GetTag;
VAR t: Tag;
BEGIN
  t := GetTag();
  WriteInt(t, 0); WriteLn
END AmbiguousType.
