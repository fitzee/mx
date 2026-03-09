MODULE CaseLeadingPipe;
FROM InOut IMPORT WriteString, WriteLn, WriteInt;

VAR
  i, b: INTEGER;

BEGIN
  FOR i := 0 TO 3 DO
    CASE i OF
    | 0: b := 10
    | 1: b := 20
    | 2: b := 30
    ELSE
      b := 99
    END;
    WriteInt(b, 1);
    WriteLn;
  END;
END CaseLeadingPipe.
