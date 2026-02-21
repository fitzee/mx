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
