MODULE Subrange;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

TYPE
  Month = [1..12];
  Digit = [0..9];
  Letter = ['A'..'Z'];
  Weekday = (Mon, Tue, Wed, Thu, Fri, Sat, Sun);
  WorkDay = [Mon..Fri];

VAR
  m: Month;
  d: Digit;
  w: WorkDay;
  i: INTEGER;

PROCEDURE MonthName(m: Month);
BEGIN
  CASE m OF
    1:  WriteString("January") |
    2:  WriteString("February") |
    3:  WriteString("March") |
    4:  WriteString("April") |
    5:  WriteString("May") |
    6:  WriteString("June") |
    7:  WriteString("July") |
    8:  WriteString("August") |
    9:  WriteString("September") |
    10: WriteString("October") |
    11: WriteString("November") |
    12: WriteString("December")
  END
END MonthName;

PROCEDURE SumDigits(n: INTEGER): INTEGER;
  VAR sum: INTEGER;
      d: Digit;
BEGIN
  sum := 0;
  WHILE n > 0 DO
    d := n MOD 10;
    sum := sum + d;
    n := n DIV 10
  END;
  RETURN sum
END SumDigits;

BEGIN
  (* Test subranges *)
  m := 6;
  WriteString("Month 6: "); MonthName(m); WriteLn;

  FOR m := 1 TO 12 DO
    WriteInt(m, 3);
    WriteString(": ");
    MonthName(m);
    WriteLn
  END;

  WriteString("Sum of digits of 12345: ");
  WriteInt(SumDigits(12345), 1);
  WriteLn;

  (* Test enum subrange *)
  w := Mon;
  WriteString("Mon ORD: "); WriteInt(ORD(w), 1); WriteLn;
  w := Fri;
  WriteString("Fri ORD: "); WriteInt(ORD(w), 1); WriteLn;

  WriteString("Done"); WriteLn
END Subrange.
