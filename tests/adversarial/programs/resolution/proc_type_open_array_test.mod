MODULE proc_type_open_array_test;
FROM InOut IMPORT WriteString, WriteLn, WriteCard;

TYPE
  StringProc = PROCEDURE(VAR ARRAY OF CHAR);

PROCEDURE FillBuf(VAR buf: ARRAY OF CHAR);
VAR i: CARDINAL;
BEGIN
  FOR i := 0 TO HIGH(buf) DO
    buf[i] := CHR(ORD('A') + (i MOD 26))
  END
END FillBuf;

VAR
  sp: StringProc;
  buf: ARRAY [0..9] OF CHAR;

BEGIN
  sp := FillBuf;
  sp(buf);
  IF buf[0] = 'A' THEN
    WriteString("proc type open array OK"); WriteLn
  END;
  IF buf[9] = 'J' THEN
    WriteString("high propagated correctly"); WriteLn
  END;
  WriteString("all proc type open array OK"); WriteLn
END proc_type_open_array_test.
