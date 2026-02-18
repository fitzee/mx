MODULE CaseTest;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

VAR i: INTEGER;

PROCEDURE DayName(day: INTEGER);
BEGIN
  CASE day OF
    1: WriteString("Monday") |
    2: WriteString("Tuesday") |
    3: WriteString("Wednesday") |
    4: WriteString("Thursday") |
    5: WriteString("Friday") |
    6: WriteString("Saturday") |
    7: WriteString("Sunday")
  ELSE
    WriteString("Unknown")
  END
END DayName;

BEGIN
  FOR i := 1 TO 7 DO
    WriteInt(i, 2);
    WriteString(": ");
    DayName(i);
    WriteLn
  END
END CaseTest.
