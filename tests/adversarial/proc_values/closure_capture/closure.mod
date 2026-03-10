MODULE Closure;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

PROCEDURE Accumulate(n: INTEGER): INTEGER;
  VAR total: INTEGER;

  PROCEDURE Add(x: INTEGER);
  BEGIN
    total := total + x
  END Add;

BEGIN
  total := 0;
  Add(n);
  Add(n * 2);
  Add(n * 3);
  RETURN total
END Accumulate;

PROCEDURE Counter(): INTEGER;
  VAR count: INTEGER;

  PROCEDURE Increment;
  BEGIN
    INC(count)
  END Increment;

  PROCEDURE GetCount(): INTEGER;
  BEGIN
    RETURN count
  END GetCount;

BEGIN
  count := 0;
  Increment;
  Increment;
  Increment;
  RETURN GetCount()
END Counter;

PROCEDURE DeepNest(n: INTEGER): INTEGER;
  VAR a: INTEGER;

  PROCEDURE Middle(): INTEGER;
    VAR b: INTEGER;

    PROCEDURE Inner(): INTEGER;
    BEGIN
      RETURN a + b
    END Inner;

  BEGIN
    b := n * 10;
    RETURN Inner()
  END Middle;

BEGIN
  a := n;
  RETURN Middle()
END DeepNest;

BEGIN
  WriteString("Accumulate(5) = ");
  WriteInt(Accumulate(5), 1);
  WriteLn;

  WriteString("Counter = ");
  WriteInt(Counter(), 1);
  WriteLn;

  WriteString("DeepNest(3) = ");
  WriteInt(DeepNest(3), 1);
  WriteLn
END Closure.
