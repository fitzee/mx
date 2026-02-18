MODULE PtrChains;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

TYPE
  Inner = RECORD
    val: INTEGER
  END;
  InnerPtr = POINTER TO Inner;
  Outer = RECORD
    name: INTEGER;
    child: InnerPtr
  END;
  OuterPtr = POINTER TO Outer;
  Node = RECORD
    data: INTEGER;
    next: POINTER TO Node
  END;
  NodePtr = POINTER TO Node;

VAR
  op: OuterPtr;
  ip: InnerPtr;
  head, p: NodePtr;
  i: INTEGER;

BEGIN
  (* Test simple pointer dereference and field access *)
  NEW(ip);
  ip^.val := 42;
  WriteString("Inner val: "); WriteInt(ip^.val, 1); WriteLn;

  (* Test nested pointer chain: outer->child->val *)
  NEW(op);
  op^.name := 100;
  op^.child := ip;
  WriteString("Outer name: "); WriteInt(op^.name, 1); WriteLn;
  WriteString("Outer->child->val: "); WriteInt(op^.child^.val, 1); WriteLn;

  (* Test linked list traversal with pointer chains *)
  head := NIL;
  FOR i := 5 TO 1 BY -1 DO
    NEW(p);
    p^.data := i;
    p^.next := head;
    head := p
  END;

  WriteString("List: ");
  p := head;
  WHILE p # NIL DO
    WriteInt(p^.data, 3);
    p := p^.next
  END;
  WriteLn;

  (* Modify through pointer chain *)
  head^.next^.data := 99;
  WriteString("Modified 2nd: ");
  p := head;
  WHILE p # NIL DO
    WriteInt(p^.data, 3);
    p := p^.next
  END;
  WriteLn;

  DISPOSE(ip);
  DISPOSE(op)
END PtrChains.
