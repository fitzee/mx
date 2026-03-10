MODULE Main;
FROM InOut IMPORT WriteString, WriteLn;
VAR
  i: INTEGER;
  x: INTEGER;
BEGIN
  WriteString("Hello from m2c build!");
  FOR x := 1 TO 5 DO
    WriteString(" Line ");
    WriteString("of output.");
  END;
  FOR i := 1 TO 5 DO
    WriteString("Line ");
    WriteString(" of output.");
  END;
  WriteLn;
END Main.
