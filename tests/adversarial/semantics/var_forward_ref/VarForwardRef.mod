MODULE VarForwardRef;
FROM InOut IMPORT WriteInt, WriteLn;

PROCEDURE ShowCount;
BEGIN
  WriteInt(count, 0);
  WriteLn;
END ShowCount;

VAR
  count: INTEGER;

BEGIN
  count := 42;
  ShowCount;
END VarForwardRef.
