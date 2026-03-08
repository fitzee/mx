# Lmdb

## Why
Provides a key/value store with MVCC concurrency: readers never block writers, and reads are zero-copy via memory-mapped B+ trees. All opaque handles are ADDRESS-typed so Modula-2 never touches LMDB's internal structs. The C bridge constructs `MDB_val` from `(ADDRESS, LONGCARD)` pairs, keeping the FFI boundary clean. Requires `liblmdb` at link time (`-llmdb`).

## Prerequisites

- **macOS**: `brew install lmdb`
- **Linux**: `apt install liblmdb-dev` (or equivalent)

## Types

- **Env** (ADDRESS) -- Opaque environment handle (`MDB_env*`). One per data directory.
- **Txn** (ADDRESS) -- Opaque transaction handle (`MDB_txn*`). Read-only or read-write.
- **Dbi** (CARDINAL) -- Named database handle within an environment.
- **Cursor** (ADDRESS) -- Opaque cursor handle (`MDB_cursor*`) for ordered iteration.
- **Status** -- `LmOk`, `LmKeyExist`, `LmNotFound`, `LmMapFull`, `LmTxnFull`, `LmError`.

## Constants

### Open Flags

| Constant | Value | Description |
|----------|-------|-------------|
| `FlagCreate` | 040000H | Create named database if it doesn't exist |
| `FlagRdOnly` | 020000H | Read-only transaction |
| `FlagNoTls` | 0200000H | Don't use thread-local reader slots |
| `FlagNoOverwrite` | 010H | Don't overwrite existing key (returns `LmKeyExist`) |
| `FlagNoSubDir` | 04000H | Path is a file, not a directory |
| `FlagNoMetaSync` | 040000H | Skip meta-page fsync on commit |
| `FlagNoSync` | 010000H | No fsync at all -- call `EnvSync` manually |
| `FlagWriteMap` | 080000H | Use writable mmap instead of `write()` |
| `FlagMapAsync` | 0100000H | Async msync with `FlagWriteMap` |

### Cursor Operations

| Constant | Value | Description |
|----------|-------|-------------|
| `CurFirst` | 0 | Position at first key |
| `CurGetCurrent` | 4 | Return current key/value without moving |
| `CurLast` | 6 | Position at last key |
| `CurNext` | 8 | Move to next key |
| `CurNextDup` | 9 | Move to next duplicate of current key |
| `CurNextNoDup` | 11 | Move to first entry of next key |
| `CurPrev` | 12 | Move to previous key |
| `CurPrevNoDup` | 14 | Move to last entry of previous key |
| `CurSet` | 15 | Position at exact key (returns `LmNotFound` if absent) |
| `CurSetRange` | 17 | Position at key >= given key (prefix scanning) |

## Procedures

### Environment

- `PROCEDURE EnvCreate(VAR env: Env): Status`
  Allocate a new environment handle. Must be configured (map size, max DBs) before opening.

- `PROCEDURE EnvOpen(env: Env; path: ARRAY OF CHAR; flags: CARDINAL; mode: CARDINAL): Status`
  Open the environment at the given directory path. `mode` is the Unix file permission (e.g. 0644). Common flags: `FlagNoSync` for high-throughput writes with manual `EnvSync`.

- `PROCEDURE EnvClose(env: Env)`
  Close the environment and release resources.

- `PROCEDURE EnvSetMapSize(env: Env; sizeBytes: LONGCARD): Status`
  Set the memory map size in bytes. Must be called before `EnvOpen`. This is the maximum database size.

- `PROCEDURE EnvSetMaxDbs(env: Env; count: CARDINAL): Status`
  Set the maximum number of named databases. Must be called before `EnvOpen`.

- `PROCEDURE EnvSetMaxReaders(env: Env; count: CARDINAL): Status`
  Set the maximum number of concurrent reader slots. Must be called before `EnvOpen`.

- `PROCEDURE EnvSync(env: Env; force: BOOLEAN): Status`
  Flush dirty pages to disk. Required when using `FlagNoSync` to ensure durability. With `force = TRUE`, a synchronous flush is performed.

### Transactions

- `PROCEDURE TxnBegin(env: Env; parent: Txn; flags: CARDINAL; VAR txn: Txn): Status`
  Begin a transaction. Pass `NIL` for `parent` for a top-level transaction. Use `FlagRdOnly` for read-only transactions (concurrent, lock-free). Only one write transaction may be active at a time.

- `PROCEDURE TxnCommit(txn: Txn): Status`
  Commit the transaction. The `txn` handle is invalidated.

- `PROCEDURE TxnAbort(txn: Txn)`
  Abort the transaction. The `txn` handle is invalidated.

- `PROCEDURE TxnReset(txn: Txn)`
  Reset a read-only transaction for reuse. Releases read locks but keeps the handle.

- `PROCEDURE TxnRenew(txn: Txn): Status`
  Renew a read-only transaction after `TxnReset`. Cheaper than `TxnBegin`.

### Database Handles

- `PROCEDURE DbiOpen(txn: Txn; name: ARRAY OF CHAR; flags: CARDINAL; VAR dbi: Dbi): Status`
  Open a named database within the environment. Use `FlagCreate` to create it if it doesn't exist. The returned `dbi` handle is valid for the lifetime of the environment.

### Key/Value Operations

- `PROCEDURE Get(txn: Txn; dbi: Dbi; key: ADDRESS; keyLen: LONGCARD; VAR valPtr: ADDRESS; VAR valLen: LONGCARD): Status`
  Retrieve a value by key. On success, `valPtr` points directly into the memory map -- valid until the transaction ends. Do not free it.

- `PROCEDURE Put(txn: Txn; dbi: Dbi; key: ADDRESS; keyLen: LONGCARD; val: ADDRESS; valLen: LONGCARD; flags: CARDINAL): Status`
  Store a key/value pair. With `FlagNoOverwrite`, returns `LmKeyExist` if the key already exists (useful for idempotent inserts).

- `PROCEDURE Del(txn: Txn; dbi: Dbi; key: ADDRESS; keyLen: LONGCARD): Status`
  Delete a key. Returns `LmNotFound` if the key doesn't exist.

### Cursors

- `PROCEDURE CursorOpen(txn: Txn; dbi: Dbi; VAR cur: Cursor): Status`
  Open a cursor on a database for ordered iteration.

- `PROCEDURE CursorClose(cur: Cursor)`
  Close the cursor.

- `PROCEDURE CursorGet(cur: Cursor; op: CARDINAL; VAR keyPtr: ADDRESS; VAR keyLen: LONGCARD; VAR valPtr: ADDRESS; VAR valLen: LONGCARD): Status`
  Position the cursor using `op` and return the key and value at the new position. Returns `LmNotFound` when no more entries.

- `PROCEDURE CursorSeek(cur: Cursor; op: CARDINAL; key: ADDRESS; keyLen: LONGCARD; VAR valPtr: ADDRESS; VAR valLen: LONGCARD): Status`
  Position the cursor at or after the given key. Commonly used with `CurSetRange` for prefix scanning. Returns the value at the positioned entry.

- `PROCEDURE CursorPut(cur: Cursor; key: ADDRESS; keyLen: LONGCARD; val: ADDRESS; valLen: LONGCARD; flags: CARDINAL): Status`
  Write a key/value pair via the cursor. Same flags as `Put`.

### Error Reporting

- `PROCEDURE ErrMsg(code: INTEGER; VAR buf: ARRAY OF CHAR)`
  Write a human-readable error description into `buf`.

## Example

```modula2
MODULE LmdbDemo;

FROM SYSTEM IMPORT ADDRESS, ADR, TSIZE;
FROM InOut IMPORT WriteString, WriteLn;
FROM Lmdb IMPORT
  Env, Txn, Dbi, Cursor, Status,
  EnvCreate, EnvOpen, EnvClose, EnvSetMapSize, EnvSetMaxDbs,
  TxnBegin, TxnCommit, TxnAbort,
  DbiOpen, Get, Put, CursorOpen, CursorClose, CursorGet,
  LmOk, LmNotFound, FlagCreate, FlagRdOnly, FlagNoOverwrite,
  CurFirst, CurNext;

VAR
  env: Env;
  txn: Txn;
  dbi: Dbi;
  cur: Cursor;
  s:   Status;
  valPtr, keyPtr: ADDRESS;
  valLen, keyLen: LONGCARD;
  key: ARRAY [0..31] OF CHAR;
  val: ARRAY [0..63] OF CHAR;
  buf: ARRAY [0..63] OF CHAR;

BEGIN
  (* Create and open environment *)
  s := EnvCreate(env);
  s := EnvSetMapSize(env, 10485760);  (* 10 MB *)
  s := EnvSetMaxDbs(env, 4);
  s := EnvOpen(env, "./testdb", 0, 644);

  (* Write some key/value pairs *)
  s := TxnBegin(env, NIL, 0, txn);
  s := DbiOpen(txn, "users", FlagCreate, dbi);

  key := "alice";
  val := "Alice Smith";
  s := Put(txn, dbi, ADR(key), 6, ADR(val), 12, 0);

  key := "bob";
  val := "Bob Jones";
  s := Put(txn, dbi, ADR(key), 4, ADR(val), 10, 0);

  s := TxnCommit(txn);

  (* Read back a single key *)
  s := TxnBegin(env, NIL, FlagRdOnly, txn);
  key := "alice";
  s := Get(txn, dbi, ADR(key), 6, valPtr, valLen);
  IF s = LmOk THEN
    WriteString("alice = ");
    (* valPtr points into the mmap -- valid until TxnAbort *)
    WriteLn
  END;

  (* Iterate all keys with a cursor *)
  s := CursorOpen(txn, dbi, cur);
  s := CursorGet(cur, CurFirst, keyPtr, keyLen, valPtr, valLen);
  WHILE s = LmOk DO
    WriteString("key found"); WriteLn;
    s := CursorGet(cur, CurNext, keyPtr, keyLen, valPtr, valLen)
  END;
  CursorClose(cur);
  TxnAbort(txn);

  EnvClose(env)
END LmdbDemo.
```

## Concurrency Model

LMDB uses MVCC (Multi-Version Concurrency Control):

- **Multiple concurrent readers** -- read-only transactions never block each other or writers. No locks, no contention.
- **Single writer** -- only one write transaction may be active at a time. A second `TxnBegin` without `FlagRdOnly` will block until the first commits or aborts.
- **Readers don't block writers** -- a write transaction can commit while read transactions are active. Readers see a consistent snapshot from when their transaction began.

This makes LMDB ideal for multi-threaded servers: worker threads acquire read-only transactions freely, while writes are serialized through a single writer (or a batching layer).

## Key Design Notes

- **Zero-copy reads**: `Get` and `CursorGet` return pointers directly into the memory-mapped file. No data is copied. The pointer is valid until the transaction ends.
- **Sorted keys**: Keys are stored in lexicographic byte order. Use big-endian encoding for integer keys to maintain correct sort order.
- **Named databases**: A single environment can hold multiple named databases (up to `EnvSetMaxDbs`). Each is an independent B+ tree with its own key space.
- **`FlagNoOverwrite`**: Returns `LmKeyExist` instead of overwriting. This is the LMDB equivalent of SQL's `INSERT OR IGNORE` -- check the return code to detect duplicates without a separate existence check.
- **`FlagNoSync`**: Disables fsync on commit for maximum write throughput. Call `EnvSync` on a timer to control the durability/performance tradeoff.

## Architecture

```
Lmdb.def          High-level Modula-2 API (types, procedures)
  |
Lmdb.mod          Maps Status codes, handles ARRAY OF CHAR → ADDRESS
  |
LmdbBridge.def    FOR "C" declarations (flat ADDRESS-based FFI)
  |
lmdb_bridge.c     C wrapper: constructs MDB_val structs, maps return codes
  |
liblmdb           System LMDB library (memory-mapped B+ tree engine)
```

The bridge layer exists because LMDB's `MDB_val` is a C struct (`{size_t mv_size; void *mv_data}`) that cannot be directly constructed from Modula-2. The bridge takes `(ADDRESS, LONGCARD)` pairs and builds the struct in C.
