# HashMap

Static hash table with FNV-1a hashing, open addressing, and linear probing. No heap allocation -- all storage is caller-provided as a flat array of Bucket records.

## Why HashMap?

Vocabulary lookup, symbol tables, and frequency counters all need fast key-value access. HashMap provides O(1) average-case insert and lookup on a fixed-capacity table without touching the heap. The caller allocates the backing array on the stack or in an arena, passes it to `Init`, and the table is ready.

The FNV-1a hash function is fast and produces good distribution for short string keys typical in source code analysis and language classification workloads.

## Types

### Bucket

```modula2
CONST MaxKeyLen = 63;

TYPE Bucket = RECORD
  key: ARRAY [0..MaxKeyLen] OF CHAR;
  val: INTEGER;
  occupied: BOOLEAN;
  deleted: BOOLEAN;
END;
```

One slot in the hash table. Keys are NUL-terminated strings up to 63 characters. The `deleted` flag marks tombstone slots left by `Remove`, which are reclaimed by subsequent `Put` calls.

### Map

```modula2
TYPE Map = RECORD
  base: ADDRESS;
  cap: CARDINAL;
  count: CARDINAL;
END;
```

Table handle. `base` points to the caller's Bucket array, `cap` is the number of Bucket slots, and `count` tracks live entries.

## Procedures

### Init

```modula2
PROCEDURE Init(VAR m: Map; buckets: ADDRESS; cap: CARDINAL);
```

Initialise a map over a caller-provided array of `cap` Bucket records. Clears all buckets and sets count to 0. Pass `ADR(myBuckets)` for the `buckets` parameter.

### Clear

```modula2
PROCEDURE Clear(VAR m: Map);
```

Remove all entries. Resets every bucket and sets count to 0.

### Put

```modula2
PROCEDURE Put(VAR m: Map; key: ARRAY OF CHAR; val: INTEGER): BOOLEAN;
```

Insert or update `key` with `val`. If `key` already exists, its value is overwritten and count is unchanged. Returns TRUE on success, FALSE if the table is full and no tombstone slot is available.

### Get

```modula2
PROCEDURE Get(VAR m: Map; key: ARRAY OF CHAR; VAR val: INTEGER): BOOLEAN;
```

Look up `key`. If found, sets `val` and returns TRUE. Returns FALSE if the key is not present.

### Contains

```modula2
PROCEDURE Contains(VAR m: Map; key: ARRAY OF CHAR): BOOLEAN;
```

Returns TRUE if `key` is present in the map.

### Remove

```modula2
PROCEDURE Remove(VAR m: Map; key: ARRAY OF CHAR): BOOLEAN;
```

Remove `key` by marking its bucket as a tombstone (`deleted=TRUE`). Returns TRUE if the key was found and removed, FALSE otherwise. Tombstone slots are reused by subsequent `Put` calls.

### Count

```modula2
PROCEDURE Count(VAR m: Map): CARDINAL;
```

Returns the number of live entries in the map.

### Hash

```modula2
PROCEDURE Hash(key: ARRAY OF CHAR): CARDINAL;
```

FNV-1a hash of a NUL-terminated or open-array string. Uses offset basis 2166136261 and prime 16777619 with XOR-then-multiply, masked to 32 bits. Deterministic and suitable for use outside the map (e.g., pre-hashing, sharding).

## Collision Resolution

The table uses linear probing. On collision, slots are scanned sequentially (wrapping at `cap`) until an empty or matching slot is found. Tombstones from `Remove` are skipped during lookup but reused during insertion, preventing probe chain breakage.

For best performance, keep the load factor below 75% (e.g., use a 128-slot table for up to 96 entries).

## Example

```modula2
FROM SYSTEM IMPORT ADR;
FROM HashMap IMPORT Bucket, Map, Init, Put, Get, Count;

VAR
  m: Map;
  buckets: ARRAY [0..63] OF Bucket;
  v: INTEGER;

Init(m, ADR(buckets), 64);
Put(m, "function", 1);
Put(m, "class", 2);
Put(m, "import", 3);

IF Get(m, "class", v) THEN
  (* v = 2 *)
END;
(* Count(m) = 3 *)
```
