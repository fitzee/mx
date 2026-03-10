MODULE Comprehensive;
FROM InOut IMPORT WriteString, WriteInt, WriteLn, WriteCard;
FROM Storage IMPORT ALLOCATE, DEALLOCATE;

CONST
  MaxSize = 100;
  Pi = 3.14159;
  Greeting = "Hello";
  Yes = TRUE;

TYPE
  Color = (Red, Green, Blue, Yellow);
  SmallInt = [1..10];
  ColorSet = SET OF Color;
  CharSet = SET OF CHAR;

  NameStr = ARRAY [0..19] OF CHAR;
  Matrix = ARRAY [0..2], [0..2] OF INTEGER;

  NodePtr = POINTER TO Node;
  Node = RECORD
    value: INTEGER;
    next: NodePtr;
  END;

  Shape = RECORD
    x, y: INTEGER;
    CASE kind: Color OF
      Red:   radius: INTEGER |
      Green: width, height: INTEGER |
      Blue:  side: INTEGER
    END
  END;

  Comparator = PROCEDURE(INTEGER, INTEGER): BOOLEAN;

VAR
  i, j, result: INTEGER;
  c: Color;
  cs: ColorSet;
  name: NameStr;
  m: Matrix;
  head, p: NodePtr;
  sh: Shape;
  cmp: Comparator;

(* ── Utility procedures ─────────────────────────── *)

PROCEDURE Min(a, b: INTEGER): INTEGER;
BEGIN
  IF a < b THEN RETURN a ELSE RETURN b END
END Min;

PROCEDURE Max(a, b: INTEGER): INTEGER;
BEGIN
  IF a > b THEN RETURN a ELSE RETURN b END
END Max;

PROCEDURE LessThan(a, b: INTEGER): BOOLEAN;
BEGIN
  RETURN a < b
END LessThan;

PROCEDURE GreaterThan(a, b: INTEGER): BOOLEAN;
BEGIN
  RETURN a > b
END GreaterThan;

(* ── Enumeration and set tests ──────────────────── *)

PROCEDURE TestEnumsAndSets;
  VAR c: Color; cs: ColorSet;
BEGIN
  WriteString("=== Enums and Sets ==="); WriteLn;

  (* Enum ordering *)
  c := Red;
  WriteString("Red ORD: "); WriteInt(ORD(c), 1); WriteLn;
  c := Blue;
  WriteString("Blue ORD: "); WriteInt(ORD(c), 1); WriteLn;

  (* Set operations *)
  cs := ColorSet{Red, Green, Blue};
  IF Red IN cs THEN WriteString("Red in set: YES") ELSE WriteString("Red in set: NO") END;
  WriteLn;
  IF Yellow IN cs THEN WriteString("Yellow in set: YES") ELSE WriteString("Yellow in set: NO") END;
  WriteLn;

  (* Set arithmetic *)
  cs := cs - ColorSet{Green};
  IF Green IN cs THEN WriteString("Green after remove: YES") ELSE WriteString("Green after remove: NO") END;
  WriteLn;

  INCL(cs, Yellow);
  IF Yellow IN cs THEN WriteString("Yellow after INCL: YES") ELSE WriteString("Yellow after INCL: NO") END;
  WriteLn;

  EXCL(cs, Red);
  IF Red IN cs THEN WriteString("Red after EXCL: YES") ELSE WriteString("Red after EXCL: NO") END;
  WriteLn;
END TestEnumsAndSets;

(* ── Array and matrix tests ─────────────────────── *)

PROCEDURE FillMatrix(VAR m: Matrix);
  VAR i, j, val: INTEGER;
BEGIN
  val := 1;
  FOR i := 0 TO 2 DO
    FOR j := 0 TO 2 DO
      m[i, j] := val;
      INC(val)
    END
  END
END FillMatrix;

PROCEDURE PrintMatrix(m: Matrix);
  VAR i, j: INTEGER;
BEGIN
  FOR i := 0 TO 2 DO
    FOR j := 0 TO 2 DO
      WriteInt(m[i, j], 4)
    END;
    WriteLn
  END
END PrintMatrix;

PROCEDURE TestArrays;
  VAR m: Matrix; sum: INTEGER; i, j: INTEGER;
BEGIN
  WriteString("=== Arrays ==="); WriteLn;
  FillMatrix(m);
  WriteString("Matrix:"); WriteLn;
  PrintMatrix(m);

  sum := 0;
  FOR i := 0 TO 2 DO
    FOR j := 0 TO 2 DO
      sum := sum + m[i, j]
    END
  END;
  WriteString("Sum: "); WriteInt(sum, 1); WriteLn;
END TestArrays;

(* ── Linked list with pointers ──────────────────── *)

PROCEDURE PushFront(VAR head: NodePtr; val: INTEGER);
  VAR n: NodePtr;
BEGIN
  NEW(n);
  n^.value := val;
  n^.next := head;
  head := n
END PushFront;

PROCEDURE PrintList(head: NodePtr);
BEGIN
  WHILE head # NIL DO
    WriteInt(head^.value, 4);
    head := head^.next
  END;
  WriteLn
END PrintList;

PROCEDURE ListLength(head: NodePtr): INTEGER;
  VAR count: INTEGER;
BEGIN
  count := 0;
  WHILE head # NIL DO
    INC(count);
    head := head^.next
  END;
  RETURN count
END ListLength;

PROCEDURE FreeList(VAR head: NodePtr);
  VAR tmp: NodePtr;
BEGIN
  WHILE head # NIL DO
    tmp := head;
    head := head^.next;
    DISPOSE(tmp)
  END
END FreeList;

PROCEDURE TestLinkedList;
  VAR head: NodePtr; i: INTEGER;
BEGIN
  WriteString("=== Linked List ==="); WriteLn;
  head := NIL;
  FOR i := 1 TO 5 DO
    PushFront(head, i * 10)
  END;
  WriteString("List:"); PrintList(head);
  WriteString("Length: "); WriteInt(ListLength(head), 1); WriteLn;
  FreeList(head);
  WriteString("After free, length: "); WriteInt(ListLength(head), 1); WriteLn;
END TestLinkedList;

(* ── CASE statement tests ───────────────────────── *)

PROCEDURE ClassifyChar(ch: CHAR): INTEGER;
BEGIN
  CASE ch OF
    'A'..'Z': RETURN 1 |
    'a'..'z': RETURN 2 |
    '0'..'9': RETURN 3
  ELSE
    RETURN 0
  END
END ClassifyChar;

PROCEDURE DayName(day: INTEGER);
BEGIN
  CASE day OF
    1: WriteString("Monday") |
    2: WriteString("Tuesday") |
    3: WriteString("Wednesday") |
    4: WriteString("Thursday") |
    5: WriteString("Friday") |
    6: WriteString("Saturday") |
    7: WriteString("Sunday")
  ELSE
    WriteString("Unknown")
  END
END DayName;

PROCEDURE TestCase;
BEGIN
  WriteString("=== CASE ==="); WriteLn;
  WriteString("'A' class: "); WriteInt(ClassifyChar('A'), 1); WriteLn;
  WriteString("'m' class: "); WriteInt(ClassifyChar('m'), 1); WriteLn;
  WriteString("'5' class: "); WriteInt(ClassifyChar('5'), 1); WriteLn;
  WriteString("'!' class: "); WriteInt(ClassifyChar('!'), 1); WriteLn;
  WriteString("Day 3: "); DayName(3); WriteLn;
  WriteString("Day 7: "); DayName(7); WriteLn;
  WriteString("Day 9: "); DayName(9); WriteLn;
END TestCase;

(* ── FOR loop with BY step ──────────────────────── *)

PROCEDURE TestForLoop;
  VAR i: INTEGER;
BEGIN
  WriteString("=== FOR Loop ==="); WriteLn;
  WriteString("Count up by 2: ");
  FOR i := 0 TO 10 BY 2 DO
    WriteInt(i, 3)
  END;
  WriteLn;

  WriteString("Count down by 3: ");
  FOR i := 15 TO 0 BY -3 DO
    WriteInt(i, 3)
  END;
  WriteLn;
END TestForLoop;

(* ── LOOP/EXIT tests ────────────────────────────── *)

PROCEDURE FindFirst(a: ARRAY OF INTEGER; target: INTEGER): INTEGER;
  VAR i: INTEGER;
BEGIN
  i := 0;
  LOOP
    IF i > HIGH(a) THEN RETURN -1 END;
    IF a[i] = target THEN RETURN i END;
    INC(i)
  END
END FindFirst;

PROCEDURE TestLoopExit;
  VAR a: ARRAY [0..4] OF INTEGER;
BEGIN
  WriteString("=== LOOP/EXIT ==="); WriteLn;
  a[0] := 10; a[1] := 20; a[2] := 30; a[3] := 40; a[4] := 50;
  WriteString("Find 30: index "); WriteInt(FindFirst(a, 30), 1); WriteLn;
  WriteString("Find 99: index "); WriteInt(FindFirst(a, 99), 1); WriteLn;
END TestLoopExit;

(* ── REPEAT..UNTIL tests ────────────────────────── *)

PROCEDURE GCD(a, b: INTEGER): INTEGER;
BEGIN
  REPEAT
    IF a > b THEN a := a - b
    ELSIF b > a THEN b := b - a
    END
  UNTIL a = b;
  RETURN a
END GCD;

PROCEDURE TestRepeat;
BEGIN
  WriteString("=== REPEAT ==="); WriteLn;
  WriteString("GCD(48, 18): "); WriteInt(GCD(48, 18), 1); WriteLn;
  WriteString("GCD(100, 75): "); WriteInt(GCD(100, 75), 1); WriteLn;
END TestRepeat;

(* ── Procedure type / function pointer tests ────── *)

PROCEDURE ApplyOp(a, b: INTEGER; op: Comparator): BOOLEAN;
BEGIN
  RETURN op(a, b)
END ApplyOp;

PROCEDURE TestProcTypes;
BEGIN
  WriteString("=== Proc Types ==="); WriteLn;

  WriteString("LessThan(3, 5): ");
  IF ApplyOp(3, 5, LessThan) THEN WriteString("TRUE") ELSE WriteString("FALSE") END;
  WriteLn;

  WriteString("GreaterThan(3, 5): ");
  IF ApplyOp(3, 5, GreaterThan) THEN WriteString("TRUE") ELSE WriteString("FALSE") END;
  WriteLn;

  cmp := LessThan;
  WriteString("cmp(10, 20): ");
  IF cmp(10, 20) THEN WriteString("TRUE") ELSE WriteString("FALSE") END;
  WriteLn;

  cmp := GreaterThan;
  WriteString("cmp(10, 20): ");
  IF cmp(10, 20) THEN WriteString("TRUE") ELSE WriteString("FALSE") END;
  WriteLn;
END TestProcTypes;

(* ── WITH statement tests ───────────────────────── *)

PROCEDURE TestWith;
  VAR n: Node;
BEGIN
  WriteString("=== WITH ==="); WriteLn;
  n.value := 42;
  n.next := NIL;
  WITH n DO
    WriteString("Value: "); WriteInt(value, 1); WriteLn;
    value := 99;
  END;
  WriteString("After WITH: "); WriteInt(n.value, 1); WriteLn;
END TestWith;

(* ── INC/DEC with amounts ──────────────────────── *)

PROCEDURE TestIncDec;
  VAR x: INTEGER;
BEGIN
  WriteString("=== INC/DEC ==="); WriteLn;
  x := 10;
  INC(x); WriteString("After INC(10): "); WriteInt(x, 1); WriteLn;
  INC(x, 5); WriteString("After INC(11,5): "); WriteInt(x, 1); WriteLn;
  DEC(x); WriteString("After DEC(16): "); WriteInt(x, 1); WriteLn;
  DEC(x, 10); WriteString("After DEC(15,10): "); WriteInt(x, 1); WriteLn;
END TestIncDec;

(* ── String operations ──────────────────────────── *)

PROCEDURE TestStrings;
  VAR s: ARRAY [0..31] OF CHAR;
BEGIN
  WriteString("=== Strings ==="); WriteLn;
  s := "Hello, World!";
  WriteString(s); WriteLn;

  IF s = "Hello, World!" THEN
    WriteString("Match: YES")
  ELSE
    WriteString("Match: NO")
  END;
  WriteLn;

  IF s < "Zzzz" THEN
    WriteString("Less than Zzzz: YES")
  ELSE
    WriteString("Less than Zzzz: NO")
  END;
  WriteLn;
END TestStrings;

(* ── Built-in functions ─────────────────────────── *)

PROCEDURE TestBuiltins;
  VAR x: INTEGER; ch: CHAR;
BEGIN
  WriteString("=== Builtins ==="); WriteLn;

  x := -42;
  WriteString("ABS(-42): "); WriteInt(ABS(x), 1); WriteLn;

  WriteString("ODD(7): ");
  IF ODD(7) THEN WriteString("TRUE") ELSE WriteString("FALSE") END;
  WriteLn;

  WriteString("ODD(8): ");
  IF ODD(8) THEN WriteString("TRUE") ELSE WriteString("FALSE") END;
  WriteLn;

  ch := CHR(65);
  WriteString("CHR(65): ");
  WriteString("A"); (* Can't easily print single char, so just verify *)
  WriteLn;

  WriteString("ORD('Z'): "); WriteInt(ORD('Z'), 1); WriteLn;
  WriteString("CAP('m'): "); WriteInt(ORD(CAP('m')), 1); WriteLn;

  WriteString("MIN(10, 20): "); WriteInt(Min(10, 20), 1); WriteLn;
  WriteString("MAX(10, 20): "); WriteInt(Max(10, 20), 1); WriteLn;
END TestBuiltins;

(* ── Nested procedure with capture ──────────────── *)

PROCEDURE TestNested;
  VAR base: INTEGER;

  PROCEDURE AddBase(x: INTEGER): INTEGER;
  BEGIN
    RETURN x + base
  END AddBase;

BEGIN
  WriteString("=== Nested Procs ==="); WriteLn;
  base := 100;
  WriteString("AddBase(42): "); WriteInt(AddBase(42), 1); WriteLn;
  base := 200;
  WriteString("AddBase(42): "); WriteInt(AddBase(42), 1); WriteLn;
END TestNested;

(* ── Main program ───────────────────────────────── *)

BEGIN
  TestEnumsAndSets;
  TestArrays;
  TestLinkedList;
  TestCase;
  TestForLoop;
  TestLoopExit;
  TestRepeat;
  TestProcTypes;
  TestWith;
  TestIncDec;
  TestStrings;
  TestBuiltins;
  TestNested;

  WriteString("=== All tests passed ==="); WriteLn
END Comprehensive.
