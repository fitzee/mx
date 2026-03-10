MODULE TypeAliasForward;
FROM InOut IMPORT WriteString, WriteLn;

TYPE
  BaseStr = ARRAY [0..20] OF CHAR;
  MyString = BaseStr;

PROCEDURE SetStr(VAR s: MyString);
BEGIN
  s := "hello world"
END SetStr;

VAR
  msg: MyString;

BEGIN
  SetStr(msg);
  WriteString(msg);
  WriteLn;
END TypeAliasForward.
