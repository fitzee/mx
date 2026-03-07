#ifndef SQLITE_BRIDGE_H
#define SQLITE_BRIDGE_H

#include <stdint.h>

/* ── Connection ────────────────────────────────────── */

int32_t m2_sqlite_open(const char *path, void **db);
int32_t m2_sqlite_close(void *db);
int32_t m2_sqlite_exec(void *db, const char *sql);

/* ── Prepared statements ───────────────────────────── */

int32_t m2_sqlite_prepare(void *db, const char *sql, void **stmt);
int32_t m2_sqlite_finalize(void *stmt);
int32_t m2_sqlite_step(void *stmt);
int32_t m2_sqlite_reset(void *stmt);

/* ── Binding ───────────────────────────────────────── */

int32_t m2_sqlite_bind_int(void *stmt, int32_t idx, int32_t val);
int32_t m2_sqlite_bind_int64(void *stmt, int32_t idx, long val);
int32_t m2_sqlite_bind_real(void *stmt, int32_t idx, double val);
int32_t m2_sqlite_bind_text(void *stmt, int32_t idx, const char *text);
int32_t m2_sqlite_bind_null(void *stmt, int32_t idx);

/* ── Column access ─────────────────────────────────── */

int32_t m2_sqlite_column_count(void *stmt);
int32_t m2_sqlite_column_type(void *stmt, int32_t idx);
int32_t m2_sqlite_column_int(void *stmt, int32_t idx);
long    m2_sqlite_column_int64(void *stmt, int32_t idx);
double  m2_sqlite_column_real(void *stmt, int32_t idx);
void    m2_sqlite_column_text(void *stmt, int32_t idx, char *buf, int32_t bufLen);

/* ── Changes / clear bindings ─────────────────────── */

int32_t m2_sqlite_changes(void *db);
int32_t m2_sqlite_clear_bindings(void *stmt);

/* ── Error reporting ───────────────────────────────── */

void m2_sqlite_errmsg(void *db, char *buf, int32_t bufLen);

#endif /* SQLITE_BRIDGE_H */
