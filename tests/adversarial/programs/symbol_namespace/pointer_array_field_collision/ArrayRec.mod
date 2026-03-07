IMPLEMENTATION MODULE ArrayRec;

PROCEDURE InitArr(VAR r: ArrBuf);
VAR i: INTEGER;
BEGIN
  FOR i := 0 TO 7 DO r.data[i] := i + 1 END;
  r.len := 8
END InitArr;

PROCEDURE SumArr(VAR r: ArrBuf): INTEGER;
VAR i, s: INTEGER;
BEGIN
  s := 0;
  FOR i := 0 TO r.len - 1 DO s := s + r.data[i] END;
  RETURN s
END SumArr;

END ArrayRec.
