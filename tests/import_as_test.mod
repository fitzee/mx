MODULE ImportAsTest;

FROM InOut IMPORT WriteString AS WS, WriteLn AS NL, WriteInt;

BEGIN
  WS("Hello from aliased import");
  NL;
  WriteInt(42, 1);
  NL;
END ImportAsTest.
