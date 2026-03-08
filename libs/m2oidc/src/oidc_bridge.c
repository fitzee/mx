/*
 * oidc_bridge.c — RSA/RS256 verification backend for m2oidc.
 *
 * Compile: cc -c oidc_bridge.c -I<openssl-include>
 * Link:    -lssl -lcrypto
 *
 * Targets OpenSSL >= 1.1.0.  Uses EVP_PKEY_fromdata (OSSL_PARAM_BLD)
 * on OpenSSL >= 3.0, falls back to RSA_new + RSA_set0_key on older.
 */

#include "oidc_bridge.h"

#include <openssl/evp.h>
#include <openssl/opensslv.h>
#include <string.h>

#if OPENSSL_VERSION_NUMBER >= 0x30000000L && !defined(LIBRESSL_VERSION_NUMBER)
#define M2_OIDC_USE_OSSL3 1
#include <openssl/param_build.h>
#include <openssl/core_names.h>
#else
#define M2_OIDC_USE_OSSL3 0
#include <openssl/rsa.h>
#include <openssl/bn.h>
#endif

/* ── Construct RSA public key from raw n + e ─────────── */

void *m2_oidc_rsa_from_ne(const unsigned char *n, int n_len,
                           const unsigned char *e, int e_len) {
    if (!n || n_len <= 0 || !e || e_len <= 0) return NULL;

#if M2_OIDC_USE_OSSL3
    {
        EVP_PKEY *pkey = NULL;
        EVP_PKEY_CTX *pctx = NULL;
        OSSL_PARAM_BLD *bld = NULL;
        OSSL_PARAM *params = NULL;
        BIGNUM *bn_n = NULL, *bn_e = NULL;

        bn_n = BN_bin2bn(n, n_len, NULL);
        bn_e = BN_bin2bn(e, e_len, NULL);
        if (!bn_n || !bn_e) goto ossl3_err;

        bld = OSSL_PARAM_BLD_new();
        if (!bld) goto ossl3_err;

        if (!OSSL_PARAM_BLD_push_BN(bld, OSSL_PKEY_PARAM_RSA_N, bn_n))
            goto ossl3_err;
        if (!OSSL_PARAM_BLD_push_BN(bld, OSSL_PKEY_PARAM_RSA_E, bn_e))
            goto ossl3_err;

        params = OSSL_PARAM_BLD_to_param(bld);
        if (!params) goto ossl3_err;

        pctx = EVP_PKEY_CTX_new_from_name(NULL, "RSA", NULL);
        if (!pctx) goto ossl3_err;

        if (EVP_PKEY_fromdata_init(pctx) <= 0) goto ossl3_err;
        if (EVP_PKEY_fromdata(pctx, &pkey, EVP_PKEY_PUBLIC_KEY,
                              params) <= 0) {
            pkey = NULL;
            goto ossl3_err;
        }

    ossl3_err:
        OSSL_PARAM_free(params);
        OSSL_PARAM_BLD_free(bld);
        EVP_PKEY_CTX_free(pctx);
        BN_free(bn_n);
        BN_free(bn_e);
        return pkey;
    }
#else
    {
        RSA *rsa = NULL;
        EVP_PKEY *pkey = NULL;
        BIGNUM *bn_n = NULL, *bn_e = NULL;

        bn_n = BN_bin2bn(n, n_len, NULL);
        bn_e = BN_bin2bn(e, e_len, NULL);
        if (!bn_n || !bn_e) goto legacy_err;

        rsa = RSA_new();
        if (!rsa) goto legacy_err;

        /* RSA_set0_key takes ownership of bn_n and bn_e on success */
        if (!RSA_set0_key(rsa, bn_n, bn_e, NULL)) goto legacy_err;
        bn_n = NULL;
        bn_e = NULL;

        pkey = EVP_PKEY_new();
        if (!pkey) { RSA_free(rsa); return NULL; }

        /* EVP_PKEY_assign_RSA takes ownership of rsa */
        if (!EVP_PKEY_assign_RSA(pkey, rsa)) {
            RSA_free(rsa);
            EVP_PKEY_free(pkey);
            return NULL;
        }

        return pkey;

    legacy_err:
        BN_free(bn_n);
        BN_free(bn_e);
        RSA_free(rsa);
        return NULL;
    }
#endif
}

/* ── Verify RS256 signature ──────────────────────────── */

int m2_oidc_rsa_verify(void *key,
                        const unsigned char *msg, int msg_len,
                        const unsigned char *sig, int sig_len) {
    EVP_MD_CTX *mdctx = NULL;
    EVP_PKEY *pkey = (EVP_PKEY *)key;
    int ret = -1;
    int rc;

    if (!pkey || !msg || msg_len <= 0 || !sig || sig_len <= 0)
        return -1;

    mdctx = EVP_MD_CTX_new();
    if (!mdctx) return -1;

    rc = EVP_DigestVerifyInit(mdctx, NULL, EVP_sha256(), NULL, pkey);
    if (rc <= 0) goto done;

    rc = EVP_DigestVerifyUpdate(mdctx, msg, (size_t)msg_len);
    if (rc <= 0) goto done;

    rc = EVP_DigestVerifyFinal(mdctx, sig, (size_t)sig_len);
    if (rc == 1) ret = 0;

done:
    EVP_MD_CTX_free(mdctx);
    return ret;
}

/* ── Free RSA key ────────────────────────────────────── */

void m2_oidc_rsa_free(void *key) {
    if (key)
        EVP_PKEY_free((EVP_PKEY *)key);
}
