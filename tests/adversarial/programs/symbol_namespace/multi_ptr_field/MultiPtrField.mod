MODULE MultiPtrField;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;
FROM Storage IMPORT ALLOCATE, DEALLOCATE;

TYPE
  Node = RECORD
    value: INTEGER;
  END;
  NodePtr = POINTER TO Node;
  Pair = RECORD
    left, right: NodePtr;
  END;

VAR
  p: Pair;

BEGIN
  NEW(p.left);
  NEW(p.right);
  p.left^.value := 1;
  p.right^.value := 2;
  WriteInt(p.left^.value, 0);
  WriteLn;
  WriteInt(p.right^.value, 0);
  WriteLn;
  DISPOSE(p.left);
  DISPOSE(p.right);
END MultiPtrField.
