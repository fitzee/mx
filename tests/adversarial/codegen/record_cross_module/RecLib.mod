IMPLEMENTATION MODULE RecLib;
FROM SYSTEM IMPORT ADDRESS;
FROM Strings IMPORT Assign;

PROCEDURE MakePair(a, b: INTEGER; VAR p: Pair);
BEGIN
  p.x := a;
  p.y := b
END MakePair;

PROCEDURE Sum(p: Pair): INTEGER;
BEGIN
  RETURN p.x + p.y
END Sum;

PROCEDURE SumVAR(VAR p: Pair): INTEGER;
BEGIN
  RETURN p.x + p.y
END SumVAR;

PROCEDURE ReadAddr(loc: ADDRESS): ADDRESS;
VAR ap: AddrPtr;
BEGIN
  ap := loc;
  RETURN ap^
END ReadAddr;

END RecLib.
