# Codec

Binary reader and writer with endian control and variable-length integer encoding. Builds on ByteBuf to provide structured, sequential access to binary data -- the kind of encoding/decoding you need for network protocols, file formats, and serialization.

## Design

The Codec works with two cursor types:

- A **Reader** wraps a `BytesView` and tracks a read position. Each read advances the cursor. If there aren't enough bytes for an operation, it sets `ok := FALSE` and leaves the cursor unchanged (so you can detect and handle truncated input cleanly).

- A **Writer** wraps a pointer to a `Buf` and appends bytes. The buffer grows automatically via ByteBuf's geometric growth policy, so you never need to pre-calculate output sizes.

All multi-byte operations come in LE (little-endian) and BE (big-endian) variants. There is no default byte order -- you always choose explicitly. The encoding uses arithmetic (multiply/divide by 256) rather than bit shifts, which works correctly with Modula-2's unsigned CARDINAL type.

## Types

### Reader

```modula2
TYPE Reader = RECORD
  v:   BytesView;
  pos: CARDINAL;
END;
```

Sequential cursor over a BytesView. The `pos` field tracks how many bytes have been consumed. You can inspect `Remaining(r)` at any time to check how much data is left.

### Writer

```modula2
TYPE Writer = RECORD
  buf: POINTER TO Buf;
END;
```

Sequential appender targeting a Buf. The Writer does not own the buffer -- it holds a pointer, so the buffer must outlive the Writer.

## Reader Procedures

### InitReader

```modula2
PROCEDURE InitReader(VAR r: Reader; v: BytesView);
```

Initialize a reader at position 0 over the given view. The view's memory must remain valid for the reader's lifetime.

```modula2
VAR b: Buf; v: BytesView; r: Reader;
Init(b, 64);
(* ... fill b with data ... *)
v := AsView(b);
InitReader(r, v);
```

### Remaining

```modula2
PROCEDURE Remaining(VAR r: Reader): CARDINAL;
```

Number of bytes between the current position and the end of the view.

### ReadU8

```modula2
PROCEDURE ReadU8(VAR r: Reader; VAR ok: BOOLEAN): CARDINAL;
```

Read one byte as an unsigned value 0..255. Advances the cursor by 1. Sets `ok := FALSE` if the view is exhausted.

### ReadU16LE, ReadU16BE

```modula2
PROCEDURE ReadU16LE(VAR r: Reader; VAR ok: BOOLEAN): CARDINAL;
PROCEDURE ReadU16BE(VAR r: Reader; VAR ok: BOOLEAN): CARDINAL;
```

Read a 16-bit unsigned integer in little-endian or big-endian byte order. Advances by 2 bytes. Sets `ok := FALSE` if fewer than 2 bytes remain (cursor unchanged).

### ReadU32LE, ReadU32BE

```modula2
PROCEDURE ReadU32LE(VAR r: Reader; VAR ok: BOOLEAN): CARDINAL;
PROCEDURE ReadU32BE(VAR r: Reader; VAR ok: BOOLEAN): CARDINAL;
```

Read a 32-bit unsigned integer. Advances by 4 bytes.

### ReadI32LE, ReadI32BE

```modula2
PROCEDURE ReadI32LE(VAR r: Reader; VAR ok: BOOLEAN): INTEGER;
PROCEDURE ReadI32BE(VAR r: Reader; VAR ok: BOOLEAN): INTEGER;
```

Read a 32-bit signed integer. Two's complement encoding -- the byte pattern is identical to unsigned, but the result is interpreted as INTEGER.

### Skip

```modula2
PROCEDURE Skip(VAR r: Reader; n: CARDINAL; VAR ok: BOOLEAN);
```

Advance the cursor by `n` bytes without reading. Sets `ok := FALSE` if fewer than `n` bytes remain (cursor unchanged). Useful for skipping over header fields or padding you don't need.

### ReadSlice

```modula2
PROCEDURE ReadSlice(VAR r: Reader; n: CARDINAL;
                    VAR out: BytesView; VAR ok: BOOLEAN);
```

Extract `n` bytes as a zero-copy sub-view. The returned view points directly into the reader's underlying data -- no copying. Advances the cursor by `n`. Sets `ok := FALSE` if fewer than `n` bytes remain.

This is the efficient way to extract a variable-length payload from a framed protocol: read the length, then `ReadSlice` to get a view of the payload without copying it.

```modula2
frameLen := ReadU32BE(r, ok);
ReadSlice(r, frameLen, payload, ok);
(* payload is a zero-copy view of the frame body *)
```

## Writer Procedures

### InitWriter

```modula2
PROCEDURE InitWriter(VAR w: Writer; VAR b: Buf);
```

Initialize a writer targeting the given buffer. Subsequent writes append to the buffer's current contents.

### WriteU8

```modula2
PROCEDURE WriteU8(VAR w: Writer; val: CARDINAL);
```

Append one byte (val masked to 0..255).

### WriteU16LE, WriteU16BE

```modula2
PROCEDURE WriteU16LE(VAR w: Writer; val: CARDINAL);
PROCEDURE WriteU16BE(VAR w: Writer; val: CARDINAL);
```

Append a 16-bit unsigned integer in little-endian or big-endian byte order (2 bytes).

### WriteU32LE, WriteU32BE

```modula2
PROCEDURE WriteU32LE(VAR w: Writer; val: CARDINAL);
PROCEDURE WriteU32BE(VAR w: Writer; val: CARDINAL);
```

Append a 32-bit unsigned integer (4 bytes).

### WriteI32LE, WriteI32BE

```modula2
PROCEDURE WriteI32LE(VAR w: Writer; val: INTEGER);
PROCEDURE WriteI32BE(VAR w: Writer; val: INTEGER);
```

Append a 32-bit signed integer (4 bytes, two's complement).

### WriteChars

```modula2
PROCEDURE WriteChars(VAR w: Writer; a: ARRAY OF CHAR; n: CARDINAL);
```

Append `n` bytes from an open CHAR array.

## Varint (LEB128 / ZigZag)

Variable-length integer encoding for compact representation of small values. Used extensively in protocol buffers, SQLite, and other binary formats.

### WriteVarU32

```modula2
PROCEDURE WriteVarU32(VAR w: Writer; val: CARDINAL);
```

Encode an unsigned 32-bit value as LEB128 (Little-Endian Base 128). Uses 1 byte for values 0..127, 2 bytes for 128..16383, up to 5 bytes for the full 32-bit range. Each byte stores 7 data bits and 1 continuation bit.

### ReadVarU32

```modula2
PROCEDURE ReadVarU32(VAR r: Reader; VAR ok: BOOLEAN): CARDINAL;
```

Decode a LEB128 unsigned varint. Sets `ok := FALSE` if the encoding uses more than 5 bytes (malformed) or the reader is exhausted. On failure the cursor is restored to its position before the call.

### WriteVarI32

```modula2
PROCEDURE WriteVarI32(VAR w: Writer; val: INTEGER);
```

Encode a signed 32-bit value using ZigZag encoding on top of LEB128. ZigZag maps signed integers to unsigned ones so that small-magnitude values (both positive and negative) use few bytes: 0 maps to 0, -1 maps to 1, 1 maps to 2, -2 maps to 3, and so on. The formula is: positive N encodes as 2*N, negative N encodes as 2*(-N)-1.

### ReadVarI32

```modula2
PROCEDURE ReadVarI32(VAR r: Reader; VAR ok: BOOLEAN): INTEGER;
```

Decode a ZigZag + LEB128 signed varint. Reverses the ZigZag mapping to recover the original signed value.

## Example

```modula2
MODULE CodecDemo;

FROM InOut IMPORT WriteString, WriteInt, WriteLn;
FROM ByteBuf IMPORT Buf, BytesView, Init, Free, AsView;
FROM Codec IMPORT Reader, Writer, InitReader, InitWriter,
                  WriteU32BE, ReadU32BE, WriteVarU32, ReadVarU32;

VAR
  b: Buf;
  w: Writer;
  r: Reader;
  v: BytesView;
  ok: BOOLEAN;
  val: CARDINAL;

BEGIN
  Init(b, 64);
  InitWriter(w, b);

  WriteU32BE(w, 12345);
  WriteVarU32(w, 300);

  v := AsView(b);
  InitReader(r, v);

  val := ReadU32BE(r, ok);
  WriteString("u32be: ");
  WriteInt(INTEGER(val), 0); WriteLn;  (* 12345 *)

  val := ReadVarU32(r, ok);
  WriteString("varint: ");
  WriteInt(INTEGER(val), 0); WriteLn;  (* 300 *)

  Free(b)
END CodecDemo.
```
