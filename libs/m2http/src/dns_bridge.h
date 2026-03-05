/*
 * dns_bridge.h — DNS resolution + socket helpers for m2http
 *
 * Provides:
 *   - Blocking getaddrinfo wrapper (IPv4 A-record only)
 *   - Non-blocking connect-by-IPv4 helper
 *   - Socket error introspection (getsockopt SO_ERROR)
 *   - errno / strerror exposure
 *
 * The M2+ layer (DNS.mod, HTTPClient.mod) owns all higher-level logic.
 */

#ifndef DNS_BRIDGE_H
#define DNS_BRIDGE_H

#include <stdint.h>

/* ── DNS resolution ──────────────────────────────────────────────── */

/* Resolve hostname to first IPv4 address.
   out_addr4 must point to 4 bytes (a,b,c,d).
   Returns 0 on success, -1 on failure. */
int32_t m2_dns_resolve_a(const void *host,
                         void *out_addr4,
                         int32_t *out_port,
                         int32_t port);

/* ── Async DNS resolution ────────────────────────────────────────── */

/* Callback signature for async DNS resolution.
   callback_id: opaque integer passed through from the caller.
   a,b,c,d: IPv4 address octets (valid only when err == 0).
   port: port number (passed through from the caller).
   err: 0 on success, -1 on resolution failure. */
typedef void (*m2_dns_callback)(int32_t callback_id,
                                 uint8_t a, uint8_t b, uint8_t c, uint8_t d,
                                 int32_t port, int32_t err);

/* Resolve host asynchronously via a detached pthread.
   The callback is invoked from the background thread when resolution
   completes.  The caller must arrange thread-safe delivery of the
   result (e.g. a pipe write to wake the event loop).
   host must remain valid only until this function returns (it is
   copied internally). */
void m2_dns_resolve_async(const char *host, int32_t port,
                           int32_t callback_id, m2_dns_callback cb);

/* ── Socket helpers ──────────────────────────────────────────────── */

/* Non-blocking connect to IPv4 address.
   Returns: 0 = already connected, 1 = in progress, -1 = error. */
int32_t m2_connect_ipv4(int32_t fd,
                        int32_t a, int32_t b, int32_t c, int32_t d,
                        int32_t port);

/* Check pending socket error (after non-blocking connect).
   Returns 0 if no error, or errno value on error. */
int32_t m2_getsockopt_error(int32_t fd);

/* ── Error ───────────────────────────────────────────────────────── */

int32_t m2_dns_errno(void);
void    m2_dns_strerror(int32_t errnum, void *buf, int32_t buflen);

#endif /* DNS_BRIDGE_H */
