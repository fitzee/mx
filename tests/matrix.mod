MODULE Matrix;
(* Matrix operations with records containing array fields *)
FROM InOut IMPORT WriteString, WriteInt, WriteLn;
FROM RealInOut IMPORT WriteFixPt;

CONST N = 3;

TYPE
  Vec = ARRAY [0..N-1] OF REAL;
  Mat = ARRAY [0..N-1], [0..N-1] OF REAL;

VAR
  A, B, C: Mat;
  v, w: Vec;
  det: REAL;

PROCEDURE MatMul(A, B: Mat; VAR C: Mat);
  VAR i, j, k: INTEGER;
      sum: REAL;
BEGIN
  FOR i := 0 TO N-1 DO
    FOR j := 0 TO N-1 DO
      sum := 0.0;
      FOR k := 0 TO N-1 DO
        sum := sum + A[i, k] * B[k, j]
      END;
      C[i, j] := sum
    END
  END
END MatMul;

PROCEDURE MatVecMul(A: Mat; v: Vec; VAR w: Vec);
  VAR i, j: INTEGER;
      sum: REAL;
BEGIN
  FOR i := 0 TO N-1 DO
    sum := 0.0;
    FOR j := 0 TO N-1 DO
      sum := sum + A[i, j] * v[j]
    END;
    w[i] := sum
  END
END MatVecMul;

PROCEDURE Transpose(A: Mat; VAR T: Mat);
  VAR i, j: INTEGER;
BEGIN
  FOR i := 0 TO N-1 DO
    FOR j := 0 TO N-1 DO
      T[j, i] := A[i, j]
    END
  END
END Transpose;

PROCEDURE Identity(VAR M: Mat);
  VAR i, j: INTEGER;
BEGIN
  FOR i := 0 TO N-1 DO
    FOR j := 0 TO N-1 DO
      IF i = j THEN M[i, j] := 1.0
      ELSE M[i, j] := 0.0
      END
    END
  END
END Identity;

PROCEDURE PrintMat(M: Mat);
  VAR i, j: INTEGER;
BEGIN
  FOR i := 0 TO N-1 DO
    FOR j := 0 TO N-1 DO
      WriteFixPt(M[i, j], 8, 2)
    END;
    WriteLn
  END
END PrintMat;

PROCEDURE PrintVec(v: Vec);
  VAR i: INTEGER;
BEGIN
  FOR i := 0 TO N-1 DO
    WriteFixPt(v[i], 8, 2)
  END;
  WriteLn
END PrintVec;

PROCEDURE Det3(M: Mat): REAL;
BEGIN
  RETURN M[0,0] * (M[1,1]*M[2,2] - M[1,2]*M[2,1])
       - M[0,1] * (M[1,0]*M[2,2] - M[1,2]*M[2,0])
       + M[0,2] * (M[1,0]*M[2,1] - M[1,1]*M[2,0])
END Det3;

BEGIN
  (* Test identity matrix *)
  WriteString("Identity:"); WriteLn;
  Identity(A);
  PrintMat(A);

  (* Set up test matrix *)
  A[0,0] := 1.0; A[0,1] := 2.0; A[0,2] := 3.0;
  A[1,0] := 4.0; A[1,1] := 5.0; A[1,2] := 6.0;
  A[2,0] := 7.0; A[2,1] := 8.0; A[2,2] := 0.0;

  WriteString("Matrix A:"); WriteLn;
  PrintMat(A);

  (* Determinant *)
  det := Det3(A);
  WriteString("Det(A) = "); WriteFixPt(det, 8, 2); WriteLn;

  (* Transpose *)
  WriteString("Transpose(A):"); WriteLn;
  Transpose(A, B);
  PrintMat(B);

  (* Matrix-vector multiply *)
  v[0] := 1.0; v[1] := 2.0; v[2] := 3.0;
  WriteString("v = "); PrintVec(v);
  MatVecMul(A, v, w);
  WriteString("A*v = "); PrintVec(w);

  (* A * I = A *)
  Identity(B);
  MatMul(A, B, C);
  WriteString("A*I:"); WriteLn;
  PrintMat(C);

  WriteString("Done"); WriteLn
END Matrix.
