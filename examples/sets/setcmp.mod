MODULE SetCmp;
(* Test set comparison operators *)
FROM InOut IMPORT WriteString, WriteLn;

VAR
  a, b, c, empty: BITSET;

PROCEDURE Check(msg: ARRAY OF CHAR; val: BOOLEAN);
BEGIN
  WriteString(msg);
  IF val THEN WriteString(": TRUE")
  ELSE WriteString(": FALSE")
  END;
  WriteLn
END Check;

BEGIN
  empty := {};
  a := {1, 2, 3};
  b := {1, 2, 3, 4, 5};
  c := {1, 2, 3};

  (* Equality *)
  Check("a = c (expect TRUE)", a = c);
  Check("a = b (expect FALSE)", a = b);
  Check("a # b (expect TRUE)", a # b);
  Check("a # c (expect FALSE)", a # c);

  (* Subset <= *)
  Check("a <= b (expect TRUE)", a <= b);
  Check("b <= a (expect FALSE)", b <= a);
  Check("a <= c (expect TRUE)", a <= c);
  Check("empty <= a (expect TRUE)", empty <= a);

  (* Superset >= *)
  Check("b >= a (expect TRUE)", b >= a);
  Check("a >= b (expect FALSE)", a >= b);
  Check("a >= c (expect TRUE)", a >= c);
  Check("a >= empty (expect TRUE)", a >= empty);

  (* Set operations *)
  Check("a + {4,5} = b (expect TRUE)", a + {4, 5} = b);
  Check("b - {4,5} = a (expect TRUE)", b - {4, 5} = a);
  Check("a * b = a (expect TRUE)", a * b = a);

  WriteString("Done"); WriteLn
END SetCmp.
