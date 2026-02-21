MODULE IntegerBounds;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;
VAR
  i: INTEGER;
  c: CARDINAL;
BEGIN
  (* Test basic integer operations *)
  i := MAX(INTEGER);
  WriteString("MaxInt:"); WriteInt(i, 0); WriteLn;

  i := MIN(INTEGER);
  WriteString("MinInt:"); WriteInt(i, 0); WriteLn;

  (* Test cardinal *)
  c := 0;
  WriteString("ZeroCard:"); WriteInt(c, 0); WriteLn;

  (* Test negative literal *)
  i := -1;
  WriteString("NegOne:"); WriteInt(i, 0); WriteLn;

  (* Test arithmetic near bounds *)
  i := MAX(INTEGER) - 1;
  i := i + 1;
  WriteString("MaxAgain:"); WriteInt(i, 0); WriteLn;

  (* Test integer division *)
  i := 7 DIV 2;
  WriteString("Div:"); WriteInt(i, 0); WriteLn;

  i := 7 MOD 2;
  WriteString("Mod:"); WriteInt(i, 0); WriteLn;

  (* Negative div/mod — M2 truncates toward zero *)
  i := (-7) DIV 2;
  WriteString("NegDiv:"); WriteInt(i, 0); WriteLn;

  i := (-7) MOD 2;
  WriteString("NegMod:"); WriteInt(i, 0); WriteLn
END IntegerBounds.
