/*
 * auth_bridge.h — Thin C wrapper around OpenSSL for m2auth.
 *
 * Backend: OpenSSL (compatible with LibreSSL >= 2.7).
 * Provides: base64url codec, HMAC-SHA256, Ed25519 (OpenSSL >= 1.1.1),
 *           constant-time compare, Unix time.
 *
 * Design: stateless and dumb.  The M2 Auth module owns all
 * token-construction and validation policy.
 */

#ifndef M2_AUTH_BRIDGE_H
#define M2_AUTH_BRIDGE_H

/* ── Initialisation ──────────────────────────────────── */

/* Idempotent.  Call before any other auth_bridge function. */
void m2_auth_init(void);

/* ── Time ────────────────────────────────────────────── */

/* Returns current Unix timestamp (seconds since epoch). */
long m2_auth_get_unix_time(void);

/* ── Base64url (RFC 4648 Section 5, no padding) ──────── */

/* Encode src[0..src_len-1] into dst.  Returns bytes written.
   dst must have room for at least ((src_len + 2) / 3) * 4 bytes. */
int m2_auth_b64url_encode(const unsigned char *src, int src_len,
                          char *dst, int dst_max);

/* Decode src[0..src_len-1] (base64url, no padding) into dst.
   Returns bytes written, or -1 on invalid input.
   dst must have room for at least (src_len * 3) / 4 + 3 bytes. */
int m2_auth_b64url_decode(const char *src, int src_len,
                          unsigned char *dst, int dst_max);

/* ── HMAC-SHA256 ─────────────────────────────────────── */

/* Compute HMAC-SHA256(key, data).  out must be 32 bytes.
   Returns 0 on success, -1 on failure. */
int m2_auth_hmac_sha256(const unsigned char *key, int key_len,
                        const unsigned char *data, int data_len,
                        unsigned char *out);

/* ── Constant-time compare ───────────────────────────── */

/* Returns 0 if a[0..len-1] == b[0..len-1], non-zero otherwise. */
int m2_auth_ct_compare(const unsigned char *a,
                       const unsigned char *b, int len);

/* ── Ed25519 (OpenSSL >= 1.1.1) ─────────────────────── */

/* Returns 1 if Ed25519 is available, 0 otherwise. */
int m2_auth_has_ed25519(void);

/* Generate an Ed25519 key pair.
   pub_out must be 32 bytes, priv_out must be 64 bytes.
   Returns 0 on success, -1 if unsupported/failed. */
int m2_auth_ed25519_keygen(unsigned char *pub_out,
                           unsigned char *priv_out);

/* Sign msg[0..msg_len-1] with priv (64 bytes).
   sig_out must be 64 bytes.
   Returns 0 on success, -1 if unsupported/failed. */
int m2_auth_ed25519_sign(const unsigned char *priv,
                         const unsigned char *msg, int msg_len,
                         unsigned char *sig_out);

/* Verify sig (64 bytes) over msg[0..msg_len-1] with pub (32 bytes).
   Returns 0 if valid, -1 if invalid or unsupported. */
int m2_auth_ed25519_verify(const unsigned char *pub,
                           const unsigned char *msg, int msg_len,
                           const unsigned char *sig);

#endif /* M2_AUTH_BRIDGE_H */
