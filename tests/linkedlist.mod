MODULE LinkedList;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

TYPE
  NodePtr = POINTER TO Node;
  Node = RECORD
    value: INTEGER;
    next: NodePtr
  END;

VAR
  head, current, newNode: NodePtr;
  i: INTEGER;

PROCEDURE PrintList(n: NodePtr);
BEGIN
  WHILE n # NIL DO
    WriteInt(n^.value, 4);
    n := n^.next
  END;
  WriteLn
END PrintList;

BEGIN
  head := NIL;
  FOR i := 5 TO 1 BY -1 DO
    NEW(newNode);
    newNode^.value := i;
    newNode^.next := head;
    head := newNode
  END;
  WriteString("List: ");
  PrintList(head)
END LinkedList.
