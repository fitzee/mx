MODULE ConstCharLiteral;
FROM InOut IMPORT WriteString, WriteInt, WriteLn, Write;

CONST
  Slash = '/';
  Star  = '*';
  Greeting = "hello";

VAR ch: CHAR;

BEGIN
  (* Single-char consts must be CHAR, not string pointers *)
  ch := Slash;
  IF ch = '/' THEN
    WriteString("slash-ok")
  ELSE
    WriteString("slash-FAIL")
  END;
  WriteLn;

  ch := Star;
  IF ch = '*' THEN
    WriteString("star-ok")
  ELSE
    WriteString("star-FAIL")
  END;
  WriteLn;

  (* Multi-char const must remain a string *)
  WriteString(Greeting);
  WriteLn;

  (* Single-char const used in expression *)
  IF Slash = '/' THEN
    WriteString("expr-ok")
  ELSE
    WriteString("expr-FAIL")
  END;
  WriteLn
END ConstCharLiteral.
