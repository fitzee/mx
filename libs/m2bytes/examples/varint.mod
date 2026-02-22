MODULE VarintDemo;
(* Varint encode/decode demo.

   Demonstrates LEB128 unsigned and ZigZag signed varint encoding.

   Build:
     m2c examples/varint.mod -I src -o varint
     ./varint

   Expected output:
     varint 0: 1 byte
     varint 127: 1 byte
     varint 128: 2 bytes
     varint 300: 2 bytes
     varint 16384: 3 bytes
     zigzag 0 -> 0
     zigzag -1 -> -1
     zigzag 1 -> 1
     zigzag -100 -> -100 *)

FROM InOut IMPORT WriteString, WriteInt, WriteCard, WriteLn;
FROM ByteBuf IMPORT Buf, BytesView, Init, Free, Clear, AsView;
FROM Codec IMPORT Reader, Writer, InitReader, InitWriter,
                  WriteVarU32, ReadVarU32,
                  WriteVarI32, ReadVarI32;

PROCEDURE ShowVarU32(val: CARDINAL);
VAR b: Buf; w: Writer; ok: BOOLEAN;
BEGIN
  Init(b, 16);
  InitWriter(w, b);
  WriteVarU32(w, val);
  WriteString("varint ");
  WriteCard(val, 0);
  WriteString(": ");
  WriteCard(b.len, 0);
  IF b.len = 1 THEN WriteString(" byte")
  ELSE WriteString(" bytes")
  END;
  WriteLn;
  Free(b)
END ShowVarU32;

PROCEDURE ShowZigZag(val: INTEGER);
VAR b: Buf; w: Writer; v: BytesView; r: Reader; ok: BOOLEAN;
    decoded: INTEGER;
BEGIN
  Init(b, 16);
  InitWriter(w, b);
  WriteVarI32(w, val);
  v := AsView(b);
  InitReader(r, v);
  decoded := ReadVarI32(r, ok);
  WriteString("zigzag ");
  WriteInt(val, 0);
  WriteString(" -> ");
  WriteInt(decoded, 0);
  WriteLn;
  Free(b)
END ShowZigZag;

BEGIN
  ShowVarU32(0);
  ShowVarU32(127);
  ShowVarU32(128);
  ShowVarU32(300);
  ShowVarU32(16384);

  ShowZigZag(0);
  ShowZigZag(-1);
  ShowZigZag(1);
  ShowZigZag(-100)
END VarintDemo.
