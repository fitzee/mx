MODULE ProcVarCollision;
FROM InOut IMPORT WriteInt, WriteLn;
IMPORT PVC_A;
IMPORT PVC_B;

TYPE CalcFunc = PROCEDURE(INTEGER): INTEGER;

VAR
  fa, fb: CalcFunc;
  r: INTEGER;
BEGIN
  fa := PVC_A.Calc;
  fb := PVC_B.Calc;
  r := fa(5);
  WriteInt(r, 0); WriteLn;   (* 105 *)
  r := fb(5);
  WriteInt(r, 0); WriteLn    (* 15 *)
END ProcVarCollision.
