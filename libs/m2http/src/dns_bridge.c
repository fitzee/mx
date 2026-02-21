/*
 * dns_bridge.c — DNS resolution + socket helpers for m2http
 *
 * Every function is a minimal wrapper around one syscall group.
 * Returns -1 on error; non-negative on success.
 * The M2+ layer owns all higher-level logic.
 */

#include "dns_bridge.h"

#include <errno.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>
#include <sys/types.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <netdb.h>
#include <arpa/inet.h>

/* ── DNS resolution ──────────────────────────────────────────────── */

int32_t m2_dns_resolve_a(const void *host,
                         void *out_addr4,
                         int32_t *out_port,
                         int32_t port)
{
    const char *hostname = (const char *)host;
    struct addrinfo hints, *res, *rp;

    memset(&hints, 0, sizeof(hints));
    hints.ai_family   = AF_INET;        /* IPv4 only */
    hints.ai_socktype = SOCK_STREAM;

    int rc = getaddrinfo(hostname, NULL, &hints, &res);
    if (rc != 0) {
        errno = ENOENT;
        return -1;
    }

    /* Take the first A record */
    for (rp = res; rp != NULL; rp = rp->ai_next) {
        if (rp->ai_family == AF_INET) {
            struct sockaddr_in *sa = (struct sockaddr_in *)rp->ai_addr;
            uint32_t addr = ntohl(sa->sin_addr.s_addr);
            uint8_t *dst = (uint8_t *)out_addr4;
            dst[0] = (uint8_t)(addr >> 24);
            dst[1] = (uint8_t)(addr >> 16);
            dst[2] = (uint8_t)(addr >>  8);
            dst[3] = (uint8_t)(addr);
            *out_port = port;
            freeaddrinfo(res);
            return 0;
        }
    }

    freeaddrinfo(res);
    errno = ENOENT;
    return -1;
}

/* ── Socket helpers ──────────────────────────────────────────────── */

int32_t m2_connect_ipv4(int32_t fd,
                        int32_t a, int32_t b, int32_t c, int32_t d,
                        int32_t port)
{
    struct sockaddr_in sa;
    memset(&sa, 0, sizeof(sa));
    sa.sin_family = AF_INET;
    sa.sin_port   = htons((uint16_t)port);
    sa.sin_addr.s_addr = htonl(
        ((uint32_t)a << 24) |
        ((uint32_t)b << 16) |
        ((uint32_t)c <<  8) |
        ((uint32_t)d)
    );

    int rc = connect(fd, (struct sockaddr *)&sa, sizeof(sa));
    if (rc == 0) return 0;                      /* connected */
    if (errno == EINPROGRESS || errno == EWOULDBLOCK) return 1; /* in progress */
    return -1;                                  /* error */
}

int32_t m2_getsockopt_error(int32_t fd)
{
    int err = 0;
    socklen_t len = sizeof(err);
    if (getsockopt(fd, SOL_SOCKET, SO_ERROR, &err, &len) < 0) {
        return errno;
    }
    return err;
}

/* ── Error ───────────────────────────────────────────────────────── */

int32_t m2_dns_errno(void)
{
    return (int32_t)errno;
}

void m2_dns_strerror(int32_t errnum, void *buf, int32_t buflen)
{
    if (buflen <= 0) return;
    char *dst = (char *)buf;
    if (strerror_r(errnum, dst, (size_t)buflen) != 0) {
        snprintf(dst, (size_t)buflen, "errno %d", (int)errnum);
    }
}
