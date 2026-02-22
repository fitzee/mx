# m2bytes

Pure Modula-2 byte buffer and binary codec library.

## Modules

- **ByteBuf** — Growable byte buffer (`Buf`) and zero-copy view (`BytesView`)
- **Codec** — Binary reader/writer with endian support and LEB128/ZigZag varint
- **Hex** — Hexadecimal encoding and decoding

## Quick Start

```modula2
FROM ByteBuf IMPORT Buf, Init, Free, AppendByte, AsView;
FROM Codec IMPORT Reader, Writer, InitReader, InitWriter,
                  WriteU32BE, ReadU32BE;

VAR b: Buf; w: Writer; r: Reader; v: BytesView; ok: BOOLEAN;

BEGIN
  Init(b, 64);
  InitWriter(w, b);
  WriteU32BE(w, 12345);

  v := AsView(b);
  InitReader(r, v);
  (* ReadU32BE(r, ok) returns 12345 *)

  Free(b)
END
```

## Build

```
m2c your_program.mod -I libs/m2bytes/src -o your_program
```

## Design

- No external C dependencies — pure Modula-2 PIM4
- Bytes stored as `ARRAY OF CHAR` with `ORD/CHR` + `MOD 256` masking for unsigned access
- Geometric buffer growth (doubles capacity)
- Zero-copy views for efficient sub-range access
- LEB128 unsigned and ZigZag signed varint encoding
- Lowercase hex output, case-insensitive hex input
