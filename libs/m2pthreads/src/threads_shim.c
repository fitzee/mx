/*
 * threads_shim.c — Minimal pthreads bridge for m2c Modula-2.
 *
 * Each function is a direct 1:1 wrapper around the corresponding
 * pthread_* call.  No logic, no allocation beyond what pthread needs.
 *
 * Function names use the m2_ prefix to avoid collisions with the
 * m2c transpiler's module_procedure naming convention.
 *
 * Compile with: cc -c threads_shim.c
 * Link with: -lpthread
 */

#include <pthread.h>
#include <stdlib.h>
#include <stdint.h>

/* ── Mutex ─────────────────────────────────────────── */

void m2_threads_mutex_init(void **m)
{
    pthread_mutex_t *mtx = (pthread_mutex_t *)malloc(sizeof(pthread_mutex_t));
    pthread_mutexattr_t attr;
    pthread_mutexattr_init(&attr);
    pthread_mutexattr_settype(&attr, PTHREAD_MUTEX_RECURSIVE);
    pthread_mutex_init(mtx, &attr);
    pthread_mutexattr_destroy(&attr);
    *m = mtx;
}

void m2_threads_mutex_destroy(void **m)
{
    if (*m != NULL) {
        pthread_mutex_destroy((pthread_mutex_t *)*m);
        free(*m);
        *m = NULL;
    }
}

void m2_threads_mutex_lock(void *m)
{
    pthread_mutex_lock((pthread_mutex_t *)m);
}

void m2_threads_mutex_unlock(void *m)
{
    pthread_mutex_unlock((pthread_mutex_t *)m);
}

/* ── Condition Variable ────────────────────────────── */

void m2_threads_cond_init(void **c)
{
    pthread_cond_t *cv = (pthread_cond_t *)malloc(sizeof(pthread_cond_t));
    pthread_cond_init(cv, NULL);
    *c = cv;
}

void m2_threads_cond_destroy(void **c)
{
    if (*c != NULL) {
        pthread_cond_destroy((pthread_cond_t *)*c);
        free(*c);
        *c = NULL;
    }
}

void m2_threads_cond_wait(void *c, void *m)
{
    pthread_cond_wait((pthread_cond_t *)c, (pthread_mutex_t *)m);
}

void m2_threads_cond_signal(void *c)
{
    pthread_cond_signal((pthread_cond_t *)c);
}

void m2_threads_cond_broadcast(void *c)
{
    pthread_cond_broadcast((pthread_cond_t *)c);
}

/* ── Thread ────────────────────────────────────────── */

typedef void (*M2ThreadProc)(void *);

typedef struct {
    M2ThreadProc proc;
    void        *arg;
} ThreadArg;

static void *thread_entry(void *raw)
{
    ThreadArg *ta = (ThreadArg *)raw;
    M2ThreadProc proc = ta->proc;
    void *arg = ta->arg;
    free(ta);
    proc(arg);
    return NULL;
}

void m2_threads_spawn(M2ThreadProc proc, void *arg)
{
    pthread_t tid;
    pthread_attr_t attr;
    ThreadArg *ta = (ThreadArg *)malloc(sizeof(ThreadArg));
    ta->proc = proc;
    ta->arg  = arg;

    pthread_attr_init(&attr);
    pthread_attr_setdetachstate(&attr, PTHREAD_CREATE_DETACHED);
    pthread_attr_setstacksize(&attr, 2 * 1024 * 1024);  /* 2 MB */
    pthread_create(&tid, &attr, thread_entry, ta);
    pthread_attr_destroy(&attr);
}
