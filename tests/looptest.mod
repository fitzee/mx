MODULE LoopTest;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

VAR i, sum: INTEGER;

BEGIN
  (* Test LOOP/EXIT *)
  i := 1;
  sum := 0;
  LOOP
    IF i > 10 THEN EXIT END;
    sum := sum + i;
    INC(i)
  END;
  WriteString("Sum 1..10 (LOOP/EXIT) = ");
  WriteInt(sum, 1);
  WriteLn;

  (* Test REPEAT/UNTIL *)
  i := 1;
  sum := 0;
  REPEAT
    sum := sum + i;
    INC(i)
  UNTIL i > 10;
  WriteString("Sum 1..10 (REPEAT) = ");
  WriteInt(sum, 1);
  WriteLn;

  (* Test nested LOOP *)
  sum := 0;
  i := 0;
  LOOP
    INC(i);
    IF i > 5 THEN EXIT END;
    IF ODD(i) THEN
      sum := sum + i
    END
  END;
  WriteString("Sum of odd 1..5 = ");
  WriteInt(sum, 1);
  WriteLn
END LoopTest.
