#include "process_bridge.h"

#include <errno.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>
#include <unistd.h>
#include <util.h>     /* forkpty() on macOS */
#include <termios.h>

/* Maximum number of arguments for spawned process */
#define MAX_ARGS 64

int32_t m2dap_spawn(const char *program, const char *args,
                    int32_t *stdin_fd, int32_t *stdout_fd,
                    int32_t *stderr_fd) {
    /*
     * Use a pseudo-tty for the child's stdin/stdout so that lldb
     * sees a terminal and flushes prompts immediately (without a tty,
     * lldb buffers stdout and the prompt never arrives).
     * stderr still uses a plain pipe.
     */
    int pty_master;
    int pipe_err[2];

    if (pipe(pipe_err) < 0) return -1;

    pid_t pid = forkpty(&pty_master, NULL, NULL, NULL);
    if (pid < 0) {
        close(pipe_err[0]); close(pipe_err[1]);
        return -1;
    }

    if (pid == 0) {
        /* Child process — stdin/stdout are the pty slave (set by forkpty) */
        close(pipe_err[0]);
        dup2(pipe_err[1], STDERR_FILENO);
        close(pipe_err[1]);

        /* Parse args into argv array */
        char *argv[MAX_ARGS];
        int argc = 0;

        argv[argc++] = (char *)program;

        if (args != NULL && args[0] != '\0') {
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
    close(pipe_err[1]);

    /* Disable echo on the pty so we don't get double output */
    struct termios t;
    if (tcgetattr(pty_master, &t) == 0) {
        t.c_lflag &= ~(ECHO | ECHONL);
        tcsetattr(pty_master, TCSANOW, &t);
    }

    /* pty_master is used for both reading and writing */
    *stdin_fd  = pty_master;
    *stdout_fd = dup(pty_master);  /* dup so caller can close independently */
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
