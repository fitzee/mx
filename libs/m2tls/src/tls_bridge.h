/*
 * tls_bridge.h — Thin C wrapper around OpenSSL/LibreSSL for m2tls.
 *
 * Backend: OpenSSL (compatible with LibreSSL >= 2.7).
 * Choice rationale: OpenSSL is the most widely deployed TLS library.
 *   Available on every major Linux, macOS (via Homebrew), and BSD.
 *   The API is stable and well-documented.  LibreSSL provides the
 *   same API surface.  mbedTLS was considered but has fewer system
 *   packages and a different API shape.
 *
 * Design: the bridge is stateless and dumb.  No event loop awareness,
 * no buffering policy, no retry logic.  The M2 TLS module owns all
 * policy decisions.
 */

#ifndef M2_TLS_BRIDGE_H
#define M2_TLS_BRIDGE_H

/* ── Initialisation ──────────────────────────────────── */

/* Idempotent.  Call before any other tls_bridge function.
   Required for OpenSSL < 1.1.0; no-op on newer versions. */
void m2_tls_init(void);

/* ── Context (SSL_CTX) ───────────────────────────────── */

/* Returns opaque ctx handle, or NULL on failure. */
void *m2_tls_ctx_create(void);
void  m2_tls_ctx_destroy(void *ctx);

/* mode: 0 = no verify, 1 = verify peer */
int  m2_tls_ctx_set_verify(void *ctx, int mode);

/* ver: 0=TLS1.0, 1=TLS1.1, 2=TLS1.2, 3=TLS1.3 */
int  m2_tls_ctx_set_min_version(void *ctx, int ver);

/* Load default CA trust store (SSL_CTX_set_default_verify_paths).
   Returns 0 on success, -1 on failure. */
int  m2_tls_ctx_load_system_roots(void *ctx);

/* Load CA bundle from a PEM file.
   Returns 0 on success, -1 on failure. */
int  m2_tls_ctx_load_ca_file(void *ctx, const char *path);

/* Load client certificate + private key from PEM files.
   Returns 0 on success, -1 on cert error, -2 on key error. */
int  m2_tls_ctx_set_client_cert(void *ctx,
                                 const char *cert_path,
                                 const char *key_path);

/* ── Server context ─────────────────────────────────── */

/* Returns opaque server ctx handle, or NULL on failure.
   Uses TLS_server_method().  Defaults: no peer verify, TLS 1.2 min. */
void *m2_tls_ctx_create_server(void);

/* Load server certificate + private key from PEM files.
   Returns 0 on success, -1 on cert error, -2 on key error. */
int  m2_tls_ctx_set_server_cert(void *ctx,
                                 const char *cert_path,
                                 const char *key_path);

/* ── ALPN ───────────────────────────────────────────── */

/* Client-side ALPN: set protocol list in wire format
   (length-prefixed, e.g. "\x02h2").
   Returns 0 on success, -1 on failure. */
int  m2_tls_ctx_set_alpn(void *ctx,
                          const unsigned char *protos,
                          int protos_len);

/* Server-side ALPN: set preferred protocol list in wire format.
   Installs a select callback.  Max 64 bytes of protos.
   Returns 0 on success, -1 on failure. */
int  m2_tls_ctx_set_alpn_server(void *ctx,
                                 const unsigned char *protos,
                                 int protos_len);

/* Query negotiated ALPN after handshake.
   Copies protocol string (e.g. "h2") into out.
   Returns length, or 0 if no ALPN negotiated. */
int  m2_tls_get_alpn(void *sess, char *out, int max);

/* ── Session (SSL) ───────────────────────────────────── */

/* Create SSL from ctx, attach to fd, set connect state.
   Returns opaque session handle or NULL. */
void *m2_tls_session_create(void *ctx, int fd);

/* Create SSL from ctx, attach to fd, set accept state (server).
   Returns opaque session handle or NULL. */
void *m2_tls_session_create_server(void *ctx, int fd);

void  m2_tls_session_destroy(void *sess);

/* Set SNI hostname for the session.
   Returns 0 on success, -1 on failure. */
int  m2_tls_session_set_sni(void *sess, const char *hostname);

/* ── Handshake ───────────────────────────────────────── */

/*  0 = complete
 *  1 = SSL_ERROR_WANT_READ   (retry after fd readable)
 *  2 = SSL_ERROR_WANT_WRITE  (retry after fd writable)
 * -1 = fatal error
 * -2 = certificate verification failed */
int  m2_tls_handshake(void *sess);

/* ── Data I/O ────────────────────────────────────────── */

/* Returns:
 *  >0 = bytes read
 *   0 = peer closed (clean shutdown)
 *  -1 = WANT_READ
 *  -2 = WANT_WRITE
 *  -3 = fatal error */
int  m2_tls_read(void *sess, char *buf, int max);

/* Returns:
 *  >0 = bytes written
 *  -1 = WANT_READ
 *  -2 = WANT_WRITE
 *  -3 = fatal error */
int  m2_tls_write(void *sess, const char *buf, int len);

/* ── Shutdown ────────────────────────────────────────── */

/*  0 = shutdown complete
 *  1 = WANT_READ
 *  2 = WANT_WRITE
 * -1 = error (can be ignored during cleanup) */
int  m2_tls_shutdown(void *sess);

/* ── Diagnostics ─────────────────────────────────────── */

/* X509 verification result code (X509_V_OK = 0). */
int  m2_tls_get_verify_result(void *sess);

/* One-line subject of peer certificate.
   Returns 0 on success, -1 if no peer cert. */
int  m2_tls_get_peer_summary(void *sess, char *out, int max);

/* Copy OpenSSL error string into out. */
void m2_tls_get_last_error(char *out, int max);

#endif /* M2_TLS_BRIDGE_H */
