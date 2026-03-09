MODULE SetInlineEnum;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

TYPE
  Color = (Red, Green, Blue);
  ColorSet = SET OF Color;

VAR
  s: ColorSet;

BEGIN
  s := ColorSet{Red, Blue};
  IF Red IN s THEN
    WriteString("R");
  END;
  IF Green IN s THEN
    WriteString("G");
  END;
  IF Blue IN s THEN
    WriteString("B");
  END;
  WriteLn;
END SetInlineEnum.
