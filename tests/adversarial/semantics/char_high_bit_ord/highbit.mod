MODULE HighBit;
(* Test that ORD correctly handles high-bit chars (0x80-0xFF).
   These chars must zero-extend, not sign-extend, when converted
   to INTEGER/CARDINAL via ORD or type transfer.

   Bug history: C backend was sign-extending char values >= 128,
   causing ORD(CHR(255)) to return -1 instead of 255. *)

FROM InOut IMPORT WriteString, WriteCard, WriteLn;

VAR
  buf: ARRAY [0..7] OF CHAR;
  i: CARDINAL;
  val: CARDINAL;

BEGIN
  (* Test individual high-bit values *)
  buf[0] := CHR(128);  (* 0x80 - min high bit *)
  buf[1] := CHR(129);  (* 0x81 *)
  buf[2] := CHR(200);  (* 0xC8 *)
  buf[3] := CHR(254);  (* 0xFE *)
  buf[4] := CHR(255);  (* 0xFF - max byte value *)
  buf[5] := CHR(0);    (* 0x00 - null *)
  buf[6] := CHR(65);   (* 0x41 - 'A' *)
  buf[7] := CHR(127);  (* 0x7F - max low bit *)

  WriteString("ORD values:"); WriteLn;
  FOR i := 0 TO 7 DO
    val := ORD(buf[i]);
    WriteCard(val, 4);
    WriteLn
  END;

  (* Verify specific critical values *)
  WriteString("Verify 128: ");
  IF ORD(CHR(128)) = 128 THEN
    WriteString("PASS")
  ELSE
    WriteString("FAIL")
  END;
  WriteLn;

  WriteString("Verify 255: ");
  IF ORD(CHR(255)) = 255 THEN
    WriteString("PASS")
  ELSE
    WriteString("FAIL")
  END;
  WriteLn;

  (* Test array indexing with high-bit value *)
  WriteString("Index test: ");
  val := ORD(buf[4]);  (* Should be 255, not -1 or wrapped *)
  IF val = 255 THEN
    WriteString("PASS")
  ELSE
    WriteString("FAIL")
  END;
  WriteLn;

  WriteString("Done"); WriteLn
END HighBit.
