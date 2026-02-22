/*
 * auth_bridge.c — OpenSSL backend for m2auth.
 *
 * Compile: cc -c auth_bridge.c -I<openssl-include>
 * Link:    -lssl -lcrypto
 *
 * Targets OpenSSL >= 1.1.0 and LibreSSL >= 2.7.
 * Ed25519 requires OpenSSL >= 1.1.1.
 */

#include "auth_bridge.h"

#include <openssl/hmac.h>
#include <openssl/crypto.h>
#include <openssl/opensslv.h>
#include <string.h>
#include <time.h>

/* OpenSSL 3.0 deprecated HMAC() one-shot; use EVP_MAC. */
#if OPENSSL_VERSION_NUMBER >= 0x30000000L && !defined(LIBRESSL_VERSION_NUMBER)
#include <openssl/evp.h>
#include <openssl/core_names.h>
#define M2_AUTH_USE_EVP_MAC 1
#else
#define M2_AUTH_USE_EVP_MAC 0
#endif

/* Ed25519 available in OpenSSL >= 1.1.1 (but not LibreSSL < 3.7) */
#if OPENSSL_VERSION_NUMBER >= 0x10101000L
#include <openssl/evp.h>
#define M2_AUTH_HAS_ED25519 1
#else
#define M2_AUTH_HAS_ED25519 0
#endif

/* ── Base64url lookup tables ─────────────────────────── */

static const char b64url_enc[] =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/* Decode table: -1 = invalid, 0..63 = value */
static const signed char b64url_dec[256] = {
    -1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,
    -1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,
    -1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,62,-1,-1,
     52,53,54,55,56,57,58,59,60,61,-1,-1,-1,-1,-1,-1,
    -1, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9,10,11,12,13,14,
    15,16,17,18,19,20,21,22,23,24,25,-1,-1,-1,-1,63,
    -1,26,27,28,29,30,31,32,33,34,35,36,37,38,39,40,
    41,42,43,44,45,46,47,48,49,50,51,-1,-1,-1,-1,-1,
    -1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,
    -1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,
    -1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,
    -1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,
    -1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,
    -1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,
    -1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,
    -1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,
};

/* ── Initialisation ──────────────────────────────────── */

void m2_auth_init(void) {
    /* OpenSSL >= 1.1.0 auto-initialises.  Nothing to do. */
}

/* ── Time ────────────────────────────────────────────── */

long m2_auth_get_unix_time(void) {
    return (long)time(NULL);
}

/* ── Base64url ───────────────────────────────────────── */

int m2_auth_b64url_encode(const unsigned char *src, int src_len,
                          char *dst, int dst_max) {
    int i, j, needed;
    unsigned int triple;

    if (src_len < 0) return -1;
    /* Compute output length (no padding) */
    needed = (src_len / 3) * 4;
    if (src_len % 3 == 1) needed += 2;
    else if (src_len % 3 == 2) needed += 3;
    if (needed > dst_max) return -1;

    j = 0;
    for (i = 0; i + 2 < src_len; i += 3) {
        triple = ((unsigned int)src[i] << 16)
               | ((unsigned int)src[i+1] << 8)
               | (unsigned int)src[i+2];
        dst[j++] = b64url_enc[(triple >> 18) & 0x3F];
        dst[j++] = b64url_enc[(triple >> 12) & 0x3F];
        dst[j++] = b64url_enc[(triple >>  6) & 0x3F];
        dst[j++] = b64url_enc[triple & 0x3F];
    }

    if (src_len % 3 == 1) {
        triple = (unsigned int)src[i] << 16;
        dst[j++] = b64url_enc[(triple >> 18) & 0x3F];
        dst[j++] = b64url_enc[(triple >> 12) & 0x3F];
    } else if (src_len % 3 == 2) {
        triple = ((unsigned int)src[i] << 16)
               | ((unsigned int)src[i+1] << 8);
        dst[j++] = b64url_enc[(triple >> 18) & 0x3F];
        dst[j++] = b64url_enc[(triple >> 12) & 0x3F];
        dst[j++] = b64url_enc[(triple >>  6) & 0x3F];
    }

    return j;
}

int m2_auth_b64url_decode(const char *src, int src_len,
                          unsigned char *dst, int dst_max) {
    int i, j, pad, needed;
    unsigned int a, b, c, d;
    const unsigned char *usrc = (const unsigned char *)src;

    if (src_len < 0) return -1;
    if (src_len == 0) { return 0; }

    /* Strip trailing '=' padding if present */
    pad = 0;
    while (src_len > 0 && src[src_len - 1] == '=') {
        pad++;
        src_len--;
    }

    /* Compute output length */
    needed = (src_len * 3) / 4;
    if (needed > dst_max) return -1;

    j = 0;
    for (i = 0; i + 3 < src_len; i += 4) {
        a = (unsigned int)b64url_dec[usrc[i]];
        b = (unsigned int)b64url_dec[usrc[i+1]];
        c = (unsigned int)b64url_dec[usrc[i+2]];
        d = (unsigned int)b64url_dec[usrc[i+3]];
        if ((a | b | c | d) & 0x80) return -1;  /* invalid char */
        dst[j++] = (unsigned char)((a << 2) | (b >> 4));
        dst[j++] = (unsigned char)(((b & 0xF) << 4) | (c >> 2));
        dst[j++] = (unsigned char)(((c & 0x3) << 6) | d);
    }

    /* Handle remainder */
    if (src_len % 4 == 2) {
        a = (unsigned int)b64url_dec[usrc[i]];
        b = (unsigned int)b64url_dec[usrc[i+1]];
        if ((a | b) & 0x80) return -1;
        dst[j++] = (unsigned char)((a << 2) | (b >> 4));
    } else if (src_len % 4 == 3) {
        a = (unsigned int)b64url_dec[usrc[i]];
        b = (unsigned int)b64url_dec[usrc[i+1]];
        c = (unsigned int)b64url_dec[usrc[i+2]];
        if ((a | b | c) & 0x80) return -1;
        dst[j++] = (unsigned char)((a << 2) | (b >> 4));
        dst[j++] = (unsigned char)(((b & 0xF) << 4) | (c >> 2));
    } else if (src_len % 4 == 1) {
        return -1;  /* invalid length */
    }

    return j;
}

/* ── HMAC-SHA256 ─────────────────────────────────────── */

int m2_auth_hmac_sha256(const unsigned char *key, int key_len,
                        const unsigned char *data, int data_len,
                        unsigned char *out) {
    if (!key || !data || !out) return -1;

#if M2_AUTH_USE_EVP_MAC
    {
        EVP_MAC *mac = NULL;
        EVP_MAC_CTX *ctx = NULL;
        OSSL_PARAM params[2];
        size_t out_len = 32;
        int ret = -1;

        mac = EVP_MAC_fetch(NULL, "HMAC", NULL);
        if (!mac) return -1;

        ctx = EVP_MAC_CTX_new(mac);
        if (!ctx) { EVP_MAC_free(mac); return -1; }

        params[0] = OSSL_PARAM_construct_utf8_string(
            OSSL_MAC_PARAM_DIGEST, "SHA256", 0);
        params[1] = OSSL_PARAM_construct_end();

        if (EVP_MAC_init(ctx, key, (size_t)key_len, params) &&
            EVP_MAC_update(ctx, data, (size_t)data_len) &&
            EVP_MAC_final(ctx, out, &out_len, 32)) {
            ret = 0;
        }

        EVP_MAC_CTX_free(ctx);
        EVP_MAC_free(mac);
        return ret;
    }
#else
    {
        unsigned int out_len = 32;
        unsigned char *result;
        result = HMAC(EVP_sha256(), key, key_len,
                      data, (size_t)data_len, out, &out_len);
        return result ? 0 : -1;
    }
#endif
}

/* ── Constant-time compare ───────────────────────────── */

int m2_auth_ct_compare(const unsigned char *a,
                       const unsigned char *b, int len) {
    if (!a || !b || len <= 0) return -1;
    return CRYPTO_memcmp(a, b, (size_t)len);
}

/* ── Ed25519 ─────────────────────────────────────────── */

int m2_auth_has_ed25519(void) {
#if M2_AUTH_HAS_ED25519
    return 1;
#else
    return 0;
#endif
}

int m2_auth_ed25519_keygen(unsigned char *pub_out,
                           unsigned char *priv_out) {
#if M2_AUTH_HAS_ED25519
    EVP_PKEY *pkey = NULL;
    EVP_PKEY_CTX *pctx = NULL;
    size_t pub_len = 32, priv_len = 64;
    int ret = -1;

    pctx = EVP_PKEY_CTX_new_id(EVP_PKEY_ED25519, NULL);
    if (!pctx) return -1;

    if (EVP_PKEY_keygen_init(pctx) <= 0) goto done;
    if (EVP_PKEY_keygen(pctx, &pkey) <= 0) goto done;

    if (EVP_PKEY_get_raw_public_key(pkey, pub_out, &pub_len) <= 0)
        goto done;
    if (EVP_PKEY_get_raw_private_key(pkey, priv_out, &priv_len) <= 0)
        goto done;

    ret = 0;
done:
    EVP_PKEY_free(pkey);
    EVP_PKEY_CTX_free(pctx);
    return ret;
#else
    (void)pub_out; (void)priv_out;
    return -1;
#endif
}

int m2_auth_ed25519_sign(const unsigned char *priv,
                         const unsigned char *msg, int msg_len,
                         unsigned char *sig_out) {
#if M2_AUTH_HAS_ED25519
    EVP_PKEY *pkey = NULL;
    EVP_MD_CTX *mdctx = NULL;
    size_t sig_len = 64;
    int ret = -1;

    pkey = EVP_PKEY_new_raw_private_key(EVP_PKEY_ED25519, NULL,
                                         priv, 32);
    if (!pkey) return -1;

    mdctx = EVP_MD_CTX_new();
    if (!mdctx) { EVP_PKEY_free(pkey); return -1; }

    if (EVP_DigestSignInit(mdctx, NULL, NULL, NULL, pkey) <= 0)
        goto done;
    if (EVP_DigestSign(mdctx, sig_out, &sig_len,
                       msg, (size_t)msg_len) <= 0)
        goto done;

    ret = 0;
done:
    EVP_MD_CTX_free(mdctx);
    EVP_PKEY_free(pkey);
    return ret;
#else
    (void)priv; (void)msg; (void)msg_len; (void)sig_out;
    return -1;
#endif
}

int m2_auth_ed25519_verify(const unsigned char *pub,
                           const unsigned char *msg, int msg_len,
                           const unsigned char *sig) {
#if M2_AUTH_HAS_ED25519
    EVP_PKEY *pkey = NULL;
    EVP_MD_CTX *mdctx = NULL;
    int ret = -1;

    pkey = EVP_PKEY_new_raw_public_key(EVP_PKEY_ED25519, NULL,
                                        pub, 32);
    if (!pkey) return -1;

    mdctx = EVP_MD_CTX_new();
    if (!mdctx) { EVP_PKEY_free(pkey); return -1; }

    if (EVP_DigestVerifyInit(mdctx, NULL, NULL, NULL, pkey) <= 0)
        goto done;
    if (EVP_DigestVerify(mdctx, sig, 64,
                         msg, (size_t)msg_len) == 1) {
        ret = 0;
    }

done:
    EVP_MD_CTX_free(mdctx);
    EVP_PKEY_free(pkey);
    return ret;
#else
    (void)pub; (void)msg; (void)msg_len; (void)sig;
    return -1;
#endif
}
