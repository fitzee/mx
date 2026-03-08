#ifndef LMDB_BRIDGE_H
#define LMDB_BRIDGE_H

#include <stdint.h>
#include <stddef.h>

/* ── Status codes returned to Modula-2 ───────────────
 *   0 = Ok
 *   1 = KeyExist (MDB_KEYEXIST)
 *   2 = NotFound (MDB_NOTFOUND)
 *   3 = MapFull  (MDB_MAP_FULL)
 *   4 = TxnFull  (MDB_TXN_FULL)
 *  -1 = Error    (any other LMDB error)
 */

/* ── Environment ──────────────────────────────────── */

int32_t m2_lmdb_env_create(void **env);
int32_t m2_lmdb_env_open(void *env, const char *path,
                          uint32_t flags, uint32_t mode);
void    m2_lmdb_env_close(void *env);
int32_t m2_lmdb_env_set_mapsize(void *env, uint64_t size);
int32_t m2_lmdb_env_set_maxdbs(void *env, uint32_t count);
int32_t m2_lmdb_env_set_maxreaders(void *env, uint32_t count);
int32_t m2_lmdb_env_sync(void *env, int32_t force);

/* ── Transactions ─────────────────────────────────── */

int32_t m2_lmdb_txn_begin(void *env, void *parent,
                           uint32_t flags, void **txn);
int32_t m2_lmdb_txn_commit(void *txn);
void    m2_lmdb_txn_abort(void *txn);
void    m2_lmdb_txn_reset(void *txn);
int32_t m2_lmdb_txn_renew(void *txn);

/* ── Database handles ─────────────────────────────── */

int32_t m2_lmdb_dbi_open(void *txn, const char *name,
                          uint32_t flags, uint32_t *dbi);

/* ── Key/Value operations ─────────────────────────── */

int32_t m2_lmdb_get(void *txn, uint32_t dbi,
                     void *key, size_t klen,
                     void **val_out, size_t *vlen_out);

int32_t m2_lmdb_put(void *txn, uint32_t dbi,
                     void *key, size_t klen,
                     void *val, size_t vlen,
                     uint32_t flags);

int32_t m2_lmdb_del(void *txn, uint32_t dbi,
                     void *key, size_t klen);

/* ── Cursors ──────────────────────────────────────── */

int32_t m2_lmdb_cursor_open(void *txn, uint32_t dbi, void **cur);
void    m2_lmdb_cursor_close(void *cur);

int32_t m2_lmdb_cursor_get(void *cur, uint32_t op,
                            void **key_out, size_t *klen_out,
                            void **val_out, size_t *vlen_out);

int32_t m2_lmdb_cursor_seek(void *cur, uint32_t op,
                             void *key, size_t klen,
                             void **val_out, size_t *vlen_out);

int32_t m2_lmdb_cursor_put(void *cur,
                            void *key, size_t klen,
                            void *val, size_t vlen,
                            uint32_t flags);

/* ── Error reporting ──────────────────────────────── */

void m2_lmdb_errmsg(int32_t code, char *buf, int32_t bufLen);

#endif /* LMDB_BRIDGE_H */
