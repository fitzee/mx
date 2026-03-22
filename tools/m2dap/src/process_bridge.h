#ifndef PROCESS_BRIDGE_H
#define PROCESS_BRIDGE_H

#include <stdint.h>

/* Spawn a subprocess with bidirectional pipes.
   program: path to executable (NUL-terminated)
   args:    space-separated arguments (NUL-terminated), may be empty
   Returns PID on success, -1 on failure.
   On success, *stdin_fd, *stdout_fd, *stderr_fd are set. */
int32_t m2dap_spawn(const char *program, const char *args,
                    int32_t *stdin_fd, int32_t *stdout_fd,
                    int32_t *stderr_fd);

/* Read up to max bytes from fd. Returns bytes read, 0 on EOF, -1 on error. */
int32_t m2dap_read(int32_t fd, char *buf, int32_t max);

/* Write len bytes to fd. Returns bytes written or -1 on error. */
int32_t m2dap_write(int32_t fd, const char *buf, int32_t len);

/* Close a file descriptor. */
void m2dap_close(int32_t fd);

/* Wait for process. block=1 for blocking wait.
   Returns exit status, or -1 if not exited yet (non-blocking). */
int32_t m2dap_waitpid(int32_t pid, int32_t block);

/* Send signal to process. sig=9 for SIGKILL, sig=15 for SIGTERM. */
int32_t m2dap_kill(int32_t pid, int32_t sig);

/* Raw read/write on file descriptors 0 (stdin) and 1 (stdout).
   These bypass stdio buffering for DAP transport. */
int32_t m2dap_read_stdin(char *buf, int32_t max);
int32_t m2dap_write_stdout(const char *buf, int32_t len);

#endif
