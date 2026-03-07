MODULE CallbackDispatch;
FROM InOut IMPORT WriteInt, WriteLn;

TYPE
  Entry = RECORD
    id, value: INTEGER
  END;
  Transform = PROCEDURE(INTEGER): INTEGER;

PROCEDURE Add10(x: INTEGER): INTEGER;
BEGIN RETURN x + 10 END Add10;

PROCEDURE Negate(x: INTEGER): INTEGER;
BEGIN RETURN -x END Negate;

PROCEDURE Square(x: INTEGER): INTEGER;
BEGIN RETURN x * x END Square;

PROCEDURE Double(x: INTEGER): INTEGER;
BEGIN RETURN x * 2 END Double;

PROCEDURE AddSelf(x: INTEGER): INTEGER;
BEGIN RETURN x + x END AddSelf;

VAR
  entries: ARRAY [0..4] OF Entry;
  transforms: ARRAY [0..4] OF Transform;
  i, result: INTEGER;

BEGIN
  entries[0].id := 0; entries[0].value := 5;
  entries[1].id := 1; entries[1].value := 3;
  entries[2].id := 2; entries[2].value := 7;
  entries[3].id := 3; entries[3].value := 4;
  entries[4].id := 4; entries[4].value := 9;

  transforms[0] := Add10;
  transforms[1] := Negate;
  transforms[2] := Square;
  transforms[3] := Double;
  transforms[4] := AddSelf;

  FOR i := 0 TO 4 DO
    result := transforms[i](entries[i].value);
    WriteInt(result, 0);
    WriteLn
  END
END CallbackDispatch.
