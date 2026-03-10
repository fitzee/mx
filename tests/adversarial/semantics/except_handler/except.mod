MODULE ExceptTest;
(* Test ISO Modula-2 exception handling *)
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

VAR
  result: INTEGER;

PROCEDURE SafeDiv(a, b: INTEGER): INTEGER;
BEGIN
  IF b = 0 THEN
    RAISE 1
  END;
  RETURN a DIV b
EXCEPT
  WriteString("Exception caught in SafeDiv!"); WriteLn;
  RETURN 0
END SafeDiv;

PROCEDURE DoWork(x: INTEGER): INTEGER;
BEGIN
  WriteString("Working with "); WriteInt(x, 1); WriteLn;
  IF x < 0 THEN
    RAISE
  END;
  RETURN x * 2
EXCEPT
  WriteString("Exception: negative input!"); WriteLn;
  RETURN -1
END DoWork;

BEGIN
  result := SafeDiv(10, 3);
  WriteString("10 / 3 = "); WriteInt(result, 1); WriteLn;

  result := SafeDiv(10, 0);
  WriteString("10 / 0 = "); WriteInt(result, 1); WriteLn;

  result := DoWork(5);
  WriteString("DoWork(5) = "); WriteInt(result, 1); WriteLn;

  result := DoWork(-3);
  WriteString("DoWork(-3) = "); WriteInt(result, 1); WriteLn;

  WriteString("Done"); WriteLn
END ExceptTest.
