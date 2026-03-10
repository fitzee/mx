MODULE PointerChain;
FROM InOut IMPORT WriteInt, WriteLn;
FROM Storage IMPORT ALLOCATE, DEALLOCATE;

TYPE
  IntProc = PROCEDURE(INTEGER): INTEGER;
  NodePtr = POINTER TO Node;
  Node = RECORD
    id, value: INTEGER;
    next: NodePtr
  END;

PROCEDURE Inc(x: INTEGER): INTEGER;
BEGIN RETURN x + 1 END Inc;

PROCEDURE DoubleIt(x: INTEGER): INTEGER;
BEGIN RETURN x * 2 END DoubleIt;

PROCEDURE AddTen(x: INTEGER): INTEGER;
BEGIN RETURN x + 10 END AddTen;

VAR
  transforms: ARRAY [0..2] OF IntProc;
  head, n1, n2, n3, p: NodePtr;
  result: INTEGER;

BEGIN
  transforms[0] := Inc;
  transforms[1] := DoubleIt;
  transforms[2] := AddTen;

  NEW(n1);
  n1^.id := 0; n1^.value := 10;
  NEW(n2);
  n2^.id := 1; n2^.value := 20;
  NEW(n3);
  n3^.id := 2; n3^.value := 50;

  n1^.next := n2;
  n2^.next := n3;
  n3^.next := NIL;
  head := n1;

  p := head;
  WHILE p # NIL DO
    result := transforms[p^.id](p^.value);
    WriteInt(result, 0);
    WriteLn;
    p := p^.next
  END;

  DISPOSE(n3);
  DISPOSE(n2);
  DISPOSE(n1)
END PointerChain.
