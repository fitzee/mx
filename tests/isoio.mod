MODULE ISOIO;
(* Test ISO standard I/O modules *)
FROM STextIO IMPORT WriteString, WriteLn, WriteChar;
FROM SWholeIO IMPORT WriteInt, WriteCard;
FROM SRealIO IMPORT WriteFixed, WriteFloat, WriteReal;

VAR
  i: INTEGER;
  r: REAL;

BEGIN
  WriteString("=== ISO Standard I/O Test ==="); WriteLn;

  (* STextIO *)
  WriteString("Hello from STextIO!"); WriteLn;
  WriteChar('A'); WriteChar('B'); WriteChar('C'); WriteLn;

  (* SWholeIO *)
  WriteString("Integer: ");
  WriteInt(42, 1); WriteLn;
  WriteString("Cardinal: ");
  WriteCard(12345, 1); WriteLn;

  (* SRealIO *)
  r := 3.14159;
  WriteString("WriteFixed: ");
  WriteFixed(r, 4, 10); WriteLn;
  WriteString("WriteFloat: ");
  WriteFloat(r, 6, 12); WriteLn;
  WriteString("WriteReal: ");
  WriteReal(r, 10); WriteLn;

  (* Multiple values *)
  WriteString("Countdown: ");
  FOR i := 10 TO 1 BY -1 DO
    WriteInt(i, 3)
  END;
  WriteLn;

  WriteString("Done"); WriteLn
END ISOIO.
