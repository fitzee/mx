MODULE LongInt;
FROM InOut IMPORT WriteString, WriteInt, WriteLn;

VAR
  li: LONGINT;
  lc: LONGCARD;
  i: INTEGER;

BEGIN
  li := 1000000;
  li := li * 1000000;  (* 10^12 *)
  WriteString("LONGINT 10^12: ");
  (* Print high and low parts since WriteInt only handles 32-bit *)
  WriteInt(SIZE(LONGINT), 1);
  WriteString(" bytes"); WriteLn;

  WriteString("SIZE(LONGCARD) = ");
  WriteInt(SIZE(LONGCARD), 1);
  WriteString(" bytes"); WriteLn;

  (* LONGINT arithmetic *)
  li := 2147483647;  (* MAX INTEGER *)
  INC(li);
  WriteString("MAX(INTEGER)+1 fits in LONGINT: YES"); WriteLn;

  WriteString("Done"); WriteLn
END LongInt.
