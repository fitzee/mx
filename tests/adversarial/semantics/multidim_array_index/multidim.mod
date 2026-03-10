MODULE MultiDim;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

CONST N = 3;

TYPE
  Matrix = ARRAY [1..N], [1..N] OF INTEGER;

VAR
  m: Matrix;
  i, j, sum: INTEGER;

BEGIN
  (* Fill matrix: m[i,j] = i * 10 + j *)
  FOR i := 1 TO N DO
    FOR j := 1 TO N DO
      m[i][j] := i * 10 + j
    END
  END;

  (* Print matrix *)
  WriteString("Matrix:"); WriteLn;
  FOR i := 1 TO N DO
    FOR j := 1 TO N DO
      WriteInt(m[i][j], 4)
    END;
    WriteLn
  END;

  (* Sum of all elements *)
  sum := 0;
  FOR i := 1 TO N DO
    FOR j := 1 TO N DO
      sum := sum + m[i][j]
    END
  END;
  WriteString("Sum: "); WriteInt(sum, 1); WriteLn
END MultiDim.
