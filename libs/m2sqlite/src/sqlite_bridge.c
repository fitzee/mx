/*
 * sqlite_bridge.c — SQLite3 bridge for the m2c Modula-2 compiler.
 *
 * Provides a flat C API wrapping SQLite3 so that Modula-2 programs
 * can call database routines via FFI.  All integer arguments use int32_t;
 * all opaque handles are passed as void*.
 *
 * Compile with:
 *   cc -c sqlite_bridge.c
 * Link with:
 *   -lsqlite3
 */

#include "sqlite_bridge.h"
#include <sqlite3.h>
#include <string.h>
#include <stdint.h>

/* ── Status codes returned to Modula-2 ─────────────── *
 *
 *   0  = Ok
 *  -1  = Error
 *   1  = Row    (step returned SQLITE_ROW)
 *   2  = Done   (step returned SQLITE_DONE)
 *   3  = Busy
 *   4  = Locked
 */

/* ── Connection ────────────────────────────────────── */

int32_t m2_sqlite_open(const char *path, void **db)
{
    int rc = sqlite3_open(path, (sqlite3 **)db);
    return rc == SQLITE_OK ? 0 : -1;
}

int32_t m2_sqlite_close(void *db)
{
    int rc = sqlite3_close((sqlite3 *)db);
    return rc == SQLITE_OK ? 0 : -1;
}

int32_t m2_sqlite_exec(void *db, const char *sql)
{
    int rc = sqlite3_exec((sqlite3 *)db, sql, NULL, NULL, NULL);
    return rc == SQLITE_OK ? 0 : -1;
}

/* ── Prepared statements ───────────────────────────── */

int32_t m2_sqlite_prepare(void *db, const char *sql, void **stmt)
{
    int rc = sqlite3_prepare_v2((sqlite3 *)db, sql, -1,
                                (sqlite3_stmt **)stmt, NULL);
    return rc == SQLITE_OK ? 0 : -1;
}

int32_t m2_sqlite_finalize(void *stmt)
{
    int rc = sqlite3_finalize((sqlite3_stmt *)stmt);
    return rc == SQLITE_OK ? 0 : -1;
}

int32_t m2_sqlite_step(void *stmt)
{
    int rc = sqlite3_step((sqlite3_stmt *)stmt);
    switch (rc) {
    case SQLITE_ROW:    return 1;   /* Row     */
    case SQLITE_DONE:   return 2;   /* Done    */
    case SQLITE_BUSY:   return 3;   /* Busy    */
    case SQLITE_LOCKED: return 4;   /* Locked  */
    default:            return -1;  /* Error   */
    }
}

int32_t m2_sqlite_reset(void *stmt)
{
    int rc = sqlite3_reset((sqlite3_stmt *)stmt);
    return rc == SQLITE_OK ? 0 : -1;
}

/* ── Binding ───────────────────────────────────────── */

int32_t m2_sqlite_bind_int(void *stmt, int32_t idx, int32_t val)
{
    int rc = sqlite3_bind_int((sqlite3_stmt *)stmt, idx, val);
    return rc == SQLITE_OK ? 0 : -1;
}

int32_t m2_sqlite_bind_int64(void *stmt, int32_t idx, long val)
{
    int rc = sqlite3_bind_int64((sqlite3_stmt *)stmt, idx, (sqlite3_int64)val);
    return rc == SQLITE_OK ? 0 : -1;
}

int32_t m2_sqlite_bind_real(void *stmt, int32_t idx, double val)
{
    int rc = sqlite3_bind_double((sqlite3_stmt *)stmt, idx, val);
    return rc == SQLITE_OK ? 0 : -1;
}

int32_t m2_sqlite_bind_text(void *stmt, int32_t idx, const char *text)
{
    int rc = sqlite3_bind_text((sqlite3_stmt *)stmt, idx, text, -1,
                               SQLITE_TRANSIENT);
    return rc == SQLITE_OK ? 0 : -1;
}

int32_t m2_sqlite_bind_null(void *stmt, int32_t idx)
{
    int rc = sqlite3_bind_null((sqlite3_stmt *)stmt, idx);
    return rc == SQLITE_OK ? 0 : -1;
}

/* ── Column access ─────────────────────────────────── */

int32_t m2_sqlite_column_count(void *stmt)
{
    return (int32_t)sqlite3_column_count((sqlite3_stmt *)stmt);
}

int32_t m2_sqlite_column_type(void *stmt, int32_t idx)
{
    int t = sqlite3_column_type((sqlite3_stmt *)stmt, idx);
    switch (t) {
    case SQLITE_INTEGER: return 0;  /* IntCol   */
    case SQLITE_FLOAT:   return 1;  /* FloatCol */
    case SQLITE_TEXT:    return 2;  /* TextCol  */
    case SQLITE_BLOB:    return 3;  /* BlobCol  */
    case SQLITE_NULL:    return 4;  /* NullCol  */
    default:             return 4;  /* NullCol  */
    }
}

int32_t m2_sqlite_column_int(void *stmt, int32_t idx)
{
    return (int32_t)sqlite3_column_int((sqlite3_stmt *)stmt, idx);
}

long m2_sqlite_column_int64(void *stmt, int32_t idx)
{
    return (long)sqlite3_column_int64((sqlite3_stmt *)stmt, idx);
}

double m2_sqlite_column_real(void *stmt, int32_t idx)
{
    return sqlite3_column_double((sqlite3_stmt *)stmt, idx);
}

void m2_sqlite_column_text(void *stmt, int32_t idx, char *buf, int32_t bufLen)
{
    const char *text;
    int32_t len;

    if (bufLen <= 0) return;

    text = (const char *)sqlite3_column_text((sqlite3_stmt *)stmt, idx);
    if (text == NULL) {
        buf[0] = '\0';
        return;
    }

    len = (int32_t)strlen(text);
    if (len >= bufLen) {
        len = bufLen - 1;
    }
    memcpy(buf, text, (size_t)len);
    buf[len] = '\0';
}

/* ── Changes / clear bindings ─────────────────────── */

int32_t m2_sqlite_changes(void *db)
{
    return (int32_t)sqlite3_changes((sqlite3 *)db);
}

int32_t m2_sqlite_clear_bindings(void *stmt)
{
    int rc = sqlite3_clear_bindings((sqlite3_stmt *)stmt);
    return rc == SQLITE_OK ? 0 : -1;
}

/* ── Error reporting ───────────────────────────────── */

void m2_sqlite_errmsg(void *db, char *buf, int32_t bufLen)
{
    const char *msg;
    int32_t len;

    if (bufLen <= 0) return;

    msg = sqlite3_errmsg((sqlite3 *)db);
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
