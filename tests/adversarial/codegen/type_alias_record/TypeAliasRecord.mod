MODULE TypeAliasRecord;
(* Regression: TYPE Alias = RecordType must emit a C typedef,
   not a duplicate struct definition. The duplicate struct caused
   C compiler errors when passing an Alias value to a procedure
   expecting the original RecordType.

   Tests:
     1. Alias variable passed to procedure taking original type
     2. Original variable passed to procedure taking original type
     3. Field access works on alias-typed variable
     4. Multiple aliases of same base type *)

FROM InOut IMPORT WriteString, WriteInt, WriteLn;

TYPE
  Vec2 = RECORD x, y: INTEGER END;
  Point = Vec2;
  Coord = Vec2;

PROCEDURE AddVec(a, b: Vec2; VAR r: Vec2);
BEGIN
  r.x := a.x + b.x;
  r.y := a.y + b.y
END AddVec;

PROCEDURE PrintVec(label: ARRAY OF CHAR; v: Vec2);
BEGIN
  WriteString(label);
  WriteString("(");
  WriteInt(v.x, 0);
  WriteString(",");
  WriteInt(v.y, 0);
  WriteString(")");
  WriteLn
END PrintVec;

VAR
  p: Point;
  c: Coord;
  r: Vec2;

BEGIN
  p.x := 1; p.y := 2;
  c.x := 3; c.y := 4;

  (* Pass alias-typed vars to procedure expecting Vec2 *)
  AddVec(p, c, r);
  PrintVec("sum=", r);

  (* Pass alias-typed var as result *)
  AddVec(p, c, p);
  PrintVec("p=", p);

  (* Verify field access on alias type *)
  WriteString("cx="); WriteInt(c.x, 0); WriteLn
END TypeAliasRecord.
