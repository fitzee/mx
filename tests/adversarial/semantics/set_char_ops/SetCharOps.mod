MODULE SetCharOps;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

TYPE
  SmallEnum = (EA, EB, EC, ED);
  SmallSet = SET OF SmallEnum;

VAR
  s: SmallSet;
  a: ARRAY [0..5] OF CHAR;

BEGIN
  s := SmallSet{};
  INCL(s, EA);
  IF EA IN s THEN
    WriteString("yes");
  ELSE
    WriteString("no");
  END;
  WriteLn;
  EXCL(s, EA);
  IF EA IN s THEN
    WriteString("yes");
  ELSE
    WriteString("no");
  END;
  WriteLn;
  (* Test char literal as array index *)
  a[0] := "H";
  a[1] := "i";
  a[2] := 0C;
  WriteString(a);
  WriteLn;
END SetCharOps.
