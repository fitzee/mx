MODULE ImportAsVar;

FROM InOut IMPORT WriteString AS WS, WriteLn AS NL, WriteInt AS WI;
FROM Strings IMPORT Length AS Len;

VAR
  s: ARRAY [0..31] OF CHAR;

BEGIN
  s := "Hello World";
  WS("Length: ");
  WI(Len(s), 1);
  NL;
  WS("Done");
  NL;
END ImportAsVar.
