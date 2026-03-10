MODULE ProcVarBasic;
FROM InOut IMPORT WriteInt, WriteLn;

TYPE IntFunc = PROCEDURE(INTEGER): INTEGER;

PROCEDURE Double(x: INTEGER): INTEGER;
BEGIN RETURN x * 2 END Double;

PROCEDURE Square(x: INTEGER): INTEGER;
BEGIN RETURN x * x END Square;

PROCEDURE Apply(f: IntFunc; x: INTEGER): INTEGER;
BEGIN RETURN f(x) END Apply;

VAR
  fn: IntFunc;
  r: INTEGER;
BEGIN
  fn := Double;
  r := fn(5);
  WriteInt(r, 0); WriteLn;    (* 10 *)

  fn := Square;
  r := fn(5);
  WriteInt(r, 0); WriteLn;    (* 25 *)

  r := Apply(Double, 7);
  WriteInt(r, 0); WriteLn;    (* 14 *)

  r := Apply(Square, 7);
  WriteInt(r, 0); WriteLn     (* 49 *)
END ProcVarBasic.
