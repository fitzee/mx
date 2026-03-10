MODULE ProcVarCross;
FROM InOut IMPORT WriteInt, WriteLn;
FROM PVMath IMPORT Inc, Dec, Negate;

TYPE UnaryOp = PROCEDURE(INTEGER): INTEGER;

VAR
  ops: ARRAY [0..2] OF UnaryOp;
  i, r: INTEGER;
BEGIN
  ops[0] := Inc;
  ops[1] := Dec;
  ops[2] := Negate;

  FOR i := 0 TO 2 DO
    r := ops[i](10);
    WriteInt(r, 0); WriteLn
  END
END ProcVarCross.
