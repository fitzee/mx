MODULE OpenArray;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

TYPE IntArray = ARRAY [1..10] OF INTEGER;

VAR
  a: IntArray;
  i: INTEGER;

PROCEDURE PrintArray(arr: ARRAY OF INTEGER);
  VAR i: INTEGER;
BEGIN
  FOR i := 0 TO HIGH(arr) DO
    WriteInt(arr[i], 4)
  END;
  WriteLn
END PrintArray;

PROCEDURE SumArray(arr: ARRAY OF INTEGER): INTEGER;
  VAR i, sum: INTEGER;
BEGIN
  sum := 0;
  FOR i := 0 TO HIGH(arr) DO
    sum := sum + arr[i]
  END;
  RETURN sum
END SumArray;

BEGIN
  FOR i := 1 TO 10 DO
    a[i] := i * i
  END;
  WriteString("Array: ");
  PrintArray(a);
  WriteString("Sum: ");
  WriteInt(SumArray(a), 1);
  WriteLn
END OpenArray.
