MODULE VarAliasing;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

VAR g: INTEGER;

PROCEDURE Swap(VAR a, b: INTEGER);
VAR t: INTEGER;
BEGIN
  t := a;
  a := b;
  b := t
END Swap;

PROCEDURE AddTo(VAR dest: INTEGER; val: INTEGER);
BEGIN
  dest := dest + val
END AddTo;

PROCEDURE DoubleViaAlias(VAR x, y: INTEGER);
(* When x and y alias the same variable *)
BEGIN
  x := x + y
END DoubleViaAlias;

VAR
  a, b: INTEGER;
  arr: ARRAY [0..4] OF INTEGER;
  i: INTEGER;

BEGIN
  (* Test 1: Normal swap *)
  a := 10; b := 20;
  Swap(a, b);
  WriteString("Swap:"); WriteInt(a, 0);
  WriteString(","); WriteInt(b, 0); WriteLn;

  (* Test 2: AddTo with value *)
  a := 5;
  AddTo(a, 3);
  WriteString("Add:"); WriteInt(a, 0); WriteLn;

  (* Test 3: Array element VAR params *)
  arr[0] := 100; arr[1] := 200;
  Swap(arr[0], arr[1]);
  WriteString("ArrSwap:"); WriteInt(arr[0], 0);
  WriteString(","); WriteInt(arr[1], 0); WriteLn;

  (* Test 4: Global variable via VAR param *)
  g := 42;
  AddTo(g, 8);
  WriteString("Global:"); WriteInt(g, 0); WriteLn;

  (* Test 5: Self-aliased VAR params — both point to same var *)
  a := 7;
  DoubleViaAlias(a, a);
  WriteString("Alias:"); WriteInt(a, 0); WriteLn
END VarAliasing.
