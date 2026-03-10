IMPLEMENTATION MODULE Stack;
FROM Storage IMPORT ALLOCATE, DEALLOCATE;

TYPE
  NodePtr = POINTER TO Node;
  Node = RECORD
    value: INTEGER;
    next: NodePtr;
  END;
  Stack = NodePtr;  (* reveal opaque type *)

PROCEDURE Create(VAR s: Stack);
BEGIN
  s := NIL
END Create;

PROCEDURE Push(VAR s: Stack; val: INTEGER);
  VAR n: NodePtr;
BEGIN
  NEW(n);
  n^.value := val;
  n^.next := s;
  s := n
END Push;

PROCEDURE Pop(VAR s: Stack; VAR val: INTEGER);
  VAR tmp: NodePtr;
BEGIN
  IF s # NIL THEN
    val := s^.value;
    tmp := s;
    s := s^.next;
    DISPOSE(tmp)
  END
END Pop;

PROCEDURE IsEmpty(s: Stack): BOOLEAN;
BEGIN
  RETURN s = NIL
END IsEmpty;

PROCEDURE Destroy(VAR s: Stack);
  VAR tmp: NodePtr;
BEGIN
  WHILE s # NIL DO
    tmp := s;
    s := s^.next;
    DISPOSE(tmp)
  END
END Destroy;

END Stack.
