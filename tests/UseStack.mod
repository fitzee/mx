MODULE UseStack;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;
IMPORT Stack;

VAR
  s: Stack.Stack;
  val: INTEGER;
  i: INTEGER;

BEGIN
  Stack.Create(s);

  FOR i := 1 TO 5 DO
    Stack.Push(s, i * 10);
    WriteString("Pushed: "); WriteInt(i * 10, 1); WriteLn
  END;

  WriteString("IsEmpty: ");
  IF Stack.IsEmpty(s) THEN WriteString("YES") ELSE WriteString("NO") END;
  WriteLn;

  WriteString("Popping: ");
  WHILE NOT Stack.IsEmpty(s) DO
    Stack.Pop(s, val);
    WriteInt(val, 4)
  END;
  WriteLn;

  WriteString("IsEmpty: ");
  IF Stack.IsEmpty(s) THEN WriteString("YES") ELSE WriteString("NO") END;
  WriteLn;

  Stack.Destroy(s);
  WriteString("Done"); WriteLn
END UseStack.
