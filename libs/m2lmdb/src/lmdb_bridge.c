/*
 * lmdb_bridge.c — LMDB bridge for the m2c Modula-2 compiler.
 *
 * Provides a flat C API wrapping liblmdb so that Modula-2 programs
 * can call LMDB routines via FFI.  All opaque handles are void*.
 * MDB_val structs are constructed in the bridge from (ptr, len) pairs
 * so Modula-2 never touches the struct directly.
 *
 * Prerequisites:
 *   macOS:  brew install lmdb
 *   Linux:  apt install liblmdb-dev  (or equivalent)
 */

#include "lmdb_bridge.h"
#include <lmdb.h>
#include <string.h>
#include <stdint.h>

/* ── Map LMDB return codes to bridge status codes ── */

static int32_t map_rc(int rc) {
    switch (rc) {
    case MDB_SUCCESS:    return 0;   /* LmOk       */
    case MDB_KEYEXIST:   return 1;   /* LmKeyExist  */
    case MDB_NOTFOUND:   return 2;   /* LmNotFound  */
    case MDB_MAP_FULL:   return 3;   /* LmMapFull   */
    case MDB_TXN_FULL:   return 4;   /* LmTxnFull   */
    default:             return -1;  /* LmError     */
    }
}

/* ── Environment ──────────────────────────────────── */

int32_t m2_lmdb_env_create(void **env) {
    return map_rc(mdb_env_create((MDB_env **)env));
}

int32_t m2_lmdb_env_open(void *env, const char *path,
                          uint32_t flags, uint32_t mode) {
    return map_rc(mdb_env_open((MDB_env *)env, path, flags, mode));
}

void m2_lmdb_env_close(void *env) {
    mdb_env_close((MDB_env *)env);
}

int32_t m2_lmdb_env_set_mapsize(void *env, uint64_t size) {
    return map_rc(mdb_env_set_mapsize((MDB_env *)env, (size_t)size));
}

int32_t m2_lmdb_env_set_maxdbs(void *env, uint32_t count) {
    return map_rc(mdb_env_set_maxdbs((MDB_env *)env, (MDB_dbi)count));
}

int32_t m2_lmdb_env_set_maxreaders(void *env, uint32_t count) {
    return map_rc(mdb_env_set_maxreaders((MDB_env *)env, count));
}

int32_t m2_lmdb_env_sync(void *env, int32_t force) {
    return map_rc(mdb_env_sync((MDB_env *)env, force));
}

/* ── Transactions ─────────────────────────────────── */

int32_t m2_lmdb_txn_begin(void *env, void *parent,
                           uint32_t flags, void **txn) {
    return map_rc(mdb_txn_begin((MDB_env *)env, (MDB_txn *)parent,
                                 flags, (MDB_txn **)txn));
}

int32_t m2_lmdb_txn_commit(void *txn) {
    return map_rc(mdb_txn_commit((MDB_txn *)txn));
}

void m2_lmdb_txn_abort(void *txn) {
    mdb_txn_abort((MDB_txn *)txn);
}

void m2_lmdb_txn_reset(void *txn) {
    mdb_txn_reset((MDB_txn *)txn);
}

int32_t m2_lmdb_txn_renew(void *txn) {
    return map_rc(mdb_txn_renew((MDB_txn *)txn));
}

/* ── Database handles ─────────────────────────────── */

int32_t m2_lmdb_dbi_open(void *txn, const char *name,
                          uint32_t flags, uint32_t *dbi) {
    return map_rc(mdb_dbi_open((MDB_txn *)txn, name, flags,
                               (MDB_dbi *)dbi));
}

/* ── Key/Value operations ─────────────────────────── */

int32_t m2_lmdb_get(void *txn, uint32_t dbi,
                     void *key, size_t klen,
                     void **val_out, size_t *vlen_out) {
    MDB_val k = { klen, key };
    MDB_val v;
    int rc = mdb_get((MDB_txn *)txn, (MDB_dbi)dbi, &k, &v);
    if (rc == 0) {
        *val_out = v.mv_data;
        *vlen_out = v.mv_size;
    }
    return map_rc(rc);
}

int32_t m2_lmdb_put(void *txn, uint32_t dbi,
                     void *key, size_t klen,
                     void *val, size_t vlen,
                     uint32_t flags) {
    MDB_val k = { klen, key };
    MDB_val v = { vlen, val };
    return map_rc(mdb_put((MDB_txn *)txn, (MDB_dbi)dbi, &k, &v, flags));
}

int32_t m2_lmdb_del(void *txn, uint32_t dbi,
                     void *key, size_t klen) {
    MDB_val k = { klen, key };
    return map_rc(mdb_del((MDB_txn *)txn, (MDB_dbi)dbi, &k, NULL));
}

/* ── Cursors ──────────────────────────────────────── */

int32_t m2_lmdb_cursor_open(void *txn, uint32_t dbi, void **cur) {
    return map_rc(mdb_cursor_open((MDB_txn *)txn, (MDB_dbi)dbi,
                                  (MDB_cursor **)cur));
}

void m2_lmdb_cursor_close(void *cur) {
    mdb_cursor_close((MDB_cursor *)cur);
}

int32_t m2_lmdb_cursor_get(void *cur, uint32_t op,
                            void **key_out, size_t *klen_out,
                            void **val_out, size_t *vlen_out) {
    MDB_val k, v;
    int rc = mdb_cursor_get((MDB_cursor *)cur, &k, &v,
                            (MDB_cursor_op)op);
    if (rc == 0) {
        *key_out  = k.mv_data;
        *klen_out = k.mv_size;
        *val_out  = v.mv_data;
        *vlen_out = v.mv_size;
    }
    return map_rc(rc);
}

int32_t m2_lmdb_cursor_seek(void *cur, uint32_t op,
                             void *key, size_t klen,
                             void **val_out, size_t *vlen_out) {
    MDB_val k = { klen, key };
    MDB_val v;
    int rc = mdb_cursor_get((MDB_cursor *)cur, &k, &v,
                            (MDB_cursor_op)op);
    if (rc == 0) {
        *val_out  = v.mv_data;
        *vlen_out = v.mv_size;
    }
    return map_rc(rc);
}

int32_t m2_lmdb_cursor_put(void *cur,
                            void *key, size_t klen,
                            void *val, size_t vlen,
                            uint32_t flags) {
    MDB_val k = { klen, key };
    MDB_val v = { vlen, val };
    return map_rc(mdb_cursor_put((MDB_cursor *)cur, &k, &v, flags));
}

/* ── Error reporting ──────────────────────────────── */

void m2_lmdb_errmsg(int32_t code, char *buf, int32_t bufLen) {
    const char *msg;
    int32_t len;

    if (bufLen <= 0) return;

    /* Map our bridge codes back to LMDB codes for mdb_strerror */
    int mdb_code;
    switch (code) {
    case 0:  mdb_code = MDB_SUCCESS;  break;
    case 1:  mdb_code = MDB_KEYEXIST; break;
    case 2:  mdb_code = MDB_NOTFOUND; break;
    case 3:  mdb_code = MDB_MAP_FULL; break;
    case 4:  mdb_code = MDB_TXN_FULL; break;
    default: mdb_code = code;         break;
    }

    msg = mdb_strerror(mdb_code);
    if (msg == NULL) {
        buf[0] = '\0';
        return;
    }

    len = (int32_t)strlen(msg);
    if (len >= bufLen) {
        len = bufLen - 1;
    }
    memcpy(buf, msg, (size_t)len);
    buf[len] = '\0';
}
