IMPLEMENTATION MODULE Promise;

FROM SYSTEM IMPORT ADDRESS, ADR;
FROM Storage IMPORT ALLOCATE, DEALLOCATE;
FROM Scheduler IMPORT Scheduler, Status, TaskProc,
                      SchedulerEnqueue;

(* ================================================================
   Internal types
   ================================================================ *)

CONST
  POOL_SH = 256;   (* shared-state pool capacity *)
  POOL_CN = 512;   (* continuation-node pool capacity *)

TYPE
  ContKind = (CKThen, CKCatch, CKFinally, CKAll, CKRace);

  SharedPtr = POINTER TO SharedRec;
  ContPtr   = POINTER TO ContRec;

  SharedRec = RECORD
    sched:    Scheduler;
    fate:     Fate;
    res:      Result;
    contHead: ContPtr;
    contTail: ContPtr;
    poolIdx:  CARDINAL;
  END;

  ContRec = RECORD
    kind:    ContKind;
    thenFn:  ThenFn;
    catchFn: CatchFn;
    voidFn:  VoidFn;
    user:    ADDRESS;
    inSh:    SharedPtr;
    outSh:   SharedPtr;
    combSt:  ADDRESS;    (* AllStatePtr or RaceStatePtr *)
    idx:     CARDINAL;   (* element index for All *)
    next:    ContPtr;
    poolIdx: CARDINAL;
  END;

  AllStatePtr = POINTER TO AllStateRec;
  AllStateRec = RECORD
    outSh:   SharedPtr;
    total:   CARDINAL;
    done:    CARDINAL;
    failed:  BOOLEAN;
    results: AllResultArray;
  END;

  RaceStatePtr = POINTER TO RaceStateRec;
  RaceStateRec = RECORD
    outSh:   SharedPtr;
    settled: BOOLEAN;
  END;

(* ================================================================
   Object pools
   ================================================================ *)

VAR
  shPool:  ARRAY [0..POOL_SH-1] OF SharedRec;
  shFree:  ARRAY [0..POOL_SH-1] OF CARDINAL;
  shTop:   INTEGER;

  cnPool:  ARRAY [0..POOL_CN-1] OF ContRec;
  cnFree:  ARRAY [0..POOL_CN-1] OF CARDINAL;
  cnTop:   INTEGER;

  poolsReady: BOOLEAN;
  execContProc: TaskProc;

PROCEDURE InitPools;
VAR i: CARDINAL;
BEGIN
  FOR i := 0 TO POOL_SH - 1 DO
    shPool[i].poolIdx  := i;
    shPool[i].fate     := Pending;
    shPool[i].contHead := NIL;
    shPool[i].contTail := NIL;
    shFree[i] := i;
  END;
  shTop := POOL_SH - 1;

  FOR i := 0 TO POOL_CN - 1 DO
    cnPool[i].poolIdx := i;
    cnPool[i].next    := NIL;
    cnFree[i] := i;
  END;
  cnTop := POOL_CN - 1;

  poolsReady := TRUE
END InitPools;

PROCEDURE AllocShared(VAR p: SharedPtr): BOOLEAN;
VAR idx: CARDINAL;
BEGIN
  IF shTop < 0 THEN RETURN FALSE END;
  idx := shFree[shTop];
  shTop := shTop - 1;
  p := ADR(shPool[idx]);
  RETURN TRUE
END AllocShared;

PROCEDURE FreeShared(p: SharedPtr);
BEGIN
  shTop := shTop + 1;
  shFree[shTop] := p^.poolIdx
END FreeShared;

PROCEDURE AllocCont(VAR c: ContPtr): BOOLEAN;
VAR idx: CARDINAL;
BEGIN
  IF cnTop < 0 THEN RETURN FALSE END;
  idx := cnFree[cnTop];
  cnTop := cnTop - 1;
  c := ADR(cnPool[idx]);
  RETURN TRUE
END AllocCont;

PROCEDURE FreeCont(c: ContPtr);
BEGIN
  cnTop := cnTop + 1;
  cnFree[cnTop] := c^.poolIdx
END FreeCont;

(* ================================================================
   Internal helpers
   ================================================================ *)

PROCEDURE AppendCont(sh: SharedPtr; c: ContPtr);
BEGIN
  c^.next := NIL;
  IF sh^.contTail = NIL THEN
    sh^.contHead := c;
    sh^.contTail := c
  ELSE
    sh^.contTail^.next := c;
    sh^.contTail := c
  END
END AppendCont;

PROCEDURE DrainConts(sh: SharedPtr);
VAR
  c:  ContPtr;
  st: Status;
BEGIN
  c := sh^.contHead;
  WHILE c # NIL DO
    st := SchedulerEnqueue(sh^.sched, execContProc, c);
    c := c^.next
  END;
  sh^.contHead := NIL;
  sh^.contTail := NIL
END DrainConts;

PROCEDURE SettleWith(sh: SharedPtr; VAR res: Result);
BEGIN
  IF sh^.fate # Pending THEN RETURN END;
  IF res.isOk THEN
    sh^.fate := Fulfilled
  ELSE
    sh^.fate := Rejected
  END;
  sh^.res := res;
  DrainConts(sh)
END SettleWith;

PROCEDURE HandleAll(c: ContPtr);
VAR
  asp: AllStatePtr;
  inRes, outRes: Result;
  v: Value;
  sh: SharedPtr;
BEGIN
  asp := c^.combSt;
  sh  := c^.inSh;
  inRes := sh^.res;

  IF asp^.failed THEN RETURN END;

  IF NOT inRes.isOk THEN
    asp^.failed := TRUE;
    SettleWith(asp^.outSh, inRes)
  ELSE
    asp^.results[c^.idx] := inRes;
    asp^.done := asp^.done + 1;
    IF asp^.done >= asp^.total THEN
      v.tag  := asp^.total;
      v.ptr  := ADR(asp^.results);
      outRes.isOk := TRUE;
      outRes.v    := v;
      SettleWith(asp^.outSh, outRes)
    END
  END
END HandleAll;

PROCEDURE HandleRace(c: ContPtr);
VAR
  rsp: RaceStatePtr;
  sh:  SharedPtr;
  inRes: Result;
BEGIN
  rsp := c^.combSt;
  IF rsp^.settled THEN RETURN END;
  rsp^.settled := TRUE;
  sh := c^.inSh;
  inRes := sh^.res;
  SettleWith(rsp^.outSh, inRes)
END HandleRace;

(* ---- Scheduler callback ---- *)

PROCEDURE ExecuteCont(data: ADDRESS);
VAR
  c:  ContPtr;
  sh: SharedPtr;
  inRes, outRes: Result;
  tf: ThenFn;
  cf: CatchFn;
  vf: VoidFn;
BEGIN
  c  := data;
  sh := c^.inSh;
  inRes := sh^.res;

  IF c^.kind = CKThen THEN
    tf := c^.thenFn;
    tf(inRes, c^.user, outRes);
    SettleWith(c^.outSh, outRes)

  ELSIF c^.kind = CKCatch THEN
    IF inRes.isOk THEN
      outRes := inRes
    ELSE
      cf := c^.catchFn;
      cf(inRes.e, c^.user, outRes)
    END;
    SettleWith(c^.outSh, outRes)

  ELSIF c^.kind = CKFinally THEN
    vf := c^.voidFn;
    vf(inRes, c^.user);
    SettleWith(c^.outSh, inRes)

  ELSIF c^.kind = CKAll THEN
    HandleAll(c)

  ELSIF c^.kind = CKRace THEN
    HandleRace(c)
  END;

  FreeCont(c)
END ExecuteCont;

(* ================================================================
   Public API -- Creation
   ================================================================ *)

PROCEDURE PromiseCreate(s: Scheduler;
                        VAR p: Promise;
                        VAR f: Future): Status;
VAR sh: SharedPtr;
BEGIN
  IF NOT poolsReady THEN InitPools END;
  IF s = NIL THEN
    p := NIL; f := NIL;
    RETURN Invalid
  END;
  IF NOT AllocShared(sh) THEN
    p := NIL; f := NIL;
    RETURN OutOfMemory
  END;
  sh^.sched    := s;
  sh^.fate     := Pending;
  sh^.res.isOk := FALSE;
  sh^.contHead := NIL;
  sh^.contTail := NIL;
  p := sh;
  f := sh;
  RETURN OK
END PromiseCreate;

(* ================================================================
   Public API -- Settlement
   ================================================================ *)

PROCEDURE Resolve(p: Promise; v: Value): Status;
VAR
  sh: SharedPtr;
  res: Result;
BEGIN
  IF p = NIL THEN RETURN Invalid END;
  sh := p;
  IF sh^.fate # Pending THEN RETURN AlreadySettled END;
  res.isOk := TRUE;
  res.v    := v;
  sh^.fate := Fulfilled;
  sh^.res  := res;
  DrainConts(sh);
  RETURN OK
END Resolve;

PROCEDURE Reject(p: Promise; e: Error): Status;
VAR
  sh: SharedPtr;
  res: Result;
BEGIN
  IF p = NIL THEN RETURN Invalid END;
  sh := p;
  IF sh^.fate # Pending THEN RETURN AlreadySettled END;
  res.isOk := FALSE;
  res.e    := e;
  sh^.fate := Rejected;
  sh^.res  := res;
  DrainConts(sh);
  RETURN OK
END Reject;

(* ================================================================
   Public API -- Inspection
   ================================================================ *)

PROCEDURE GetFate(f: Future; VAR fate: Fate): Status;
VAR sh: SharedPtr;
BEGIN
  IF f = NIL THEN RETURN Invalid END;
  sh := f;
  fate := sh^.fate;
  RETURN OK
END GetFate;

PROCEDURE GetResultIfSettled(f: Future;
                             VAR settled: BOOLEAN;
                             VAR res: Result): Status;
VAR sh: SharedPtr;
BEGIN
  IF f = NIL THEN
    settled := FALSE;
    RETURN Invalid
  END;
  sh := f;
  IF sh^.fate = Pending THEN
    settled := FALSE
  ELSE
    settled := TRUE;
    res := sh^.res
  END;
  RETURN OK
END GetResultIfSettled;

(* ================================================================
   Public API -- Chaining
   ================================================================ *)

PROCEDURE Map(s: Scheduler; f: Future;
              fn: ThenFn; user: ADDRESS;
              VAR out: Future): Status;
VAR
  inSh, outSh: SharedPtr;
  c:  ContPtr;
  p:  Promise;
  st: Status;
BEGIN
  IF (s = NIL) OR (f = NIL) THEN
    out := NIL;
    RETURN Invalid
  END;
  inSh := f;
  st := PromiseCreate(s, p, out);
  IF st # OK THEN RETURN st END;
  outSh := out;

  IF NOT AllocCont(c) THEN
    out := NIL;
    RETURN OutOfMemory
  END;
  c^.kind   := CKThen;
  c^.thenFn := fn;
  c^.user   := user;
  c^.inSh   := inSh;
  c^.outSh  := outSh;
  c^.next   := NIL;

  IF inSh^.fate # Pending THEN
    st := SchedulerEnqueue(s, ExecuteCont, c)
  ELSE
    AppendCont(inSh, c)
  END;
  RETURN OK
END Map;

PROCEDURE OnReject(s: Scheduler; f: Future;
                   fn: CatchFn; user: ADDRESS;
                   VAR out: Future): Status;
VAR
  inSh, outSh: SharedPtr;
  c:  ContPtr;
  p:  Promise;
  st: Status;
BEGIN
  IF (s = NIL) OR (f = NIL) THEN
    out := NIL;
    RETURN Invalid
  END;
  inSh := f;
  st := PromiseCreate(s, p, out);
  IF st # OK THEN RETURN st END;
  outSh := out;

  IF NOT AllocCont(c) THEN
    out := NIL;
    RETURN OutOfMemory
  END;
  c^.kind    := CKCatch;
  c^.catchFn := fn;
  c^.user    := user;
  c^.inSh    := inSh;
  c^.outSh   := outSh;
  c^.next    := NIL;

  IF inSh^.fate # Pending THEN
    st := SchedulerEnqueue(s, ExecuteCont, c)
  ELSE
    AppendCont(inSh, c)
  END;
  RETURN OK
END OnReject;

PROCEDURE OnSettle(s: Scheduler; f: Future;
                   fn: VoidFn; user: ADDRESS;
                   VAR out: Future): Status;
VAR
  inSh, outSh: SharedPtr;
  c:  ContPtr;
  p:  Promise;
  st: Status;
BEGIN
  IF (s = NIL) OR (f = NIL) THEN
    out := NIL;
    RETURN Invalid
  END;
  inSh := f;
  st := PromiseCreate(s, p, out);
  IF st # OK THEN RETURN st END;
  outSh := out;

  IF NOT AllocCont(c) THEN
    out := NIL;
    RETURN OutOfMemory
  END;
  c^.kind   := CKFinally;
  c^.voidFn := fn;
  c^.user   := user;
  c^.inSh   := inSh;
  c^.outSh  := outSh;
  c^.next   := NIL;

  IF inSh^.fate # Pending THEN
    st := SchedulerEnqueue(s, ExecuteCont, c)
  ELSE
    AppendCont(inSh, c)
  END;
  RETURN OK
END OnSettle;

(* ================================================================
   Public API -- Combinators
   ================================================================ *)

PROCEDURE All(s: Scheduler; fs: ARRAY OF Future;
              VAR out: Future): Status;
VAR
  n, i: CARDINAL;
  inSh: SharedPtr;
  p:    Promise;
  outSh: SharedPtr;
  st:   Status;
  asp:  AllStatePtr;
  c:    ContPtr;
BEGIN
  n := HIGH(fs) + 1;
  IF (s = NIL) OR (n = 0) OR (n > MAX_ALL_SIZE) THEN
    out := NIL;
    RETURN Invalid
  END;
  st := PromiseCreate(s, p, out);
  IF st # OK THEN RETURN st END;
  outSh := out;

  NEW(asp);
  IF asp = NIL THEN
    out := NIL;
    RETURN OutOfMemory
  END;
  asp^.outSh  := outSh;
  asp^.total  := n;
  asp^.done   := 0;
  asp^.failed := FALSE;

  FOR i := 0 TO n - 1 DO
    IF NOT AllocCont(c) THEN
      out := NIL;
      RETURN OutOfMemory
    END;
    inSh := fs[i];
    c^.kind   := CKAll;
    c^.inSh   := inSh;
    c^.outSh  := outSh;
    c^.combSt := asp;
    c^.idx    := i;
    c^.next   := NIL;
    c^.user   := NIL;

    IF inSh^.fate # Pending THEN
      st := SchedulerEnqueue(s, ExecuteCont, c)
    ELSE
      AppendCont(inSh, c)
    END
  END;
  RETURN OK
END All;

PROCEDURE Race(s: Scheduler; fs: ARRAY OF Future;
               VAR out: Future): Status;
VAR
  n, i: CARDINAL;
  inSh: SharedPtr;
  p:    Promise;
  outSh: SharedPtr;
  st:   Status;
  rsp:  RaceStatePtr;
  c:    ContPtr;
BEGIN
  n := HIGH(fs) + 1;
  IF (s = NIL) OR (n = 0) THEN
    out := NIL;
    RETURN Invalid
  END;
  st := PromiseCreate(s, p, out);
  IF st # OK THEN RETURN st END;
  outSh := out;

  NEW(rsp);
  IF rsp = NIL THEN
    out := NIL;
    RETURN OutOfMemory
  END;
  rsp^.outSh   := outSh;
  rsp^.settled  := FALSE;

  FOR i := 0 TO n - 1 DO
    IF NOT AllocCont(c) THEN
      out := NIL;
      RETURN OutOfMemory
    END;
    inSh := fs[i];
    c^.kind   := CKRace;
    c^.inSh   := inSh;
    c^.outSh  := outSh;
    c^.combSt := rsp;
    c^.idx    := i;
    c^.next   := NIL;
    c^.user   := NIL;

    IF inSh^.fate # Pending THEN
      st := SchedulerEnqueue(s, ExecuteCont, c)
    ELSE
      AppendCont(inSh, c)
    END
  END;
  RETURN OK
END Race;

(* ================================================================
   Public API -- Helpers
   ================================================================ *)

PROCEDURE MakeValue(tag: INTEGER; ptr: ADDRESS; VAR v: Value);
BEGIN
  v.tag := tag;
  v.ptr := ptr
END MakeValue;

PROCEDURE MakeError(code: INTEGER; ptr: ADDRESS; VAR e: Error);
BEGIN
  e.code := code;
  e.ptr  := ptr
END MakeError;

PROCEDURE Ok(v: Value; VAR r: Result);
BEGIN
  r.isOk := TRUE;
  r.v    := v
END Ok;

PROCEDURE Fail(e: Error; VAR r: Result);
BEGIN
  r.isOk := FALSE;
  r.e    := e
END Fail;

BEGIN
  poolsReady := FALSE;
  execContProc := ExecuteCont
END Promise.
