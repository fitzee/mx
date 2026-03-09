MODULE VariantLeadingPipe;
FROM InOut IMPORT WriteString, WriteLn, WriteInt;

TYPE
  Rec = RECORD
    CASE tag: CARDINAL OF
    | 1: a: INTEGER
    | 2: b: INTEGER
    END
  END;

VAR
  r: Rec;

BEGIN
  r.tag := 1;
  r.a := 55;
  WriteString("a=");
  WriteInt(r.a, 1);
  WriteLn;

  r.tag := 2;
  r.b := 77;
  WriteString("b=");
  WriteInt(r.b, 1);
  WriteLn;
END VariantLeadingPipe.
