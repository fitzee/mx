MODULE EnumCase;
(* Test enumeration types with CASE statements and ORD/VAL *)
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

TYPE
  Color = (Red, Green, Blue, Yellow, Cyan, Magenta, White, Black);
  Day = (Mon, Tue, Wed, Thu, Fri, Sat, Sun);

VAR
  c: Color;
  d: Day;
  i: INTEGER;

PROCEDURE ColorName(c: Color);
BEGIN
  CASE c OF
    Red:     WriteString("Red") |
    Green:   WriteString("Green") |
    Blue:    WriteString("Blue") |
    Yellow:  WriteString("Yellow") |
    Cyan:    WriteString("Cyan") |
    Magenta: WriteString("Magenta") |
    White:   WriteString("White") |
    Black:   WriteString("Black")
  END
END ColorName;

PROCEDURE DayName(d: Day);
BEGIN
  CASE d OF
    Mon: WriteString("Monday") |
    Tue: WriteString("Tuesday") |
    Wed: WriteString("Wednesday") |
    Thu: WriteString("Thursday") |
    Fri: WriteString("Friday") |
    Sat: WriteString("Saturday") |
    Sun: WriteString("Sunday")
  END
END DayName;

PROCEDURE IsWeekend(d: Day): BOOLEAN;
BEGIN
  RETURN (d = Sat) OR (d = Sun)
END IsWeekend;

BEGIN
  (* Enumerate all colors *)
  WriteString("Colors: ");
  FOR i := 0 TO 7 DO
    c := VAL(Color, i);
    ColorName(c);
    WriteString(" ")
  END;
  WriteLn;

  (* Enumerate days *)
  WriteString("Days: ");
  FOR i := 0 TO 6 DO
    d := VAL(Day, i);
    DayName(d);
    IF IsWeekend(d) THEN WriteString("*") END;
    WriteString(" ")
  END;
  WriteLn;

  (* ORD test *)
  WriteString("ORD(Red) = "); WriteInt(ORD(Red), 1); WriteLn;
  WriteString("ORD(Black) = "); WriteInt(ORD(Black), 1); WriteLn;
  WriteString("ORD(Mon) = "); WriteInt(ORD(Mon), 1); WriteLn;
  WriteString("ORD(Sun) = "); WriteInt(ORD(Sun), 1); WriteLn;

  WriteString("Done"); WriteLn
END EnumCase.
