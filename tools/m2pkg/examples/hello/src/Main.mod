MODULE Main;

FROM InOut IMPORT WriteString, WriteLn;


BEGIN
  WriteString("Hello from m2pkg!"); WriteLn;
  WriteLn(I2c())
END Main.
