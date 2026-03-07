#ifndef ZLIB_BRIDGE_H
#define ZLIB_BRIDGE_H
#include <stdint.h>
void *m2_deflate_init(int32_t level, int32_t windowBits);
int32_t m2_deflate(void *s, void *src, int32_t srcLen,
                    void *dst, int32_t dstMax,
                    int32_t *produced, int32_t flush);
int32_t m2_deflate_end(void *s);
void *m2_inflate_init(int32_t windowBits);
int32_t m2_inflate(void *s, void *src, int32_t srcLen,
                    void *dst, int32_t dstMax, int32_t *produced);
int32_t m2_inflate_end(void *s);
#endif
