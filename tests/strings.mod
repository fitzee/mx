MODULE StringsTest;
FROM InOut IMPORT WriteString, WriteInt, WriteLn, Write;

TYPE
  Str = ARRAY [0..79] OF CHAR;

VAR
  s: Str;
  ch: CHAR;
  i: INTEGER;

PROCEDURE PrintStr(s: ARRAY OF CHAR);
  VAR i: INTEGER;
BEGIN
  i := 0;
  WHILE (i <= HIGH(s)) AND (s[i] # 0C) DO
    Write(s[i]);
    INC(i)
  END
END PrintStr;

PROCEDURE StrLen(s: ARRAY OF CHAR): INTEGER;
  VAR i: INTEGER;
BEGIN
  i := 0;
  WHILE (i <= HIGH(s)) AND (s[i] # 0C) DO
    INC(i)
  END;
  RETURN i
END StrLen;

BEGIN
  (* Test character operations *)
  ch := 'A';
  WriteString("Char: "); Write(ch); WriteLn;
  WriteString("ORD('A') = "); WriteInt(ORD(ch), 1); WriteLn;
  WriteString("CHR(66) = "); Write(CHR(66)); WriteLn;
  WriteString("CAP('z') = "); Write(CAP('z')); WriteLn;

  (* Test string array *)
  s[0] := 'H';
  s[1] := 'e';
  s[2] := 'l';
  s[3] := 'l';
  s[4] := 'o';
  s[5] := 0C;
  WriteString("String: ");
  PrintStr(s);
  WriteLn;
  WriteString("Length: ");
  WriteInt(StrLen(s), 1);
  WriteLn
END StringsTest.
