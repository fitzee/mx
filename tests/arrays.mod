MODULE Arrays;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

CONST
  N = 10;

TYPE
  IntArray = ARRAY [1..N] OF INTEGER;

VAR
  a: IntArray;
  i, sum: INTEGER;

BEGIN
  FOR i := 1 TO N DO
    a[i] := i * i
  END;
  sum := 0;
  FOR i := 1 TO N DO
    sum := sum + a[i]
  END;
  WriteString("Sum of squares 1..10 = ");
  WriteInt(sum, 0);
  WriteLn
END Arrays.
