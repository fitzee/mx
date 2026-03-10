MODULE SubrangeBaseType;
FROM InOut IMPORT WriteString, WriteLn, WriteInt;

TYPE
  SmallInt = INTEGER [-10 .. 10];
  Idx = CARDINAL [0 .. 99];

VAR
  s: SmallInt;
  i: Idx;

BEGIN
  s := -5;
  WriteString("s=");
  WriteInt(s, 1);
  WriteLn;

  s := 10;
  WriteString("s=");
  WriteInt(s, 1);
  WriteLn;

  i := 42;
  WriteString("i=");
  WriteInt(i, 1);
  WriteLn;
END SubrangeBaseType.
