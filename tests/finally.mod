MODULE FinallyTest;
(* Test ISO Modula-2 FINALLY clause *)
FROM InOut IMPORT WriteString, WriteLn;

VAR
  i: INTEGER;

BEGIN
  WriteString("Module init"); WriteLn;
  FOR i := 1 TO 3 DO
    WriteString("Working..."); WriteLn
  END;
  WriteString("Module body done"); WriteLn
FINALLY
  WriteString("FINALLY: cleanup executed"); WriteLn
END FinallyTest.
