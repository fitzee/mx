MODULE Fibonacci;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

VAR i, a, b, temp: INTEGER;

BEGIN
  a := 0;
  b := 1;
  WriteString("Fibonacci sequence:");
  WriteLn;
  FOR i := 1 TO 20 DO
    WriteInt(a, 8);
    temp := a + b;
    a := b;
    b := temp
  END;
  WriteLn
END Fibonacci.
