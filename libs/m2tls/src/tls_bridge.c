/*
 * tls_bridge.c — OpenSSL/LibreSSL backend for m2tls.
 *
 * Compile: cc -c tls_bridge.c -I<openssl-include>
 * Link:    -lssl -lcrypto
 *
 * Targets OpenSSL >= 1.1.0 and LibreSSL >= 2.7.
 */

#include "tls_bridge.h"

#include <openssl/ssl.h>
#include <openssl/err.h>
#include <openssl/x509.h>
#include <openssl/x509v3.h>
#include <string.h>

/* ── Compat shims ────────────────────────────────────── */

/* OpenSSL 3.0 renamed SSL_get_peer_certificate. */
#if OPENSSL_VERSION_NUMBER >= 0x30000000L && !defined(LIBRESSL_VERSION_NUMBER)
#define M2_GET_PEER_CERT(s) SSL_get1_peer_certificate(s)
#else
#define M2_GET_PEER_CERT(s) SSL_get_peer_certificate(s)
#endif

/* ── Initialisation ──────────────────────────────────── */

void m2_tls_init(void) {
#if OPENSSL_VERSION_NUMBER < 0x10100000L
    static int done = 0;
    if (!done) {
        SSL_library_init();
        SSL_load_error_strings();
        OpenSSL_add_all_algorithms();
        done = 1;
    }
#endif
    /* OpenSSL >= 1.1.0 auto-initialises on first use. */
}

/* ── Context ─────────────────────────────────────────── */

void *m2_tls_ctx_create(void) {
    SSL_CTX *ctx;
    m2_tls_init();
    ctx = SSL_CTX_new(TLS_client_method());
    if (!ctx) return NULL;
    /* Sane defaults: verify peer, TLS 1.2 minimum. */
    SSL_CTX_set_verify(ctx, SSL_VERIFY_PEER, NULL);
    SSL_CTX_set_min_proto_version(ctx, TLS1_2_VERSION);
    return ctx;
}

void m2_tls_ctx_destroy(void *ctx) {
    if (ctx) SSL_CTX_free((SSL_CTX *)ctx);
}

int m2_tls_ctx_set_verify(void *ctx, int mode) {
    if (!ctx) return -1;
    if (mode)
        SSL_CTX_set_verify((SSL_CTX *)ctx, SSL_VERIFY_PEER, NULL);
    else
        SSL_CTX_set_verify((SSL_CTX *)ctx, SSL_VERIFY_NONE, NULL);
    return 0;
}

int m2_tls_ctx_set_min_version(void *ctx, int ver) {
    int v;
    if (!ctx) return -1;
    switch (ver) {
        case 0: v = TLS1_VERSION;   break;
        case 1: v = TLS1_1_VERSION; break;
        case 2: v = TLS1_2_VERSION; break;
#ifdef TLS1_3_VERSION
        case 3: v = TLS1_3_VERSION; break;
#else
        case 3: return -1; /* TLS 1.3 not available in this build */
#endif
        default: return -1;
    }
    return SSL_CTX_set_min_proto_version((SSL_CTX *)ctx, v) == 1 ? 0 : -1;
}

int m2_tls_ctx_load_system_roots(void *ctx) {
    if (!ctx) return -1;
    return SSL_CTX_set_default_verify_paths((SSL_CTX *)ctx) == 1 ? 0 : -1;
}

int m2_tls_ctx_load_ca_file(void *ctx, const char *path) {
    if (!ctx || !path) return -1;
    return SSL_CTX_load_verify_locations((SSL_CTX *)ctx, path, NULL) == 1
               ? 0 : -1;
}

int m2_tls_ctx_set_client_cert(void *ctx,
                                const char *cert_path,
                                const char *key_path) {
    if (!ctx || !cert_path || !key_path) return -1;
    if (SSL_CTX_use_certificate_file((SSL_CTX *)ctx, cert_path,
                                      SSL_FILETYPE_PEM) != 1)
        return -1;
    if (SSL_CTX_use_PrivateKey_file((SSL_CTX *)ctx, key_path,
                                     SSL_FILETYPE_PEM) != 1)
        return -2;
    if (SSL_CTX_check_private_key((SSL_CTX *)ctx) != 1)
        return -2;
    return 0;
}

/* ── Server context ──────────────────────────────────── */

void *m2_tls_ctx_create_server(void) {
    SSL_CTX *ctx;
    m2_tls_init();
    ctx = SSL_CTX_new(TLS_server_method());
    if (!ctx) return NULL;
    /* Servers don't verify clients by default.  TLS 1.2 minimum. */
    SSL_CTX_set_verify(ctx, SSL_VERIFY_NONE, NULL);
    SSL_CTX_set_min_proto_version(ctx, TLS1_2_VERSION);
    return ctx;
}

int m2_tls_ctx_set_server_cert(void *ctx,
                                const char *cert_path,
                                const char *key_path) {
    if (!ctx || !cert_path || !key_path) return -1;
    if (SSL_CTX_use_certificate_file((SSL_CTX *)ctx, cert_path,
                                      SSL_FILETYPE_PEM) != 1)
        return -1;
    if (SSL_CTX_use_PrivateKey_file((SSL_CTX *)ctx, key_path,
                                     SSL_FILETYPE_PEM) != 1)
        return -2;
    if (SSL_CTX_check_private_key((SSL_CTX *)ctx) != 1)
        return -2;
    return 0;
}

/* ── ALPN ────────────────────────────────────────────── */

int m2_tls_ctx_set_alpn(void *ctx,
                         const unsigned char *protos,
                         int protos_len) {
    if (!ctx || !protos || protos_len <= 0) return -1;
    /* SSL_CTX_set_alpn_protos returns 0 on success. */
    return SSL_CTX_set_alpn_protos((SSL_CTX *)ctx, protos,
                                    (unsigned int)protos_len) == 0
               ? 0 : -1;
}

/* Static buffer for server ALPN preferred list (one server ctx per process). */
static unsigned char s_alpn_protos[64];
static int           s_alpn_protos_len = 0;

static int m2_alpn_select_cb(SSL *ssl,
                              const unsigned char **out,
                              unsigned char *outlen,
                              const unsigned char *in,
                              unsigned int inlen,
                              void *arg) {
    (void)ssl; (void)arg;
    if (s_alpn_protos_len == 0)
        return SSL_TLSEXT_ERR_NOACK;
    if (SSL_select_next_proto((unsigned char **)out, outlen,
                               s_alpn_protos, (unsigned int)s_alpn_protos_len,
                               in, inlen) != OPENSSL_NPN_NEGOTIATED)
        return SSL_TLSEXT_ERR_NOACK;
    return SSL_TLSEXT_ERR_OK;
}

int m2_tls_ctx_set_alpn_server(void *ctx,
                                const unsigned char *protos,
                                int protos_len) {
    if (!ctx || !protos || protos_len <= 0 || protos_len > 64)
        return -1;
    memcpy(s_alpn_protos, protos, (size_t)protos_len);
    s_alpn_protos_len = protos_len;
    SSL_CTX_set_alpn_select_cb((SSL_CTX *)ctx, m2_alpn_select_cb, NULL);
    return 0;
}

int m2_tls_get_alpn(void *sess, char *out, int max) {
    const unsigned char *proto = NULL;
    unsigned int len = 0;
    int copy;
    if (!sess || !out || max <= 0) return 0;
    SSL_get0_alpn_selected((const SSL *)sess, &proto, &len);
    if (!proto || len == 0) {
        out[0] = '\0';
        return 0;
    }
    copy = (int)len;
    if (copy >= max) copy = max - 1;
    memcpy(out, proto, (size_t)copy);
    out[copy] = '\0';
    return copy;
}

/* ── Session ─────────────────────────────────────────── */

void *m2_tls_session_create(void *ctx, int fd) {
    SSL *ssl;
    if (!ctx) return NULL;
    ssl = SSL_new((SSL_CTX *)ctx);
    if (!ssl) return NULL;
    if (SSL_set_fd(ssl, fd) != 1) {
        SSL_free(ssl);
        return NULL;
    }
    SSL_set_connect_state(ssl);
    return ssl;
}

void *m2_tls_session_create_server(void *ctx, int fd) {
    SSL *ssl;
    if (!ctx) return NULL;
    ssl = SSL_new((SSL_CTX *)ctx);
    if (!ssl) return NULL;
    if (SSL_set_fd(ssl, fd) != 1) {
        SSL_free(ssl);
        return NULL;
    }
    SSL_set_accept_state(ssl);
    return ssl;
}

void m2_tls_session_destroy(void *sess) {
    if (sess) SSL_free((SSL *)sess);
}

int m2_tls_session_set_sni(void *sess, const char *hostname) {
    if (!sess || !hostname) return -1;
    return SSL_set_tlsext_host_name((SSL *)sess, hostname) == 1 ? 0 : -1;
}

/* ── Handshake ───────────────────────────────────────── */

int m2_tls_handshake(void *sess) {
    int rc, err;
    long vr;
    if (!sess) return -1;
    ERR_clear_error();
    rc = SSL_do_handshake((SSL *)sess);
    if (rc == 1) return 0;  /* success */
    err = SSL_get_error((SSL *)sess, rc);
    switch (err) {
        case SSL_ERROR_WANT_READ:  return 1;
        case SSL_ERROR_WANT_WRITE: return 2;
        default:
            vr = SSL_get_verify_result((SSL *)sess);
            if (vr != X509_V_OK) return -2;
            return -1;
    }
}

/* ── Data I/O ────────────────────────────────────────── */

int m2_tls_read(void *sess, char *buf, int max) {
    int rc, err;
    if (!sess || !buf || max <= 0) return -3;
    ERR_clear_error();
    rc = SSL_read((SSL *)sess, buf, max);
    if (rc > 0) return rc;
    if (rc == 0) {
        err = SSL_get_error((SSL *)sess, rc);
        if (err == SSL_ERROR_ZERO_RETURN) return 0;  /* clean shutdown */
        return 0;  /* treat any 0-return as closed */
    }
    err = SSL_get_error((SSL *)sess, rc);
    switch (err) {
        case SSL_ERROR_WANT_READ:  return -1;
        case SSL_ERROR_WANT_WRITE: return -2;
        default:                   return -3;
    }
}

int m2_tls_write(void *sess, const char *buf, int len) {
    int rc, err;
    if (!sess || !buf || len <= 0) return -3;
    ERR_clear_error();
    rc = SSL_write((SSL *)sess, buf, len);
    if (rc > 0) return rc;
    err = SSL_get_error((SSL *)sess, rc);
    switch (err) {
        case SSL_ERROR_WANT_READ:  return -1;
        case SSL_ERROR_WANT_WRITE: return -2;
        default:                   return -3;
    }
}

/* ── Shutdown ────────────────────────────────────────── */

int m2_tls_shutdown(void *sess) {
    int rc, err;
    if (!sess) return -1;
    ERR_clear_error();
    rc = SSL_shutdown((SSL *)sess);
    if (rc == 1) return 0;   /* bidirectional shutdown complete */
    if (rc == 0) {
        /* First close_notify sent; call again for bidirectional.
           For non-blocking cleanup we just accept unidirectional. */
        return 0;
    }
    err = SSL_get_error((SSL *)sess, rc);
    switch (err) {
        case SSL_ERROR_WANT_READ:  return 1;
        case SSL_ERROR_WANT_WRITE: return 2;
        default:                   return -1;
    }
}

/* ── Diagnostics ─────────────────────────────────────── */

int m2_tls_get_verify_result(void *sess) {
    if (!sess) return -1;
    return (int)SSL_get_verify_result((SSL *)sess);
}

int m2_tls_get_peer_summary(void *sess, char *out, int max) {
    X509 *cert;
    X509_NAME *subj;
    if (!sess || !out || max <= 0) return -1;
    out[0] = '\0';
    cert = M2_GET_PEER_CERT((SSL *)sess);
    if (!cert) return -1;
    subj = X509_get_subject_name(cert);
    if (subj)
        X509_NAME_oneline(subj, out, max);
    else
        out[0] = '\0';
    X509_free(cert);
    return 0;
}

void m2_tls_get_last_error(char *out, int max) {
    unsigned long err;
    if (!out || max <= 0) return;
    err = ERR_peek_last_error();
    if (err == 0) {
        out[0] = '\0';
        return;
    }
    ERR_error_string_n(err, out, (size_t)max);
}
