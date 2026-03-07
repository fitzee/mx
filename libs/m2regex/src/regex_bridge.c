#include "regex_bridge.h"
#include <regex.h>
#include <stdlib.h>
#include <string.h>

static char last_error[256] = {0};
static int last_errcode = 0;
static regex_t *last_failed_re = NULL;

void *m2_regex_compile(const char *pattern) {
    regex_t *re = (regex_t *)malloc(sizeof(regex_t));
    if (!re) return NULL;
    int rc = regcomp(re, pattern, REG_EXTENDED);
    if (rc != 0) {
        regerror(rc, re, last_error, sizeof(last_error));
        regfree(re);
        free(re);
        return NULL;
    }
    return re;
}

void m2_regex_free(void *re) {
    if (re) {
        regfree((regex_t *)re);
        free(re);
    }
}

int32_t m2_regex_test(void *re, const char *text) {
    if (!re || !text) return 0;
    return regexec((regex_t *)re, text, 0, NULL, 0) == 0 ? 1 : 0;
}

int32_t m2_regex_find(void *re, const char *text, int32_t *start, int32_t *len) {
    if (!re || !text) return -1;
    regmatch_t match;
    int rc = regexec((regex_t *)re, text, 1, &match, 0);
    if (rc == REG_NOMATCH) return 1;  /* NoMatch */
    if (rc != 0) return -1;  /* Error */
    *start = (int32_t)match.rm_so;
    *len = (int32_t)(match.rm_eo - match.rm_so);
    return 0;  /* Ok */
}

int32_t m2_regex_find_all(void *re, const char *text,
                           int32_t *starts, int32_t *lens,
                           int32_t maxMatches, int32_t *count) {
    if (!re || !text) return -1;
    *count = 0;
    const char *p = text;
    int offset = 0;
    regmatch_t match;
    while (*count < maxMatches) {
        int rc = regexec((regex_t *)re, p, 1, &match, (*count > 0) ? REG_NOTBOL : 0);
        if (rc == REG_NOMATCH) break;
        if (rc != 0) return -1;
        starts[*count] = offset + (int32_t)match.rm_so;
        lens[*count] = (int32_t)(match.rm_eo - match.rm_so);
        if (match.rm_eo == match.rm_so) {
            /* zero-length match — advance by one to avoid infinite loop */
            offset += (int32_t)match.rm_so + 1;
            p += match.rm_so + 1;
            if (*p == '\0') break;
        } else {
            offset += (int32_t)match.rm_eo;
            p += match.rm_eo;
        }
        (*count)++;
    }
    return (*count > 0) ? 0 : 1;
}

void m2_regex_error(char *buf, int32_t bufLen) {
    if (bufLen <= 0) return;
    int32_t len = (int32_t)strlen(last_error);
    if (len >= bufLen) len = bufLen - 1;
    memcpy(buf, last_error, (size_t)len);
    buf[len] = '\0';
}
