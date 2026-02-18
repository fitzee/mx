MODULE Advanced;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;
FROM Storage IMPORT ALLOCATE, DEALLOCATE;

TYPE
  (* Binary search tree *)
  TreePtr = POINTER TO TreeNode;
  TreeNode = RECORD
    value: INTEGER;
    left, right: TreePtr
  END;

VAR
  root: TreePtr;
  i: INTEGER;

PROCEDURE Insert(VAR t: TreePtr; val: INTEGER);
BEGIN
  IF t = NIL THEN
    NEW(t);
    t^.value := val;
    t^.left := NIL;
    t^.right := NIL
  ELSIF val < t^.value THEN
    Insert(t^.left, val)
  ELSIF val > t^.value THEN
    Insert(t^.right, val)
  END
  (* Duplicate values are ignored *)
END Insert;

PROCEDURE InOrder(t: TreePtr);
BEGIN
  IF t # NIL THEN
    InOrder(t^.left);
    WriteInt(t^.value, 4);
    InOrder(t^.right)
  END
END InOrder;

PROCEDURE Count(t: TreePtr): INTEGER;
BEGIN
  IF t = NIL THEN
    RETURN 0
  ELSE
    RETURN 1 + Count(t^.left) + Count(t^.right)
  END
END Count;

PROCEDURE Search(t: TreePtr; val: INTEGER): BOOLEAN;
BEGIN
  IF t = NIL THEN
    RETURN FALSE
  ELSIF val = t^.value THEN
    RETURN TRUE
  ELSIF val < t^.value THEN
    RETURN Search(t^.left, val)
  ELSE
    RETURN Search(t^.right, val)
  END
END Search;

PROCEDURE FreeTree(VAR t: TreePtr);
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

  (* Insert values in non-sorted order *)
  Insert(root, 50);
  Insert(root, 30);
  Insert(root, 70);
  Insert(root, 20);
  Insert(root, 40);
  Insert(root, 60);
  Insert(root, 80);

  WriteString("In-order:");
  InOrder(root);
  WriteLn;

  WriteString("Count: ");
  WriteInt(Count(root), 1);
  WriteLn;

  WriteString("Search(40): ");
  IF Search(root, 40) THEN WriteString("TRUE") ELSE WriteString("FALSE") END;
  WriteLn;

  WriteString("Search(45): ");
  IF Search(root, 45) THEN WriteString("TRUE") ELSE WriteString("FALSE") END;
  WriteLn;

  FreeTree(root);
  WriteString("After free, root = NIL: ");
  IF root = NIL THEN WriteString("TRUE") ELSE WriteString("FALSE") END;
  WriteLn
END Advanced.
