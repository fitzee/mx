/*
 * poller_bridge.h — Platform I/O polling bridge for Modula-2+
 *
 * Abstracts kqueue (macOS/BSD), epoll (Linux), and poll (fallback)
 * behind a uniform C API.  The M2+ layer (Poller.mod) owns all
 * higher-level logic; this bridge is intentionally thin.
 */

#ifndef POLLER_BRIDGE_H
#define POLLER_BRIDGE_H

#include <stdint.h>

/* Event flags (bitmask) */
#define M2_EV_READ   1
#define M2_EV_WRITE  2
#define M2_EV_ERROR  4
#define M2_EV_HUP    8

/* Returned event: which fd is ready and for what */
typedef struct {
    int32_t fd;
    int32_t events;   /* bitmask of M2_EV_* */
} m2_poll_event;

/* Create a poller instance.  Returns handle >= 0, or -1 on error. */
int32_t m2_poller_create(void);

/* Destroy a poller instance. */
void m2_poller_destroy(int32_t handle);

/* Add fd to poller with interest in events (M2_EV_READ|M2_EV_WRITE).
   Returns 0 on success, -1 on error. */
int32_t m2_poller_add(int32_t handle, int32_t fd, int32_t events);

/* Modify interest set for an already-added fd.
   Returns 0 on success, -1 on error. */
int32_t m2_poller_mod(int32_t handle, int32_t fd, int32_t events);

/* Remove fd from poller.
   Returns 0 on success, -1 on error. */
int32_t m2_poller_del(int32_t handle, int32_t fd);

/* Wait for events.  timeout_ms = -1 blocks indefinitely, 0 = poll.
   Returns number of ready events (written to out), or -1 on error.
   out must have room for max_events entries. */
int32_t m2_poller_wait(int32_t handle, int32_t timeout_ms,
                       m2_poll_event *out, int32_t max_events);

/* Current monotonic time in milliseconds (wraps at ~24.8 days). */
int32_t m2_now_ms(void);

#endif /* POLLER_BRIDGE_H */
