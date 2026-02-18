MODULE SetOps2;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

VAR
  s1, s2, s3: BITSET;
  i: INTEGER;

BEGIN
  (* Build sets *)
  s1 := {1, 3, 5, 7};      (* odd bits *)
  s2 := {2, 3, 6, 7};      (* some overlap *)

  (* Union: s1 + s2 = {1,2,3,5,6,7} *)
  s3 := s1 + s2;
  WriteString("Union:        ");
  FOR i := 0 TO 9 DO
    IF i IN s3 THEN WriteInt(i, 3) END
  END;
  WriteLn;

  (* Intersection: s1 * s2 = {3,7} *)
  s3 := s1 * s2;
  WriteString("Intersection: ");
  FOR i := 0 TO 9 DO
    IF i IN s3 THEN WriteInt(i, 3) END
  END;
  WriteLn;

  (* Difference: s1 - s2 = {1,5} *)
  s3 := s1 - s2;
  WriteString("Difference:   ");
  FOR i := 0 TO 9 DO
    IF i IN s3 THEN WriteInt(i, 3) END
  END;
  WriteLn;

  (* Symmetric difference: s1 / s2 = {1,2,5,6} *)
  s3 := s1 / s2;
  WriteString("Sym Diff:     ");
  FOR i := 0 TO 9 DO
    IF i IN s3 THEN WriteInt(i, 3) END
  END;
  WriteLn;

  (* Compound: (s1 + s2) * {1, 2, 3} = {1, 2, 3} *)
  s3 := (s1 + s2) * {1, 2, 3};
  WriteString("Compound:     ");
  FOR i := 0 TO 9 DO
    IF i IN s3 THEN WriteInt(i, 3) END
  END;
  WriteLn;

  (* Empty set *)
  s3 := s1 * {};
  WriteString("s1 * {} =     ");
  IF s3 = {} THEN WriteString("empty") ELSE WriteString("not empty") END;
  WriteLn
END SetOps2.
