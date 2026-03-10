MODULE RepeatTest;
(* Test REPEAT..UNTIL and FOR..BY loops *)
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

VAR
  i, sum, n: INTEGER;

BEGIN
  (* REPEAT..UNTIL: sum 1..10 *)
  sum := 0;
  i := 1;
  REPEAT
    sum := sum + i;
    INC(i)
  UNTIL i > 10;
  WriteString("REPEAT sum 1..10 = "); WriteInt(sum, 1); WriteLn;

  (* FOR..BY with step 2 *)
  sum := 0;
  FOR i := 1 TO 10 BY 2 DO
    sum := sum + i
  END;
  WriteString("FOR BY 2 (odd 1..10) = "); WriteInt(sum, 1); WriteLn;

  (* FOR..BY with negative step *)
  sum := 0;
  FOR i := 10 TO 1 BY -1 DO
    sum := sum + i
  END;
  WriteString("FOR BY -1 (10..1) = "); WriteInt(sum, 1); WriteLn;

  (* FOR..BY with step 3 *)
  sum := 0;
  FOR i := 0 TO 20 BY 3 DO
    sum := sum + i
  END;
  WriteString("FOR BY 3 (0,3,6..18) = "); WriteInt(sum, 1); WriteLn;

  (* Nested REPEAT *)
  n := 0;
  i := 1;
  REPEAT
    sum := 0;
    REPEAT
      INC(sum);
      INC(n)
    UNTIL sum >= i;
    INC(i)
  UNTIL i > 5;
  WriteString("Nested REPEAT count = "); WriteInt(n, 1); WriteLn;

  WriteString("Done"); WriteLn
END RepeatTest.
