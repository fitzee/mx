MODULE NulCharAssign;
FROM InOut IMPORT WriteString, WriteLn, WriteInt;

VAR
  ch: CHAR;
  a: ARRAY [0..3] OF CHAR;

BEGIN
  ch := "";
  WriteString("nul=");
  WriteInt(ORD(ch), 1);
  WriteLn;

  ch := "X";
  WriteString("ch=");
  WriteInt(ORD(ch), 1);
  WriteLn;

  a[0] := "";
  a[1] := "A";
  WriteString("a0=");
  WriteInt(ORD(a[0]), 1);
  WriteLn;
  WriteString("a1=");
  WriteInt(ORD(a[1]), 1);
  WriteLn;
END NulCharAssign.
