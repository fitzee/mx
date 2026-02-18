MODULE Sorting;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

CONST N = 10;

TYPE IntArray = ARRAY [0..N-1] OF INTEGER;

VAR
  a: IntArray;
  i: INTEGER;

PROCEDURE Swap(VAR x, y: INTEGER);
  VAR t: INTEGER;
BEGIN
  t := x; x := y; y := t
END Swap;

PROCEDURE BubbleSort(VAR a: ARRAY OF INTEGER);
  VAR i, j, n: INTEGER;
BEGIN
  n := HIGH(a);
  FOR i := 0 TO n - 1 DO
    FOR j := 0 TO n - 1 - i DO
      IF a[j] > a[j+1] THEN
        Swap(a[j], a[j+1])
      END
    END
  END
END BubbleSort;

PROCEDURE InsertionSort(VAR a: ARRAY OF INTEGER);
  VAR i, j, key, n: INTEGER;
BEGIN
  n := HIGH(a);
  FOR i := 1 TO n DO
    key := a[i];
    j := i - 1;
    WHILE (j >= 0) AND (a[j] > key) DO
      a[j+1] := a[j];
      DEC(j)
    END;
    a[j+1] := key
  END
END InsertionSort;

PROCEDURE PrintArray(a: ARRAY OF INTEGER);
  VAR i: INTEGER;
BEGIN
  FOR i := 0 TO HIGH(a) DO
    WriteInt(a[i], 4)
  END;
  WriteLn
END PrintArray;

PROCEDURE IsSorted(a: ARRAY OF INTEGER): BOOLEAN;
  VAR i: INTEGER;
BEGIN
  FOR i := 0 TO HIGH(a) - 1 DO
    IF a[i] > a[i+1] THEN RETURN FALSE END
  END;
  RETURN TRUE
END IsSorted;

BEGIN
  (* Initialize with random-looking values *)
  a[0] := 42; a[1] := 17; a[2] := 85; a[3] := 3;
  a[4] := 61; a[5] := 29; a[6] := 94; a[7] := 50;
  a[8] := 11; a[9] := 73;

  WriteString("Original:  "); PrintArray(a);

  BubbleSort(a);
  WriteString("BubSort:   "); PrintArray(a);
  WriteString("Sorted: ");
  IF IsSorted(a) THEN WriteString("YES") ELSE WriteString("NO") END;
  WriteLn;

  (* Re-randomize *)
  a[0] := 55; a[1] := 8; a[2] := 99; a[3] := 22;
  a[4] := 77; a[5] := 13; a[6] := 66; a[7] := 44;
  a[8] := 31; a[9] := 88;

  InsertionSort(a);
  WriteString("InsSort:   "); PrintArray(a);
  WriteString("Sorted: ");
  IF IsSorted(a) THEN WriteString("YES") ELSE WriteString("NO") END;
  WriteLn
END Sorting.
