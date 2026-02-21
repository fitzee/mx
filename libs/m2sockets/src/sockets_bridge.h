#ifndef SOCKETS_BRIDGE_H
#define SOCKETS_BRIDGE_H

#include <stdint.h>

/* Lifecycle */
int32_t m2_socket(int32_t family, int32_t socktype);
int32_t m2_close(int32_t fd);
int32_t m2_shutdown(int32_t fd, int32_t how);

/* Server */
int32_t m2_bind_any(int32_t fd, int32_t port);
int32_t m2_listen(int32_t fd, int32_t backlog);
int32_t m2_accept(int32_t fd,
                  int32_t *out_fd,
                  int32_t *out_family,
                  int32_t *out_port,
                  void    *out_addr4);  /* 4 bytes */

/* Client */
int32_t m2_connect_host_port(int32_t fd, void *host, int32_t port);

/* I/O */
int32_t m2_send(int32_t fd, void *buf, int32_t len);
int32_t m2_recv(int32_t fd, void *buf, int32_t max);

/* Options */
int32_t m2_set_nonblocking(int32_t fd, int32_t enable);
int32_t m2_set_reuseaddr(int32_t fd, int32_t enable);

/* Error */
int32_t m2_errno(void);
void    m2_strerror(int32_t errnum, void *buf, int32_t buflen);

#endif
