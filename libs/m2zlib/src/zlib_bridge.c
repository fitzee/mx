/*
 * zlib_bridge.c — zlib compression bridge for the m2c Modula-2 compiler.
 *
 * Provides a flat C API wrapping zlib's deflate/inflate so that Modula-2
 * programs can call compression routines via FFI.  All integer arguments
 * use int32_t; the opaque stream handle is passed as void*.
 *
 * Compile with:
 *   cc -c zlib_bridge.c
 * Link with:
 *   -lz
 */

#include "zlib_bridge.h"
#include <zlib.h>
#include <stdlib.h>
#include <string.h>

/* ── Return codes ─────────────────────────────────────────────── */

#define RC_OK         0
#define RC_STREAM_END 1
#define RC_NEED_MORE  2
#define RC_ERROR     -1

/* ── Deflate ──────────────────────────────────────────────────── */

void *m2_deflate_init(int32_t level, int32_t windowBits) {
    z_stream *s = (z_stream *)malloc(sizeof(z_stream));
    if (!s) return NULL;
    memset(s, 0, sizeof(z_stream));

    int ret = deflateInit2(s, (int)level, Z_DEFLATED,
                           (int)windowBits, 8, Z_DEFAULT_STRATEGY);
    if (ret != Z_OK) {
        free(s);
        return NULL;
    }
    return (void *)s;
}

int32_t m2_deflate(void *s, void *src, int32_t srcLen,
                    void *dst, int32_t dstMax,
                    int32_t *produced, int32_t flush) {
    if (!s) return RC_ERROR;
    z_stream *zs = (z_stream *)s;

    zs->next_in  = (Bytef *)src;
    zs->avail_in = (uInt)srcLen;
    zs->next_out  = (Bytef *)dst;
    zs->avail_out = (uInt)dstMax;

    int zflush = flush ? Z_FINISH : Z_NO_FLUSH;
    int ret = deflate(zs, zflush);

    *produced = (int32_t)(dstMax - (int32_t)zs->avail_out);

    switch (ret) {
    case Z_OK:         return RC_OK;
    case Z_STREAM_END: return RC_STREAM_END;
    case Z_BUF_ERROR:  return RC_NEED_MORE;
    default:           return RC_ERROR;
    }
}

int32_t m2_deflate_end(void *s) {
    if (!s) return RC_ERROR;
    z_stream *zs = (z_stream *)s;
    int ret = deflateEnd(zs);
    free(zs);
    return (ret == Z_OK) ? RC_OK : RC_ERROR;
}

/* ── Inflate ──────────────────────────────────────────────────── */

void *m2_inflate_init(int32_t windowBits) {
    z_stream *s = (z_stream *)malloc(sizeof(z_stream));
    if (!s) return NULL;
    memset(s, 0, sizeof(z_stream));

    int ret = inflateInit2(s, (int)windowBits);
    if (ret != Z_OK) {
        free(s);
        return NULL;
    }
    return (void *)s;
}

int32_t m2_inflate(void *s, void *src, int32_t srcLen,
                    void *dst, int32_t dstMax, int32_t *produced) {
    if (!s) return RC_ERROR;
    z_stream *zs = (z_stream *)s;

    zs->next_in  = (Bytef *)src;
    zs->avail_in = (uInt)srcLen;
    zs->next_out  = (Bytef *)dst;
    zs->avail_out = (uInt)dstMax;

    int ret = inflate(zs, Z_NO_FLUSH);

    *produced = (int32_t)(dstMax - (int32_t)zs->avail_out);

    switch (ret) {
    case Z_OK:         return RC_OK;
    case Z_STREAM_END: return RC_STREAM_END;
    case Z_BUF_ERROR:  return RC_NEED_MORE;
    default:           return RC_ERROR;
    }
}

int32_t m2_inflate_end(void *s) {
    if (!s) return RC_ERROR;
    z_stream *zs = (z_stream *)s;
    int ret = inflateEnd(zs);
    free(zs);
    return (ret == Z_OK) ? RC_OK : RC_ERROR;
}
