MODULE M2PlusExceptions;
(* Test Modula-2+ enhanced exception handling: TRY/EXCEPT/FINALLY, RAISE, EXCEPTION *)
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

EXCEPTION DivByZero;
EXCEPTION NegativeInput;

VAR result: INTEGER;

PROCEDURE SafeDiv(a, b: INTEGER): INTEGER;
BEGIN
  IF b = 0 THEN
    RAISE DivByZero
  END;
  RETURN a DIV b
END SafeDiv;

PROCEDURE TestExceptions;
VAR x: INTEGER;
BEGIN
  (* Test TRY/EXCEPT with named exception *)
  TRY
    x := SafeDiv(10, 0);
    WriteString("Should not reach here"); WriteLn
  EXCEPT
    WriteString("Caught exception in SafeDiv"); WriteLn;
    x := -1
  FINALLY
    WriteString("Finally block executed"); WriteLn
  END;
  WriteString("x = "); WriteInt(x, 1); WriteLn;

  (* Test TRY without exception *)
  TRY
    x := SafeDiv(10, 2);
    WriteString("Normal result: "); WriteInt(x, 1); WriteLn
  EXCEPT
    WriteString("Should not reach here"); WriteLn
  FINALLY
    WriteString("Finally always runs"); WriteLn
  END;

  (* Nested TRY *)
  TRY
    TRY
      x := SafeDiv(100, 0)
    EXCEPT
      WriteString("Inner handler caught it"); WriteLn;
      x := 0
    END
  EXCEPT
    WriteString("Outer handler - should not reach"); WriteLn
  END;
  WriteString("Nested result: "); WriteInt(x, 1); WriteLn
END TestExceptions;

BEGIN
  WriteString("=== M2+ Exception Test ==="); WriteLn;
  TestExceptions;
  WriteString("Done"); WriteLn
END M2PlusExceptions.
