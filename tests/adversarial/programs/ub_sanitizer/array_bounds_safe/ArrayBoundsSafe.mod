MODULE ArrayBoundsSafe;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;
VAR
  a: ARRAY [0..9] OF INTEGER;
  i, sum: INTEGER;
BEGIN
  (* Fill array *)
  FOR i := 0 TO 9 DO
    a[i] := i * i
  END;

  (* Sum array *)
  sum := 0;
  FOR i := 0 TO 9 DO
    sum := sum + a[i]
  END;
  WriteString("Sum:"); WriteInt(sum, 0); WriteLn;

  (* Boundary access *)
  WriteString("First:"); WriteInt(a[0], 0); WriteLn;
  WriteString("Last:"); WriteInt(a[9], 0); WriteLn;

  (* Nested array copy *)
  FOR i := 1 TO 9 DO
    a[i] := a[i-1] + 1
  END;
  WriteString("Chain:"); WriteInt(a[9], 0); WriteLn
END ArrayBoundsSafe.
