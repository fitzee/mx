MODULE HexDemo;
(* Hex encode/decode demo.

   Build:
     m2c examples/hex.mod -I src -o hex
     ./hex

   Expected output:
     encode: deadbeef
     decode ok: 4 bytes
     byte 0: 222
     byte 1: 173
     byte 2: 190
     byte 3: 239 *)

FROM InOut IMPORT WriteString, WriteInt, WriteCard, Write, WriteLn;
FROM ByteBuf IMPORT Buf, Init, Free, Clear, AppendByte, GetByte;
FROM Hex IMPORT Encode, Decode;

VAR
  src, dst: Buf;
  hexStr: ARRAY [0..63] OF CHAR;
  hexLen: CARDINAL;
  ok: BOOLEAN;
  i: CARDINAL;
  inp: ARRAY [0..7] OF CHAR;

BEGIN
  Init(src, 16);
  AppendByte(src, 222);  (* 0xDE *)
  AppendByte(src, 173);  (* 0xAD *)
  AppendByte(src, 190);  (* 0xBE *)
  AppendByte(src, 239);  (* 0xEF *)

  Encode(src, 4, hexStr, hexLen, ok);
  WriteString("encode: ");
  i := 0;
  WHILE i < hexLen DO
    Write(hexStr[i]);
    INC(i)
  END;
  WriteLn;

  Init(dst, 16);
  inp := "deadbeef";
  Decode(inp, 8, dst, ok);
  IF ok THEN
    WriteString("decode ok: ");
    WriteCard(dst.len, 0);
    WriteString(" bytes"); WriteLn;
    i := 0;
    WHILE i < dst.len DO
      WriteString("byte ");
      WriteCard(i, 0);
      WriteString(": ");
      WriteCard(GetByte(dst, i), 0);
      WriteLn;
      INC(i)
    END
  ELSE
    WriteString("decode failed"); WriteLn
  END;

  Free(src);
  Free(dst)
END HexDemo.
