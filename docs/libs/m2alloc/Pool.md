# Pool

Fixed-size block allocator with intrusive free list. Operates entirely within a caller-provided backing store -- no heap allocation, no OS syscalls.

## Why Pool?

When all allocations are the same size (linked-list nodes, event records, connection handles), a pool is optimal: O(1) alloc and free, zero fragmentation, zero per-block overhead beyond the block itself. The free list is intrusive (stored inside free blocks), so no separate bookkeeping memory is needed.

The backing store is caller-provided, so Pool works without an OS allocator.

## Types

### Pool

```modula2
TYPE Pool = RECORD
  base:        ADDRESS;
  size:        CARDINAL;
  blockSize:   CARDINAL;
  blockCount:  CARDINAL;
  freeHead:    ADDRESS;
  inUse:       CARDINAL;
  highwater:   CARDINAL;
  invalidFree: CARDINAL;
  poison:      BOOLEAN;
END;
```

| Field | Purpose |
|-------|---------|
| `base` | Start of the backing store |
| `size` | Total size in bytes |
| `blockSize` | Size of each block (rounded up to ADDRESS alignment) |
| `blockCount` | Number of blocks that fit |
| `freeHead` | Head of the intrusive free list |
| `inUse` | Number of blocks currently allocated |
| `highwater` | Peak `inUse` ever reached |
| `invalidFree` | Count of invalid Free calls |
| `poison` | Whether poison patterns are active |

## Procedures

### Init

```modula2
PROCEDURE Init(VAR p: Pool; base: ADDRESS; size: CARDINAL;
               blockSize: CARDINAL; VAR ok: BOOLEAN);
```

Initialise a pool over `[base..base+size)` with blocks of `blockSize` bytes. Block size is rounded up to ADDRESS alignment. On success, the free list is built and `ok=TRUE`. On failure (zero blocks fit), `ok=FALSE`.

The free list is built so that first alloc returns the lowest address.

### Alloc

```modula2
PROCEDURE Alloc(VAR p: Pool; VAR out: ADDRESS; VAR ok: BOOLEAN);
```

Pop one block from the free list. On success, `out` points to the block and `ok=TRUE`. On failure (pool exhausted), `out=NIL` and `ok=FALSE`.

If poison is on, the block is filled with `0CDH`.

### Free

```modula2
PROCEDURE Free(VAR p: Pool; addr: ADDRESS; VAR ok: BOOLEAN);
```

Return a block to the pool. Validates that `addr` is not NIL, falls within the pool's range, and is aligned to blockSize boundaries. On success, `ok=TRUE`. On failure, `ok=FALSE` and `invalidFree` is incremented.

If poison is on, the block is filled with `0DDH` (then the next pointer is written at offset 0).

Double-free is not detected in O(1); it is the caller's responsibility. The poison pattern helps identify double-frees during debugging.

### InUse / HighWater / InvalidFrees

```modula2
PROCEDURE InUse(VAR p: Pool): CARDINAL;
PROCEDURE HighWater(VAR p: Pool): CARDINAL;
PROCEDURE InvalidFrees(VAR p: Pool): CARDINAL;
```

Diagnostic counters. InUse is the current allocation count, HighWater is the peak, InvalidFrees counts rejected Free calls.

### PoisonOn / PoisonOff

```modula2
PROCEDURE PoisonOn(VAR p: Pool);
PROCEDURE PoisonOff(VAR p: Pool);
```

Toggle poison mode. When on, Alloc fills blocks with `0CDH` and Free fills with `0DDH`. The next pointer in freed blocks occupies the first ADDRESS-sized bytes.

## Example

```modula2
VAR buf: ARRAY [0..4095] OF CHAR;
    p: Pool;
    ok: BOOLEAN;
    node: ADDRESS;
BEGIN
  Init(p, ADR(buf), SIZE(buf), 64, ok);
  Alloc(p, node, ok);
  (* ... use node ... *)
  Free(p, node, ok);
```
