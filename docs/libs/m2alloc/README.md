# m2alloc

## Why
Arena and pool allocators that operate entirely within caller-provided backing stores. No heap allocation, no OS syscalls -- suitable for embedded systems, real-time code, and any context where deterministic memory management is required.

## Modules

### Arena

Bump allocator with mark/reset semantics and optional memory poisoning for debugging.

#### Types

| Type | Description |
|------|-------------|
| `Arena` | Allocator state record with `base`, `size`, `pos`, `highwater`, `failed`, `poison`, and `overflow` fields. |
| `OverflowProc` | `PROCEDURE(ADDRESS, CARDINAL)` -- callback invoked when `Alloc` would fail. Receives the arena address and the requested size. May grow the backing store, log, or halt. |

#### Procedures

**Lifecycle**

```modula2
PROCEDURE Init(VAR a: Arena; base: ADDRESS; size: CARDINAL);
```
Initialize an arena over the backing store `[base..base+size)`. Sets `pos=0`, `highwater=0`, `failed=0`, `poison=FALSE`.

**Allocation**

```modula2
PROCEDURE Alloc(VAR a: Arena; n: CARDINAL; align: CARDINAL;
                VAR p: ADDRESS; VAR ok: BOOLEAN);
```
Allocate `n` bytes with the given alignment (must be a power of two). On success: `p` points to the allocated region, `ok=TRUE`. On failure: `p=NIL`, `ok=FALSE`, failed-allocs counter incremented.

**Mark / Reset**

```modula2
PROCEDURE Mark(VAR a: Arena): CARDINAL;
```
Return the current position for later `ResetTo`.

```modula2
PROCEDURE ResetTo(VAR a: Arena; mark: CARDINAL);
```
Reset `pos` to `mark`. If `mark > pos`, this is a no-op. If poisoning is on, freed bytes are zero-filled.

```modula2
PROCEDURE Clear(VAR a: Arena);
```
Reset `pos` to 0.

**Queries**

```modula2
PROCEDURE Remaining(VAR a: Arena): CARDINAL;
```
Bytes remaining in the arena.

```modula2
PROCEDURE HighWater(VAR a: Arena): CARDINAL;
```
Peak allocation position ever reached.

```modula2
PROCEDURE FailedAllocs(VAR a: Arena): CARDINAL;
```
Number of failed `Alloc` calls.

**Poisoning**

```modula2
PROCEDURE PoisonOn(VAR a: Arena);
```
Enable poison: `Alloc` fills allocated bytes with `0CDH`, `ResetTo` fills freed bytes with `0`.

```modula2
PROCEDURE PoisonOff(VAR a: Arena);
```
Disable poison.

**Overflow Handling**

```modula2
PROCEDURE SetOverflowHandler(VAR a: Arena; handler: OverflowProc);
```
Set a callback invoked when `Alloc` would fail. The handler may grow the backing store, log, or halt. If the handler does not resolve the overflow, `Alloc` returns `NIL`.

---

### Pool

Fixed-size block allocator with an intrusive LIFO free list and optional poisoning.

#### Types

| Type | Description |
|------|-------------|
| `Pool` | Allocator state record with `base`, `size`, `blockSize`, `blockCount`, `freeHead`, `inUse`, `highwater`, `invalidFree`, and `poison` fields. |

#### Procedures

**Lifecycle**

```modula2
PROCEDURE Init(VAR p: Pool; base: ADDRESS; size: CARDINAL;
               blockSize: CARDINAL; VAR ok: BOOLEAN);
```
Initialize a pool over `[base..base+size)` with the given block size (rounded up to `ADDRESS` alignment). On success: `ok=TRUE` and the free list is built. Fails if `blockSize` is too small or zero blocks fit.

**Allocation**

```modula2
PROCEDURE Alloc(VAR p: Pool; VAR out: ADDRESS; VAR ok: BOOLEAN);
```
Allocate one block. On success: `out` points to the block. On failure (pool exhausted): `out=NIL`, `ok=FALSE`.

```modula2
PROCEDURE Free(VAR p: Pool; addr: ADDRESS; VAR ok: BOOLEAN);
```
Return a block to the pool. Validates that `addr` is non-NIL, within pool range, and aligned. On failure: `ok=FALSE`, invalid-frees counter incremented.

**Queries**

```modula2
PROCEDURE InUse(VAR p: Pool): CARDINAL;
```
Number of blocks currently allocated.

```modula2
PROCEDURE HighWater(VAR p: Pool): CARDINAL;
```
Peak number of blocks ever allocated simultaneously.

```modula2
PROCEDURE InvalidFrees(VAR p: Pool): CARDINAL;
```
Number of invalid `Free` calls.

**Poisoning**

```modula2
PROCEDURE PoisonOn(VAR p: Pool);
```
Enable poison: `Alloc` fills with `0CDH`, `Free` fills with `0DDH`.

```modula2
PROCEDURE PoisonOff(VAR p: Pool);
```
Disable poison.

---

### AllocUtil

Pure utility procedures for pointer arithmetic, alignment, and byte-level memory access. No state, no side effects.

#### Types

| Type | Description |
|------|-------------|
| `ByteArray` | `ARRAY [0..65535] OF CHAR` -- overlay for indexed byte access. |
| `BytePtr` | `POINTER TO ByteArray` -- cast an `ADDRESS` to access individual bytes. |
| `AddrPtr` | `POINTER TO ADDRESS` -- read/write an `ADDRESS`-sized value at any location. |

#### Procedures

**Alignment**

```modula2
PROCEDURE IsPowerOfTwo(x: CARDINAL): BOOLEAN;
```
Returns `TRUE` if `x` is a power of two (`x > 0`).

```modula2
PROCEDURE AlignUp(x, align: CARDINAL): CARDINAL;
```
Round `x` up to the next multiple of `align`. `align` must be a power of two; otherwise returns `x` unchanged.

**Pointer Arithmetic**

```modula2
PROCEDURE PtrAdd(base: ADDRESS; offset: CARDINAL): ADDRESS;
```
Return `base + offset` as an `ADDRESS`.

```modula2
PROCEDURE PtrDiff(a, b: ADDRESS): CARDINAL;
```
Return `a - b` as a `CARDINAL`. Returns `0` if `b >= a`.

**Byte Access**

```modula2
PROCEDURE FillBytes(base: ADDRESS; count: CARDINAL; val: CARDINAL);
```
Fill `count` bytes starting at `base` with `val` (0..255).

```modula2
PROCEDURE ReadAddr(loc: ADDRESS): ADDRESS;
```
Read an `ADDRESS` from memory location `loc`.

```modula2
PROCEDURE WriteAddr(loc: ADDRESS; val: ADDRESS);
```
Write `val` as an `ADDRESS` at memory location `loc`.

## Example

```modula2
MODULE AllocDemo;

FROM SYSTEM IMPORT ADDRESS, ADR, SIZE;
FROM Arena IMPORT Arena, Init, Alloc, Mark, ResetTo, Clear,
                  Remaining, PoisonOn;
FROM Pool IMPORT Pool;

VAR
  arenaBuf: ARRAY [0..4095] OF CHAR;
  poolBuf:  ARRAY [0..4095] OF CHAR;
  a: Arena;
  p: Pool;
  ptr1, ptr2: ADDRESS;
  mark: CARDINAL;
  ok: BOOLEAN;

BEGIN
  (* Arena: bump allocator with mark/reset *)
  Init(a, ADR(arenaBuf), SIZE(arenaBuf));
  PoisonOn(a);

  mark := Mark(a);
  Alloc(a, 128, 8, ptr1, ok);   (* 128 bytes, 8-byte aligned *)
  Alloc(a, 256, 16, ptr2, ok);  (* 256 bytes, 16-byte aligned *)
  ResetTo(a, mark);              (* free both allocations at once *)

  (* Pool: fixed-size block allocator *)
  Pool.Init(p, ADR(poolBuf), SIZE(poolBuf), 64, ok);
  Pool.Alloc(p, ptr1, ok);      (* get a 64-byte block *)
  Pool.Alloc(p, ptr2, ok);      (* get another *)
  Pool.Free(p, ptr1, ok);       (* return first block *)

  Clear(a);
END AllocDemo.
```
