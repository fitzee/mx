MODULE BinTree;
(* Binary search tree with insert, search, and in-order traversal *)
FROM InOut IMPORT WriteString, WriteInt, WriteLn;
FROM Storage IMPORT ALLOCATE, DEALLOCATE;

TYPE
  NodePtr = POINTER TO Node;
  Node = RECORD
    val: INTEGER;
    left, right: NodePtr
  END;

VAR
  root: NodePtr;
  i: INTEGER;

PROCEDURE Insert(VAR t: NodePtr; v: INTEGER);
BEGIN
  IF t = NIL THEN
    NEW(t);
    t^.val := v;
    t^.left := NIL;
    t^.right := NIL
  ELSIF v < t^.val THEN
    Insert(t^.left, v)
  ELSIF v > t^.val THEN
    Insert(t^.right, v)
  END
  (* equal values ignored *)
END Insert;

PROCEDURE Search(t: NodePtr; v: INTEGER): BOOLEAN;
BEGIN
  IF t = NIL THEN
    RETURN FALSE
  ELSIF v = t^.val THEN
    RETURN TRUE
  ELSIF v < t^.val THEN
    RETURN Search(t^.left, v)
  ELSE
    RETURN Search(t^.right, v)
  END
END Search;

PROCEDURE PrintInOrder(t: NodePtr);
BEGIN
  IF t # NIL THEN
    PrintInOrder(t^.left);
    WriteInt(t^.val, 4);
    PrintInOrder(t^.right)
  END
END PrintInOrder;

PROCEDURE Size(t: NodePtr): INTEGER;
BEGIN
  IF t = NIL THEN
    RETURN 0
  ELSE
    RETURN 1 + Size(t^.left) + Size(t^.right)
  END
END Size;

PROCEDURE FreeTree(VAR t: NodePtr);
BEGIN
  IF t # NIL THEN
    FreeTree(t^.left);
    FreeTree(t^.right);
    DISPOSE(t);
    t := NIL
  END
END FreeTree;

BEGIN
  root := NIL;

  (* Insert values *)
  Insert(root, 50);
  Insert(root, 30);
  Insert(root, 70);
  Insert(root, 20);
  Insert(root, 40);
  Insert(root, 60);
  Insert(root, 80);
  Insert(root, 10);
  Insert(root, 35);
  Insert(root, 45);

  (* In-order traversal should produce sorted output *)
  WriteString("In-order: ");
  PrintInOrder(root);
  WriteLn;

  (* Size *)
  WriteString("Size = "); WriteInt(Size(root), 1); WriteLn;

  (* Search *)
  IF Search(root, 35) THEN WriteString("Found 35: YES") ELSE WriteString("Found 35: NO") END; WriteLn;
  IF Search(root, 42) THEN WriteString("Found 42: YES") ELSE WriteString("Found 42: NO") END; WriteLn;
  IF Search(root, 80) THEN WriteString("Found 80: YES") ELSE WriteString("Found 80: NO") END; WriteLn;
  IF Search(root, 99) THEN WriteString("Found 99: YES") ELSE WriteString("Found 99: NO") END; WriteLn;

  (* Duplicate insert should not change size *)
  Insert(root, 50);
  Insert(root, 30);
  WriteString("Size after dup inserts = "); WriteInt(Size(root), 1); WriteLn;

  FreeTree(root);
  WriteString("Done"); WriteLn
END BinTree.
