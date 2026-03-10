MODULE EnumTest;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

TYPE
  Color = (Red, Green, Blue, Yellow, Cyan, Magenta);
  Day = (Mon, Tue, Wed, Thu, Fri, Sat, Sun);

VAR
  c: Color;
  d: Day;
  i: INTEGER;

PROCEDURE ColorName(c: Color);
BEGIN
  CASE ORD(c) OF
    0: WriteString("Red") |
    1: WriteString("Green") |
    2: WriteString("Blue") |
    3: WriteString("Yellow") |
    4: WriteString("Cyan") |
    5: WriteString("Magenta")
  END
END ColorName;

BEGIN
  (* Test enum values *)
  WriteString("Red = "); WriteInt(ORD(Red), 1); WriteLn;
  WriteString("Blue = "); WriteInt(ORD(Blue), 1); WriteLn;
  WriteString("Magenta = "); WriteInt(ORD(Magenta), 1); WriteLn;

  (* Loop through colors *)
  WriteString("Colors: ");
  FOR i := 0 TO 5 DO
    c := VAL(Color, i);
    ColorName(c);
    WriteString(" ")
  END;
  WriteLn;

  (* Test day ordinals *)
  d := Mon;
  WriteString("Mon = "); WriteInt(ORD(d), 1); WriteLn;
  d := Sun;
  WriteString("Sun = "); WriteInt(ORD(d), 1); WriteLn
END EnumTest.
