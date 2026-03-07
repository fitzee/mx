MODULE ValLongint;
(* Test VAL(LONGINT, x) conversion works in embedded modules.
   Regression: LONG() was emitted as-is in C, causing undeclared function error. *)
FROM InOut IMPORT WriteString, WriteLn, WriteInt;

VAR
  n: LONGINT;
  c: CARDINAL;
  i: INTEGER;
BEGIN
  c := 42;
  n := VAL(LONGINT, c);
  IF n = LONG(42) THEN
    WriteString("cardinal-ok"); WriteLn
  END;

  i := 100;
  n := VAL(LONGINT, i);
  IF n = LONG(100) THEN
    WriteString("integer-ok"); WriteLn
  END;

  n := LONG(0) + VAL(LONGINT, c) + VAL(LONGINT, i);
  WriteInt(VAL(INTEGER, n), 1); WriteLn
END ValLongint.
