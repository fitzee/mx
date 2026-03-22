MODULE RecordCrossModule;
(* Regression: cross-module POINTER TO ADDRESS deref loads wrong type
   in LLVM backend. Also tests imported enum arrays in records. *)

FROM SYSTEM IMPORT ADDRESS, ADR;
FROM FieldDef IMPORT FieldId, Resolve;
FROM PlanDef IMPORT Plan, InitPlan;
FROM RecLib IMPORT Pair, MakePair, Sum, ReadAddr;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

VAR
  p: Pair;
  plan: Plan;
  r: INTEGER;
  a: ADDRESS;
  target: INTEGER;

BEGIN
  MakePair(10, 32, p);
  r := Sum(p);
  WriteString("sum=");
  WriteInt(r, 0);
  WriteLn;

  InitPlan(plan);
  plan.proj.fields[0] := Resolve("name");
  plan.proj.count := 1;
  WriteString("tag=");
  WriteInt(plan.tag, 0);
  WriteLn;
  WriteString("count=");
  WriteInt(plan.proj.count, 0);
  WriteLn;
  WriteString("limit=");
  WriteInt(plan.limit, 0);
  WriteLn;

  target := 777;
  a := ADR(target);
  a := ReadAddr(ADR(a));
  WriteString("addr=");
  IF a = ADR(target) THEN
    WriteString("ok")
  ELSE
    WriteString("fail")
  END;
  WriteLn
END RecordCrossModule.
