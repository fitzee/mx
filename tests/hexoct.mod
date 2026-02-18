MODULE HexOct;
FROM InOut IMPORT WriteString, WriteInt, WriteHex, WriteOct, WriteLn;

VAR x: INTEGER;
    c: CHAR;

BEGIN
  (* Hex literals *)
  x := 0FFH;
  WriteString("0FFH = "); WriteInt(x, 1); WriteLn;

  x := 0AH;
  WriteString("0AH = "); WriteInt(x, 1); WriteLn;

  (* Octal literals *)
  x := 10B;
  WriteString("10B = "); WriteInt(x, 1); WriteLn;

  x := 77B;
  WriteString("77B = "); WriteInt(x, 1); WriteLn;

  (* Hex/Oct output *)
  x := 255;
  WriteString("255 hex: "); WriteHex(x, 4); WriteLn;
  WriteString("255 oct: "); WriteOct(x, 4); WriteLn;

  (* Char from octal *)
  c := 101C;  (* 'A' in octal *)
  WriteString("101C = "); WriteInt(ORD(c), 1); WriteLn;

  WriteString("Done"); WriteLn
END HexOct.
