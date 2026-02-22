MODULE Framing;
(* Length-prefixed framing demo.

   Writes a frame: u32be length + payload bytes.
   Then reads it back and prints the payload.

   Build:
     m2c examples/framing.mod -I src -o framing
     ./framing

   Expected output:
     wrote frame: 5 bytes payload
     read frame: 5 bytes
     payload: Hello *)

FROM InOut IMPORT WriteString, WriteInt, Write, WriteLn;
FROM ByteBuf IMPORT Buf, BytesView, Init, Free, Clear, AsView,
                     AppendByte, GetByte;
FROM Codec IMPORT Reader, Writer, InitReader, InitWriter,
                  WriteU32BE, ReadU32BE, WriteU8, ReadU8;

VAR
  b: Buf;
  w: Writer;
  r: Reader;
  v: BytesView;
  ok: BOOLEAN;
  frameLen, i, ch: CARDINAL;
  payload: ARRAY [0..4] OF CHAR;

BEGIN
  payload[0] := 'H';
  payload[1] := 'e';
  payload[2] := 'l';
  payload[3] := 'l';
  payload[4] := 'o';

  (* write frame *)
  Init(b, 64);
  InitWriter(w, b);
  WriteU32BE(w, 5);  (* length prefix *)
  i := 0;
  WHILE i <= 4 DO
    WriteU8(w, ORD(payload[i]));
    INC(i)
  END;
  WriteString("wrote frame: 5 bytes payload"); WriteLn;

  (* read it back *)
  v := AsView(b);
  InitReader(r, v);
  frameLen := ReadU32BE(r, ok);
  WriteString("read frame: ");
  WriteInt(INTEGER(frameLen), 0);
  WriteString(" bytes"); WriteLn;

  WriteString("payload: ");
  i := 0;
  WHILE i < frameLen DO
    ch := ReadU8(r, ok);
    Write(CHR(ch));
    INC(i)
  END;
  WriteLn;

  Free(b)
END Framing.
