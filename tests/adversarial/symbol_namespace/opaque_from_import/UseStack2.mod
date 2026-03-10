MODULE UseStack2;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;
FROM Stack IMPORT Stack, Create, Push, Pop, IsEmpty, Destroy;

VAR
  s: Stack;
  val: INTEGER;
  i: INTEGER;

BEGIN
  Create(s);

  FOR i := 1 TO 5 DO
    Push(s, i * 10);
    WriteString("Pushed: "); WriteInt(i * 10, 1); WriteLn
  END;

  WriteString("IsEmpty: ");
  IF IsEmpty(s) THEN WriteString("YES") ELSE WriteString("NO") END;
  WriteLn;

  WriteString("Popping: ");
  WHILE NOT IsEmpty(s) DO
    Pop(s, val);
    WriteInt(val, 4)
  END;
  WriteLn;

  WriteString("IsEmpty: ");
  IF IsEmpty(s) THEN WriteString("YES") ELSE WriteString("NO") END;
  WriteLn;

  Destroy(s);
  WriteString("Done"); WriteLn
END UseStack2.
