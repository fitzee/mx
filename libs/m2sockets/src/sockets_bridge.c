/*
 * sockets_bridge.c — Thin POSIX/BSD sockets bridge for Modula-2+
 *
 * Every function is a minimal wrapper around one syscall group.
 * Returns -1 on error (errno set); non-negative on success.
 * The M2+ layer (Sockets.mod) owns all higher-level logic.
 */

#include "sockets_bridge.h"

#include <errno.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>
#include <fcntl.h>
#include <sys/types.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <netdb.h>
#include <arpa/inet.h>

/* ── Lifecycle ──────────────────────────────────────────── */

int32_t m2_socket(int32_t family, int32_t socktype)
{
    int fd = socket(family, socktype, 0);
    return (int32_t)fd;          /* -1 on error */
}

int32_t m2_close(int32_t fd)
{
    return close(fd) == 0 ? 0 : -1;
}

int32_t m2_shutdown(int32_t fd, int32_t how)
{
    return shutdown(fd, how) == 0 ? 0 : -1;
}

/* ── Server ─────────────────────────────────────────────── */

int32_t m2_bind_any(int32_t fd, int32_t port)
{
    struct sockaddr_in sa;
    memset(&sa, 0, sizeof(sa));
    sa.sin_family      = AF_INET;
    sa.sin_addr.s_addr = htonl(INADDR_ANY);
    sa.sin_port        = htons((uint16_t)port);
    return bind(fd, (struct sockaddr *)&sa, sizeof(sa)) == 0 ? 0 : -1;
}

int32_t m2_listen(int32_t fd, int32_t backlog)
{
    return listen(fd, backlog) == 0 ? 0 : -1;
}

int32_t m2_accept(int32_t fd,
                  int32_t *out_fd,
                  int32_t *out_family,
                  int32_t *out_port,
                  void    *out_addr4)
{
    struct sockaddr_in sa;
    socklen_t len = sizeof(sa);
    int cfd = accept(fd, (struct sockaddr *)&sa, &len);
    if (cfd < 0) return -1;

    *out_fd     = (int32_t)cfd;
    *out_family = (int32_t)sa.sin_family;
    *out_port   = (int32_t)ntohs(sa.sin_port);

    /* Copy raw IPv4 bytes */
    uint8_t *dst = (uint8_t *)out_addr4;
    uint32_t addr = ntohl(sa.sin_addr.s_addr);
    dst[0] = (uint8_t)(addr >> 24);
    dst[1] = (uint8_t)(addr >> 16);
    dst[2] = (uint8_t)(addr >>  8);
    dst[3] = (uint8_t)(addr);

    return 0;
}

/* ── Client ─────────────────────────────────────────────── */

int32_t m2_connect_host_port(int32_t fd, void *host, int32_t port)
{
    const char *hostname = (const char *)host;
    char port_str[16];
    snprintf(port_str, sizeof(port_str), "%d", (int)port);

    struct addrinfo hints, *res, *rp;
    memset(&hints, 0, sizeof(hints));
    hints.ai_family   = AF_INET;       /* IPv4 only */
    hints.ai_socktype = SOCK_STREAM;

    int rc = getaddrinfo(hostname, port_str, &hints, &res);
    if (rc != 0) {
        errno = ENOENT;                /* no better mapping */
        return -1;
    }

    int ret = -1;
    for (rp = res; rp != NULL; rp = rp->ai_next) {
        if (connect(fd, rp->ai_addr, rp->ai_addrlen) == 0) {
            ret = 0;
            break;
        }
    }
    freeaddrinfo(res);
    return ret;
}

/* ── I/O ────────────────────────────────────────────────── */

int32_t m2_send(int32_t fd, void *buf, int32_t len)
{
    ssize_t n = send(fd, buf, (size_t)len, 0);
    return (int32_t)n;           /* -1 on error, else bytes sent */
}

int32_t m2_recv(int32_t fd, void *buf, int32_t max)
{
    ssize_t n = recv(fd, buf, (size_t)max, 0);
    return (int32_t)n;           /* -1 on error, 0 on close, else bytes read */
}

/* ── Options ────────────────────────────────────────────── */

int32_t m2_set_nonblocking(int32_t fd, int32_t enable)
{
    int flags = fcntl(fd, F_GETFL, 0);
    if (flags < 0) return -1;
    if (enable)
        flags |= O_NONBLOCK;
    else
        flags &= ~O_NONBLOCK;
    return fcntl(fd, F_SETFL, flags) == 0 ? 0 : -1;
}

int32_t m2_set_reuseaddr(int32_t fd, int32_t enable)
{
    int val = enable ? 1 : 0;
    return setsockopt(fd, SOL_SOCKET, SO_REUSEADDR,
                      &val, sizeof(val)) == 0 ? 0 : -1;
}

int32_t m2_set_reuseport(int32_t fd, int32_t enable)
{
    int val = enable ? 1 : 0;
    return setsockopt(fd, SOL_SOCKET, SO_REUSEPORT,
                      &val, sizeof(val)) == 0 ? 0 : -1;
}

/* ── UDP I/O ───────────────────────────────────────────── */

int32_t m2_sendto(int32_t fd, void *buf, int32_t len,
                   uint8_t a, uint8_t b, uint8_t c, uint8_t d, int32_t port)
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

    ssize_t n = sendto(fd, buf, (size_t)len, 0,
                        (struct sockaddr *)&sa, sizeof(sa));
    return (int32_t)n;  /* -1 on error, else bytes sent */
}

int32_t m2_recvfrom(int32_t fd, void *buf, int32_t maxlen,
                     void *addr_out, int32_t *port_out)
{
    struct sockaddr_in sa;
    socklen_t slen = sizeof(sa);
    memset(&sa, 0, sizeof(sa));

    ssize_t n = recvfrom(fd, buf, (size_t)maxlen, 0,
                          (struct sockaddr *)&sa, &slen);
    if (n < 0) return -1;

    /* Copy sender address from network to host byte order */
    uint32_t addr = ntohl(sa.sin_addr.s_addr);
    uint8_t *dst = (uint8_t *)addr_out;
    dst[0] = (uint8_t)(addr >> 24);
    dst[1] = (uint8_t)(addr >> 16);
    dst[2] = (uint8_t)(addr >>  8);
    dst[3] = (uint8_t)(addr);
    *port_out = (int32_t)ntohs(sa.sin_port);

    return (int32_t)n;
}

int32_t m2_set_multicast(int32_t fd, const char *group, int32_t join)
{
    struct ip_mreq mreq;
    memset(&mreq, 0, sizeof(mreq));

    if (inet_pton(AF_INET, group, &mreq.imr_multiaddr) != 1)
        return -1;

    mreq.imr_interface.s_addr = htonl(INADDR_ANY);

    int opt = join ? IP_ADD_MEMBERSHIP : IP_DROP_MEMBERSHIP;
    return setsockopt(fd, IPPROTO_IP, opt, &mreq, sizeof(mreq)) == 0 ? 0 : -1;
}

int32_t m2_set_broadcast(int32_t fd, int32_t enable)
{
    int val = enable ? 1 : 0;
    return setsockopt(fd, SOL_SOCKET, SO_BROADCAST,
                      &val, sizeof(val)) == 0 ? 0 : -1;
}

/* ── Error ──────────────────────────────────────────────── */

int32_t m2_errno(void)
{
    return (int32_t)errno;
}

void m2_strerror(int32_t errnum, void *buf, int32_t buflen)
{
    if (buflen <= 0) return;
    char *dst = (char *)buf;
#ifdef __APPLE__
    /* macOS strerror_r is XSI-compliant (returns int) */
    if (strerror_r(errnum, dst, (size_t)buflen) != 0) {
        snprintf(dst, (size_t)buflen, "errno %d", (int)errnum);
    }
#else
    /* Linux: try XSI version */
    if (strerror_r(errnum, dst, (size_t)buflen) != 0) {
        snprintf(dst, (size_t)buflen, "errno %d", (int)errnum);
    }
#endif
}
