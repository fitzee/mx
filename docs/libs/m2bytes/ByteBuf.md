# ByteBuf

Growable byte buffer and zero-copy view. The foundation for all binary I/O in m2bytes -- buffers hold raw bytes, views provide lightweight read access without copying, and the Codec module builds on both for structured binary encoding/decoding.

## Why ByteBuf?

Modula-2's built-in `ARRAY OF CHAR` is fixed-size and stack-allocated, which doesn't work for variable-length binary data like network packets, serialized messages, or file contents of unknown size. ByteBuf provides a heap-allocated buffer that grows geometrically (doubling) on demand, paired with a zero-copy view type for efficient read access.

Bytes are stored as `CHAR` internally. Since `CHAR` is signed in C, all byte access goes through `GetByte`/`SetByte` which apply `MOD 256` masking to ensure values are always in the 0..255 range.

## Types

### BytesView

```modula2
TYPE BytesView = RECORD
  base: ADDRESS;
  len:  CARDINAL;
END;
```

A lightweight, zero-copy reference to a contiguous byte range. Views are cheap to create and pass around (two words), but they do **not** own the underlying memory. A view is valid only while the source buffer is not grown, reallocated, or freed.

Views are the primary input for the Codec `Reader` -- convert a buffer to a view with `AsView`, then wrap the view in a Reader for structured decoding.

### Buf

```modula2
TYPE Buf = RECORD
  data: BufPtr;
  len:  CARDINAL;
  cap:  CARDINAL;
END;
```

Heap-allocated growable buffer. `len` is the number of bytes currently stored, `cap` is the allocated capacity. When an append would exceed capacity, the buffer automatically doubles (or grows to the needed size, whichever is larger).

| Constant | Value | Purpose |
|----------|-------|---------|
| `MaxBufCap` | 65535 | Maximum indexable capacity. Operations that would exceed this fail gracefully. |

## Procedures

### Init

```modula2
PROCEDURE Init(VAR b: Buf; initialCap: CARDINAL);
```

Allocate a buffer with the given initial capacity (clamped to MaxBufCap). Sets `len` to 0. Always call `Free` when done to avoid leaking the backing memory.

```modula2
VAR b: Buf;
Init(b, 256);
(* b.len = 0, b.cap = 256, ready for use *)
```

### Free

```modula2
PROCEDURE Free(VAR b: Buf);
```

Release the backing memory and reset `len`/`cap` to 0. After this call, the buffer must be re-initialized with `Init` before reuse. Safe to call on an already-freed buffer.

### Clear

```modula2
PROCEDURE Clear(VAR b: Buf);
```

Reset `len` to 0 without freeing the backing memory. The allocated capacity is retained, so subsequent appends avoid reallocation if they fit within the existing capacity. Useful for reusing a buffer across multiple encode/decode cycles.

### Reserve

```modula2
PROCEDURE Reserve(VAR b: Buf; extra: CARDINAL): BOOLEAN;
```

Ensure the buffer has room for at least `extra` more bytes beyond the current `len`. If the current capacity is insufficient, the buffer grows geometrically (doubles, or to the exact needed size, whichever is larger). Returns FALSE if the resulting capacity would exceed MaxBufCap.

You rarely need to call this directly -- `AppendByte` and `AppendChars` call it automatically. Use it when you know in advance how much space you'll need and want to avoid multiple reallocations.

### AppendByte

```modula2
PROCEDURE AppendByte(VAR b: Buf; x: CARDINAL);
```

Append a single byte. `x` is masked to 0..255 via `MOD 256`.

### AppendChars

```modula2
PROCEDURE AppendChars(VAR b: Buf; a: ARRAY OF CHAR; n: CARDINAL);
```

Append the first `n` bytes from an open CHAR array. If `n` exceeds `HIGH(a)+1`, it is clamped.

### AppendView

```modula2
PROCEDURE AppendView(VAR b: Buf; v: BytesView);
```

Append the entire contents of a BytesView to the buffer.

### GetByte

```modula2
PROCEDURE GetByte(VAR b: Buf; idx: CARDINAL): CARDINAL;
```

Read the byte at index `idx` as an unsigned value 0..255. Returns 0 if `idx >= len` (out of bounds access is safe, not an error).

### SetByte

```modula2
PROCEDURE SetByte(VAR b: Buf; idx: CARDINAL; val: CARDINAL);
```

Write a byte at index `idx`. `val` is masked to 0..255. No-op if `idx >= len`.

### AsView

```modula2
PROCEDURE AsView(VAR b: Buf): BytesView;
```

Create a zero-copy view of the buffer's current contents. The view is a snapshot of `(base, len)` at call time -- it does not track subsequent appends or growth. **Invalidated** if the buffer is grown (which may reallocate the backing memory) or freed.

```modula2
Init(b, 64);
AppendByte(b, 72);  (* 'H' *)
AppendByte(b, 105); (* 'i' *)
v := AsView(b);
(* v.len = 2, ViewGetByte(v, 0) = 72 *)
```

### Truncate

```modula2
PROCEDURE Truncate(VAR b: Buf; newLen: CARDINAL);
```

Reduce the buffer length to `newLen`. No-op if `newLen >= len`. Does not free or reallocate memory -- the capacity is unchanged.

### ViewGetByte

```modula2
PROCEDURE ViewGetByte(v: BytesView; idx: CARDINAL): CARDINAL;
```

Read a byte from a view as an unsigned value 0..255. Returns 0 if `idx >= v.len`. Note: the view is passed by value (not VAR), so this is safe to call on temporary views.

## Example

```modula2
MODULE BufDemo;

FROM InOut IMPORT WriteString, WriteCard, WriteLn;
FROM ByteBuf IMPORT Buf, BytesView, Init, Free, AppendByte, GetByte, AsView, ViewGetByte;

VAR
  b: Buf;
  v: BytesView;

BEGIN
  Init(b, 16);

  AppendByte(b, 72);   (* H *)
  AppendByte(b, 101);  (* e *)
  AppendByte(b, 108);  (* l *)
  AppendByte(b, 108);  (* l *)
  AppendByte(b, 111);  (* o *)

  WriteString("length: ");
  WriteCard(b.len, 0); WriteLn;  (* 5 *)

  v := AsView(b);
  WriteString("first byte: ");
  WriteCard(ViewGetByte(v, 0), 0); WriteLn;  (* 72 *)

  Free(b)
END BufDemo.
```
