MODULE type_cast_test;
FROM InOut IMPORT WriteString, WriteLn;
FROM SYSTEM IMPORT ADDRESS, WORD, BYTE, ADR;

TYPE
  Rec = RECORD x: INTEGER; y: INTEGER END;
  RecPtr = POINTER TO Rec;

VAR
  r: Rec;
  p: RecPtr;
  a: ADDRESS;
  w: WORD;
  b: BYTE;

BEGIN
  r.x := 42;
  r.y := 99;
  a := ADR(r);
  p := RecPtr(a);
  IF p^.x = 42 THEN
    WriteString("type cast OK"); WriteLn
  END;
  w := WORD(1234);
  b := BYTE(255);
  a := ADDRESS(ADR(r));
  WriteString("all casts OK"); WriteLn
END type_cast_test.
