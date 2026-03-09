MODULE PtrToRecord;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;
FROM Storage IMPORT ALLOCATE, DEALLOCATE;

TYPE
  NodePtr = POINTER TO RECORD
    value: INTEGER;
    next: NodePtr;
  END;

VAR
  p, q: NodePtr;

BEGIN
  NEW(p);
  p^.value := 10;
  p^.next := NIL;
  NEW(q);
  q^.value := 20;
  q^.next := p;
  WriteInt(q^.next^.value, 0);
  WriteLn;
  WriteInt(q^.value, 0);
  WriteLn;
  DISPOSE(q);
  DISPOSE(p);
END PtrToRecord.
