#include "process_bridge.h"

#include <errno.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>
#include <unistd.h>

/* Maximum number of arguments for spawned process */
#define MAX_ARGS 64

int32_t m2dap_spawn(const char *program, const char *args,
                    int32_t *stdin_fd, int32_t *stdout_fd,
                    int32_t *stderr_fd) {
    int pipe_in[2];   /* parent writes, child reads */
    int pipe_out[2];  /* child writes, parent reads */
    int pipe_err[2];  /* child stderr, parent reads */

    if (pipe(pipe_in) < 0)  return -1;
    if (pipe(pipe_out) < 0) { close(pipe_in[0]); close(pipe_in[1]); return -1; }
    if (pipe(pipe_err) < 0) {
        close(pipe_in[0]); close(pipe_in[1]);
        close(pipe_out[0]); close(pipe_out[1]);
        return -1;
    }

    pid_t pid = fork();
    if (pid < 0) {
        close(pipe_in[0]); close(pipe_in[1]);
        close(pipe_out[0]); close(pipe_out[1]);
        close(pipe_err[0]); close(pipe_err[1]);
        return -1;
    }

    if (pid == 0) {
        /* Child process */
        dup2(pipe_in[0], STDIN_FILENO);
        dup2(pipe_out[1], STDOUT_FILENO);
        dup2(pipe_err[1], STDERR_FILENO);

        close(pipe_in[0]); close(pipe_in[1]);
        close(pipe_out[0]); close(pipe_out[1]);
        close(pipe_err[0]); close(pipe_err[1]);

        /* Parse args into argv array */
        char *argv[MAX_ARGS];
        int argc = 0;

        argv[argc++] = (char *)program;

        if (args != NULL && args[0] != '\0') {
            /* Copy args so we can tokenize */
            char *args_copy = strdup(args);
            if (args_copy) {
                char *tok = strtok(args_copy, " ");
                while (tok != NULL && argc < MAX_ARGS - 1) {
                    argv[argc++] = tok;
                    tok = strtok(NULL, " ");
                }
            }
        }
        argv[argc] = NULL;

        execvp(program, argv);
        _exit(127);
    }

    /* Parent process */
    close(pipe_in[0]);
    close(pipe_out[1]);
    close(pipe_err[1]);

    *stdin_fd  = pipe_in[1];
    *stdout_fd = pipe_out[0];
    *stderr_fd = pipe_err[0];

    return (int32_t)pid;
}

int32_t m2dap_read(int32_t fd, char *buf, int32_t max) {
    ssize_t n = read(fd, buf, (size_t)max);
    if (n < 0) {
        if (errno == EINTR) return 0;
        return -1;
    }
    return (int32_t)n;
}

int32_t m2dap_write(int32_t fd, const char *buf, int32_t len) {
    ssize_t total = 0;
    while (total < len) {
        ssize_t n = write(fd, buf + total, (size_t)(len - total));
        if (n < 0) {
            if (errno == EINTR) continue;
            return -1;
        }
        total += n;
    }
    return (int32_t)total;
}

void m2dap_close(int32_t fd) {
    close(fd);
}

int32_t m2dap_waitpid(int32_t pid, int32_t block) {
    int status;
    pid_t result = waitpid((pid_t)pid, &status, block ? 0 : WNOHANG);
    if (result < 0) return -1;
    if (result == 0) return -1;  /* Not exited yet (non-blocking) */
    if (WIFEXITED(status)) return WEXITSTATUS(status);
    if (WIFSIGNALED(status)) return 128 + WTERMSIG(status);
    return -1;
}

int32_t m2dap_kill(int32_t pid, int32_t sig) {
    return kill((pid_t)pid, sig) == 0 ? 0 : -1;
}

int32_t m2dap_read_stdin(char *buf, int32_t max) {
    ssize_t n = read(STDIN_FILENO, buf, (size_t)max);
    if (n < 0) {
        if (errno == EINTR) return 0;
        return -1;
    }
    return (int32_t)n;
}

int32_t m2dap_write_stdout(const char *buf, int32_t len) {
    ssize_t total = 0;
    while (total < len) {
        ssize_t n = write(STDOUT_FILENO, buf + total, (size_t)(len - total));
        if (n < 0) {
            if (errno == EINTR) continue;
            return -1;
        }
        total += n;
    }
    return (int32_t)total;
}
