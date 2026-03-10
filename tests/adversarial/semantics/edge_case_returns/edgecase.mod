MODULE EdgeCase;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

CONST
  N = 10;
  M = N * 2;     (* constant expression *)
  K = M + 5;     (* chained constant *)

TYPE
  SmallRange = [0..N];
  Index = [1..100];
  Color = (Red, Green, Blue);

VAR
  i: INTEGER;
  c: Color;
  x: CARDINAL;

PROCEDURE TestHALT;
(* HALT tested indirectly - we don't call it to avoid terminating *)
BEGIN
  WriteString("HALT test skipped (would exit)");
  WriteLn
END TestHALT;

PROCEDURE Max3(a, b, c: INTEGER): INTEGER;
  VAR result: INTEGER;
BEGIN
  result := a;
  IF b > result THEN result := b END;
  IF c > result THEN result := c END;
  RETURN result
END Max3;

PROCEDURE TestMultiReturn(n: INTEGER): INTEGER;
BEGIN
  IF n < 0 THEN
    RETURN -n
  ELSIF n = 0 THEN
    RETURN 0
  ELSE
    RETURN n * 2
  END
END TestMultiReturn;

PROCEDURE TestINCDEC;
  VAR x, y: INTEGER;
BEGIN
  x := 10;
  INC(x);        (* x = 11 *)
  INC(x, 5);     (* x = 16 *)
  DEC(x);        (* x = 15 *)
  DEC(x, 3);     (* x = 12 *)
  WriteString("INC/DEC: "); WriteInt(x, 1); WriteLn;

  y := 0;
  INC(y, 100);
  DEC(y, 50);
  WriteString("INC/DEC 2: "); WriteInt(y, 1); WriteLn
END TestINCDEC;

PROCEDURE TestEnum;
BEGIN
  c := Red;
  WriteString("ORD(Red) = "); WriteInt(ORD(c), 1); WriteLn;
  c := Blue;
  WriteString("ORD(Blue) = "); WriteInt(ORD(c), 1); WriteLn
END TestEnum;

PROCEDURE TestABS;
  VAR a: INTEGER;
BEGIN
  a := -42;
  WriteString("ABS(-42) = "); WriteInt(ABS(a), 1); WriteLn;
  a := 42;
  WriteString("ABS(42) = "); WriteInt(ABS(a), 1); WriteLn
END TestABS;

PROCEDURE TestCHR;
  VAR ch: CHAR;
BEGIN
  ch := CHR(65);
  WriteString("CHR(65) = ");
  IF ch = 'A' THEN WriteString("A") ELSE WriteString("?") END;
  WriteLn
END TestCHR;

BEGIN
  WriteString("K = "); WriteInt(K, 1); WriteLn;
  WriteString("Max3(3,7,5) = "); WriteInt(Max3(3, 7, 5), 1); WriteLn;
  WriteString("MultiReturn(-5) = "); WriteInt(TestMultiReturn(-5), 1); WriteLn;
  WriteString("MultiReturn(0) = "); WriteInt(TestMultiReturn(0), 1); WriteLn;
  WriteString("MultiReturn(3) = "); WriteInt(TestMultiReturn(3), 1); WriteLn;
  TestINCDEC;
  TestEnum;
  TestABS;
  TestCHR
END EdgeCase.
