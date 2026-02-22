# Arena

Bump allocator with mark/reset and optional poisoning. Operates entirely within a caller-provided backing store -- no heap allocation, no OS syscalls.

## Why Arena?

Many workloads allocate a burst of short-lived objects (parser nodes, per-request temporaries, frame-local scratch space) and then discard them all at once. Arena makes this pattern zero-overhead: allocation is a pointer bump, deallocation is a single reset. No per-object free, no fragmentation, no bookkeeping.

The backing store is caller-provided (a stack array, a ByteBuf region, or any ADDRESS+size pair), so Arena works without an OS allocator.

## Types

### Arena

```modula2
TYPE Arena = RECORD
  base:      ADDRESS;
  size:      CARDINAL;
  pos:       CARDINAL;
  highwater: CARDINAL;
  failed:    CARDINAL;
  poison:    BOOLEAN;
END;
```

| Field | Purpose |
|-------|---------|
| `base` | Start of the backing store |
| `size` | Total size in bytes |
| `pos` | Current allocation cursor |
| `highwater` | Peak `pos` ever reached |
| `failed` | Count of failed allocations |
| `poison` | Whether poison patterns are active |

## Procedures

### Init

```modula2
PROCEDURE Init(VAR a: Arena; base: ADDRESS; size: CARDINAL);
```

Initialise an arena over `[base..base+size)`. Sets `pos=0`, `highwater=0`, `failed=0`, `poison=FALSE`.

### Alloc

```modula2
PROCEDURE Alloc(VAR a: Arena; n: CARDINAL; align: CARDINAL;
                VAR p: ADDRESS; VAR ok: BOOLEAN);
```

Allocate `n` bytes with `align`-byte alignment (must be power-of-two; 0 or invalid treated as 1). On success, `p` points to the allocated region and `ok=TRUE`. On failure (not enough space), `p=NIL`, `ok=FALSE`, and `failed` is incremented.

If poison is on, allocated bytes are filled with `0CDH`.

### Mark

```modula2
PROCEDURE Mark(VAR a: Arena): CARDINAL;
```

Return the current position for later use with `ResetTo`.

### ResetTo

```modula2
PROCEDURE ResetTo(VAR a: Arena; mark: CARDINAL);
```

Reset `pos` to `mark`. If `mark > pos`, this is a no-op. If poison is on, freed bytes are filled with `0`.

### Clear

```modula2
PROCEDURE Clear(VAR a: Arena);
```

Reset `pos` to 0. Equivalent to `ResetTo(a, 0)`.

### Remaining

```modula2
PROCEDURE Remaining(VAR a: Arena): CARDINAL;
```

Return `size - pos`: the number of bytes still available.

### HighWater / FailedAllocs

```modula2
PROCEDURE HighWater(VAR a: Arena): CARDINAL;
PROCEDURE FailedAllocs(VAR a: Arena): CARDINAL;
```

Diagnostic counters. HighWater is the peak `pos` ever reached; FailedAllocs is the number of times Alloc returned `ok=FALSE`.

### PoisonOn / PoisonOff

```modula2
PROCEDURE PoisonOn(VAR a: Arena);
PROCEDURE PoisonOff(VAR a: Arena);
```

Toggle poison mode. When on, Alloc fills with `0CDH` and ResetTo fills freed regions with `0`. Useful for catching use-after-free bugs.

## Example

```modula2
VAR buf: ARRAY [0..4095] OF CHAR;
    a: Arena;
    p: ADDRESS;
    ok: BOOLEAN;
    m: CARDINAL;
BEGIN
  Init(a, ADR(buf), SIZE(buf));
  m := Mark(a);
  Alloc(a, 128, 8, p, ok);   (* 128 bytes, 8-aligned *)
  (* ... use p ... *)
  ResetTo(a, m);              (* free everything since mark *)
```
