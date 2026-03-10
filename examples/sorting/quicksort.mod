MODULE QuickSort;
(* Quicksort implementation *)
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

CONST N = 20;

TYPE IntArray = ARRAY [0..N-1] OF INTEGER;

VAR
  arr: IntArray;
  i: INTEGER;

PROCEDURE Swap(VAR a, b: INTEGER);
  VAR tmp: INTEGER;
BEGIN
  tmp := a;
  a := b;
  b := tmp
END Swap;

PROCEDURE Partition(VAR a: IntArray; lo, hi: INTEGER): INTEGER;
  VAR pivot, i, j: INTEGER;
BEGIN
  pivot := a[hi];
  i := lo;
  FOR j := lo TO hi - 1 DO
    IF a[j] <= pivot THEN
      Swap(a[i], a[j]);
      INC(i)
    END
  END;
  Swap(a[i], a[hi]);
  RETURN i
END Partition;

PROCEDURE QSort(VAR a: IntArray; lo, hi: INTEGER);
  VAR p: INTEGER;
BEGIN
  IF lo < hi THEN
    p := Partition(a, lo, hi);
    QSort(a, lo, p - 1);
    QSort(a, p + 1, hi)
  END
END QSort;

PROCEDURE PrintArray(a: IntArray);
  VAR i: INTEGER;
BEGIN
  FOR i := 0 TO N - 1 DO
    WriteInt(a[i], 5)
  END;
  WriteLn
END PrintArray;

PROCEDURE IsSorted(a: IntArray): BOOLEAN;
  VAR i: INTEGER;
BEGIN
  FOR i := 0 TO N - 2 DO
    IF a[i] > a[i + 1] THEN
      RETURN FALSE
    END
  END;
  RETURN TRUE
END IsSorted;

BEGIN
  (* Initialize with pseudo-random values *)
  arr[0] := 73; arr[1] := 12; arr[2] := 98; arr[3] := 41;
  arr[4] := 55; arr[5] := 27; arr[6] := 86; arr[7] := 33;
  arr[8] := 64; arr[9] := 19; arr[10] := 7; arr[11] := 91;
  arr[12] := 45; arr[13] := 3; arr[14] := 78; arr[15] := 52;
  arr[16] := 16; arr[17] := 69; arr[18] := 38; arr[19] := 84;

  WriteString("Before: ");
  PrintArray(arr);

  QSort(arr, 0, N - 1);

  WriteString("After:  ");
  PrintArray(arr);

  IF IsSorted(arr) THEN
    WriteString("Sorted: YES")
  ELSE
    WriteString("Sorted: NO")
  END;
  WriteLn;

  WriteString("Done"); WriteLn
END QuickSort.
