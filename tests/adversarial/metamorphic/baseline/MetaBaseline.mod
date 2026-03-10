MODULE MetaBaseline;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

PROCEDURE Fib(n: INTEGER): INTEGER;
BEGIN
  IF n <= 1 THEN RETURN n
  ELSE RETURN Fib(n-1) + Fib(n-2)
  END
END Fib;

PROCEDURE Factorial(n: INTEGER): INTEGER;
VAR r, i: INTEGER;
BEGIN
  r := 1;
  FOR i := 2 TO n DO r := r * i END;
  RETURN r
END Factorial;

PROCEDURE GCD(a, b: INTEGER): INTEGER;
BEGIN
  WHILE b # 0 DO
    a := a MOD b;
    IF a = 0 THEN RETURN b END;
    b := b MOD a;
    IF b = 0 THEN RETURN a END
  END;
  RETURN a
END GCD;

VAR i: INTEGER;

BEGIN
  (* Fibonacci *)
  FOR i := 0 TO 10 DO
    WriteInt(Fib(i), 4)
  END;
  WriteLn;

  (* Factorials *)
  FOR i := 1 TO 10 DO
    WriteInt(Factorial(i), 8)
  END;
  WriteLn;

  (* GCD pairs *)
  WriteInt(GCD(48, 18), 0); WriteLn;
  WriteInt(GCD(100, 75), 0); WriteLn;
  WriteInt(GCD(17, 13), 0); WriteLn
END MetaBaseline.
