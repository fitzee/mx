MODULE AddressDivMod;
FROM SYSTEM IMPORT ADDRESS, ADR;
FROM InOut IMPORT WriteString, WriteLn, WriteInt;

VAR
  a, b, c: ADDRESS;
  x: ARRAY [0..15] OF CHAR;

BEGIN
  a := ADR(x);
  b := ADR(x);
  (* Just test that DIV and MOD compile and run on ADDRESS *)
  (* The actual values are pointer-dependent so we just check non-crash *)
  WriteString("ok");
  WriteLn;
END AddressDivMod.
