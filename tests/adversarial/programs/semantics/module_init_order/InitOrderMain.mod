MODULE InitOrderMain;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;
FROM InitB IMPORT GetB;
VAR x: INTEGER;
BEGIN
  WriteString("Main"); WriteLn;
  x := GetB();
  WriteString("Result:"); WriteInt(x, 0); WriteLn
END InitOrderMain.
