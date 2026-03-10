MODULE DivMod;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

VAR a, b, d, m: INTEGER;

BEGIN
  (* Test PIM4 DIV and MOD semantics *)
  (* PIM4: DIV truncates toward negative infinity *)
  (* PIM4: MOD result is always non-negative (for positive divisor) *)

  a := 7; b := 3;
  d := a DIV b;
  m := a MOD b;
  WriteString("7 DIV 3 = "); WriteInt(d, 1);
  WriteString("  7 MOD 3 = "); WriteInt(m, 1); WriteLn;

  a := -7; b := 3;
  d := a DIV b;
  m := a MOD b;
  WriteString("-7 DIV 3 = "); WriteInt(d, 1);
  WriteString("  -7 MOD 3 = "); WriteInt(m, 1); WriteLn;

  a := 7; b := -3;
  d := a DIV b;
  m := a MOD b;
  WriteString("7 DIV -3 = "); WriteInt(d, 1);
  WriteString("  7 MOD -3 = "); WriteInt(m, 1); WriteLn;

  a := -7; b := -3;
  d := a DIV b;
  m := a MOD b;
  WriteString("-7 DIV -3 = "); WriteInt(d, 1);
  WriteString("  -7 MOD -3 = "); WriteInt(m, 1); WriteLn;

  (* Test real division *)
  WriteString("Real: 7/3 = ");
  (* We can't easily print float without RealInOut, but test it compiles *)

  WriteString("Done."); WriteLn
END DivMod.
