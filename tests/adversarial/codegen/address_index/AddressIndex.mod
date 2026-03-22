MODULE AddressIndex;
(* m2plus: ADDRESS^[i] byte-level indexing.
   ADDRESS should be treatable as POINTER TO ARRAY OF CHAR
   for byte-level read and write through indexing. *)

FROM SYSTEM IMPORT ADDRESS, ADR;
FROM InOut IMPORT Write, WriteString, WriteInt, WriteLn;
FROM Storage IMPORT ALLOCATE;

VAR
  buf: ARRAY [0..7] OF CHAR;
  p: ADDRESS;
  i: INTEGER;
  ch: CHAR;

BEGIN
  (* Fill buffer with known values *)
  buf[0] := 'H';
  buf[1] := 'e';
  buf[2] := 'l';
  buf[3] := 'l';
  buf[4] := 'o';
  buf[5] := CHR(0);

  (* Read through ADDRESS pointer *)
  p := ADR(buf);
  i := 0;
  WHILE i < 5 DO
    ch := p^[i];
    WriteString("r");
    WriteInt(i, 0);
    WriteString("=");
    Write(ch);
    WriteLn;
    INC(i)
  END;

  (* Write through ADDRESS pointer *)
  p^[0] := 'W';
  p^[1] := 'o';
  p^[2] := 'r';
  p^[3] := 'l';
  p^[4] := 'd';
  WriteString("mod=");
  WriteString(buf);
  WriteLn;

  (* Shift bytes forward (the h2check pattern) *)
  i := 0;
  WHILE i < 3 DO
    p^[i] := p^[i + 2];
    INC(i)
  END;
  p^[3] := CHR(0);
  WriteString("shift=");
  WriteString(buf);
  WriteLn
END AddressIndex.
