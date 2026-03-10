MODULE StrLib;
(* Test Strings standard library module *)
FROM InOut IMPORT WriteString, WriteInt, WriteLn;
FROM Strings IMPORT Length, Concat, Copy, Pos, Assign, Delete, Insert;

VAR
  s1, s2, s3: ARRAY [0..79] OF CHAR;
  pos: CARDINAL;

BEGIN
  (* Length *)
  s1 := "Hello";
  WriteString("Length('Hello') = "); WriteInt(Length(s1), 1); WriteLn;

  (* Concat *)
  s1 := "Hello";
  s2 := " World";
  Concat(s1, s2, s3);
  WriteString("Concat: "); WriteString(s3); WriteLn;

  (* Copy (extract substring) *)
  s1 := "Hello World";
  Copy(s1, 6, 5, s2);
  WriteString("Copy(6,5): "); WriteString(s2); WriteLn;

  (* Pos (find substring) *)
  s1 := "Hello World";
  pos := Pos("World", s1);
  WriteString("Pos('World'): "); WriteInt(pos, 1); WriteLn;

  pos := Pos("xyz", s1);
  WriteString("Pos('xyz'): "); WriteInt(pos, 1); WriteLn;

  (* Assign *)
  Assign("Copied!", s1);
  WriteString("Assign: "); WriteString(s1); WriteLn;

  (* Delete *)
  s1 := "Hello World";
  Delete(s1, 5, 6);
  WriteString("Delete(5,6): "); WriteString(s1); WriteLn;

  (* Insert *)
  s1 := "Hello!";
  Insert(" World", s1, 5);
  WriteString("Insert: "); WriteString(s1); WriteLn;

  WriteString("Done"); WriteLn
END StrLib.
