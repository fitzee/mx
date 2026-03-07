IMPLEMENTATION MODULE EventLoop;

FROM SYSTEM IMPORT ADDRESS, TSIZE;
FROM Storage IMPORT ALLOCATE, DEALLOCATE;
FROM Scheduler IMPORT Scheduler, TaskProc,
                      SchedulerCreate, SchedulerDestroy,
                      SchedulerEnqueue, SchedulerPump;
FROM Timers IMPORT TimerId, TimerQueue;
FROM Poller IMPORT PollEvent, EventBuf, MaxEvents, EvRead, EvWrite;
IMPORT Poller;
IMPORT Timers;
IMPORT Scheduler;

(* ── Internal types ────────────────────────────────────────────── *)

CONST
  MaxWatchers = 512;
  SchedCapacity = 1024;

TYPE
  WatcherEntry = RECORD
    fd     : INTEGER;
    events : INTEGER;
    cb     : WatcherProc;
    user   : ADDRESS;
    active : BOOLEAN;
  END;

  LoopRec = RECORD
    poller      : INTEGER;     (* Poller handle *)
    timers      : TimerQueue;
    sched       : Scheduler;
    watchers    : ARRAY [0..MaxWatchers-1] OF WatcherEntry;
    nWatchers   : INTEGER;
    running     : BOOLEAN;
    stopFlag    : BOOLEAN;
  END;

  LoopPtr = POINTER TO LoopRec;

(* ── Watcher lookup ────────────────────────────────────────────── *)

PROCEDURE FindWatcher(lp: LoopPtr; fd: INTEGER): INTEGER;
VAR i: INTEGER;
BEGIN
  FOR i := 0 TO MaxWatchers - 1 DO
    IF lp^.watchers[i].active AND (lp^.watchers[i].fd = fd) THEN
      RETURN i
    END
  END;
  RETURN -1
END FindWatcher;

(* ── Public procedures ─────────────────────────────────────────── *)

PROCEDURE Create(VAR out: Loop): Status;
VAR
  lp: LoopPtr;
  pst: Poller.Status;
  sst: Scheduler.Status;
  tst: Timers.Status;
  i: INTEGER;
BEGIN
  ALLOCATE(lp, TSIZE(LoopRec));
  IF lp = NIL THEN
    out := NIL;
    RETURN PoolExhausted
  END;

  (* Create poller *)
  pst := Poller.Create(lp^.poller);
  IF pst # Poller.OK THEN
    DEALLOCATE(lp, TSIZE(LoopRec));
    out := NIL;
    RETURN SysError
  END;

  (* Create scheduler *)
  sst := SchedulerCreate(SchedCapacity, lp^.sched);
  IF sst # Scheduler.OK THEN
    pst := Poller.Destroy(lp^.poller);
    DEALLOCATE(lp, TSIZE(LoopRec));
    out := NIL;
    RETURN PoolExhausted
  END;

  (* Create timer queue *)
  tst := Timers.Create(lp^.sched, lp^.timers);
  IF tst # Timers.OK THEN
    sst := SchedulerDestroy(lp^.sched);
    pst := Poller.Destroy(lp^.poller);
    DEALLOCATE(lp, TSIZE(LoopRec));
    out := NIL;
    RETURN PoolExhausted
  END;

  lp^.nWatchers := 0;
  lp^.running := FALSE;
  lp^.stopFlag := FALSE;
  FOR i := 0 TO MaxWatchers - 1 DO
    lp^.watchers[i].active := FALSE
  END;

  out := lp;
  RETURN OK
END Create;

PROCEDURE Destroy(VAR lp: Loop): Status;
VAR p: LoopPtr; tst: Timers.Status; sst: Scheduler.Status; pst: Poller.Status;
BEGIN
  IF lp = NIL THEN RETURN Invalid END;
  p := lp;
  tst := Timers.Destroy(p^.timers);
  sst := SchedulerDestroy(p^.sched);
  pst := Poller.Destroy(p^.poller);
  DEALLOCATE(p, TSIZE(LoopRec));
  lp := NIL;
  RETURN OK
END Destroy;

(* ── Timers ────────────────────────────────────────────────────── *)

PROCEDURE SetTimeout(lp: Loop; delayMs: INTEGER;
                     cb: TaskProc; user: ADDRESS;
                     VAR id: TimerId): Status;
VAR p: LoopPtr; now: INTEGER; tst: Timers.Status;
BEGIN
  IF lp = NIL THEN RETURN Invalid END;
  p := lp;
  now := Poller.NowMs();
  tst := Timers.SetTimeout(p^.timers, now, delayMs, cb, user, id);
  IF tst # Timers.OK THEN RETURN PoolExhausted END;
  RETURN OK
END SetTimeout;

PROCEDURE SetInterval(lp: Loop; intervalMs: INTEGER;
                      cb: TaskProc; user: ADDRESS;
                      VAR id: TimerId): Status;
VAR p: LoopPtr; now: INTEGER; tst: Timers.Status;
BEGIN
  IF lp = NIL THEN RETURN Invalid END;
  p := lp;
  now := Poller.NowMs();
  tst := Timers.SetInterval(p^.timers, now, intervalMs, cb, user, id);
  IF tst # Timers.OK THEN RETURN PoolExhausted END;
  RETURN OK
END SetInterval;

PROCEDURE CancelTimer(lp: Loop; id: TimerId): Status;
VAR p: LoopPtr; tst: Timers.Status;
BEGIN
  IF lp = NIL THEN RETURN Invalid END;
  p := lp;
  tst := Timers.Cancel(p^.timers, id);
  RETURN OK
END CancelTimer;

(* ── I/O Watchers ──────────────────────────────────────────────── *)

PROCEDURE WatchFd(lp: Loop; fd, events: INTEGER;
                  cb: WatcherProc; user: ADDRESS): Status;
VAR p: LoopPtr; i: INTEGER; pst: Poller.Status;
BEGIN
  IF lp = NIL THEN RETURN Invalid END;
  p := lp;
  IF p^.nWatchers >= MaxWatchers THEN RETURN PoolExhausted END;

  pst := Poller.Add(p^.poller, fd, events);
  IF pst # Poller.OK THEN RETURN SysError END;

  (* Find a free slot *)
  FOR i := 0 TO MaxWatchers - 1 DO
    IF NOT p^.watchers[i].active THEN
      p^.watchers[i].fd := fd;
      p^.watchers[i].events := events;
      p^.watchers[i].cb := cb;
      p^.watchers[i].user := user;
      p^.watchers[i].active := TRUE;
      INC(p^.nWatchers);
      RETURN OK
    END
  END;
  RETURN PoolExhausted
END WatchFd;

PROCEDURE ModifyFd(lp: Loop; fd, events: INTEGER): Status;
VAR p: LoopPtr; idx: INTEGER; pst: Poller.Status;
BEGIN
  IF lp = NIL THEN RETURN Invalid END;
  p := lp;
  idx := FindWatcher(p, fd);
  IF idx < 0 THEN RETURN Invalid END;
  pst := Poller.Modify(p^.poller, fd, events);
  IF pst # Poller.OK THEN RETURN SysError END;
  p^.watchers[idx].events := events;
  RETURN OK
END ModifyFd;

PROCEDURE UnwatchFd(lp: Loop; fd: INTEGER): Status;
VAR p: LoopPtr; idx: INTEGER; pst: Poller.Status;
BEGIN
  IF lp = NIL THEN RETURN Invalid END;
  p := lp;
  idx := FindWatcher(p, fd);
  IF idx < 0 THEN RETURN Invalid END;
  pst := Poller.Remove(p^.poller, fd);
  p^.watchers[idx].active := FALSE;
  DEC(p^.nWatchers);
  RETURN OK
END UnwatchFd;

(* ── Scheduling ────────────────────────────────────────────────── *)

PROCEDURE Enqueue(lp: Loop; cb: TaskProc; user: ADDRESS): Status;
VAR p: LoopPtr; sst: Scheduler.Status;
BEGIN
  IF lp = NIL THEN RETURN Invalid END;
  p := lp;
  sst := SchedulerEnqueue(p^.sched, cb, user);
  IF sst # Scheduler.OK THEN RETURN PoolExhausted END;
  RETURN OK
END Enqueue;

PROCEDURE GetScheduler(lp: Loop): Scheduler;
VAR p: LoopPtr;
BEGIN
  IF lp = NIL THEN RETURN NIL END;
  p := lp;
  RETURN p^.sched
END GetScheduler;

(* ── Running ───────────────────────────────────────────────────── *)

PROCEDURE RunOnce(lp: Loop): BOOLEAN;
VAR
  p: LoopPtr;
  now, timeout, count, i, idx: INTEGER;
  buf: EventBuf;
  pst: Poller.Status;
  tst: Timers.Status;
  sst: Scheduler.Status;
  didWork: BOOLEAN;
BEGIN
  IF lp = NIL THEN RETURN FALSE END;
  p := lp;

  now := Poller.NowMs();

  (* 1. Compute timeout from nearest timer *)
  timeout := Timers.NextDeadline(p^.timers, now);

  (* If no timers and no watchers, just drain scheduler *)
  IF (timeout < 0) AND (p^.nWatchers = 0) THEN
    sst := SchedulerPump(p^.sched, 256, didWork);
    RETURN didWork
  END;

  (* 2. Poll for I/O events *)
  IF p^.nWatchers > 0 THEN
    IF timeout < 0 THEN
      (* No timers, but watchers exist: block up to 100ms *)
      timeout := 100
    END;
    pst := Poller.Wait(p^.poller, timeout, buf, count);

    (* 3. Dispatch watcher callbacks inline *)
    IF count > 0 THEN
      FOR i := 0 TO count - 1 DO
        idx := FindWatcher(p, buf[i].fd);
        IF idx >= 0 THEN
          p^.watchers[idx].cb(buf[i].fd, buf[i].events,
                               p^.watchers[idx].user)
        END
      END
    END
  ELSE
    (* No watchers, but timers exist: sleep until timer *)
    IF timeout > 0 THEN
      pst := Poller.Wait(p^.poller, timeout, buf, count)
    END
  END;

  (* 4. Tick timers *)
  now := Poller.NowMs();
  tst := Timers.Tick(p^.timers, now);

  (* 5. Pump scheduler *)
  didWork := TRUE;
  WHILE didWork DO
    sst := SchedulerPump(p^.sched, 256, didWork)
  END;

  (* Return TRUE if there's still work to do *)
  RETURN (p^.nWatchers > 0) OR
         (Timers.ActiveCount(p^.timers) > 0)
END RunOnce;

PROCEDURE Run(lp: Loop);
VAR p: LoopPtr; hasWork: BOOLEAN;
BEGIN
  IF lp = NIL THEN RETURN END;
  p := lp;
  p^.running := TRUE;
  p^.stopFlag := FALSE;
  LOOP
    hasWork := RunOnce(lp);
    IF p^.stopFlag THEN EXIT END;
    IF NOT hasWork THEN EXIT END
  END;
  p^.running := FALSE
END Run;

PROCEDURE Stop(lp: Loop);
VAR p: LoopPtr;
BEGIN
  IF lp = NIL THEN RETURN END;
  p := lp;
  p^.stopFlag := TRUE
END Stop;

END EventLoop.
