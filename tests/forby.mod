MODULE ForBy;
(* Comprehensive test of FOR loops with various BY clauses *)
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

CONST
  Step = 3;
  NegStep = -2;

VAR
  i, sum: INTEGER;

BEGIN
  (* Forward by 1 (default) *)
  sum := 0;
  FOR i := 1 TO 5 DO
    sum := sum + i
  END;
  WriteString("BY 1: "); WriteInt(sum, 1); WriteLn;

  (* Forward by 2 *)
  sum := 0;
  FOR i := 0 TO 10 BY 2 DO
    sum := sum + i
  END;
  WriteString("BY 2: "); WriteInt(sum, 1); WriteLn;

  (* Forward by constant *)
  sum := 0;
  FOR i := 0 TO 12 BY Step DO
    sum := sum + i
  END;
  WriteString("BY Step(3): "); WriteInt(sum, 1); WriteLn;

  (* Backward by -1 *)
  sum := 0;
  FOR i := 10 TO 1 BY -1 DO
    sum := sum + i
  END;
  WriteString("BY -1: "); WriteInt(sum, 1); WriteLn;

  (* Backward by -2 *)
  sum := 0;
  FOR i := 10 TO 0 BY -2 DO
    sum := sum + i
  END;
  WriteString("BY -2: "); WriteInt(sum, 1); WriteLn;

  (* Backward by negative constant *)
  sum := 0;
  FOR i := 10 TO 0 BY NegStep DO
    sum := sum + i
  END;
  WriteString("BY NegStep(-2): "); WriteInt(sum, 1); WriteLn;

  (* Single iteration *)
  sum := 0;
  FOR i := 5 TO 5 DO
    sum := sum + i
  END;
  WriteString("Single iter: "); WriteInt(sum, 1); WriteLn;

  (* Empty range (start > end, step > 0) *)
  sum := 0;
  FOR i := 10 TO 5 DO
    sum := sum + i
  END;
  WriteString("Empty range: "); WriteInt(sum, 1); WriteLn;

  WriteString("Done"); WriteLn
END ForBy.
