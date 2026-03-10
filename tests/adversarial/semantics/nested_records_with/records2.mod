MODULE Records2;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

TYPE
  Date = RECORD
    year, month, day: INTEGER
  END;

  Person = RECORD
    age: INTEGER;
    birthdate: Date
  END;

VAR
  d: Date;
  p: Person;
  a: ARRAY [0..2] OF Date;
  i: INTEGER;

PROCEDURE PrintDate(d: Date);
BEGIN
  WriteInt(d.year, 4); WriteString("-");
  WriteInt(d.month, 2); WriteString("-");
  WriteInt(d.day, 2)
END PrintDate;

PROCEDURE MakeDate(y, m, d: INTEGER): Date;
  VAR result: Date;
BEGIN
  result.year := y;
  result.month := m;
  result.day := d;
  RETURN result
END MakeDate;

PROCEDURE DaysBetween(d1, d2: Date): INTEGER;
BEGIN
  (* Simplified: just compare years *)
  RETURN (d2.year - d1.year) * 365 + (d2.month - d1.month) * 30 + (d2.day - d1.day)
END DaysBetween;

BEGIN
  d := MakeDate(2024, 6, 15);
  WriteString("Date: "); PrintDate(d); WriteLn;

  p.age := 30;
  p.birthdate := MakeDate(1994, 3, 10);
  WriteString("Age: "); WriteInt(p.age, 1); WriteLn;
  WriteString("Born: "); PrintDate(p.birthdate); WriteLn;

  (* WITH on nested record *)
  WITH p DO
    WriteString("WITH age: "); WriteInt(age, 1); WriteLn;
    WITH birthdate DO
      WriteString("WITH year: "); WriteInt(year, 1); WriteLn
    END
  END;

  (* Array of records *)
  a[0] := MakeDate(2000, 1, 1);
  a[1] := MakeDate(2010, 6, 15);
  a[2] := MakeDate(2020, 12, 25);

  WriteString("Dates: ");
  FOR i := 0 TO 2 DO
    PrintDate(a[i]);
    IF i < 2 THEN WriteString(", ") END
  END;
  WriteLn;

  WriteString("Days between [0] and [2]: ");
  WriteInt(DaysBetween(a[0], a[2]), 1);
  WriteLn
END Records2.
