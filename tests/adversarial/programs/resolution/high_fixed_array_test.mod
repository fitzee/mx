MODULE high_fixed_array_test;
FROM InOut IMPORT WriteString, WriteLn, WriteCard;

TYPE
  Buf = ARRAY [0..9] OF CHAR;
  Rec = RECORD
    name: ARRAY [0..31] OF CHAR;
    data: ARRAY [0..7] OF INTEGER
  END;

VAR
  arr: ARRAY [0..4] OF INTEGER;
  buf: Buf;
  rec: Rec;

BEGIN
  IF HIGH(arr) = 4 THEN
    WriteString("HIGH local array OK"); WriteLn
  END;
  IF HIGH(buf) = 9 THEN
    WriteString("HIGH named type OK"); WriteLn
  END;
  IF HIGH(rec.name) = 31 THEN
    WriteString("HIGH record char field OK"); WriteLn
  END;
  IF HIGH(rec.data) = 7 THEN
    WriteString("HIGH record int field OK"); WriteLn
  END;
  WriteString("all HIGH OK"); WriteLn
END high_fixed_array_test.
