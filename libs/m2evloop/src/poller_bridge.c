/*
 * poller_bridge.c — Platform I/O polling bridge for Modula-2+
 *
 * Uses kqueue on macOS/BSD, epoll on Linux, poll as fallback.
 * Every function is a minimal wrapper.  Returns -1 on error.
 * The M2+ layer (Poller.mod) owns all higher-level logic.
 */

#include "poller_bridge.h"

#include <errno.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

/* ── Platform detection ──────────────────────────────────────────── */

#if defined(__APPLE__) || defined(__FreeBSD__) || defined(__OpenBSD__) || defined(__NetBSD__)
  #define USE_KQUEUE 1
#elif defined(__linux__)
  #define USE_EPOLL 1
#else
  #define USE_POLL 1
#endif

#if defined(USE_KQUEUE)
  #include <sys/event.h>
#elif defined(USE_EPOLL)
  #include <sys/epoll.h>
#elif defined(USE_POLL)
  #include <poll.h>
#endif

/* ── Internal state ──────────────────────────────────────────────── */

#define MAX_POLLERS 16

typedef struct {
    int active;
    int backend_fd;        /* kqueue fd or epoll fd; -1 for poll */
#if defined(USE_POLL)
    struct pollfd *pfds;
    int nfds;
    int capacity;
#endif
} poller_state;

static poller_state g_pollers[MAX_POLLERS];
static int g_init_done = 0;

static void ensure_init(void)
{
    if (g_init_done) return;
    memset(g_pollers, 0, sizeof(g_pollers));
    for (int i = 0; i < MAX_POLLERS; i++) {
        g_pollers[i].backend_fd = -1;
    }
    g_init_done = 1;
}

/* ── Create / Destroy ────────────────────────────────────────────── */

int32_t m2_poller_create(void)
{
    ensure_init();

    /* Find a free slot */
    int slot = -1;
    for (int i = 0; i < MAX_POLLERS; i++) {
        if (!g_pollers[i].active) { slot = i; break; }
    }
    if (slot < 0) return -1;

    poller_state *ps = &g_pollers[slot];

#if defined(USE_KQUEUE)
    int kq = kqueue();
    if (kq < 0) return -1;
    ps->backend_fd = kq;
#elif defined(USE_EPOLL)
    int ep = epoll_create1(0);
    if (ep < 0) return -1;
    ps->backend_fd = ep;
#elif defined(USE_POLL)
    ps->capacity = 64;
    ps->pfds = (struct pollfd *)calloc((size_t)ps->capacity,
                                        sizeof(struct pollfd));
    if (!ps->pfds) return -1;
    ps->nfds = 0;
    ps->backend_fd = -1;
#endif

    ps->active = 1;
    return (int32_t)slot;
}

void m2_poller_destroy(int32_t handle)
{
    if (handle < 0 || handle >= MAX_POLLERS) return;
    poller_state *ps = &g_pollers[handle];
    if (!ps->active) return;

#if defined(USE_KQUEUE) || defined(USE_EPOLL)
    if (ps->backend_fd >= 0) close(ps->backend_fd);
#elif defined(USE_POLL)
    free(ps->pfds);
    ps->pfds = NULL;
    ps->nfds = 0;
#endif

    ps->backend_fd = -1;
    ps->active = 0;
}

/* ── Add / Modify / Delete ───────────────────────────────────────── */

#if defined(USE_KQUEUE)

static int kq_update(int kq, int fd, int events, int add)
{
    struct kevent changes[2];
    int n = 0;

    if (add) {
        /* Add filters for requested events */
        if (events & M2_EV_READ) {
            EV_SET(&changes[n], fd, EVFILT_READ, EV_ADD | EV_CLEAR, 0, 0, NULL);
            n++;
        }
        if (events & M2_EV_WRITE) {
            EV_SET(&changes[n], fd, EVFILT_WRITE, EV_ADD | EV_CLEAR, 0, 0, NULL);
            n++;
        }
    } else {
        /* Delete all filters */
        EV_SET(&changes[n], fd, EVFILT_READ, EV_DELETE, 0, 0, NULL);
        n++;
        EV_SET(&changes[n], fd, EVFILT_WRITE, EV_DELETE, 0, 0, NULL);
        n++;
    }

    if (n == 0) return 0;
    /* kevent delete may return ENOENT for unregistered filters; ignore */
    int rc = kevent(kq, changes, n, NULL, 0, NULL);
    if (rc < 0 && add) return -1;
    return 0;
}

#endif /* USE_KQUEUE */

int32_t m2_poller_add(int32_t handle, int32_t fd, int32_t events)
{
    if (handle < 0 || handle >= MAX_POLLERS) return -1;
    poller_state *ps = &g_pollers[handle];
    if (!ps->active) return -1;

#if defined(USE_KQUEUE)
    return kq_update(ps->backend_fd, fd, events, 1);
#elif defined(USE_EPOLL)
    struct epoll_event ev;
    memset(&ev, 0, sizeof(ev));
    ev.data.fd = fd;
    if (events & M2_EV_READ)  ev.events |= EPOLLIN;
    if (events & M2_EV_WRITE) ev.events |= EPOLLOUT;
    return epoll_ctl(ps->backend_fd, EPOLL_CTL_ADD, fd, &ev) == 0 ? 0 : -1;
#elif defined(USE_POLL)
    if (ps->nfds >= ps->capacity) {
        int newcap = ps->capacity * 2;
        struct pollfd *np = (struct pollfd *)realloc(ps->pfds,
                            (size_t)newcap * sizeof(struct pollfd));
        if (!np) return -1;
        ps->pfds = np;
        ps->capacity = newcap;
    }
    struct pollfd *pf = &ps->pfds[ps->nfds];
    pf->fd = fd;
    pf->events = 0;
    if (events & M2_EV_READ)  pf->events |= POLLIN;
    if (events & M2_EV_WRITE) pf->events |= POLLOUT;
    pf->revents = 0;
    ps->nfds++;
    return 0;
#endif
}

int32_t m2_poller_mod(int32_t handle, int32_t fd, int32_t events)
{
    if (handle < 0 || handle >= MAX_POLLERS) return -1;
    poller_state *ps = &g_pollers[handle];
    if (!ps->active) return -1;

#if defined(USE_KQUEUE)
    /* For kqueue, delete and re-add */
    kq_update(ps->backend_fd, fd, 0, 0);
    return kq_update(ps->backend_fd, fd, events, 1);
#elif defined(USE_EPOLL)
    struct epoll_event ev;
    memset(&ev, 0, sizeof(ev));
    ev.data.fd = fd;
    if (events & M2_EV_READ)  ev.events |= EPOLLIN;
    if (events & M2_EV_WRITE) ev.events |= EPOLLOUT;
    return epoll_ctl(ps->backend_fd, EPOLL_CTL_MOD, fd, &ev) == 0 ? 0 : -1;
#elif defined(USE_POLL)
    for (int i = 0; i < ps->nfds; i++) {
        if (ps->pfds[i].fd == fd) {
            ps->pfds[i].events = 0;
            if (events & M2_EV_READ)  ps->pfds[i].events |= POLLIN;
            if (events & M2_EV_WRITE) ps->pfds[i].events |= POLLOUT;
            return 0;
        }
    }
    return -1;
#endif
}

int32_t m2_poller_del(int32_t handle, int32_t fd)
{
    if (handle < 0 || handle >= MAX_POLLERS) return -1;
    poller_state *ps = &g_pollers[handle];
    if (!ps->active) return -1;

#if defined(USE_KQUEUE)
    return kq_update(ps->backend_fd, fd, 0, 0);
#elif defined(USE_EPOLL)
    return epoll_ctl(ps->backend_fd, EPOLL_CTL_DEL, fd, NULL) == 0 ? 0 : -1;
#elif defined(USE_POLL)
    for (int i = 0; i < ps->nfds; i++) {
        if (ps->pfds[i].fd == fd) {
            /* Swap-remove */
            ps->pfds[i] = ps->pfds[ps->nfds - 1];
            ps->nfds--;
            return 0;
        }
    }
    return -1;
#endif
}

/* ── Wait ────────────────────────────────────────────────────────── */

int32_t m2_poller_wait(int32_t handle, int32_t timeout_ms,
                       m2_poll_event *out, int32_t max_events)
{
    if (handle < 0 || handle >= MAX_POLLERS) return -1;
    poller_state *ps = &g_pollers[handle];
    if (!ps->active) return -1;
    if (max_events <= 0) return 0;

#if defined(USE_KQUEUE)
    struct kevent kevs[64];
    int maxk = max_events < 64 ? max_events : 64;

    struct timespec ts, *tsp = NULL;
    if (timeout_ms >= 0) {
        ts.tv_sec  = timeout_ms / 1000;
        ts.tv_nsec = (timeout_ms % 1000) * 1000000L;
        tsp = &ts;
    }

    int n = kevent(ps->backend_fd, NULL, 0, kevs, maxk, tsp);
    if (n < 0) {
        if (errno == EINTR) return 0;
        return -1;
    }

    for (int i = 0; i < n; i++) {
        out[i].fd = (int32_t)kevs[i].ident;
        out[i].events = 0;
        if (kevs[i].filter == EVFILT_READ)  out[i].events |= M2_EV_READ;
        if (kevs[i].filter == EVFILT_WRITE) out[i].events |= M2_EV_WRITE;
        if (kevs[i].flags & EV_ERROR)       out[i].events |= M2_EV_ERROR;
        if (kevs[i].flags & EV_EOF)         out[i].events |= M2_EV_HUP;
    }
    return (int32_t)n;

#elif defined(USE_EPOLL)
    struct epoll_event evs[64];
    int maxe = max_events < 64 ? max_events : 64;

    int n = epoll_wait(ps->backend_fd, evs, maxe, timeout_ms);
    if (n < 0) {
        if (errno == EINTR) return 0;
        return -1;
    }

    for (int i = 0; i < n; i++) {
        out[i].fd = evs[i].data.fd;
        out[i].events = 0;
        if (evs[i].events & EPOLLIN)  out[i].events |= M2_EV_READ;
        if (evs[i].events & EPOLLOUT) out[i].events |= M2_EV_WRITE;
        if (evs[i].events & EPOLLERR) out[i].events |= M2_EV_ERROR;
        if (evs[i].events & EPOLLHUP) out[i].events |= M2_EV_HUP;
    }
    return (int32_t)n;

#elif defined(USE_POLL)
    if (ps->nfds == 0) {
        if (timeout_ms > 0) {
            struct timespec ts;
            ts.tv_sec  = timeout_ms / 1000;
            ts.tv_nsec = (timeout_ms % 1000) * 1000000L;
            nanosleep(&ts, NULL);
        }
        return 0;
    }

    int n = poll(ps->pfds, (nfds_t)ps->nfds, timeout_ms);
    if (n < 0) {
        if (errno == EINTR) return 0;
        return -1;
    }

    int count = 0;
    for (int i = 0; i < ps->nfds && count < max_events; i++) {
        if (ps->pfds[i].revents == 0) continue;
        out[count].fd = ps->pfds[i].fd;
        out[count].events = 0;
        if (ps->pfds[i].revents & POLLIN)  out[count].events |= M2_EV_READ;
        if (ps->pfds[i].revents & POLLOUT) out[count].events |= M2_EV_WRITE;
        if (ps->pfds[i].revents & POLLERR) out[count].events |= M2_EV_ERROR;
        if (ps->pfds[i].revents & POLLHUP) out[count].events |= M2_EV_HUP;
        count++;
    }
    return (int32_t)count;
#endif
}

/* ── Monotonic clock ─────────────────────────────────────────────── */

int32_t m2_now_ms(void)
{
#if defined(__APPLE__)
    /* clock_gettime is available on macOS 10.12+ */
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    /* Truncate to 32 bits — wraps every ~24.8 days */
    uint64_t ms = (uint64_t)ts.tv_sec * 1000ULL +
                  (uint64_t)ts.tv_nsec / 1000000ULL;
    return (int32_t)(ms & 0xFFFFFFFF);
#elif defined(__linux__)
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    uint64_t ms = (uint64_t)ts.tv_sec * 1000ULL +
                  (uint64_t)ts.tv_nsec / 1000000ULL;
    return (int32_t)(ms & 0xFFFFFFFF);
#else
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    uint64_t ms = (uint64_t)ts.tv_sec * 1000ULL +
                  (uint64_t)ts.tv_nsec / 1000000ULL;
    return (int32_t)(ms & 0xFFFFFFFF);
#endif
}
