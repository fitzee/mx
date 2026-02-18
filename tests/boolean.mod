MODULE BooleanTest;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

VAR
  a, b, c: BOOLEAN;
  x: INTEGER;

PROCEDURE BoolStr(b: BOOLEAN);
BEGIN
  IF b THEN
    WriteString("TRUE")
  ELSE
    WriteString("FALSE")
  END
END BoolStr;

BEGIN
  (* Test boolean operations *)
  a := TRUE;
  b := FALSE;

  WriteString("TRUE AND FALSE = "); BoolStr(a AND b); WriteLn;
  WriteString("TRUE OR FALSE = "); BoolStr(a OR b); WriteLn;
  WriteString("NOT TRUE = "); BoolStr(NOT a); WriteLn;
  WriteString("NOT FALSE = "); BoolStr(NOT b); WriteLn;

  (* Test relational *)
  x := 42;
  WriteString("42 = 42: "); BoolStr(x = 42); WriteLn;
  WriteString("42 # 42: "); BoolStr(x # 42); WriteLn;
  WriteString("42 > 10: "); BoolStr(x > 10); WriteLn;
  WriteString("42 < 10: "); BoolStr(x < 10); WriteLn;
  WriteString("42 >= 42: "); BoolStr(x >= 42); WriteLn;
  WriteString("42 <= 41: "); BoolStr(x <= 41); WriteLn;

  (* Test ODD *)
  WriteString("ODD(3) = "); BoolStr(ODD(3)); WriteLn;
  WriteString("ODD(4) = "); BoolStr(ODD(4)); WriteLn;

  (* Test short-circuit evaluation *)
  c := (x > 0) AND (x < 100);
  WriteString("0 < 42 < 100: "); BoolStr(c); WriteLn
END BooleanTest.
