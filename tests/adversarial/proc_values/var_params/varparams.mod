MODULE VarParams;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

VAR a, b: INTEGER;

PROCEDURE Swap(VAR x, y: INTEGER);
  VAR tmp: INTEGER;
BEGIN
  tmp := x;
  x := y;
  y := tmp
END Swap;

PROCEDURE DoubleIt(VAR n: INTEGER);
BEGIN
  n := n * 2
END DoubleIt;

PROCEDURE AddTo(VAR acc: INTEGER; val: INTEGER);
BEGIN
  acc := acc + val
END AddTo;

BEGIN
  a := 10;
  b := 20;
  WriteString("Before swap: a="); WriteInt(a, 1);
  WriteString(" b="); WriteInt(b, 1); WriteLn;

  Swap(a, b);
  WriteString("After swap:  a="); WriteInt(a, 1);
  WriteString(" b="); WriteInt(b, 1); WriteLn;

  DoubleIt(a);
  WriteString("After double a: a="); WriteInt(a, 1); WriteLn;

  AddTo(b, 5);
  WriteString("After add 5 to b: b="); WriteInt(b, 1); WriteLn
END VarParams.
