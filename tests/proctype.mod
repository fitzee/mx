MODULE ProcType;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

TYPE
  IntFunc = PROCEDURE(INTEGER): INTEGER;
  IntProc = PROCEDURE(INTEGER);

VAR
  f: IntFunc;
  p: IntProc;
  result: INTEGER;

PROCEDURE Double(n: INTEGER): INTEGER;
BEGIN
  RETURN n * 2
END Double;

PROCEDURE Square(n: INTEGER): INTEGER;
BEGIN
  RETURN n * n
END Square;

PROCEDURE PrintVal(n: INTEGER);
BEGIN
  WriteInt(n, 1); WriteLn
END PrintVal;

PROCEDURE Apply(func: IntFunc; val: INTEGER): INTEGER;
BEGIN
  RETURN func(val)
END Apply;

PROCEDURE ApplyAndPrint(func: IntFunc; val: INTEGER);
BEGIN
  WriteString("Result: ");
  WriteInt(func(val), 1);
  WriteLn
END ApplyAndPrint;

PROCEDURE Map(func: IntFunc; a: ARRAY OF INTEGER);
  VAR i: INTEGER;
BEGIN
  FOR i := 0 TO HIGH(a) DO
    WriteInt(func(a[i]), 4)
  END;
  WriteLn
END Map;

VAR
  arr: ARRAY [0..4] OF INTEGER;
  i: INTEGER;

BEGIN
  (* Assign procedure to procedure variable *)
  f := Double;
  result := f(5);
  WriteString("Double(5) = "); WriteInt(result, 1); WriteLn;

  f := Square;
  result := f(5);
  WriteString("Square(5) = "); WriteInt(result, 1); WriteLn;

  (* Pass procedure as argument *)
  WriteString("Apply(Double, 7) = "); WriteInt(Apply(Double, 7), 1); WriteLn;
  WriteString("Apply(Square, 7) = "); WriteInt(Apply(Square, 7), 1); WriteLn;

  ApplyAndPrint(Double, 10);
  ApplyAndPrint(Square, 10);

  (* Map function over array *)
  FOR i := 0 TO 4 DO arr[i] := i + 1 END;
  WriteString("Double:");
  Map(Double, arr);
  WriteString("Square:");
  Map(Square, arr)
END ProcType.
