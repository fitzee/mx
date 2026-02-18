MODULE Nested;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

VAR result: INTEGER;

PROCEDURE Outer(n: INTEGER): INTEGER;
  VAR sum: INTEGER;

  PROCEDURE Inner(x: INTEGER): INTEGER;
  BEGIN
    RETURN x * x
  END Inner;

BEGIN
  sum := 0;
  WHILE n > 0 DO
    sum := sum + Inner(n);
    DEC(n)
  END;
  RETURN sum
END Outer;

BEGIN
  result := Outer(5);
  WriteString("Sum of squares 1..5 = ");
  WriteInt(result, 1);
  WriteLn;

  result := Outer(10);
  WriteString("Sum of squares 1..10 = ");
  WriteInt(result, 1);
  WriteLn
END Nested.
