# mx Modula-2 Quick Reference

Full syntax rules are defined in **docs/lang/grammar.md**. Read that file. This is a compact subset for fast lookup.

## Module Skeletons

### Program Module

```modula-2
MODULE Hello;
FROM InOut IMPORT WriteString, WriteLn;
BEGIN
  WriteString("Hello, world!");
  WriteLn
END Hello.
```

### Definition Module (.def)

```modula-2
DEFINITION MODULE Stack;

CONST MaxSize = 256;

TYPE
  T = RECORD
    data: ARRAY [0..255] OF INTEGER;
    top: INTEGER;
  END;

PROCEDURE Init(VAR s: T);
PROCEDURE Push(VAR s: T; val: INTEGER);
PROCEDURE Pop(VAR s: T): INTEGER;
PROCEDURE IsEmpty(VAR s: T): BOOLEAN;

END Stack.
```

### Implementation Module (.mod)

```modula-2
IMPLEMENTATION MODULE Stack;

PROCEDURE Init(VAR s: T);
BEGIN
  s.top := 0
END Init;

PROCEDURE Push(VAR s: T; val: INTEGER);
BEGIN
  s.data[s.top] := val;
  INC(s.top)
END Push;

PROCEDURE Pop(VAR s: T): INTEGER;
VAR val: INTEGER;
BEGIN
  DEC(s.top);
  val := s.data[s.top];
  RETURN val
END Pop;

PROCEDURE IsEmpty(VAR s: T): BOOLEAN;
BEGIN
  RETURN s.top = 0
END IsEmpty;

END Stack.
```

## Import Syntax

```modula-2
FROM InOut IMPORT WriteString, WriteInt, WriteLn;
FROM SYSTEM IMPORT ADDRESS, ADR;
FROM ByteBuf IMPORT Buf, Init, Free, AppendByte;
IMPORT FileSystem;   (* qualified: FileSystem.Lookup, FileSystem.Close *)
```

## Procedure Declaration

```modula-2
PROCEDURE Name(a: INTEGER; VAR b: ARRAY OF CHAR): BOOLEAN;
VAR local: INTEGER;
BEGIN
  (* body *)
  RETURN TRUE
END Name;
```

## Types

```
INTEGER      signed 32-bit
CARDINAL     unsigned 32-bit
LONGINT      signed 64-bit
LONGCARD     unsigned 64-bit
REAL         32-bit float
LONGREAL     64-bit float
BOOLEAN      TRUE / FALSE
CHAR         single byte
ADDRESS      void pointer (FROM SYSTEM)
BITSET       set of 0..31
```

## Variable Declaration

```modula-2
VAR
  i, j: INTEGER;
  name: ARRAY [0..63] OF CHAR;
  buf: ARRAY [0..1023] OF CHAR;
  ok: BOOLEAN;
  ptr: ADDRESS;
```

## Control Flow

```modula-2
(* IF *)
IF x > 0 THEN
  y := 1
ELSIF x = 0 THEN
  y := 0
ELSE
  y := -1
END;

(* WHILE *)
WHILE i < n DO
  INC(i)
END;

(* FOR *)
FOR i := 0 TO n - 1 DO
  Process(i)
END;

(* REPEAT *)
REPEAT
  Read(ch)
UNTIL ch = 0C;

(* LOOP with EXIT *)
LOOP
  Read(ch);
  IF ch = 0C THEN EXIT END;
  Process(ch)
END;

(* CASE *)
CASE ch OF
  'a'..'z': HandleLower |
  'A'..'Z': HandleUpper |
  '0'..'9': HandleDigit
ELSE
  HandleOther
END;
```

## Operators

```
:=           assignment
=  #         equal, not-equal
<  <=  >  >= comparison
+  -  *      arithmetic
/            real division
DIV  MOD     integer division, modulo
AND  OR  NOT boolean
IN           set membership
^            pointer dereference
```

## String Handling

```modula-2
VAR s: ARRAY [0..63] OF CHAR;
    len: INTEGER;

(* Strings module *)
FROM Strings IMPORT Assign, Length, Concat, Pos, Copy;

Assign("hello", s);
len := Length(s);
Concat(s, " world", s);

(* Character-level access *)
IF s[0] = 'h' THEN (* ... *) END;

(* HIGH gives last valid index of open array parameter *)
PROCEDURE PrintLen(s: ARRAY OF CHAR): CARDINAL;
BEGIN
  RETURN HIGH(s) + 1
END PrintLen;
```

## Common Builtins

```
INC(x)       increment by 1
INC(x, n)    increment by n
DEC(x)       decrement by 1
DEC(x, n)    decrement by n
HIGH(a)      last index of open array
ORD(ch)      character to ordinal
CHR(n)       ordinal to character
SIZE(T)      size of type in bytes
NEW(p)       allocate pointer
DISPOSE(p)   deallocate pointer
ABS(x)       absolute value
ODD(x)       TRUE if odd
CAP(ch)      uppercase character
HALT         terminate program
```

## Open Array Parameters

```modula-2
(* accepts any size array *)
PROCEDURE Sum(a: ARRAY OF INTEGER): INTEGER;
VAR i, total: INTEGER;
BEGIN
  total := 0;
  FOR i := 0 TO HIGH(a) DO
    total := total + a[i]
  END;
  RETURN total
END Sum;
```

## Record and Pointer

```modula-2
TYPE
  NodePtr = POINTER TO Node;
  Node = RECORD
    val: INTEGER;
    next: NodePtr;
  END;

VAR p: NodePtr;
NEW(p);
p^.val := 42;
p^.next := NIL;
```

## SET Operations

```modula-2
TYPE Color = (Red, Green, Blue, Yellow);
TYPE ColorSet = SET OF Color;

VAR s, t: ColorSet;
    b: BITSET;

s := ColorSet{Red, Blue};
t := ColorSet{Blue, Yellow};

(* set operators *)
s + t        (* union: {Red, Blue, Yellow} *)
s * t        (* intersection: {Blue} *)
s - t        (* difference: {Red} *)
s / t        (* symmetric difference: {Red, Yellow} *)

(* membership test *)
IF Red IN s THEN (* ... *) END;

(* add / remove element *)
INCL(s, Green);
EXCL(s, Red);

(* BITSET is SET OF [0..31] *)
b := BITSET{0, 3, 7};
IF 3 IN b THEN (* ... *) END;
```

## Enumeration Types

```modula-2
TYPE
  Direction = (North, South, East, West);

VAR d: Direction;

d := North;
IF d = South THEN (* ... *) END;

(* use with CASE *)
CASE d OF
  North: GoUp |
  South: GoDown |
  East:  GoRight |
  West:  GoLeft
END;

(* ORD/VAL conversions *)
i := ORD(South);        (* 1 *)
d := VAL(Direction, 2); (* East *)
```

## Subrange Types

```modula-2
TYPE
  Month = [1..12];
  Uppercase = ['A'..'Z'];
  SmallCard = [0..255];

VAR m: Month;
m := 6;   (* OK *)
(* m := 13 would be out of range *)
```

## Variant Records

```modula-2
TYPE
  ShapeKind = (Circle, Rect, Triangle);
  Shape = RECORD
    x, y: INTEGER;
    CASE kind: ShapeKind OF
      Circle:   radius: INTEGER |
      Rect:     w, h: INTEGER |
      Triangle: x2, y2, x3, y3: INTEGER
    END
  END;

VAR s: Shape;
s.kind := Circle;
s.radius := 10;
```

## WITH Statement

```modula-2
TYPE Point = RECORD x, y: INTEGER END;
VAR p: Point;

WITH p DO
  x := 10;
  y := 20
END;
```

## FOR .. BY (Step)

```modula-2
(* count by 2 *)
FOR i := 0 TO 20 BY 2 DO
  WriteInt(i, 4)
END;

(* count backwards *)
FOR i := 10 TO 0 BY -1 DO
  WriteInt(i, 4)
END;
```

## Type Transfer (Unsafe Cast)

```modula-2
FROM SYSTEM IMPORT ADDRESS, ADR;

(* Assignment between compatible pointer types *)
VAR
  addr: ADDRESS;
  p: POINTER TO INTEGER;

addr := ADR(someVar);
p := addr;   (* ADDRESS is assignment-compatible with any pointer *)
```

## Comments

```modula-2
(* This is a comment *)

(* Comments nest properly:
   (* inner comment *)
   still inside outer comment *)

(* WARNING: do not put ** inside comments -- it opens a nested comment *)
```
