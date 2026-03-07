# HashMap

## Why
Provides a static, open-addressing hash table using FNV-1a hashing and linear probing, operating entirely on caller-provided arrays with no heap allocation.

## Types

- **Bucket** -- A single table entry containing:
  - `key: ARRAY [0..MaxKeyLen] OF CHAR` -- The key string.
  - `val: INTEGER` -- The associated value.
  - `occupied: BOOLEAN` -- Whether this bucket holds a live entry.
  - `deleted: BOOLEAN` -- Whether this bucket is a tombstone (removed entry).

- **Map** -- The hash table handle containing:
  - `base: ADDRESS` -- Pointer to the caller-provided Bucket array.
  - `cap: CARDINAL` -- Total capacity (number of Bucket slots).
  - `count: CARDINAL` -- Number of live entries.

## Constants

- `MaxKeyLen = 63` -- Maximum length of a key string (excluding NUL terminator).

## Procedures

- `PROCEDURE Init(VAR m: Map; buckets: ADDRESS; cap: CARDINAL)`
  Initialise a map over a caller-provided array of `cap` Bucket records. Clears all buckets.

- `PROCEDURE Clear(VAR m: Map)`
  Remove all entries and reset count to 0.

- `PROCEDURE Put(VAR m: Map; key: ARRAY OF CHAR; val: INTEGER): BOOLEAN`
  Insert or update a key-value pair. Returns TRUE on success, FALSE if the table is full.

- `PROCEDURE Get(VAR m: Map; key: ARRAY OF CHAR; VAR val: INTEGER): BOOLEAN`
  Look up a key. Sets val and returns TRUE if found, FALSE otherwise.

- `PROCEDURE Contains(VAR m: Map; key: ARRAY OF CHAR): BOOLEAN`
  Returns TRUE if the key is present in the map.

- `PROCEDURE Remove(VAR m: Map; key: ARRAY OF CHAR): BOOLEAN`
  Remove a key. Returns TRUE if found and removed, FALSE otherwise.

- `PROCEDURE Count(VAR m: Map): CARDINAL`
  Returns the number of live entries in the map.

- `PROCEDURE Hash(key: ARRAY OF CHAR): CARDINAL`
  Compute the FNV-1a hash of a string. Useful for manual bucket inspection or secondary hashing.

## Example

```modula2
MODULE HashMapExample;

FROM SYSTEM IMPORT ADR;
FROM HashMap IMPORT Map, Bucket, Init, Put, Get, Contains, Remove, Count;

CONST
  TableSize = 128;

VAR
  m: Map;
  buckets: ARRAY [0..TableSize-1] OF Bucket;
  val: INTEGER;

BEGIN
  Init(m, ADR(buckets), TableSize);

  (* Insert entries *)
  IF Put(m, "width", 800) THEN END;
  IF Put(m, "height", 600) THEN END;
  IF Put(m, "depth", 32) THEN END;

  (* Lookup *)
  IF Get(m, "width", val) THEN
    (* val = 800 *)
  END;

  (* Check existence *)
  IF Contains(m, "height") THEN
    (* present *)
  END;

  (* Update an existing key *)
  IF Put(m, "width", 1024) THEN END;

  (* Remove *)
  IF Remove(m, "depth") THEN END;

  (* Count returns 2 (width + height) *)
END HashMapExample.
```
