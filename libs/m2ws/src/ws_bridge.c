/*
 * ws_bridge.c -- WebSocket C helpers for m2ws
 *
 * Every function is a minimal, self-contained implementation.
 * No external dependencies beyond <stdint.h> and <string.h>.
 * The M2 layer owns all higher-level protocol logic.
 */

#include "ws_bridge.h"

#include <stdint.h>
#include <string.h>

/* ── SHA-1 ──────────────────────────────────────────────────────────── */

/* Minimal SHA-1 implementation (RFC 3174).
   Used only for Sec-WebSocket-Accept computation. */

static uint32_t sha1_rotl(uint32_t x, int n)
{
    return (x << n) | (x >> (32 - n));
}

void m2_ws_sha1(const void *data, int32_t len, void *out20)
{
    uint32_t h0 = 0x67452301;
    uint32_t h1 = 0xEFCDAB89;
    uint32_t h2 = 0x98BADCFE;
    uint32_t h3 = 0x10325476;
    uint32_t h4 = 0xC3D2E1F0;

    const uint8_t *msg = (const uint8_t *)data;
    uint64_t bitLen = (uint64_t)len * 8;

    /* Compute padded length: msg + 1 byte (0x80) + padding + 8 bytes (length) */
    int32_t padded = ((len + 1 + 8 + 63) / 64) * 64;
    uint8_t block[128]; /* max 2 blocks for typical WebSocket key+GUID */
    memset(block, 0, sizeof(block));

    int32_t offset = 0;
    while (offset < padded) {
        /* Fill block */
        memset(block, 0, 64);
        int32_t copyLen = len - offset;
        if (copyLen > 64) copyLen = 64;
        if (copyLen > 0) memcpy(block, msg + offset, (size_t)copyLen);

        /* Append 0x80 after message */
        if (offset + 64 > len && offset <= len) {
            int32_t padPos = len - offset;
            if (padPos < 64) block[padPos] = 0x80;
        }

        /* Append length in last 8 bytes of last block */
        if (offset + 64 == padded) {
            block[56] = (uint8_t)(bitLen >> 56);
            block[57] = (uint8_t)(bitLen >> 48);
            block[58] = (uint8_t)(bitLen >> 40);
            block[59] = (uint8_t)(bitLen >> 32);
            block[60] = (uint8_t)(bitLen >> 24);
            block[61] = (uint8_t)(bitLen >> 16);
            block[62] = (uint8_t)(bitLen >> 8);
            block[63] = (uint8_t)(bitLen);
        }

        /* Process block */
        uint32_t w[80];
        for (int i = 0; i < 16; i++) {
            w[i] = ((uint32_t)block[i*4] << 24) |
                   ((uint32_t)block[i*4+1] << 16) |
                   ((uint32_t)block[i*4+2] << 8) |
                   ((uint32_t)block[i*4+3]);
        }
        for (int i = 16; i < 80; i++) {
            w[i] = sha1_rotl(w[i-3] ^ w[i-8] ^ w[i-14] ^ w[i-16], 1);
        }

        uint32_t a = h0, b = h1, c = h2, d = h3, e = h4;
        for (int i = 0; i < 80; i++) {
            uint32_t f, k;
            if (i < 20) {
                f = (b & c) | ((~b) & d);
                k = 0x5A827999;
            } else if (i < 40) {
                f = b ^ c ^ d;
                k = 0x6ED9EBA1;
            } else if (i < 60) {
                f = (b & c) | (b & d) | (c & d);
                k = 0x8F1BBCDC;
            } else {
                f = b ^ c ^ d;
                k = 0xCA62C1D6;
            }
            uint32_t temp = sha1_rotl(a, 5) + f + e + k + w[i];
            e = d;
            d = c;
            c = sha1_rotl(b, 30);
            b = a;
            a = temp;
        }
        h0 += a; h1 += b; h2 += c; h3 += d; h4 += e;

        offset += 64;
    }

    uint8_t *out = (uint8_t *)out20;
    out[0]  = (uint8_t)(h0 >> 24); out[1]  = (uint8_t)(h0 >> 16);
    out[2]  = (uint8_t)(h0 >> 8);  out[3]  = (uint8_t)(h0);
    out[4]  = (uint8_t)(h1 >> 24); out[5]  = (uint8_t)(h1 >> 16);
    out[6]  = (uint8_t)(h1 >> 8);  out[7]  = (uint8_t)(h1);
    out[8]  = (uint8_t)(h2 >> 24); out[9]  = (uint8_t)(h2 >> 16);
    out[10] = (uint8_t)(h2 >> 8);  out[11] = (uint8_t)(h2);
    out[12] = (uint8_t)(h3 >> 24); out[13] = (uint8_t)(h3 >> 16);
    out[14] = (uint8_t)(h3 >> 8);  out[15] = (uint8_t)(h3);
    out[16] = (uint8_t)(h4 >> 24); out[17] = (uint8_t)(h4 >> 16);
    out[18] = (uint8_t)(h4 >> 8);  out[19] = (uint8_t)(h4);
}

/* ── Base64 ─────────────────────────────────────────────────────────── */

static const char b64_table[] =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

void m2_ws_base64_encode(const void *in, int32_t inLen,
                         void *out, int32_t maxOut,
                         int32_t *outLen)
{
    const uint8_t *src = (const uint8_t *)in;
    char *dst = (char *)out;
    int32_t i = 0, j = 0;
    int32_t remaining;

    while (i < inLen && j + 4 <= maxOut) {
        remaining = inLen - i;
        uint32_t a = (uint32_t)src[i];
        uint32_t b = (remaining > 1) ? (uint32_t)src[i + 1] : 0;
        uint32_t c = (remaining > 2) ? (uint32_t)src[i + 2] : 0;
        uint32_t triple = (a << 16) | (b << 8) | c;

        dst[j++] = b64_table[(triple >> 18) & 0x3F];
        dst[j++] = b64_table[(triple >> 12) & 0x3F];
        dst[j++] = (remaining > 1) ? b64_table[(triple >> 6) & 0x3F] : '=';
        dst[j++] = (remaining > 2) ? b64_table[triple & 0x3F] : '=';

        i += (remaining >= 3) ? 3 : remaining;
    }

    if (j < maxOut) dst[j] = '\0';
    *outLen = j;
}

/* ── XOR mask ───────────────────────────────────────────────────────── */

void m2_ws_apply_mask(void *data, int32_t len,
                      const void *mask, int32_t offset)
{
    uint8_t *d = (uint8_t *)data;
    const uint8_t *m = (const uint8_t *)mask;

    for (int32_t i = 0; i < len; i++) {
        d[i] ^= m[(offset + i) & 3];
    }
}

/* ── Random mask key ────────────────────────────────────────────────── */

static uint32_t ws_prng_state = 0xDEADBEEF;

void m2_ws_random_mask(void *out)
{
    uint8_t *dst = (uint8_t *)out;
    for (int i = 0; i < 4; i++) {
        ws_prng_state = ws_prng_state * 1103515245 + 12345;
        dst[i] = (uint8_t)((ws_prng_state >> 16) & 0xFF);
    }
}
