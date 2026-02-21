# Buffers

Binary-safe dynamic buffer for I/O, protocol parsing, and data accumulation.

## Overview

`Buffers` provides a contiguous byte buffer with a read/write cursor model. It is the foundational data structure for the m2http networking stack, used to accumulate incoming socket data, build outgoing requests, and hold response bodies.

## Design Goals

- **Zero per-append allocation**: The backing storage is allocated once at creation and never reallocated. Growth is purely logical (the usable capacity limit increases).
- **Binary safe**: Handles arbitrary byte values including NUL. No string-termination assumptions.
- **Protocol-parsing friendly**: FindCRLF, PeekByte, CopyOut, and Consume enable incremental line-oriented parsing without copying.
- **Zero-copy I/O**: WritePtr/AdvanceWrite for `recv()`, SlicePtr/SliceLen for `send()`.

## Architecture

```
                   rpos           wpos            cap          MaxCap
                    |               |              |              |
   ┌────────────────┼───────────────┼──────────────┼──────────────┐
   │   consumed     │  readable     │  writable    │  reserved    │
   └────────────────┼───────────────┼──────────────┼──────────────┘
                    └── Length() ───┘
                                    └─ Remaining() ┘

   Compact() slides readable bytes to offset 0.
   Growth (Growable mode) doubles cap up to MaxCap.
```

## Internal Data Structures

```modula2
TYPE
  BufRec = RECORD
    data: ARRAY [0..MaxCap-1] OF CHAR;   (* 64 KB backing store *)
    cap:  INTEGER;    (* logical capacity, <= MaxCap *)
    rpos: INTEGER;    (* read cursor *)
    wpos: INTEGER;    (* write cursor *)
    mode: GrowMode;   (* Fixed or Growable *)
  END;
```

- `data` is always `MaxCap` (65536) bytes. This avoids reallocation entirely.
- `cap` starts at `initialCap` and can grow (in Growable mode) by doubling up to `MaxCap`.
- The **readable window** is `data[rpos..wpos)`. Length = `wpos - rpos`.
- The **writable window** is `data[wpos..cap)`. Remaining = `cap - wpos`.

## Memory Model

Each `Buffer` is a heap-allocated `BufRec` (~65 KB). Created with `ALLOCATE`, freed with `DEALLOCATE` via `Destroy`.

| Operation    | Allocation | Notes                                    |
|--------------|------------|------------------------------------------|
| Create       | 1 ALLOCATE | ~65 KB for BufRec                        |
| Destroy      | 1 DEALLOCATE | Sets handle to NIL                     |
| AppendByte   | None       | Writes into existing backing store       |
| Compact      | None       | Byte-by-byte memmove within data array   |
| Growth       | None       | Only increases cap field, no realloc     |

## Growth Modes

| Mode      | Behavior                                              |
|-----------|-------------------------------------------------------|
| `Fixed`   | Capacity never changes. Returns `Full` when exceeded. |
| `Growable`| Capacity doubles on demand (4K → 8K → ... → 64K).    |

Growth is a single integer assignment (`bp^.cap := newCap`). The physical array is always 64 KB.

## Error Model

| Status       | Meaning                                   |
|--------------|-------------------------------------------|
| `OK`         | Operation succeeded.                      |
| `Invalid`    | NIL buffer handle or bad parameters.      |
| `Full`       | No space for write (Fixed mode or MaxCap).|
| `Empty`      | Read/peek beyond available data.          |
| `OutOfMemory`| Heap allocation failed during Create.     |

All procedures return `Status`. Callers should check but can safely ignore in fire-and-forget scenarios (e.g. appending to a known-large buffer).

## Performance Characteristics

- **AppendByte**: O(1) amortized, O(n) worst case when Compact is needed.
- **AppendBytes/AppendString**: O(n) byte copy, no allocation.
- **FindCRLF**: O(n) linear scan from rpos.
- **Compact**: O(n) byte-by-byte slide. Called explicitly by the application.
- **PeekByte**: O(1) direct array access.
- **SlicePtr/WritePtr**: O(1), returns ADDRESS into backing array.

## Limitations

- Maximum buffer size is 64 KB (`MaxCap = 65536`).
- No automatic compaction — callers must call `Compact` to reclaim consumed space.
- Byte-by-byte copy (no `memcpy`), acceptable for typical HTTP payloads.
- Not thread-safe — designed for single-threaded event-loop use.

## Future Extension Points

- C bridge `memcpy`/`memmove` for bulk copy performance.
- Scatter-gather I/O support (linked buffer chains).
- Configurable MaxCap for larger payloads.
- Reference-counted shared buffers for zero-copy pipeline stages.

## API Reference

### Constants

| Constant     | Value | Description                      |
|--------------|-------|----------------------------------|
| `DefaultCap` | 4096  | Default initial capacity.        |
| `MaxCap`     | 65536 | Maximum buffer capacity (64 KB). |

### Types

**`Buffer`** — Opaque handle (`ADDRESS`).

**`GrowMode`** — `(Fixed, Growable)`.

**`Status`** — `(OK, Invalid, Full, Empty, OutOfMemory)`.

### Lifecycle

```modula2
PROCEDURE Create(initialCap: INTEGER; mode: GrowMode;
                 VAR out: Buffer): Status;
```

Allocate a new buffer with the given initial logical capacity and growth mode.

```modula2
PROCEDURE Destroy(VAR b: Buffer): Status;
```

Free the buffer. Sets `b` to `NIL`.

### Writing

```modula2
PROCEDURE AppendByte(b: Buffer; ch: CHAR): Status;
PROCEDURE AppendBytes(b: Buffer; VAR data: ARRAY OF CHAR;
                      len: INTEGER): Status;
PROCEDURE AppendString(b: Buffer; VAR s: ARRAY OF CHAR): Status;
```

Append data to the write position. `AppendString` stops at NUL.

### Reading

```modula2
PROCEDURE PeekByte(b: Buffer; offset: INTEGER;
                   VAR ch: CHAR): Status;
```

Read a byte at `offset` from the read position without consuming it.

```modula2
PROCEDURE Consume(b: Buffer; n: INTEGER): Status;
```

Advance the read position by `n` bytes. Auto-resets both cursors to 0 when fully consumed.

```modula2
PROCEDURE CopyOut(b: Buffer; offset, len: INTEGER;
                  VAR dst: ARRAY OF CHAR): Status;
```

Copy `len` bytes from read position + offset into `dst`.

### State

```modula2
PROCEDURE Length(b: Buffer): INTEGER;
PROCEDURE Capacity(b: Buffer): INTEGER;
PROCEDURE Remaining(b: Buffer): INTEGER;
PROCEDURE Clear(b: Buffer): Status;
PROCEDURE Compact(b: Buffer): Status;
```

`Compact` slides unread bytes to offset 0, reclaiming consumed space.

### Zero-Copy Access

```modula2
PROCEDURE SlicePtr(b: Buffer): ADDRESS;   (* ptr to first readable byte *)
PROCEDURE SliceLen(b: Buffer): INTEGER;   (* readable byte count *)
PROCEDURE WritePtr(b: Buffer): ADDRESS;   (* ptr to first writable byte *)
PROCEDURE AdvanceWrite(b: Buffer; n: INTEGER): Status;
```

Use `WritePtr`/`AdvanceWrite` after external writes (e.g. `recv()` into the buffer). Use `SlicePtr`/`SliceLen` for external reads (e.g. `send()` from the buffer).

### Search

```modula2
PROCEDURE FindByte(b: Buffer; ch: CHAR;
                   VAR pos: INTEGER): BOOLEAN;
PROCEDURE FindCRLF(b: Buffer; VAR pos: INTEGER): BOOLEAN;
```

Search the readable window. Returns offset relative to read position.

## See Also

- [HTTPClient](HTTPClient.md) — Primary consumer (recv buffer, response body)
- [Net-Architecture](Net-Architecture.md) — Overall networking stack design
