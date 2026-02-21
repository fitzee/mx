IMPLEMENTATION MODULE Scheduler;

FROM SYSTEM IMPORT ADDRESS;
FROM Storage IMPORT ALLOCATE, DEALLOCATE;

CONST
  MAXQ = 4096;

TYPE
  TaskNode = RECORD
    cb:   TaskProc;
    data: ADDRESS;
  END;

  SchedulerPtr = POINTER TO SchedulerRec;
  SchedulerRec = RECORD
    q:     ARRAY [0..MAXQ-1] OF TaskNode;
    cap:   CARDINAL;
    head:  CARDINAL;
    tail:  CARDINAL;
    count: CARDINAL;
  END;

PROCEDURE SchedulerCreate(capacity: CARDINAL;
                          VAR out: Scheduler): Status;
VAR sp: SchedulerPtr;
BEGIN
  IF capacity = 0 THEN
    out := NIL;
    RETURN Invalid
  END;
  IF capacity > MAXQ THEN
    capacity := MAXQ
  END;
  NEW(sp);
  IF sp = NIL THEN
    out := NIL;
    RETURN OutOfMemory
  END;
  sp^.cap   := capacity;
  sp^.head  := 0;
  sp^.tail  := 0;
  sp^.count := 0;
  out := sp;
  RETURN OK
END SchedulerCreate;

PROCEDURE SchedulerDestroy(VAR s: Scheduler): Status;
VAR sp: SchedulerPtr;
BEGIN
  IF s = NIL THEN RETURN Invalid END;
  sp := s;
  DISPOSE(sp);
  s := NIL;
  RETURN OK
END SchedulerDestroy;

PROCEDURE SchedulerEnqueue(s: Scheduler;
                           cb: TaskProc;
                           user: ADDRESS): Status;
VAR sp: SchedulerPtr;
BEGIN
  IF s = NIL THEN RETURN Invalid END;
  sp := s;
  IF sp^.count >= sp^.cap THEN RETURN OutOfMemory END;
  sp^.q[sp^.tail].cb   := cb;
  sp^.q[sp^.tail].data := user;
  sp^.tail  := (sp^.tail + 1) MOD sp^.cap;
  sp^.count := sp^.count + 1;
  RETURN OK
END SchedulerEnqueue;

PROCEDURE SchedulerPump(s: Scheduler;
                        maxSteps: CARDINAL;
                        VAR didWork: BOOLEAN): Status;
VAR
  sp:    SchedulerPtr;
  steps: CARDINAL;
  fn:    TaskProc;
  arg:   ADDRESS;
BEGIN
  IF s = NIL THEN
    didWork := FALSE;
    RETURN Invalid
  END;
  sp := s;
  didWork := FALSE;
  steps   := 0;
  WHILE (steps < maxSteps) AND (sp^.count > 0) DO
    fn       := sp^.q[sp^.head].cb;
    arg      := sp^.q[sp^.head].data;
    sp^.head := (sp^.head + 1) MOD sp^.cap;
    sp^.count := sp^.count - 1;
    fn(arg);
    didWork := TRUE;
    steps   := steps + 1
  END;
  RETURN OK
END SchedulerPump;

END Scheduler.
