MODULE Factorial;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

VAR n: INTEGER;

PROCEDURE Fact(n: INTEGER): INTEGER;
BEGIN
  IF n <= 1 THEN
    RETURN 1
  ELSE
    RETURN n * Fact(n - 1)
  END
END Fact;

BEGIN
  WriteString("Factorials:");
  WriteLn;
  FOR n := 1 TO 10 DO
    WriteInt(n, 3);
    WriteString("! = ");
    WriteInt(Fact(n), 10);
    WriteLn
  END
END Factorial.
