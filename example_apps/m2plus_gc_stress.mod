MODULE M2PlusGCStress;
(* GC stress test: allocate many REF objects, let them go out of scope *)
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

TYPE
  Node = RECORD
    value: INTEGER;
    next: POINTER TO Node;
  END;
  NodePtr = POINTER TO Node;

  IntRef = REF INTEGER;
  Point = RECORD
    x, y: INTEGER;
  END;
  PointRef = REF Point;

  (* Bigger struct to stress GC more *)
  BigRec = RECORD
    data: ARRAY [0..63] OF INTEGER;
    label: ARRAY [0..31] OF CHAR;
  END;
  BigRef = REF BigRec;

VAR
  i, j, sum: INTEGER;
  ir: IntRef;
  pr: PointRef;
  br: BigRef;
  head, tmp: NodePtr;

(* Build a linked list of n nodes, return the head *)
PROCEDURE BuildList(n: INTEGER): NodePtr;
VAR h, node: NodePtr;
    k: INTEGER;
BEGIN
  h := NIL;
  FOR k := 1 TO n DO
    NEW(node);
    node^.value := k;
    node^.next := h;
    h := node
  END;
  RETURN h
END BuildList;

(* Sum all values in a linked list *)
PROCEDURE SumList(h: NodePtr): INTEGER;
VAR s: INTEGER;
    p: NodePtr;
BEGIN
  s := 0;
  p := h;
  WHILE p # NIL DO
    s := s + p^.value;
    p := p^.next
  END;
  RETURN s
END SumList;

BEGIN
  WriteString("=== M2+ GC Stress Test ==="); WriteLn;

  (* Test 1: Rapid REF INTEGER allocation *)
  WriteString("Test 1: 10000 REF INTEGER allocations"); WriteLn;
  sum := 0;
  FOR i := 1 TO 10000 DO
    NEW(ir);
    ir^ := i;
    sum := sum + ir^
  END;
  WriteString("  Sum = "); WriteInt(sum, 1); WriteLn;
  IF sum = 50005000 THEN
    WriteString("  PASS"); WriteLn
  ELSE
    WriteString("  FAIL"); WriteLn
  END;

  (* Test 2: REF Point allocation *)
  WriteString("Test 2: 5000 REF Point allocations"); WriteLn;
  sum := 0;
  FOR i := 1 TO 5000 DO
    NEW(pr);
    pr^.x := i;
    pr^.y := i * 2;
    sum := sum + pr^.x + pr^.y
  END;
  WriteString("  Sum = "); WriteInt(sum, 1); WriteLn;
  IF sum = 37507500 THEN
    WriteString("  PASS"); WriteLn
  ELSE
    WriteString("  FAIL"); WriteLn
  END;

  (* Test 3: Large struct allocation *)
  WriteString("Test 3: 2000 BigRef allocations"); WriteLn;
  sum := 0;
  FOR i := 1 TO 2000 DO
    NEW(br);
    br^.data[0] := i;
    br^.data[63] := i * 3;
    sum := sum + br^.data[0] + br^.data[63]
  END;
  WriteString("  Sum = "); WriteInt(sum, 1); WriteLn;
  IF sum = 8004000 THEN
    WriteString("  PASS"); WriteLn
  ELSE
    WriteString("  FAIL"); WriteLn
  END;

  (* Test 4: Linked list with GC — build and discard repeatedly *)
  WriteString("Test 4: Build/discard linked lists (GC pressure)"); WriteLn;
  sum := 0;
  FOR i := 1 TO 100 DO
    head := BuildList(100);
    sum := sum + SumList(head)
    (* head goes out of scope on each iteration — GC should reclaim *)
  END;
  WriteString("  Sum = "); WriteInt(sum, 1); WriteLn;
  IF sum = 505000 THEN
    WriteString("  PASS"); WriteLn
  ELSE
    WriteString("  FAIL"); WriteLn
  END;

  (* Test 5: Long linked list *)
  WriteString("Test 5: Single 10000-node linked list"); WriteLn;
  head := BuildList(10000);
  sum := SumList(head);
  WriteString("  Sum = "); WriteInt(sum, 1); WriteLn;
  IF sum = 50005000 THEN
    WriteString("  PASS"); WriteLn
  ELSE
    WriteString("  FAIL"); WriteLn
  END;

  WriteString("Done"); WriteLn
END M2PlusGCStress.
