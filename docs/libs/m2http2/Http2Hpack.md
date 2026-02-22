# Http2Hpack

HPACK header compression (RFC 7541). Integer encoding/decoding, static table lookup, dynamic table with FIFO eviction, and full header block encode/decode.

## Limitations

No Huffman encoding or decoding. Literal strings are sent and received in their raw form. This is valid per RFC 7541 (Huffman is optional).

## Procedures

### Integer Codec

```modula2
PROCEDURE EncodeInt(VAR b: Buf; value: CARDINAL;
                    prefixBits: CARDINAL; mask: CARDINAL);
PROCEDURE DecodeInt(firstByte: CARDINAL; prefixBits: CARDINAL;
                    v: BytesView; VAR pos: CARDINAL;
                    VAR ok: BOOLEAN): CARDINAL;
```

RFC 7541 Section 5.1 variable-length integer encoding. `mask` is OR'd into the first byte's upper bits (e.g. 128 for indexed header field representation). `firstByte` is already masked to the prefix bits by the caller.

### Static Table

```modula2
PROCEDURE StaticLookup(index: CARDINAL; VAR entry: HeaderEntry; VAR ok: BOOLEAN);
PROCEDURE StaticFind(name: ARRAY OF CHAR; nameLen: CARDINAL;
                     value: ARRAY OF CHAR; valLen: CARDINAL;
                     nameOnly: BOOLEAN): CARDINAL;
```

61 entries per RFC 7541 Appendix A. `StaticFind` returns the 1-based index of a matching entry (exact match preferred over name-only), or 0 if not found.

### Dynamic Table

```modula2
TYPE DynTable = RECORD
  entries:  ARRAY [0..127] OF HeaderEntry;
  head:     CARDINAL;
  count:    CARDINAL;
  byteSize: CARDINAL;
  maxSize:  CARDINAL;
END;
```

Ring buffer with FIFO eviction. Entry size = nameLen + valLen + 32 (RFC 7541 Section 4.1).

```modula2
PROCEDURE DynInit(VAR dt: DynTable; maxSize: CARDINAL);
PROCEDURE DynInsert(VAR dt: DynTable; name: ARRAY OF CHAR; nameLen: CARDINAL;
                    value: ARRAY OF CHAR; valLen: CARDINAL);
PROCEDURE DynLookup(VAR dt: DynTable; index: CARDINAL;
                    VAR entry: HeaderEntry; VAR ok: BOOLEAN);
PROCEDURE DynResize(VAR dt: DynTable; newMaxSize: CARDINAL);
PROCEDURE DynCount(VAR dt: DynTable): CARDINAL;
```

### Header Block

```modula2
PROCEDURE DecodeHeaderBlock(v: BytesView; VAR dt: DynTable;
                            VAR headers: ARRAY OF HeaderEntry;
                            maxOut: CARDINAL;
                            VAR numHeaders: CARDINAL; VAR ok: BOOLEAN);
PROCEDURE EncodeHeaderBlock(VAR b: Buf; VAR dt: DynTable;
                            VAR headers: ARRAY OF HeaderEntry;
                            numHeaders: CARDINAL);
```

Full header block decode/encode. Supports indexed, literal with incremental indexing, literal without indexing, and dynamic table size updates.

## Usage

```modula2
VAR dt: DynTable;
    hdrs: ARRAY [0..15] OF HeaderEntry;
    buf: Buf;
DynInit(dt, 4096);
(* ... fill hdrs ... *)
Init(buf, 1024);
EncodeHeaderBlock(buf, dt, hdrs, 3);
```
