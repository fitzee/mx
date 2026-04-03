#include "metrics_bridge.h"
#include <stdlib.h>
#include <string.h>
#include <sys/resource.h>

#ifdef __APPLE__
#include <sys/sysctl.h>
#include <mach/mach.h>
#include <mach/host_info.h>
#include <mach/mach_host.h>
#endif

#ifdef __linux__
#include <sys/sysinfo.h>
#include <stdio.h>
#endif

/* ── Load average (POSIX) ─────────────────────────── */

static void get_load_avg(m2_metrics_snapshot_t *s) {
    double avg[3];
    if (getloadavg(avg, 3) == 3) {
        s->load_1  = (int64_t)(avg[0] * 1000.0);
        s->load_5  = (int64_t)(avg[1] * 1000.0);
        s->load_15 = (int64_t)(avg[2] * 1000.0);
    }
}

/* ── Process stats (POSIX getrusage) ──────────────── */

static void get_process_stats(m2_metrics_snapshot_t *s) {
    struct rusage ru;
    if (getrusage(RUSAGE_SELF, &ru) == 0) {
        s->cpu_user_us = (int64_t)ru.ru_utime.tv_sec * 1000000
                       + (int64_t)ru.ru_utime.tv_usec;
        s->cpu_sys_us  = (int64_t)ru.ru_stime.tv_sec * 1000000
                       + (int64_t)ru.ru_stime.tv_usec;
#ifdef __APPLE__
        /* macOS: ru_maxrss is in bytes */
        s->max_rss_kb = (int64_t)ru.ru_maxrss / 1024;
#else
        /* Linux: ru_maxrss is in kilobytes */
        s->max_rss_kb = (int64_t)ru.ru_maxrss;
#endif
    }
}

/* ── macOS: memory + current RSS ──────────────────── */

#ifdef __APPLE__

static void get_memory_info(m2_metrics_snapshot_t *s) {
    /* Total physical memory via sysctl */
    int64_t phys = 0;
    size_t len = sizeof(phys);
    int mib[2] = { CTL_HW, HW_MEMSIZE };
    if (sysctl(mib, 2, &phys, &len, NULL, 0) == 0) {
        s->mem_total_mb = phys / (1024 * 1024);
    }

    /* Free memory via host_statistics64 */
    mach_port_t host = mach_host_self();
    vm_statistics64_data_t vm;
    mach_msg_type_number_t count = HOST_VM_INFO64_COUNT;
    if (host_statistics64(host, HOST_VM_INFO64,
                          (host_info64_t)&vm, &count) == KERN_SUCCESS) {
        int64_t page = (int64_t)vm_page_size;
        s->mem_free_mb = (int64_t)(vm.free_count + vm.inactive_count)
                       * page / (1024 * 1024);
    }

    /* Current RSS via task_info */
    struct mach_task_basic_info info;
    mach_msg_type_number_t info_count = MACH_TASK_BASIC_INFO_COUNT;
    if (task_info(mach_task_self(), MACH_TASK_BASIC_INFO,
                  (task_info_t)&info, &info_count) == KERN_SUCCESS) {
        s->rss_kb = (int64_t)info.resident_size / 1024;
    }
}

#endif /* __APPLE__ */

/* ── Linux: memory + current RSS ──────────────────── */

#ifdef __linux__

static void get_memory_info(m2_metrics_snapshot_t *s) {
    /* Total and free memory via sysinfo */
    struct sysinfo si;
    if (sysinfo(&si) == 0) {
        s->mem_total_mb = (int64_t)si.totalram * si.mem_unit / (1024 * 1024);
        s->mem_free_mb  = (int64_t)(si.freeram + si.bufferram)
                        * si.mem_unit / (1024 * 1024);
    }

    /* Current RSS from /proc/self/status */
    FILE *f = fopen("/proc/self/status", "r");
    if (f) {
        char line[256];
        while (fgets(line, sizeof(line), f)) {
            if (strncmp(line, "VmRSS:", 6) == 0) {
                int64_t val = 0;
                const char *p = line + 6;
                while (*p == ' ' || *p == '\t') p++;
                while (*p >= '0' && *p <= '9') {
                    val = val * 10 + (*p - '0');
                    p++;
                }
                s->rss_kb = val; /* VmRSS is in kB */
                break;
            }
        }
        fclose(f);
    }
}

#endif /* __linux__ */

/* ── Public API ───────────────────────────────────── */

int32_t m2_metrics_snapshot(m2_metrics_snapshot_t *snap) {
    if (!snap) return -1;
    memset(snap, 0, sizeof(*snap));
    get_load_avg(snap);
    get_process_stats(snap);
    get_memory_info(snap);
    return 0;
}
