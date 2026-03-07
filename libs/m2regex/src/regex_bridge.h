#ifndef REGEX_BRIDGE_H
#define REGEX_BRIDGE_H
#include <stdint.h>

void *m2_regex_compile(const char *pattern);
void m2_regex_free(void *re);
int32_t m2_regex_test(void *re, const char *text);
int32_t m2_regex_find(void *re, const char *text, int32_t *start, int32_t *len);
int32_t m2_regex_find_all(void *re, const char *text,
                           int32_t *starts, int32_t *lens,
                           int32_t maxMatches, int32_t *count);
void m2_regex_error(char *buf, int32_t bufLen);

#endif
