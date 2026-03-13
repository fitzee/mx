MODULE LongintDivMod;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

VAR
  a, b, q, r: LONGINT;
  ok: BOOLEAN;

PROCEDURE FloorDivMod(dividend, divisor: LONGINT;
                      VAR qOut, rOut: LONGINT);
(* Exercise LONGINT DIV/MOD through procedure parameters *)
BEGIN
  qOut := dividend DIV divisor;
  rOut := dividend MOD divisor
END FloorDivMod;

BEGIN
  (* --- Test 1: Large positive DIV/MOD (> 2^31) --- *)
  a := 3000000000;  (* > MAX(INTEGER) = 2147483647 *)
  b := 1000000000;
  q := a DIV b;
  r := a MOD b;
  IF (q = 3) AND (r = 0) THEN
    WriteString("large positive: PASS")
  ELSE
    WriteString("large positive: FAIL")
  END;
  WriteLn;

  (* --- Test 2: Large positive non-exact --- *)
  a := 5000000000;
  b := 3000000000;
  q := a DIV b;
  r := a MOD b;
  (* 5000000000 DIV 3000000000 = 1, MOD = 2000000000 *)
  IF (q = 1) AND (r = 2000000000) THEN
    WriteString("large non-exact: PASS")
  ELSE
    WriteString("large non-exact: FAIL")
  END;
  WriteLn;

  (* --- Test 3: PIM4 floored division: (-7) DIV 2 = -4 --- *)
  (* PIM4 specifies DIV truncates toward negative infinity *)
  a := -7;
  b := 2;
  q := a DIV b;
  r := a MOD b;
  WriteString("(-7) DIV 2 = "); WriteInt(SHORT(q), 1);
  WriteString("  MOD = "); WriteInt(SHORT(r), 1); WriteLn;
  IF (q = -4) AND (r = 1) THEN
    WriteString("floor div small: PASS")
  ELSE
    WriteString("floor div small: FAIL")
  END;
  WriteLn;

  (* --- Test 4: PIM4 floored division with 64-bit values --- *)
  (* (-3000000007) DIV 2 should be -1500000004 (floored) *)
  (* (-3000000007) MOD 2 should be 1 *)
  a := -3000000007;
  b := 2;
  q := a DIV b;
  r := a MOD b;
  IF (q = -1500000004) AND (r = 1) THEN
    WriteString("floor div 64: PASS")
  ELSE
    WriteString("floor div 64: FAIL")
  END;
  WriteLn;

  (* --- Test 5: Negative MOD with positive divisor --- *)
  (* PIM4: (-13) MOD 5 = 2  (not -3) *)
  a := -13;
  b := 5;
  q := a DIV b;
  r := a MOD b;
  IF (q = -3) AND (r = 2) THEN
    WriteString("neg mod pos: PASS")
  ELSE
    WriteString("neg mod pos: FAIL")
  END;
  WriteLn;

  (* --- Test 6: Through procedure parameters --- *)
  FloorDivMod(-3000000007, 2, q, r);
  IF (q = -1500000004) AND (r = 1) THEN
    WriteString("proc floor 64: PASS")
  ELSE
    WriteString("proc floor 64: FAIL")
  END;
  WriteLn;

  (* --- Test 7: Positive through procedure parameters --- *)
  FloorDivMod(5000000000, 3000000000, q, r);
  IF (q = 1) AND (r = 2000000000) THEN
    WriteString("proc large pos: PASS")
  ELSE
    WriteString("proc large pos: FAIL")
  END;
  WriteLn;

  WriteString("all longint div mod OK"); WriteLn
END LongintDivMod.
