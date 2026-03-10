MODULE ProcBuiltinType;
FROM InOut IMPORT WriteString, WriteLn;

VAR
  p: PROC;

PROCEDURE Hello;
BEGIN
  WriteString("hello");
  WriteLn;
END Hello;

BEGIN
  p := Hello;
  p;
END ProcBuiltinType.
