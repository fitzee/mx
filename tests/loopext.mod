MODULE LoopExt;
(* Test LOOP/EXIT, nested loops, and complex control flow *)
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

VAR
  i, j, found: INTEGER;
  arr: ARRAY [0..9] OF INTEGER;

BEGIN
  (* Initialize array *)
  arr[0] := 15; arr[1] := 23; arr[2] := 7; arr[3] := 42;
  arr[4] := 11; arr[5] := 99; arr[6] := 3; arr[7] := 56;
  arr[8] := 31; arr[9] := 8;

  (* Linear search using LOOP/EXIT *)
  i := 0;
  found := -1;
  LOOP
    IF i > 9 THEN EXIT END;
    IF arr[i] = 42 THEN
      found := i;
      EXIT
    END;
    INC(i)
  END;
  WriteString("Found 42 at index "); WriteInt(found, 1); WriteLn;

  (* Search for non-existent value *)
  i := 0;
  found := -1;
  LOOP
    IF i > 9 THEN EXIT END;
    IF arr[i] = 100 THEN
      found := i;
      EXIT
    END;
    INC(i)
  END;
  WriteString("Found 100 at index "); WriteInt(found, 1); WriteLn;

  (* Bubble sort using nested WHILE/LOOP *)
  i := 9;
  WHILE i > 0 DO
    j := 0;
    WHILE j < i DO
      IF arr[j] > arr[j+1] THEN
        (* swap *)
        found := arr[j];
        arr[j] := arr[j+1];
        arr[j+1] := found
      END;
      INC(j)
    END;
    DEC(i)
  END;

  WriteString("Sorted: ");
  FOR i := 0 TO 9 DO
    WriteInt(arr[i], 4)
  END;
  WriteLn;

  WriteString("Done"); WriteLn
END LoopExt.
