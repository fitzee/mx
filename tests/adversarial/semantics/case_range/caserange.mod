MODULE CaseRange;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

PROCEDURE Classify(n: INTEGER);
BEGIN
  CASE n OF
    1..5:     WriteString("small")
  | 6..10:    WriteString("medium")
  | 11..100:  WriteString("large")
  ELSE
    WriteString("out of range")
  END
END Classify;

PROCEDURE ClassifyChar(ch: CHAR);
BEGIN
  CASE ch OF
    'A'..'Z':  WriteString("uppercase")
  | 'a'..'z':  WriteString("lowercase")
  | '0'..'9':  WriteString("digit")
  ELSE
    WriteString("other")
  END
END ClassifyChar;

VAR i: INTEGER;

BEGIN
  WriteString("3: "); Classify(3); WriteLn;
  WriteString("7: "); Classify(7); WriteLn;
  WriteString("50: "); Classify(50); WriteLn;
  WriteString("200: "); Classify(200); WriteLn;
  WriteString("H: "); ClassifyChar('H'); WriteLn;
  WriteString("x: "); ClassifyChar('x'); WriteLn;
  WriteString("5: "); ClassifyChar('5'); WriteLn;
  WriteString("!: "); ClassifyChar('!'); WriteLn
END CaseRange.
