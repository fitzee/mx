#ifndef METRICS_BRIDGE_H
#define METRICS_BRIDGE_H

#include <stdint.h>

/*
 * System metrics snapshot — 9 int64_t fields.
 *
 * load_avg fields are scaled ×1000 (e.g. 1.50 → 1500).
 * Memory fields are in megabytes.
 * CPU times are in microseconds.
 * RSS fields are in kilobytes.
 */
typedef struct {
    int64_t load_1;       /* 1-min load avg × 1000  */
    int64_t load_5;       /* 5-min load avg × 1000  */
    int64_t load_15;      /* 15-min load avg × 1000 */
    int64_t mem_total_mb; /* total physical memory   */
    int64_t mem_free_mb;  /* free physical memory    */
    int64_t cpu_user_us;  /* user CPU time (µs)      */
    int64_t cpu_sys_us;   /* system CPU time (µs)    */
    int64_t rss_kb;       /* current RSS (KB)        */
    int64_t max_rss_kb;   /* peak RSS (KB)           */
} m2_metrics_snapshot_t;

/* Populate all fields of *snap. Returns 0 on success, -1 on error. */
int32_t m2_metrics_snapshot(m2_metrics_snapshot_t *snap);

#endif /* METRICS_BRIDGE_H */
