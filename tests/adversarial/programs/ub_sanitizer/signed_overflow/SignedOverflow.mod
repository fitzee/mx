MODULE SignedOverflow;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;
VAR
  a, b, c: INTEGER;
BEGIN
  (* Safe multiplications *)
  a := 1000;
  b := 1000;
  c := a * b;
  WriteString("Mul:"); WriteInt(c, 0); WriteLn;

  (* Safe additions *)
  a := 100000;
  b := 200000;
  c := a + b;
  WriteString("Add:"); WriteInt(c, 0); WriteLn;

  (* Safe subtractions *)
  c := b - a;
  WriteString("Sub:"); WriteInt(c, 0); WriteLn;

  (* Negation *)
  a := 42;
  c := -a;
  WriteString("Neg:"); WriteInt(c, 0); WriteLn;

  (* Chained safe operations *)
  a := 10; b := 20;
  c := (a + b) * (a - b) + a;
  WriteString("Chain:"); WriteInt(c, 0); WriteLn
END SignedOverflow.
