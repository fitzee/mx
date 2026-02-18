MODULE StrArith;
(* Test string operations and character arithmetic *)
FROM InOut IMPORT WriteString, WriteInt, WriteLn, WriteCard;

VAR
  s: ARRAY [0..79] OF CHAR;
  ch: CHAR;
  i, len: INTEGER;

PROCEDURE StrLen(s: ARRAY OF CHAR): INTEGER;
  VAR i: INTEGER;
BEGIN
  i := 0;
  WHILE (i <= HIGH(s)) AND (s[i] # 0C) DO
    INC(i)
  END;
  RETURN i
END StrLen;

PROCEDURE ToUpper(VAR s: ARRAY OF CHAR);
  VAR i: INTEGER;
BEGIN
  i := 0;
  WHILE (i <= HIGH(s)) AND (s[i] # 0C) DO
    IF (s[i] >= 'a') AND (s[i] <= 'z') THEN
      s[i] := CHR(ORD(s[i]) - ORD('a') + ORD('A'))
    END;
    INC(i)
  END
END ToUpper;

PROCEDURE Reverse(VAR s: ARRAY OF CHAR);
  VAR i, j: INTEGER;
      tmp: CHAR;
BEGIN
  j := StrLen(s) - 1;
  i := 0;
  WHILE i < j DO
    tmp := s[i];
    s[i] := s[j];
    s[j] := tmp;
    INC(i);
    DEC(j)
  END
END Reverse;

PROCEDURE IsPalindrome(s: ARRAY OF CHAR): BOOLEAN;
  VAR i, j: INTEGER;
BEGIN
  j := StrLen(s) - 1;
  i := 0;
  WHILE i < j DO
    IF s[i] # s[j] THEN RETURN FALSE END;
    INC(i); DEC(j)
  END;
  RETURN TRUE
END IsPalindrome;

BEGIN
  (* Character arithmetic *)
  ch := 'A';
  WriteString("A+3 = "); WriteString(" ");
  ch := CHR(ORD(ch) + 3);
  s[0] := ch; s[1] := 0C;
  WriteString(s); WriteLn;

  WriteString("ORD('Z') = "); WriteInt(ORD('Z'), 1); WriteLn;
  WriteString("CHR(48) = ");
  s[0] := CHR(48); s[1] := 0C;
  WriteString(s); WriteLn;

  (* CAP test *)
  WriteString("CAP('g') = ");
  s[0] := CAP('g'); s[1] := 0C;
  WriteString(s); WriteLn;

  (* String length *)
  s := "Hello, World!";
  len := StrLen(s);
  WriteString("Length of '"); WriteString(s); WriteString("' = ");
  WriteInt(len, 1); WriteLn;

  (* ToUpper *)
  s := "hello";
  ToUpper(s);
  WriteString("ToUpper: "); WriteString(s); WriteLn;

  (* Reverse *)
  s := "abcde";
  Reverse(s);
  WriteString("Reverse 'abcde': "); WriteString(s); WriteLn;

  (* Palindrome check *)
  s := "racecar";
  IF IsPalindrome(s) THEN
    WriteString("'racecar' is a palindrome: TRUE")
  ELSE
    WriteString("'racecar' is a palindrome: FALSE")
  END;
  WriteLn;

  s := "hello";
  IF IsPalindrome(s) THEN
    WriteString("'hello' is a palindrome: TRUE")
  ELSE
    WriteString("'hello' is a palindrome: FALSE")
  END;
  WriteLn;

  WriteString("Done"); WriteLn
END StrArith.
