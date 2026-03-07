IMPLEMENTATION MODULE Threads;

FROM SYSTEM IMPORT ADDRESS, ADR;
FROM ThreadsBridge IMPORT
  m2_threads_mutex_init, m2_threads_mutex_destroy,
  m2_threads_mutex_lock, m2_threads_mutex_unlock,
  m2_threads_cond_init, m2_threads_cond_destroy,
  m2_threads_cond_wait, m2_threads_cond_signal, m2_threads_cond_broadcast,
  m2_threads_spawn;

(* ── Mutex ─────────────────────────────────────────── *)

PROCEDURE MutexInit(VAR m: Mutex);
BEGIN
  m2_threads_mutex_init(m)
END MutexInit;

PROCEDURE MutexDestroy(VAR m: Mutex);
BEGIN
  m2_threads_mutex_destroy(m)
END MutexDestroy;

PROCEDURE MutexLock(m: Mutex);
BEGIN
  m2_threads_mutex_lock(m)
END MutexLock;

PROCEDURE MutexUnlock(m: Mutex);
BEGIN
  m2_threads_mutex_unlock(m)
END MutexUnlock;

(* ── Condition Variable ────────────────────────────── *)

PROCEDURE CondInit(VAR c: Cond);
BEGIN
  m2_threads_cond_init(c)
END CondInit;

PROCEDURE CondDestroy(VAR c: Cond);
BEGIN
  m2_threads_cond_destroy(c)
END CondDestroy;

PROCEDURE CondWait(c: Cond; m: Mutex);
BEGIN
  m2_threads_cond_wait(c, m)
END CondWait;

PROCEDURE CondSignal(c: Cond);
BEGIN
  m2_threads_cond_signal(c)
END CondSignal;

PROCEDURE CondBroadcast(c: Cond);
BEGIN
  m2_threads_cond_broadcast(c)
END CondBroadcast;

(* ── Thread ────────────────────────────────────────── *)

PROCEDURE SpawnThread(proc: ThreadProc; arg: ADDRESS);
BEGIN
  m2_threads_spawn(proc, arg)
END SpawnThread;

END Threads.
