MODULE FuncDeref;
FROM InOut IMPORT WriteInt, WriteString, WriteLn;
FROM Storage IMPORT ALLOCATE;

TYPE
  IntPtr = POINTER TO INTEGER;
  PtrPtr = POINTER TO IntPtr;

VAR
  p: IntPtr;
  pp: PtrPtr;
  arr: ARRAY [0..2] OF IntPtr;
  val: INTEGER;
  i: CARDINAL;

PROCEDURE MakeInt(v: INTEGER): IntPtr;
VAR q: IntPtr;
BEGIN
  ALLOCATE(q, 4);
  q^ := v;
  RETURN q
END MakeInt;

PROCEDURE GetPtr(VAR x: IntPtr): PtrPtr;
VAR q: PtrPtr;
BEGIN
  ALLOCATE(q, 8);
  q^ := x;
  RETURN q
END GetPtr;

PROCEDURE Elem(VAR a: ARRAY OF IntPtr; idx: CARDINAL): IntPtr;
BEGIN
  RETURN a[idx]
END Elem;

BEGIN
  (* Basic: dereference function result *)
  val := MakeInt(10)^;
  WriteString("basic: "); WriteInt(val, 0); WriteLn;

  (* Assign through dereferenced function result *)
  p := MakeInt(0);
  MakeInt(0);  (* ensure no aliasing *)
  p^ := MakeInt(20)^;
  WriteString("assign: "); WriteInt(p^, 0); WriteLn;

  (* Double dereference: Func()^^ via pointer-to-pointer *)
  p := MakeInt(30);
  pp := GetPtr(p);
  val := pp^^;
  WriteString("double: "); WriteInt(val, 0); WriteLn;

  (* Dereference in expression context *)
  val := MakeInt(17)^ + MakeInt(25)^;
  WriteString("expr: "); WriteInt(val, 0); WriteLn;

  (* Array element dereference via function *)
  FOR i := 0 TO 2 DO
    arr[i] := MakeInt((i + 1) * 100)
  END;
  val := Elem(arr, 0)^;
  WriteString("elem0: "); WriteInt(val, 0); WriteLn;
  val := Elem(arr, 2)^;
  WriteString("elem2: "); WriteInt(val, 0); WriteLn
END FuncDeref.
