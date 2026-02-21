MODULE NestedCompound;
FROM InOut IMPORT WriteInt, WriteLn;

TYPE
  Pair = RECORD
    a, b: INTEGER
  END;
  Row = RECORD
    data: ARRAY [0..3] OF INTEGER;
    len: INTEGER
  END;

VAR
  pairs: ARRAY [0..2] OF Pair;
  row: Row;
  i, sum: INTEGER;

PROCEDURE InitPairs;
VAR k: INTEGER;
BEGIN
  FOR k := 0 TO 2 DO
    pairs[k].a := k * 10;
    pairs[k].b := k * 10 + 1
  END
END InitPairs;

PROCEDURE FillRow(VAR r: Row);
VAR k: INTEGER;
BEGIN
  r.len := 4;
  FOR k := 0 TO 3 DO
    r.data[k] := (k + 1) * 5
  END
END FillRow;

BEGIN
  InitPairs;
  (* Check array of record *)
  FOR i := 0 TO 2 DO
    WriteInt(pairs[i].a, 0); WriteLn;
    WriteInt(pairs[i].b, 0); WriteLn
  END;
  (* pairs: 0,1,10,11,20,21 *)

  FillRow(row);
  (* Check record with array field *)
  WriteInt(row.len, 0); WriteLn;  (* 4 *)
  sum := 0;
  FOR i := 0 TO 3 DO
    sum := sum + row.data[i]
  END;
  WriteInt(sum, 0); WriteLn       (* 5+10+15+20=50 *)
END NestedCompound.
