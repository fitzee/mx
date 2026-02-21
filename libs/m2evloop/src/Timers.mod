IMPLEMENTATION MODULE Timers;

FROM SYSTEM IMPORT ADDRESS, TSIZE;
FROM Storage IMPORT ALLOCATE, DEALLOCATE;
FROM Scheduler IMPORT Scheduler, TaskProc;
IMPORT Scheduler;

(* ── Internal types ────────────────────────────────────────────── *)

TYPE
  TimerEntry = RECORD
    deadline : INTEGER;    (* absolute time in ms *)
    cb       : TaskProc;
    user     : ADDRESS;
    id       : TimerId;
    interval : INTEGER;    (* 0 = one-shot, >0 = repeating *)
    active   : BOOLEAN;
  END;

  QueueRec = RECORD
    sched    : Scheduler;
    heap     : ARRAY [0..MaxTimers-1] OF INTEGER;  (* indices into pool *)
    heapSize : INTEGER;
    pool     : ARRAY [0..MaxTimers-1] OF TimerEntry;
    nextId   : INTEGER;
  END;

  QueuePtr = POINTER TO QueueRec;

(* ── Signed-difference time comparison (wrap-safe) ─────────────── *)

PROCEDURE TimeBefore(a, b: INTEGER): BOOLEAN;
(* Returns TRUE if a is before b, handling 32-bit wrap *)
BEGIN
  RETURN (a - b) < 0
END TimeBefore;

(* ── Min-heap operations ───────────────────────────────────────── *)

PROCEDURE HeapSwap(VAR q: QueueRec; i, j: INTEGER);
VAR tmp: INTEGER;
BEGIN
  tmp := q.heap[i];
  q.heap[i] := q.heap[j];
  q.heap[j] := tmp
END HeapSwap;

PROCEDURE SiftUp(VAR q: QueueRec; pos: INTEGER);
VAR parent: INTEGER;
BEGIN
  WHILE pos > 0 DO
    parent := (pos - 1) DIV 2;
    IF TimeBefore(q.pool[q.heap[pos]].deadline,
                  q.pool[q.heap[parent]].deadline) THEN
      HeapSwap(q, pos, parent);
      pos := parent
    ELSE
      RETURN
    END
  END
END SiftUp;

PROCEDURE SiftDown(VAR q: QueueRec; pos: INTEGER);
VAR left, right, smallest: INTEGER;
BEGIN
  LOOP
    left := 2 * pos + 1;
    right := 2 * pos + 2;
    smallest := pos;

    IF (left < q.heapSize) AND
       TimeBefore(q.pool[q.heap[left]].deadline,
                  q.pool[q.heap[smallest]].deadline) THEN
      smallest := left
    END;
    IF (right < q.heapSize) AND
       TimeBefore(q.pool[q.heap[right]].deadline,
                  q.pool[q.heap[smallest]].deadline) THEN
      smallest := right
    END;

    IF smallest = pos THEN RETURN END;
    HeapSwap(q, pos, smallest);
    pos := smallest
  END
END SiftDown;

PROCEDURE HeapPush(VAR q: QueueRec; poolIdx: INTEGER);
BEGIN
  q.heap[q.heapSize] := poolIdx;
  INC(q.heapSize);
  SiftUp(q, q.heapSize - 1)
END HeapPush;

PROCEDURE HeapPop(VAR q: QueueRec): INTEGER;
VAR top: INTEGER;
BEGIN
  top := q.heap[0];
  DEC(q.heapSize);
  IF q.heapSize > 0 THEN
    q.heap[0] := q.heap[q.heapSize];
    SiftDown(q, 0)
  END;
  RETURN top
END HeapPop;

(* ── Pool allocation ───────────────────────────────────────────── *)

PROCEDURE AllocSlot(VAR q: QueueRec; VAR slot: INTEGER): BOOLEAN;
VAR i: INTEGER;
BEGIN
  FOR i := 0 TO MaxTimers - 1 DO
    IF NOT q.pool[i].active THEN
      slot := i;
      RETURN TRUE
    END
  END;
  RETURN FALSE
END AllocSlot;

(* ── Public procedures ─────────────────────────────────────────── *)

PROCEDURE Create(sched: Scheduler;
                 VAR out: TimerQueue): Status;
VAR qp: QueuePtr; i: INTEGER;
BEGIN
  IF sched = NIL THEN
    out := NIL;
    RETURN Invalid
  END;
  ALLOCATE(qp, TSIZE(QueueRec));
  IF qp = NIL THEN
    out := NIL;
    RETURN PoolExhausted
  END;
  qp^.sched := sched;
  qp^.heapSize := 0;
  qp^.nextId := 1;
  FOR i := 0 TO MaxTimers - 1 DO
    qp^.pool[i].active := FALSE
  END;
  out := qp;
  RETURN OK
END Create;

PROCEDURE Destroy(VAR q: TimerQueue): Status;
VAR qp: QueuePtr;
BEGIN
  IF q = NIL THEN RETURN Invalid END;
  qp := q;
  DEALLOCATE(qp, TSIZE(QueueRec));
  q := NIL;
  RETURN OK
END Destroy;

PROCEDURE SetTimeout(q: TimerQueue; now, delayMs: INTEGER;
                     cb: TaskProc; user: ADDRESS;
                     VAR id: TimerId): Status;
VAR qp: QueuePtr; slot: INTEGER;
BEGIN
  IF q = NIL THEN RETURN Invalid END;
  qp := q;
  IF NOT AllocSlot(qp^, slot) THEN
    RETURN PoolExhausted
  END;
  qp^.pool[slot].deadline := now + delayMs;
  qp^.pool[slot].cb := cb;
  qp^.pool[slot].user := user;
  qp^.pool[slot].id := qp^.nextId;
  qp^.pool[slot].interval := 0;
  qp^.pool[slot].active := TRUE;
  id := qp^.nextId;
  INC(qp^.nextId);
  HeapPush(qp^, slot);
  RETURN OK
END SetTimeout;

PROCEDURE SetInterval(q: TimerQueue; now, intervalMs: INTEGER;
                      cb: TaskProc; user: ADDRESS;
                      VAR id: TimerId): Status;
VAR qp: QueuePtr; slot: INTEGER;
BEGIN
  IF q = NIL THEN RETURN Invalid END;
  qp := q;
  IF NOT AllocSlot(qp^, slot) THEN
    RETURN PoolExhausted
  END;
  qp^.pool[slot].deadline := now + intervalMs;
  qp^.pool[slot].cb := cb;
  qp^.pool[slot].user := user;
  qp^.pool[slot].id := qp^.nextId;
  qp^.pool[slot].interval := intervalMs;
  qp^.pool[slot].active := TRUE;
  id := qp^.nextId;
  INC(qp^.nextId);
  HeapPush(qp^, slot);
  RETURN OK
END SetInterval;

PROCEDURE Cancel(q: TimerQueue; id: TimerId): Status;
VAR qp: QueuePtr; i: INTEGER;
BEGIN
  IF q = NIL THEN RETURN Invalid END;
  qp := q;
  (* Mark as inactive; will be skipped during Tick *)
  FOR i := 0 TO MaxTimers - 1 DO
    IF qp^.pool[i].active AND (qp^.pool[i].id = id) THEN
      qp^.pool[i].active := FALSE;
      RETURN OK
    END
  END;
  RETURN OK   (* already cancelled or fired *)
END Cancel;

PROCEDURE ActiveCount(q: TimerQueue): INTEGER;
VAR qp: QueuePtr; i, count: INTEGER;
BEGIN
  IF q = NIL THEN RETURN 0 END;
  qp := q;
  count := 0;
  FOR i := 0 TO MaxTimers - 1 DO
    IF qp^.pool[i].active THEN INC(count) END
  END;
  RETURN count
END ActiveCount;

PROCEDURE NextDeadline(q: TimerQueue; now: INTEGER): INTEGER;
VAR qp: QueuePtr; diff: INTEGER;
BEGIN
  IF q = NIL THEN RETURN -1 END;
  qp := q;
  (* Skip inactive entries at top of heap *)
  WHILE (qp^.heapSize > 0) AND
        NOT qp^.pool[qp^.heap[0]].active DO
    DEC(qp^.heapSize);
    IF qp^.heapSize > 0 THEN
      qp^.heap[0] := qp^.heap[qp^.heapSize];
      SiftDown(qp^, 0)
    END
  END;
  IF qp^.heapSize = 0 THEN RETURN -1 END;
  diff := qp^.pool[qp^.heap[0]].deadline - now;
  IF diff < 0 THEN RETURN 0 END;
  RETURN diff
END NextDeadline;

PROCEDURE Tick(q: TimerQueue; now: INTEGER): Status;
VAR
  qp: QueuePtr;
  idx: INTEGER;
  dummy: Scheduler.Status;
BEGIN
  IF q = NIL THEN RETURN Invalid END;
  qp := q;
  LOOP
    (* Clean inactive entries at heap top *)
    WHILE (qp^.heapSize > 0) AND
          NOT qp^.pool[qp^.heap[0]].active DO
      idx := HeapPop(qp^)
    END;

    IF qp^.heapSize = 0 THEN EXIT END;

    idx := qp^.heap[0];
    IF TimeBefore(now, qp^.pool[idx].deadline) THEN
      EXIT   (* next timer is in the future *)
    END;

    (* Pop and fire *)
    idx := HeapPop(qp^);
    IF qp^.pool[idx].active THEN
      (* Enqueue callback on scheduler *)
      dummy := Scheduler.SchedulerEnqueue(qp^.sched,
                                          qp^.pool[idx].cb,
                                          qp^.pool[idx].user);

      IF qp^.pool[idx].interval > 0 THEN
        (* Repeating: reschedule from current deadline *)
        qp^.pool[idx].deadline :=
          qp^.pool[idx].deadline + qp^.pool[idx].interval;
        HeapPush(qp^, idx)
      ELSE
        (* One-shot: deactivate *)
        qp^.pool[idx].active := FALSE
      END
    END
  END;
  RETURN OK
END Tick;

END Timers.
