MODULE AdrValueArrayParam;
(* Regression: ADR(param) where param is a non-VAR fixed-size array
   must emit the array data address, not the address of the pointer
   variable that C uses to pass the decayed array.

   Covers: value param, VAR param, local var, nested proc, and
   double-indirection (passing ADR result to another procedure). *)

FROM SYSTEM IMPORT ADDRESS, ADR;
FROM InOut IMPORT WriteString, WriteLn;
FROM Sys IMPORT m2sys_str_eq;

TYPE Buf = ARRAY [0..3] OF CHAR;

PROCEDURE Fill(VAR b: Buf);
BEGIN
  b[0] := 'a'; b[1] := 'b'; b[2] := 'c'; b[3] := 0C
END Fill;

(* Case 1: ADR of by-value array param — the original bug *)
PROCEDURE TestValue(a: Buf): BOOLEAN;
VAR local: Buf;
BEGIN
  Fill(local);
  RETURN m2sys_str_eq(ADR(a), ADR(local)) = 1
END TestValue;

(* Case 2: ADR of VAR array param — must still work *)
PROCEDURE TestVar(VAR a: Buf): BOOLEAN;
VAR local: Buf;
BEGIN
  Fill(local);
  RETURN m2sys_str_eq(ADR(a), ADR(local)) = 1
END TestVar;

(* Case 3: pass ADR of value param to a helper *)
PROCEDURE CheckAddr(addr: ADDRESS; expected: ADDRESS): BOOLEAN;
BEGIN
  RETURN m2sys_str_eq(addr, expected) = 1
END CheckAddr;

PROCEDURE TestIndirect(a: Buf): BOOLEAN;
VAR local: Buf;
BEGIN
  Fill(local);
  RETURN CheckAddr(ADR(a), ADR(local))
END TestIndirect;

(* Case 4: nested procedure using ADR on outer's value param
   — the value param is captured/passed, ADR must still work *)
PROCEDURE TestNested(a: Buf): BOOLEAN;
VAR local: Buf;

  PROCEDURE Inner(): BOOLEAN;
  BEGIN
    RETURN m2sys_str_eq(ADR(a), ADR(local)) = 1
  END Inner;

BEGIN
  Fill(local);
  RETURN Inner()
END TestNested;

(* Case 5: two value array params in same procedure *)
PROCEDURE TestTwoParams(a, b: Buf): BOOLEAN;
BEGIN
  RETURN m2sys_str_eq(ADR(a), ADR(b)) = 1
END TestTwoParams;

VAR x, y: Buf;
BEGIN
  Fill(x);
  Fill(y);

  (* Case 1: value param *)
  IF TestValue(x) THEN
    WriteString("value: PASS")
  ELSE
    WriteString("value: FAIL")
  END;
  WriteLn;

  (* Case 2: VAR param *)
  IF TestVar(x) THEN
    WriteString("var: PASS")
  ELSE
    WriteString("var: FAIL")
  END;
  WriteLn;

  (* Case 3: indirect via ADDRESS *)
  IF TestIndirect(x) THEN
    WriteString("indirect: PASS")
  ELSE
    WriteString("indirect: FAIL")
  END;
  WriteLn;

  (* Case 4: nested proc *)
  IF TestNested(x) THEN
    WriteString("nested: PASS")
  ELSE
    WriteString("nested: FAIL")
  END;
  WriteLn;

  (* Case 5: two value params with same content *)
  IF TestTwoParams(x, y) THEN
    WriteString("two_params: PASS")
  ELSE
    WriteString("two_params: FAIL")
  END;
  WriteLn
END AdrValueArrayParam.
