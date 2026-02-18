MODULE DList;
(* Doubly-linked list with forward and backward traversal *)
FROM InOut IMPORT WriteString, WriteInt, WriteLn;
FROM Storage IMPORT ALLOCATE, DEALLOCATE;

TYPE
  NodePtr = POINTER TO Node;
  Node = RECORD
    val: INTEGER;
    next, prev: NodePtr
  END;

  List = RECORD
    head, tail: NodePtr;
    count: INTEGER
  END;

VAR
  L: List;
  p: NodePtr;
  i: INTEGER;

PROCEDURE Init(VAR L: List);
BEGIN
  L.head := NIL;
  L.tail := NIL;
  L.count := 0
END Init;

PROCEDURE Append(VAR L: List; v: INTEGER);
  VAR n: NodePtr;
BEGIN
  NEW(n);
  n^.val := v;
  n^.next := NIL;
  n^.prev := L.tail;
  IF L.tail # NIL THEN
    L.tail^.next := n
  ELSE
    L.head := n
  END;
  L.tail := n;
  INC(L.count)
END Append;

PROCEDURE Prepend(VAR L: List; v: INTEGER);
  VAR n: NodePtr;
BEGIN
  NEW(n);
  n^.val := v;
  n^.prev := NIL;
  n^.next := L.head;
  IF L.head # NIL THEN
    L.head^.prev := n
  ELSE
    L.tail := n
  END;
  L.head := n;
  INC(L.count)
END Prepend;

PROCEDURE PrintForward(L: List);
  VAR p: NodePtr;
BEGIN
  p := L.head;
  WHILE p # NIL DO
    WriteInt(p^.val, 4);
    p := p^.next
  END;
  WriteLn
END PrintForward;

PROCEDURE PrintBackward(L: List);
  VAR p: NodePtr;
BEGIN
  p := L.tail;
  WHILE p # NIL DO
    WriteInt(p^.val, 4);
    p := p^.prev
  END;
  WriteLn
END PrintBackward;

PROCEDURE RemoveFirst(VAR L: List);
  VAR old: NodePtr;
BEGIN
  IF L.head # NIL THEN
    old := L.head;
    L.head := L.head^.next;
    IF L.head # NIL THEN
      L.head^.prev := NIL
    ELSE
      L.tail := NIL
    END;
    DISPOSE(old);
    DEC(L.count)
  END
END RemoveFirst;

PROCEDURE RemoveLast(VAR L: List);
  VAR old: NodePtr;
BEGIN
  IF L.tail # NIL THEN
    old := L.tail;
    L.tail := L.tail^.prev;
    IF L.tail # NIL THEN
      L.tail^.next := NIL
    ELSE
      L.head := NIL
    END;
    DISPOSE(old);
    DEC(L.count)
  END
END RemoveLast;

BEGIN
  Init(L);

  (* Build list: 1 2 3 4 5 *)
  FOR i := 1 TO 5 DO
    Append(L, i)
  END;

  WriteString("Forward: ");
  PrintForward(L);
  WriteString("Backward: ");
  PrintBackward(L);
  WriteString("Count = "); WriteInt(L.count, 1); WriteLn;

  (* Prepend 0 and -1 *)
  Prepend(L, 0);
  Prepend(L, -1);
  WriteString("After prepend: ");
  PrintForward(L);

  (* Remove from both ends *)
  RemoveFirst(L);
  RemoveLast(L);
  WriteString("After remove ends: ");
  PrintForward(L);
  WriteString("Count = "); WriteInt(L.count, 1); WriteLn;

  WriteString("Done"); WriteLn
END DList.
