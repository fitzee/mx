MODULE Main;

FROM InOut IMPORT WriteString, WriteLn;


BEGIN
  WriteString("Hello from mxpkg!"); WriteLn;
  WriteLn(I2c())
END Main.
