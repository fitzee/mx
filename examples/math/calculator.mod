MODULE Calculator;
(* A simple expression evaluator demonstrating many language features *)
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

CONST
  MaxStack = 20;

TYPE
  Operation = (OpAdd, OpSub, OpMul, OpDiv, OpMod, OpNeg, OpPush);

  Instruction = RECORD
    op: Operation;
    value: INTEGER;
  END;

  Program = ARRAY [0..MaxStack-1] OF Instruction;

  IntStack = RECORD
    data: ARRAY [0..MaxStack-1] OF INTEGER;
    top: INTEGER;
  END;

VAR
  stack: IntStack;
  prog: Program;
  pc: INTEGER;

(* Stack operations *)

PROCEDURE InitStack(VAR s: IntStack);
BEGIN
  s.top := -1
END InitStack;

PROCEDURE Push(VAR s: IntStack; val: INTEGER);
BEGIN
  INC(s.top);
  s.data[s.top] := val
END Push;

PROCEDURE Pop(VAR s: IntStack): INTEGER;
  VAR val: INTEGER;
BEGIN
  val := s.data[s.top];
  DEC(s.top);
  RETURN val
END Pop;

PROCEDURE Top(s: IntStack): INTEGER;
BEGIN
  RETURN s.data[s.top]
END Top;

PROCEDURE IsEmpty(s: IntStack): BOOLEAN;
BEGIN
  RETURN s.top < 0
END IsEmpty;

(* Instruction creation *)

PROCEDURE MakePush(val: INTEGER): Instruction;
  VAR inst: Instruction;
BEGIN
  inst.op := OpPush;
  inst.value := val;
  RETURN inst
END MakePush;

PROCEDURE MakeOp(op: Operation): Instruction;
  VAR inst: Instruction;
BEGIN
  inst.op := op;
  inst.value := 0;
  RETURN inst
END MakeOp;

(* Execute a program *)

PROCEDURE Execute(VAR s: IntStack; prog: ARRAY OF Instruction; len: INTEGER);
  VAR i, a, b: INTEGER;
BEGIN
  InitStack(s);
  FOR i := 0 TO len - 1 DO
    CASE ORD(prog[i].op) OF
      6: (* OpPush *)
        Push(s, prog[i].value) |
      0: (* OpAdd *)
        b := Pop(s); a := Pop(s);
        Push(s, a + b) |
      1: (* OpSub *)
        b := Pop(s); a := Pop(s);
        Push(s, a - b) |
      2: (* OpMul *)
        b := Pop(s); a := Pop(s);
        Push(s, a * b) |
      3: (* OpDiv *)
        b := Pop(s); a := Pop(s);
        Push(s, a DIV b) |
      4: (* OpMod *)
        b := Pop(s); a := Pop(s);
        Push(s, a MOD b) |
      5: (* OpNeg *)
        a := Pop(s);
        Push(s, -a)
    END
  END
END Execute;

(* Test: compute (3 + 4) * (10 - 2) = 7 * 8 = 56 *)

PROCEDURE TestExpr1;
BEGIN
  WriteString("Test 1: (3 + 4) * (10 - 2)"); WriteLn;
  prog[0] := MakePush(3);
  prog[1] := MakePush(4);
  prog[2] := MakeOp(OpAdd);
  prog[3] := MakePush(10);
  prog[4] := MakePush(2);
  prog[5] := MakeOp(OpSub);
  prog[6] := MakeOp(OpMul);
  Execute(stack, prog, 7);
  WriteString("Result: "); WriteInt(Top(stack), 1); WriteLn;
END TestExpr1;

(* Test: compute 100 MOD 7 = 2 *)

PROCEDURE TestExpr2;
BEGIN
  WriteString("Test 2: 100 MOD 7"); WriteLn;
  prog[0] := MakePush(100);
  prog[1] := MakePush(7);
  prog[2] := MakeOp(OpMod);
  Execute(stack, prog, 3);
  WriteString("Result: "); WriteInt(Top(stack), 1); WriteLn;
END TestExpr2;

(* Test: compute -(5 * 6) + 100 = -30 + 100 = 70 *)

PROCEDURE TestExpr3;
BEGIN
  WriteString("Test 3: -(5 * 6) + 100"); WriteLn;
  prog[0] := MakePush(5);
  prog[1] := MakePush(6);
  prog[2] := MakeOp(OpMul);
  prog[3] := MakeOp(OpNeg);
  prog[4] := MakePush(100);
  prog[5] := MakeOp(OpAdd);
  Execute(stack, prog, 6);
  WriteString("Result: "); WriteInt(Top(stack), 1); WriteLn;
END TestExpr3;

BEGIN
  TestExpr1;
  TestExpr2;
  TestExpr3;
  WriteString("Done"); WriteLn
END Calculator.
