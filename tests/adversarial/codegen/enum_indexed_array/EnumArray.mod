MODULE EnumArray;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

TYPE Color = (Red, Green, Blue);
     Direction = (North, South, East, West);

VAR names: ARRAY Color OF ARRAY [0..10] OF CHAR;
    values: ARRAY Direction OF INTEGER;
    c: Color;
    d: Direction;

BEGIN
  (* Test ARRAY EnumType OF ARRAY — enum-indexed string table *)
  names[Red] := "red";
  names[Green] := "green";
  names[Blue] := "blue";

  FOR c := Red TO Blue DO
    WriteString(names[c]);
    WriteString(" ")
  END;
  WriteLn;

  (* Test ARRAY EnumType OF INTEGER — enum-indexed value array *)
  values[North] := 0;
  values[South] := 180;
  values[East] := 90;
  values[West] := 270;

  FOR d := North TO West DO
    WriteInt(values[d], 0);
    WriteString(" ")
  END;
  WriteLn;

  (* Verify no corruption — write all then read all back *)
  values[North] := 111;
  values[South] := 222;
  values[East] := 333;
  values[West] := 444;
  WriteInt(values[North], 0); WriteString(" ");
  WriteInt(values[South], 0); WriteString(" ");
  WriteInt(values[East], 0); WriteString(" ");
  WriteInt(values[West], 0);
  WriteLn
END EnumArray.
