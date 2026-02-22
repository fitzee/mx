# Hex

Hexadecimal encoding and decoding. Converts between binary byte data in a ByteBuf and printable hex character strings. Useful for debugging binary protocols, displaying cryptographic hashes, or implementing text-based wire formats.

## Behavior

- **Encoding** produces lowercase hex characters (`0-9`, `a-f`). Each input byte becomes exactly 2 hex characters.
- **Decoding** accepts both uppercase and lowercase input. It validates strictly: odd-length strings and non-hex characters cause `ok := FALSE`.
- **No heap allocation.** Output buffers are caller-provided (ARRAY OF CHAR for encoding, Buf for decoding).

## Procedures

### Encode

```modula2
PROCEDURE Encode(VAR src: Buf; nBytes: CARDINAL;
                 VAR out: ARRAY OF CHAR;
                 VAR outLen: CARDINAL;
                 VAR ok: BOOLEAN);
```

Encode the first `nBytes` bytes of `src` into hex characters. The output array `out` must be large enough to hold `2 * nBytes` characters. `outLen` is set to the number of characters actually written. Sets `ok := FALSE` if `out` is too small.

```modula2
VAR b: Buf; hex: ARRAY [0..63] OF CHAR; hexLen: CARDINAL; ok: BOOLEAN;
Init(b, 16);
AppendByte(b, 222);  (* 0xDE *)
AppendByte(b, 173);  (* 0xAD *)
AppendByte(b, 190);  (* 0xBE *)
AppendByte(b, 239);  (* 0xEF *)
Encode(b, 4, hex, hexLen, ok);
(* hex = "deadbeef", hexLen = 8 *)
```

### Decode

```modula2
PROCEDURE Decode(s: ARRAY OF CHAR; sLen: CARDINAL;
                 VAR dst: Buf;
                 VAR ok: BOOLEAN);
```

Decode `sLen` hex characters from `s` and **append** the resulting bytes to `dst`. The destination buffer is not cleared first, so you can decode multiple hex strings into the same buffer. Sets `ok := FALSE` if `sLen` is odd or any character is not a valid hex digit.

```modula2
VAR dst: Buf; ok: BOOLEAN;
Init(dst, 16);
Decode("cafebabe", 8, dst, ok);
(* dst.len = 4, bytes: 202, 254, 186, 190 *)
```

### ByteToHex

```modula2
PROCEDURE ByteToHex(val: CARDINAL; VAR hi, lo: CHAR);
```

Encode a single byte value (0..255) into its two hex character representation. `hi` receives the high nibble, `lo` the low nibble. For example, `ByteToHex(255, hi, lo)` sets `hi := 'f'` and `lo := 'f'`.

### HexToByte

```modula2
PROCEDURE HexToByte(hi, lo: CHAR; VAR val: CARDINAL; VAR ok: BOOLEAN);
```

Decode two hex characters into a single byte value (0..255). Accepts both uppercase and lowercase. Sets `ok := FALSE` if either character is not a valid hex digit (`0-9`, `a-f`, `A-F`).

## Example

```modula2
MODULE HexDemo;

FROM InOut IMPORT WriteString, WriteCard, Write, WriteLn;
FROM ByteBuf IMPORT Buf, Init, Free, AppendByte, GetByte;
FROM Hex IMPORT Encode, Decode;

VAR
  src, dst: Buf;
  hex: ARRAY [0..63] OF CHAR;
  hexLen, i: CARDINAL;
  ok: BOOLEAN;

BEGIN
  (* Encode bytes to hex *)
  Init(src, 16);
  AppendByte(src, 222);  (* 0xDE *)
  AppendByte(src, 173);  (* 0xAD *)
  AppendByte(src, 190);  (* 0xBE *)
  AppendByte(src, 239);  (* 0xEF *)

  Encode(src, 4, hex, hexLen, ok);
  WriteString("hex: ");
  i := 0;
  WHILE i < hexLen DO Write(hex[i]); INC(i) END;
  WriteLn;
  (* Output: hex: deadbeef *)

  (* Decode hex back to bytes *)
  Init(dst, 16);
  Decode("deadbeef", 8, dst, ok);
  WriteString("bytes: ");
  WriteCard(dst.len, 0);
  WriteLn;
  (* Output: bytes: 4 *)

  Free(src);
  Free(dst)
END HexDemo.
```
