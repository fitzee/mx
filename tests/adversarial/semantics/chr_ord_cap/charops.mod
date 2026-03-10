MODULE CharOps;
FROM InOut IMPORT WriteString, WriteInt, WriteLn, Write;

VAR
  ch: CHAR;
  s: ARRAY [0..31] OF CHAR;
  i: INTEGER;

PROCEDURE IsUpper(ch: CHAR): BOOLEAN;
BEGIN
  RETURN (ch >= 'A') AND (ch <= 'Z')
END IsUpper;

PROCEDURE IsLower(ch: CHAR): BOOLEAN;
BEGIN
  RETURN (ch >= 'a') AND (ch <= 'z')
END IsLower;

PROCEDURE IsDigit(ch: CHAR): BOOLEAN;
BEGIN
  RETURN (ch >= '0') AND (ch <= '9')
END IsDigit;

PROCEDURE ToUpper(ch: CHAR): CHAR;
BEGIN
  IF IsLower(ch) THEN
    RETURN CHR(ORD(ch) - ORD('a') + ORD('A'))
  ELSE
    RETURN ch
  END
END ToUpper;

PROCEDURE ToLower(ch: CHAR): CHAR;
BEGIN
  IF IsUpper(ch) THEN
    RETURN CHR(ORD(ch) - ORD('A') + ORD('a'))
  ELSE
    RETURN ch
  END
END ToLower;

PROCEDURE StrLen(s: ARRAY OF CHAR): INTEGER;
  VAR i: INTEGER;
BEGIN
  i := 0;
  WHILE (i <= HIGH(s)) AND (s[i] # 0C) DO
    INC(i)
  END;
  RETURN i
END StrLen;

PROCEDURE StrUpper(VAR s: ARRAY OF CHAR);
  VAR i: INTEGER;
BEGIN
  i := 0;
  WHILE (i <= HIGH(s)) AND (s[i] # 0C) DO
    s[i] := ToUpper(s[i]);
    INC(i)
  END
END StrUpper;

BEGIN
  (* Character classification *)
  ch := 'A';
  WriteString("IsUpper('A'): ");
  IF IsUpper(ch) THEN WriteString("YES") ELSE WriteString("NO") END;
  WriteLn;

  ch := 'z';
  WriteString("IsLower('z'): ");
  IF IsLower(ch) THEN WriteString("YES") ELSE WriteString("NO") END;
  WriteLn;

  ch := '5';
  WriteString("IsDigit('5'): ");
  IF IsDigit(ch) THEN WriteString("YES") ELSE WriteString("NO") END;
  WriteLn;

  (* Character conversion *)
  WriteString("ToUpper('m') = ");
  Write(ToUpper('m')); WriteLn;

  WriteString("ToLower('G') = ");
  Write(ToLower('G')); WriteLn;

  WriteString("CAP('q') = ");
  Write(CAP('q')); WriteLn;

  (* String operations *)
  s := "hello world";
  WriteString("Original: "); WriteString(s); WriteLn;
  WriteString("Length: "); WriteInt(StrLen(s), 1); WriteLn;

  StrUpper(s);
  WriteString("Upper: "); WriteString(s); WriteLn;

  (* Write individual chars *)
  WriteString("Chars: ");
  FOR i := 0 TO 4 DO
    Write(s[i])
  END;
  WriteLn;

  WriteString("Done"); WriteLn
END CharOps.
