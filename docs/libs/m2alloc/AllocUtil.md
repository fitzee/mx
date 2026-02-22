# AllocUtil

Shared helpers for pointer arithmetic, alignment, and byte access. All procedures are pure functions or simple memory read/write operations with no state or side effects.

## Why AllocUtil?

Arena and Pool both need pointer math, alignment rounding, and byte-level memory access. AllocUtil factors these out so both allocators share one tested implementation. The `BytePtr` overlay pattern (casting ADDRESS to a pointer to a large CHAR array) is proven in ByteBuf and RpcFrame.

## Types

### ByteArray / BytePtr

```modula2
TYPE
  ByteArray = ARRAY [0..65535] OF CHAR;
  BytePtr = POINTER TO ByteArray;
```

Overlay for indexed byte access on any ADDRESS. Cast an ADDRESS to BytePtr, then use `bp^[i]` to read or write individual bytes. Values are CHAR (0..255 via ORD/CHR).

### AddrPtr

```modula2
TYPE AddrPtr = POINTER TO ADDRESS;
```

Overlay for reading/writing an ADDRESS-sized value at any memory location. Used by Pool to store intrusive free-list next pointers.

## Procedures

### IsPowerOfTwo

```modula2
PROCEDURE IsPowerOfTwo(x: CARDINAL): BOOLEAN;
```

Returns TRUE if x is a power of two (1, 2, 4, 8, ...). Returns FALSE for 0.

### AlignUp

```modula2
PROCEDURE AlignUp(x, align: CARDINAL): CARDINAL;
```

Round x up to the next multiple of align. Align must be a power of two and greater than 0; otherwise x is returned unchanged. Uses integer arithmetic: `((x + align - 1) DIV align) * align`.

### PtrAdd

```modula2
PROCEDURE PtrAdd(base: ADDRESS; offset: CARDINAL): ADDRESS;
```

Return `base + offset` as an ADDRESS. Uses `VAL(ADDRESS, VAL(LONGINT, base) + VAL(LONGINT, offset))` for correct C codegen.

### PtrDiff

```modula2
PROCEDURE PtrDiff(a, b: ADDRESS): CARDINAL;
```

Return `a - b` as a CARDINAL. Returns 0 if `b >= a`.

### FillBytes

```modula2
PROCEDURE FillBytes(base: ADDRESS; count: CARDINAL; val: CARDINAL);
```

Fill `count` bytes starting at `base` with `val` (masked to 0..255). Uses BytePtr for indexed access.

### ReadAddr

```modula2
PROCEDURE ReadAddr(loc: ADDRESS): ADDRESS;
```

Read an ADDRESS from the memory location `loc` via AddrPtr cast.

### WriteAddr

```modula2
PROCEDURE WriteAddr(loc: ADDRESS; val: ADDRESS);
```

Write `val` as an ADDRESS at the memory location `loc` via AddrPtr cast.
