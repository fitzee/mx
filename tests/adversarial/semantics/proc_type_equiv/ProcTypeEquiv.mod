MODULE ProcTypeEquiv;
FROM SYSTEM IMPORT ADDRESS;
FROM InOut IMPORT WriteString, WriteLn, WriteInt;

TYPE
  CompareFunc = PROCEDURE (INTEGER, INTEGER): INTEGER;
  AliasedFunc = CompareFunc;

PROCEDURE RealCompare(a, b: INTEGER): INTEGER;
BEGIN
  IF a < b THEN RETURN -1
  ELSIF a > b THEN RETURN 1
  ELSE RETURN 0
  END
END RealCompare;

PROCEDURE GetCompare(): AliasedFunc;
BEGIN
  RETURN RealCompare
END GetCompare;

VAR
  cmp: CompareFunc;
  result: INTEGER;

BEGIN
  cmp := GetCompare();
  result := cmp(10, 20);
  WriteString("result=");
  WriteInt(result, 1);
  WriteLn;

  result := cmp(5, 5);
  WriteString("result=");
  WriteInt(result, 1);
  WriteLn;
END ProcTypeEquiv.
