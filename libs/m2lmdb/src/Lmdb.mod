IMPLEMENTATION MODULE Lmdb;
(* LMDB high-level interface wrapping lmdb_bridge.c.
   Maps INTEGER bridge return codes to Status enum values.
   Handles ARRAY OF CHAR → ADDRESS conversion for paths and names. *)

FROM SYSTEM IMPORT ADDRESS, ADR;
FROM LmdbBridge IMPORT
  m2_lmdb_env_create, m2_lmdb_env_open, m2_lmdb_env_close,
  m2_lmdb_env_set_mapsize, m2_lmdb_env_set_maxdbs,
  m2_lmdb_env_set_maxreaders, m2_lmdb_env_sync,
  m2_lmdb_txn_begin, m2_lmdb_txn_commit, m2_lmdb_txn_abort,
  m2_lmdb_txn_reset, m2_lmdb_txn_renew,
  m2_lmdb_dbi_open,
  m2_lmdb_get, m2_lmdb_put, m2_lmdb_del,
  m2_lmdb_cursor_open, m2_lmdb_cursor_close,
  m2_lmdb_cursor_get, m2_lmdb_cursor_seek, m2_lmdb_cursor_put,
  m2_lmdb_errmsg;

(* ── Status mapping ──────────────────────────────── *)

PROCEDURE MapStatus(rc: INTEGER): Status;
BEGIN
  IF    rc = 0  THEN RETURN LmOk
  ELSIF rc = 1  THEN RETURN LmKeyExist
  ELSIF rc = 2  THEN RETURN LmNotFound
  ELSIF rc = 3  THEN RETURN LmMapFull
  ELSIF rc = 4  THEN RETURN LmTxnFull
  ELSE               RETURN LmError
  END
END MapStatus;

(* ── String copy helper ──────────────────────────── *)

(* CopyStr copies src to dst with NUL termination.
   dst must be at least 512 bytes. *)
PROCEDURE CopyStr(src: ARRAY OF CHAR; VAR dst: ARRAY OF CHAR);
VAR i: CARDINAL;
BEGIN
  i := 0;
  WHILE (i <= HIGH(src)) AND (src[i] # 0C) AND (i < HIGH(dst)) DO
    dst[i] := src[i];
    INC(i)
  END;
  dst[i] := 0C
END CopyStr;

(* ── Environment ────────────────────────────────── *)

PROCEDURE EnvCreate(VAR env: Env): Status;
VAR rc: INTEGER;
BEGIN
  rc := m2_lmdb_env_create(env);
  RETURN MapStatus(rc)
END EnvCreate;

PROCEDURE EnvOpen(env: Env; path: ARRAY OF CHAR;
                  flags: CARDINAL; mode: CARDINAL): Status;
VAR
  rc:   INTEGER;
  buf:  ARRAY [0..511] OF CHAR;
BEGIN
  CopyStr(path, buf);
  rc := m2_lmdb_env_open(env, ADR(buf), flags, mode);
  RETURN MapStatus(rc)
END EnvOpen;

PROCEDURE EnvClose(env: Env);
BEGIN
  m2_lmdb_env_close(env)
END EnvClose;

PROCEDURE EnvSetMapSize(env: Env; sizeBytes: LONGCARD): Status;
VAR rc: INTEGER;
BEGIN
  rc := m2_lmdb_env_set_mapsize(env, sizeBytes);
  RETURN MapStatus(rc)
END EnvSetMapSize;

PROCEDURE EnvSetMaxDbs(env: Env; count: CARDINAL): Status;
VAR rc: INTEGER;
BEGIN
  rc := m2_lmdb_env_set_maxdbs(env, count);
  RETURN MapStatus(rc)
END EnvSetMaxDbs;

PROCEDURE EnvSetMaxReaders(env: Env; count: CARDINAL): Status;
VAR rc: INTEGER;
BEGIN
  rc := m2_lmdb_env_set_maxreaders(env, count);
  RETURN MapStatus(rc)
END EnvSetMaxReaders;

PROCEDURE EnvSync(env: Env; force: BOOLEAN): Status;
VAR rc: INTEGER; f: INTEGER;
BEGIN
  IF force THEN f := 1 ELSE f := 0 END;
  rc := m2_lmdb_env_sync(env, f);
  RETURN MapStatus(rc)
END EnvSync;

(* ── Transactions ───────────────────────────────── *)

PROCEDURE TxnBegin(env: Env; parent: Txn; flags: CARDINAL;
                   VAR txn: Txn): Status;
VAR rc: INTEGER;
BEGIN
  rc := m2_lmdb_txn_begin(env, parent, flags, txn);
  RETURN MapStatus(rc)
END TxnBegin;

PROCEDURE TxnCommit(txn: Txn): Status;
VAR rc: INTEGER;
BEGIN
  rc := m2_lmdb_txn_commit(txn);
  RETURN MapStatus(rc)
END TxnCommit;

PROCEDURE TxnAbort(txn: Txn);
BEGIN
  m2_lmdb_txn_abort(txn)
END TxnAbort;

PROCEDURE TxnReset(txn: Txn);
BEGIN
  m2_lmdb_txn_reset(txn)
END TxnReset;

PROCEDURE TxnRenew(txn: Txn): Status;
VAR rc: INTEGER;
BEGIN
  rc := m2_lmdb_txn_renew(txn);
  RETURN MapStatus(rc)
END TxnRenew;

(* ── Database handles ───────────────────────────── *)

PROCEDURE DbiOpen(txn: Txn; name: ARRAY OF CHAR;
                  flags: CARDINAL; VAR dbi: Dbi): Status;
VAR
  rc:  INTEGER;
  buf: ARRAY [0..127] OF CHAR;
BEGIN
  CopyStr(name, buf);
  rc := m2_lmdb_dbi_open(txn, ADR(buf), flags, dbi);
  RETURN MapStatus(rc)
END DbiOpen;

(* ── Key/Value operations ───────────────────────── *)

PROCEDURE Get(txn: Txn; dbi: Dbi;
              key: ADDRESS; keyLen: LONGCARD;
              VAR valPtr: ADDRESS; VAR valLen: LONGCARD): Status;
VAR rc: INTEGER;
BEGIN
  rc := m2_lmdb_get(txn, dbi, key, keyLen, valPtr, valLen);
  RETURN MapStatus(rc)
END Get;

PROCEDURE Put(txn: Txn; dbi: Dbi;
              key: ADDRESS; keyLen: LONGCARD;
              val: ADDRESS; valLen: LONGCARD;
              flags: CARDINAL): Status;
VAR rc: INTEGER;
BEGIN
  rc := m2_lmdb_put(txn, dbi, key, keyLen, val, valLen, flags);
  RETURN MapStatus(rc)
END Put;

PROCEDURE Del(txn: Txn; dbi: Dbi;
              key: ADDRESS; keyLen: LONGCARD): Status;
VAR rc: INTEGER;
BEGIN
  rc := m2_lmdb_del(txn, dbi, key, keyLen);
  RETURN MapStatus(rc)
END Del;

(* ── Cursors ────────────────────────────────────── *)

PROCEDURE CursorOpen(txn: Txn; dbi: Dbi;
                     VAR cur: Cursor): Status;
VAR rc: INTEGER;
BEGIN
  rc := m2_lmdb_cursor_open(txn, dbi, cur);
  RETURN MapStatus(rc)
END CursorOpen;

PROCEDURE CursorClose(cur: Cursor);
BEGIN
  m2_lmdb_cursor_close(cur)
END CursorClose;

PROCEDURE CursorGet(cur: Cursor; op: CARDINAL;
                    VAR keyPtr: ADDRESS; VAR keyLen: LONGCARD;
                    VAR valPtr: ADDRESS; VAR valLen: LONGCARD): Status;
VAR rc: INTEGER;
BEGIN
  rc := m2_lmdb_cursor_get(cur, op, keyPtr, keyLen, valPtr, valLen);
  RETURN MapStatus(rc)
END CursorGet;

PROCEDURE CursorSeek(cur: Cursor; op: CARDINAL;
                     key: ADDRESS; keyLen: LONGCARD;
                     VAR valPtr: ADDRESS; VAR valLen: LONGCARD): Status;
VAR rc: INTEGER;
BEGIN
  rc := m2_lmdb_cursor_seek(cur, op, key, keyLen, valPtr, valLen);
  RETURN MapStatus(rc)
END CursorSeek;

PROCEDURE CursorPut(cur: Cursor;
                    key: ADDRESS; keyLen: LONGCARD;
                    val: ADDRESS; valLen: LONGCARD;
                    flags: CARDINAL): Status;
VAR rc: INTEGER;
BEGIN
  rc := m2_lmdb_cursor_put(cur, key, keyLen, val, valLen, flags);
  RETURN MapStatus(rc)
END CursorPut;

(* ── Error reporting ────────────────────────────── *)

PROCEDURE ErrMsg(code: INTEGER; VAR buf: ARRAY OF CHAR);
BEGIN
  m2_lmdb_errmsg(code, ADR(buf), VAL(INTEGER, HIGH(buf) + 1))
END ErrMsg;

END Lmdb.
