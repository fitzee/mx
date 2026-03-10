MODULE StringOps;
FROM InOut IMPORT WriteString, WriteInt, WriteLn, Write;
FROM Strings IMPORT Length, Pos, Concat, Copy, Insert, Delete;

VAR
  s1, s2, result: ARRAY [0..63] OF CHAR;
  pos: CARDINAL;
  len: CARDINAL;

BEGIN
  (* Test Length *)
  s1 := "Hello";
  WriteString("Length of 'Hello': "); WriteInt(Length(s1), 1); WriteLn;

  (* Test Concat *)
  s1 := "Hello, ";
  s2 := "World!";
  Concat(s1, s2, result);
  WriteString("Concat: "); WriteString(result); WriteLn;

  (* Test Copy *)
  s1 := "Hello, World!";
  Copy(s1, 7, 5, result);
  WriteString("Copy(7,5): "); WriteString(result); WriteLn;

  (* Test Pos *)
  s1 := "World";
  s2 := "Hello, World!";
  pos := Pos(s1, s2);
  WriteString("Pos of 'World' in 'Hello, World!': "); WriteInt(pos, 1); WriteLn;

  (* Test Insert *)
  s1 := "Hello World";
  Insert(", ", s1, 5);
  WriteString("After Insert: "); WriteString(s1); WriteLn;

  (* Test Delete *)
  s1 := "Hello, World!";
  Delete(s1, 5, 2);
  WriteString("After Delete: "); WriteString(s1); WriteLn;

  WriteString("Done"); WriteLn
END StringOps.
