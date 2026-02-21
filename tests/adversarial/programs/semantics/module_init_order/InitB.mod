IMPLEMENTATION MODULE InitB;
FROM InOut IMPORT WriteString, WriteLn;
FROM InitA IMPORT GetA;
VAR dummy: INTEGER;

PROCEDURE GetB(): INTEGER;
BEGIN RETURN GetA() + 1 END GetB;

BEGIN
  dummy := GetA();
  WriteString("InitB"); WriteLn
END InitB.
