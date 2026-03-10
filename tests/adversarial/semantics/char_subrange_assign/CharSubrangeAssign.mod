MODULE CharSubrangeAssign;
FROM InOut IMPORT WriteString, WriteLn, WriteInt;

TYPE
  UpperCase = ['A'..'Z'];

VAR
  s: UpperCase;

BEGIN
  s := 'A';
  WriteString("s=");
  WriteInt(ORD(s), 1);
  WriteLn;

  s := 'Z';
  WriteString("s=");
  WriteInt(ORD(s), 1);
  WriteLn;

  s := 'M';
  WriteString("s=");
  WriteInt(ORD(s), 1);
  WriteLn;
END CharSubrangeAssign.
