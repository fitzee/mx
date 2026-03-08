/*
 * oidc_bridge.h — Thin C wrapper for RSA/RS256 verification (OpenSSL).
 *
 * Provides only three functions:
 *   1. Construct an RSA public key from raw big-endian n + e bytes
 *   2. Verify an RS256 (RSASSA-PKCS1-v1_5 + SHA-256) signature
 *   3. Free an RSA key handle
 *
 * All key memory is managed by OpenSSL.  The M2 Jwks/Oidc modules
 * own all parsing and validation logic.
 */

#ifndef M2_OIDC_BRIDGE_H
#define M2_OIDC_BRIDGE_H

/* Construct an RSA public key from raw big-endian modulus (n) and
   exponent (e) byte arrays.
   Returns an opaque key handle (EVP_PKEY*), or NULL on failure. */
void *m2_oidc_rsa_from_ne(const unsigned char *n, int n_len,
                           const unsigned char *e, int e_len);

/* Verify RS256 (RSASSA-PKCS1-v1_5 + SHA-256) signature.
   key: opaque handle from m2_oidc_rsa_from_ne.
   msg/msg_len: the signing input (base64url "header.payload").
   sig/sig_len: the raw decoded signature bytes.
   Returns 0 if valid, -1 if invalid or error. */
int m2_oidc_rsa_verify(void *key,
                        const unsigned char *msg, int msg_len,
                        const unsigned char *sig, int sig_len);

/* Free an RSA key handle returned by m2_oidc_rsa_from_ne. */
void m2_oidc_rsa_free(void *key);

#endif /* M2_OIDC_BRIDGE_H */
