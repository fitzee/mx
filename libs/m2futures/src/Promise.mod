IMPLEMENTATION MODULE Promise;
(* Single-threaded scheduler confinement assumed throughout.
   All pools, globals, and state are unsynchronized.

   Ownership model:
   - SharedRec.refCount counts ALL references: the external handle
     (from PromiseCreate) and continuation references (from
     Map/OnReject/OnSettle/All/Race).
   - PromiseCreate sets refCount := 1. Promise and Future alias the
     same SharedRec; they share one reference. The caller must call
     exactly one of PromiseRelease or FutureRelease — not both.
   - Each continuation Retains outSh when attached. ExecuteCont
     Releases outSh when the continuation runs.
   - PromiseRelease / FutureRelease Release the external handle ref.
   - TryReclaim frees the pool slot when refCount = 0 AND no
     queued continuations remain.
   - Combiner state (AllStateRec/RaceStateRec) is heap-allocated,
     owned by the output SharedRec via combSt/combKind, freed
     when the output SharedRec is reclaimed.
   - CancMapRec is heap-allocated per MapCancellable call,
     freed inside CancellableThen.
   - CancelRec.refCount counts external handle + internal refs
     from MapCancellable + dispatch ref from Cancel/OnCancel.
     Freed when refCount reaches 0. The dispatching flag prevents
     duplicate scheduling; the dispatch ref keeps the token alive
     until all enqueued ExecCancelCB steps complete. *)

FROM SYSTEM IMPORT ADDRESS, ADR;
FROM Scheduler IMPORT Scheduler, Status, TaskProc,
                      SchedulerEnqueue;

(* ================================================================
   Internal types
   ================================================================ *)

CONST
  POOL_SH = 256;
  POOL_CN = 512;

TYPE
  ContKind = (CKThen, CKCatch, CKFinally, CKAll, CKRace);
  CombKind = (CombNone, CombAll, CombRace);

  SharedPtr = POINTER TO SharedRec;
  ContPtr   = POINTER TO ContRec;

  SharedRec = RECORD
    sched:    Scheduler;
    fate:     Fate;
    res:      Result;
    contHead: ContPtr;
    contTail: ContPtr;
    refCount: CARDINAL;   (* external handles + continuation refs *)
    combSt:   ADDRESS;    (* owned AllStatePtr or RaceStatePtr *)
    combKind: CombKind;
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
    combSt:  ADDRESS;     (* borrowed from outSh *)
    idx:     CARDINAL;
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
    shPool[i].refCount := 0;
    shPool[i].combSt   := NIL;
    shPool[i].combKind := CombNone;
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

(* Return a shared-state slot to the pool. Only called from
   TryReclaim after all refs are gone. Never call directly
   from API code. *)
PROCEDURE FreeShared(p: SharedPtr);
BEGIN
  p^.fate     := Pending;
  p^.contHead := NIL;
  p^.contTail := NIL;
  p^.refCount := 0;
  p^.combSt   := NIL;
  p^.combKind := CombNone;
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
  c^.inSh   := NIL;
  c^.outSh  := NIL;
  c^.combSt := NIL;
  c^.user   := NIL;
  c^.next   := NIL;
  cnTop := cnTop + 1;
  cnFree[cnTop] := c^.poolIdx
END FreeCont;

(* ================================================================
   Reference counting and reclamation
   ================================================================ *)

PROCEDURE Retain(sh: SharedPtr);
BEGIN
  sh^.refCount := sh^.refCount + 1
END Retain;

(* Try to reclaim a shared state. Reclaims when no references
   remain and no continuations are queued. An abandoned pending
   future (caller error) is also reclaimable if unreferenced. *)
PROCEDURE TryReclaim(sh: SharedPtr);
VAR asp: AllStatePtr; rsp: RaceStatePtr;
BEGIN
  IF sh^.refCount > 0 THEN RETURN END;
  IF sh^.contHead # NIL THEN RETURN END;
  (* Free owned combiner state *)
  IF sh^.combSt # NIL THEN
    IF sh^.combKind = CombAll THEN
      asp := sh^.combSt;
      DISPOSE(asp)
    ELSIF sh^.combKind = CombRace THEN
      rsp := sh^.combSt;
      DISPOSE(rsp)
    END;
    sh^.combSt   := NIL;
    sh^.combKind := CombNone
  END;
  FreeShared(sh)
END TryReclaim;

(* Decrement refcount. If it reaches zero, try to reclaim. *)
PROCEDURE Release(sh: SharedPtr);
BEGIN
  IF sh^.refCount = 0 THEN RETURN END;
  sh^.refCount := sh^.refCount - 1;
  TryReclaim(sh)
END Release;

(* ================================================================
   Internal helpers
   ================================================================ *)

PROCEDURE InitResult(VAR r: Result);
BEGIN
  r.isOk  := FALSE;
  r.v.tag := 0;
  r.v.ptr := NIL;
  r.e.code := 0;
  r.e.ptr  := NIL
END InitResult;

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

(* Detach the entire continuation chain. Returns the old head. *)
PROCEDURE DetachConts(sh: SharedPtr): ContPtr;
VAR head: ContPtr;
BEGIN
  head := sh^.contHead;
  sh^.contHead := NIL;
  sh^.contTail := NIL;
  RETURN head
END DetachConts;

(* Drain continuations: detach chain, enqueue in order.
   Captures next pointer before each enqueue to avoid
   use-after-free if the scheduler were ever to pump inline.
   On partial enqueue failure, restores remaining unqueued
   continuations back onto the shared state. *)
PROCEDURE DrainConts(sh: SharedPtr): Status;
VAR
  c, next: ContPtr;
  st: Status;
BEGIN
  c := DetachConts(sh);
  WHILE c # NIL DO
    next := c^.next;
    st := SchedulerEnqueue(sh^.sched, execContProc, c);
    IF st # OK THEN
      (* Restore c and everything after it *)
      sh^.contHead := c;
      WHILE c^.next # NIL DO
        c := c^.next
      END;
      sh^.contTail := c;
      RETURN OutOfMemory
    END;
    c := next
  END;
  RETURN OK
END DrainConts;

PROCEDURE SettleWith(sh: SharedPtr; VAR res: Result): Status;
BEGIN
  IF sh^.fate # Pending THEN RETURN AlreadySettled END;
  IF res.isOk THEN
    sh^.fate := Fulfilled
  ELSE
    sh^.fate := Rejected
  END;
  sh^.res := res;
  RETURN DrainConts(sh)
END SettleWith;

PROCEDURE HandleAll(c: ContPtr);
VAR
  asp: AllStatePtr;
  inRes, outRes: Result;
  v: Value;
BEGIN
  asp := c^.combSt;
  inRes := c^.inSh^.res;

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
  inRes: Result;
BEGIN
  rsp := c^.combSt;
  IF rsp^.settled THEN RETURN END;
  rsp^.settled := TRUE;
  inRes := c^.inSh^.res;
  SettleWith(rsp^.outSh, inRes)
END HandleRace;

(* ---- Scheduler callback ---- *)

PROCEDURE ExecuteCont(data: ADDRESS);
VAR
  c:    ContPtr;
  inSh: SharedPtr;
  outSh: SharedPtr;
  inRes, outRes: Result;
  tf: ThenFn;
  cf: CatchFn;
  vf: VoidFn;
BEGIN
  c  := data;
  inSh  := c^.inSh;   (* save before FreeCont clears fields *)
  outSh := c^.outSh;
  inRes := inSh^.res;

  (* Defensive init so buggy callbacks cannot leave
     uninitialized memory being settled. *)
  InitResult(outRes);

  IF c^.kind = CKThen THEN
    tf := c^.thenFn;
    IF tf # NIL THEN
      tf(inRes, c^.user, outRes)
    END;
    IF outSh # NIL THEN
      SettleWith(outSh, outRes)
    END

  ELSIF c^.kind = CKCatch THEN
    IF inRes.isOk THEN
      outRes := inRes
    ELSE
      cf := c^.catchFn;
      IF cf # NIL THEN
        cf(inRes.e, c^.user, outRes)
      END
    END;
    IF outSh # NIL THEN
      SettleWith(outSh, outRes)
    END

  ELSIF c^.kind = CKFinally THEN
    vf := c^.voidFn;
    IF vf # NIL THEN
      vf(inRes, c^.user)
    END;
    IF outSh # NIL THEN
      SettleWith(outSh, inRes)
    END

  ELSIF c^.kind = CKAll THEN
    HandleAll(c)

  ELSIF c^.kind = CKRace THEN
    HandleRace(c)
  END;

  (* Free the continuation first (returns pool slot),
     then release the outSh ref the continuation held.
     Release may trigger TryReclaim which is safe because
     FreeCont already cleared c's fields. *)
  FreeCont(c);
  IF outSh # NIL THEN
    Release(outSh)
  END;
  (* Try reclaim the input shared state. It may now be
     reclaimable if caller already released their handle
     and this was the last continuation referencing it
     indirectly (via being queued on it). inSh is not
     refcounted by conts (conts only refcount outSh),
     so this just checks the existing conditions. *)
  TryReclaim(inSh)
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
  InitResult(sh^.res);
  sh^.contHead := NIL;
  sh^.contTail := NIL;
  sh^.refCount := 1;  (* one ref shared by the p/f alias pair *)
  sh^.combSt   := NIL;
  sh^.combKind := CombNone;
  p := sh;  (* alias — same SharedRec *)
  f := sh;  (* caller must release exactly one, not both *)
  RETURN OK
END PromiseCreate;

(* ================================================================
   Public API -- Lifetime
   ================================================================ *)

PROCEDURE PromiseRelease(VAR p: Promise);
VAR sh: SharedPtr;
BEGIN
  IF p = NIL THEN RETURN END;
  sh := p;
  p := NIL;
  Release(sh)
END PromiseRelease;

PROCEDURE FutureRelease(VAR f: Future);
VAR sh: SharedPtr;
BEGIN
  IF f = NIL THEN RETURN END;
  sh := f;
  f := NIL;
  Release(sh)
END FutureRelease;

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
  RETURN SettleWith(sh, res)
  (* Caller still holds external handle ref. They must call
     PromiseRelease when done with the handle. *)
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
  RETURN SettleWith(sh, res)
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

   Each chaining operation:
   1. Creates a new SharedRec for the output (refCount = 1)
   2. Allocates a continuation
   3. The continuation Retains outSh (+1, now refCount = 2)
   4. Returns out (the external handle holds ref #1)
   5. When the cont executes, it Releases outSh (back to 1)
   6. When the caller FutureReleases out, it drops to 0 → reclaim

   On failure after step 1, we Release the creation ref.
   On failure after step 3, we Release the cont ref, free the
   cont, then Release the creation ref.
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
  (* outSh refCount = 1 (creation ref, will be returned as out) *)

  IF NOT AllocCont(c) THEN
    Release(outSh);  (* drop creation ref → reclaim *)
    out := NIL;
    RETURN OutOfMemory
  END;
  c^.kind   := CKThen;
  c^.thenFn := fn;
  c^.user   := user;
  c^.inSh   := inSh;
  c^.outSh  := outSh;
  c^.combSt := NIL;
  c^.next   := NIL;
  Retain(outSh);  (* cont ref: refCount = 2 *)

  IF inSh^.fate # Pending THEN
    st := SchedulerEnqueue(s, execContProc, c);
    IF st # OK THEN
      (* Undo cont ref, free cont, drop creation ref *)
      Release(outSh);  (* cont ref: 2 → 1 *)
      FreeCont(c);
      Release(outSh);  (* creation ref: 1 → 0 → reclaim *)
      out := NIL;
      RETURN OutOfMemory
    END
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
    Release(outSh);
    out := NIL;
    RETURN OutOfMemory
  END;
  c^.kind    := CKCatch;
  c^.catchFn := fn;
  c^.user    := user;
  c^.inSh    := inSh;
  c^.outSh   := outSh;
  c^.combSt  := NIL;
  c^.next    := NIL;
  Retain(outSh);

  IF inSh^.fate # Pending THEN
    st := SchedulerEnqueue(s, execContProc, c);
    IF st # OK THEN
      Release(outSh);
      FreeCont(c);
      Release(outSh);
      out := NIL;
      RETURN OutOfMemory
    END
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
    Release(outSh);
    out := NIL;
    RETURN OutOfMemory
  END;
  c^.kind   := CKFinally;
  c^.voidFn := fn;
  c^.user   := user;
  c^.inSh   := inSh;
  c^.outSh  := outSh;
  c^.combSt := NIL;
  c^.next   := NIL;
  Retain(outSh);

  IF inSh^.fate # Pending THEN
    st := SchedulerEnqueue(s, execContProc, c);
    IF st # OK THEN
      Release(outSh);
      FreeCont(c);
      Release(outSh);
      out := NIL;
      RETURN OutOfMemory
    END
  ELSE
    AppendCont(inSh, c)
  END;
  RETURN OK
END OnSettle;

(* ================================================================
   Public API -- Combinators

   Construction is best-effort. All continuations are pre-allocated
   before any are attached. If pre-allocation fails, full cleanup
   occurs. If enqueue fails partway during attachment, already-
   attached conts remain live (they hold refs to outSh and will
   release them when they execute). Remaining unattached conts
   are freed. The caller receives OutOfMemory and out = NIL.
   The creation ref on outSh is released on failure; outSh will
   be reclaimed when all live conts have executed and released.
   ================================================================ *)

PROCEDURE All(s: Scheduler; fs: ARRAY OF Future;
              VAR out: Future): Status;
VAR
  n, i, j, allocated: CARDINAL;
  inSh: SharedPtr;
  p:    Promise;
  outSh: SharedPtr;
  st:   Status;
  asp:  AllStatePtr;
  c:    ContPtr;
  conts: ARRAY [0..MAX_ALL_SIZE-1] OF ContPtr;
BEGIN
  n := HIGH(fs) + 1;
  IF (s = NIL) OR (n = 0) OR (n > MAX_ALL_SIZE) THEN
    out := NIL;
    RETURN Invalid
  END;
  FOR i := 0 TO n - 1 DO
    IF fs[i] = NIL THEN
      out := NIL;
      RETURN Invalid
    END
  END;

  st := PromiseCreate(s, p, out);
  IF st # OK THEN RETURN st END;
  outSh := out;
  (* outSh refCount = 1 (creation ref) *)

  NEW(asp);
  IF asp = NIL THEN
    Release(outSh);
    out := NIL;
    RETURN OutOfMemory
  END;
  asp^.outSh  := outSh;
  asp^.total  := n;
  asp^.done   := 0;
  asp^.failed := FALSE;

  outSh^.combSt   := asp;
  outSh^.combKind := CombAll;

  (* Pre-allocate all continuations.
     If any allocation fails, free all allocated conts,
     detach combiner state, dispose it, release outSh. *)
  allocated := 0;
  FOR i := 0 TO n - 1 DO
    IF NOT AllocCont(c) THEN
      FOR j := 0 TO allocated - 1 DO
        FreeCont(conts[j])
      END;
      outSh^.combSt   := NIL;
      outSh^.combKind := CombNone;
      DISPOSE(asp);
      Release(outSh);
      out := NIL;
      RETURN OutOfMemory
    END;
    conts[i] := c;
    allocated := allocated + 1
  END;

  (* Attach/enqueue. Each cont Retains outSh. *)
  FOR i := 0 TO n - 1 DO
    c := conts[i];
    inSh := fs[i];
    c^.kind   := CKAll;
    c^.inSh   := inSh;
    c^.outSh  := outSh;
    c^.combSt := asp;
    c^.idx    := i;
    c^.next   := NIL;
    c^.user   := NIL;
    Retain(outSh);  (* cont ref *)

    IF inSh^.fate # Pending THEN
      st := SchedulerEnqueue(s, execContProc, c);
      IF st # OK THEN
        Release(outSh);  (* undo this cont's Retain *)
        FreeCont(c);
        FOR j := i + 1 TO n - 1 DO
          FreeCont(conts[j])
        END;
        (* Release creation ref. Already-attached conts (0..i-1)
           still hold refs; outSh lives until they all execute. *)
        Release(outSh);
        out := NIL;
        RETURN OutOfMemory
      END
    ELSE
      AppendCont(inSh, c)
    END
  END;
  RETURN OK
END All;

PROCEDURE Race(s: Scheduler; fs: ARRAY OF Future;
               VAR out: Future): Status;
VAR
  n, i, j, allocated: CARDINAL;
  inSh: SharedPtr;
  p:    Promise;
  outSh: SharedPtr;
  st:   Status;
  rsp:  RaceStatePtr;
  c:    ContPtr;
  conts: ARRAY [0..MAX_ALL_SIZE-1] OF ContPtr;
BEGIN
  n := HIGH(fs) + 1;
  IF (s = NIL) OR (n = 0) OR (n > MAX_ALL_SIZE) THEN
    out := NIL;
    RETURN Invalid
  END;
  FOR i := 0 TO n - 1 DO
    IF fs[i] = NIL THEN
      out := NIL;
      RETURN Invalid
    END
  END;

  st := PromiseCreate(s, p, out);
  IF st # OK THEN RETURN st END;
  outSh := out;

  NEW(rsp);
  IF rsp = NIL THEN
    Release(outSh);
    out := NIL;
    RETURN OutOfMemory
  END;
  rsp^.outSh  := outSh;
  rsp^.settled := FALSE;

  outSh^.combSt   := rsp;
  outSh^.combKind := CombRace;

  allocated := 0;
  FOR i := 0 TO n - 1 DO
    IF NOT AllocCont(c) THEN
      FOR j := 0 TO allocated - 1 DO
        FreeCont(conts[j])
      END;
      outSh^.combSt   := NIL;
      outSh^.combKind := CombNone;
      DISPOSE(rsp);
      Release(outSh);
      out := NIL;
      RETURN OutOfMemory
    END;
    conts[i] := c;
    allocated := allocated + 1
  END;

  FOR i := 0 TO n - 1 DO
    c := conts[i];
    inSh := fs[i];
    c^.kind   := CKRace;
    c^.inSh   := inSh;
    c^.outSh  := outSh;
    c^.combSt := rsp;
    c^.idx    := i;
    c^.next   := NIL;
    c^.user   := NIL;
    Retain(outSh);

    IF inSh^.fate # Pending THEN
      st := SchedulerEnqueue(s, execContProc, c);
      IF st # OK THEN
        Release(outSh);
        FreeCont(c);
        FOR j := i + 1 TO n - 1 DO
          FreeCont(conts[j])
        END;
        Release(outSh);
        out := NIL;
        RETURN OutOfMemory
      END
    ELSE
      AppendCont(inSh, c)
    END
  END;
  RETURN OK
END Race;

(* ================================================================
   Public API -- Cancellation
   ================================================================ *)

CONST
  POOL_CT = 64;
  MaxCancelCBs = 8;

TYPE
  CancelCB = RECORD
    fn:  VoidFn;
    ctx: ADDRESS;
  END;

  CancelRec = RECORD
    cancelled:   BOOLEAN;
    dispatching: BOOLEAN;    (* TRUE while ExecCancelCB is queued/running *)
    sched:       Scheduler;
    cbs:         ARRAY [0..MaxCancelCBs-1] OF CancelCB;
    cbCount:     INTEGER;
    cbNext:      INTEGER;
    refCount:    CARDINAL;  (* external + internal + dispatch refs *)
    poolIdx:     CARDINAL;
  END;

  CancelPtr = POINTER TO CancelRec;

VAR
  ctPool:  ARRAY [0..POOL_CT-1] OF CancelRec;
  ctFree:  ARRAY [0..POOL_CT-1] OF CARDINAL;
  ctTop:   INTEGER;
  ctReady: BOOLEAN;

PROCEDURE InitCtPool;
VAR i: CARDINAL;
BEGIN
  FOR i := 0 TO POOL_CT - 1 DO
    ctPool[i].poolIdx   := i;
    ctPool[i].cancelled   := FALSE;
    ctPool[i].dispatching := FALSE;
    ctPool[i].cbCount     := 0;
    ctPool[i].cbNext      := 0;
    ctPool[i].refCount    := 0;
    ctFree[i] := i;
  END;
  ctTop := POOL_CT - 1;
  ctReady := TRUE
END InitCtPool;

PROCEDURE AllocCancel(VAR p: CancelPtr): BOOLEAN;
VAR idx: CARDINAL;
BEGIN
  IF ctTop < 0 THEN RETURN FALSE END;
  idx := ctFree[ctTop];
  ctTop := ctTop - 1;
  p := ADR(ctPool[idx]);
  RETURN TRUE
END AllocCancel;

PROCEDURE FreeCancel(p: CancelPtr);
BEGIN
  p^.cancelled   := FALSE;
  p^.dispatching := FALSE;
  p^.cbCount     := 0;
  p^.cbNext      := 0;
  p^.refCount    := 0;
  ctTop := ctTop + 1;
  ctFree[ctTop] := p^.poolIdx
END FreeCancel;

PROCEDURE RetainCancel(cp: CancelPtr);
BEGIN
  cp^.refCount := cp^.refCount + 1
END RetainCancel;

(* Decrement cancel token refcount. Free when it reaches 0. *)
PROCEDURE ReleaseCancel(cp: CancelPtr);
BEGIN
  IF cp^.refCount = 0 THEN RETURN END;
  cp^.refCount := cp^.refCount - 1;
  IF cp^.refCount = 0 THEN
    FreeCancel(cp)
  END
END ReleaseCancel;

PROCEDURE CancelTokenCreate(s: Scheduler; VAR ct: CancelToken): Status;
VAR cp: CancelPtr;
BEGIN
  IF NOT ctReady THEN InitCtPool END;
  IF s = NIL THEN
    ct := NIL;
    RETURN Invalid
  END;
  IF NOT AllocCancel(cp) THEN
    ct := NIL;
    RETURN OutOfMemory
  END;
  cp^.cancelled   := FALSE;
  cp^.dispatching := FALSE;
  cp^.sched       := s;
  cp^.cbCount     := 0;
  cp^.cbNext      := 0;
  cp^.refCount    := 1;  (* external handle *)
  ct := cp;
  RETURN OK
END CancelTokenCreate;

(* Release the external cancel token reference.
   The pool slot is freed when refCount reaches 0. *)
PROCEDURE CancelTokenDestroy(VAR ct: CancelToken);
VAR cp: CancelPtr;
BEGIN
  IF ct = NIL THEN RETURN END;
  cp := ct;
  ct := NIL;
  ReleaseCancel(cp)
END CancelTokenDestroy;

(* Scheduler callback for cancel notification dispatch.
   Dispatches one callback per pump step using cbNext index.
   Holds a dispatch ref (acquired by Cancel/OnCancel) that is
   released when all callbacks have been dispatched or when
   enqueue of the next step fails. *)
PROCEDURE ExecCancelCB(data: ADDRESS);
VAR
  cp: CancelPtr;
  r: Result;
  idx: INTEGER;
  st: Status;
BEGIN
  cp := data;
  IF cp^.cbNext >= cp^.cbCount THEN
    cp^.dispatching := FALSE;
    ReleaseCancel(cp);  (* drop dispatch ref *)
    RETURN
  END;
  r.isOk := FALSE;
  r.e.code := -1;
  r.e.ptr := NIL;
  idx := cp^.cbNext;
  cp^.cbNext := cp^.cbNext + 1;
  cp^.cbs[idx].fn(r, cp^.cbs[idx].ctx);
  (* After the callback, check if more remain. Note: the callback
     itself may have appended new entries via OnCancel, so cbCount
     may have grown since we entered this invocation. *)
  IF cp^.cbNext < cp^.cbCount THEN
    st := SchedulerEnqueue(cp^.sched, ExecCancelCB, cp);
    IF st # OK THEN
      cp^.dispatching := FALSE;
      ReleaseCancel(cp)  (* drop dispatch ref *)
    END
  ELSE
    cp^.dispatching := FALSE;
    ReleaseCancel(cp)  (* drop dispatch ref *)
  END
END ExecCancelCB;

(* Cancel enqueues callback dispatch through the scheduler.
   Acquires a dispatch ref to keep the token alive until all
   callbacks have been dispatched. If the scheduler queue is
   full, the token is marked cancelled but pending callbacks
   may not fire; the dispatch ref is released immediately. *)
PROCEDURE Cancel(ct: CancelToken);
VAR
  cp: CancelPtr;
  st: Status;
BEGIN
  IF ct = NIL THEN RETURN END;
  cp := ct;
  IF cp^.cancelled THEN RETURN END;
  cp^.cancelled := TRUE;
  cp^.cbNext := 0;
  IF cp^.cbCount > 0 THEN
    cp^.dispatching := TRUE;
    RetainCancel(cp);  (* dispatch ref *)
    st := SchedulerEnqueue(cp^.sched, ExecCancelCB, cp);
    IF st # OK THEN
      cp^.dispatching := FALSE;
      ReleaseCancel(cp)  (* drop dispatch ref *)
    END
  END
END Cancel;

PROCEDURE IsCancelled(ct: CancelToken): BOOLEAN;
VAR cp: CancelPtr;
BEGIN
  IF ct = NIL THEN RETURN FALSE END;
  cp := ct;
  RETURN cp^.cancelled
END IsCancelled;

PROCEDURE OnCancel(ct: CancelToken; fn: VoidFn; ctx: ADDRESS);
VAR
  cp: CancelPtr;
  st: Status;
BEGIN
  IF ct = NIL THEN RETURN END;
  IF fn = NIL THEN RETURN END;
  cp := ct;
  IF cp^.cbCount >= MaxCancelCBs THEN RETURN END;
  cp^.cbs[cp^.cbCount].fn  := fn;
  cp^.cbs[cp^.cbCount].ctx := ctx;
  INC(cp^.cbCount);
  IF cp^.cancelled THEN
    IF NOT cp^.dispatching THEN
      (* No active dispatch — start one for the new callback. *)
      cp^.cbNext := cp^.cbCount - 1;
      cp^.dispatching := TRUE;
      RetainCancel(cp);  (* dispatch ref *)
      st := SchedulerEnqueue(cp^.sched, ExecCancelCB, cp);
      IF st # OK THEN
        cp^.dispatching := FALSE;
        ReleaseCancel(cp)  (* drop dispatch ref *)
      END
    END
    (* If already dispatching, the active dispatcher will pick up
       the new callback naturally: it checks cbNext < cbCount after
       each step, and cbCount was just incremented. *)
  END
END OnCancel;

(* ---- MapCancellable ---- *)

TYPE
  CancMapRec = RECORD
    fn:   ThenFn;
    user: ADDRESS;
    ct:   CancelPtr;  (* retained reference *)
  END;
  CancMapPtr = POINTER TO CancMapRec;

(* Wrapper callback for MapCancellable. Checks cancellation,
   invokes the user fn, then releases the cancel token ref
   and frees the wrapper record. *)
PROCEDURE CancellableThen(inRes: Result; user: ADDRESS; VAR outRes: Result);
VAR
  cm: CancMapPtr;
  cp: CancelPtr;
BEGIN
  cm := user;
  cp := cm^.ct;
  IF cp^.cancelled THEN
    outRes.isOk := FALSE;
    outRes.e.code := -1;
    outRes.e.ptr := NIL
  ELSE
    IF cm^.fn # NIL THEN
      cm^.fn(inRes, cm^.user, outRes)
    ELSE
      outRes.isOk := FALSE;
      outRes.e.code := -2;
      outRes.e.ptr := NIL
    END
  END;
  ReleaseCancel(cp);  (* drop internal ref *)
  DISPOSE(cm)
END CancellableThen;

PROCEDURE MapCancellable(s: Scheduler; f: Future;
                         fn: ThenFn; user: ADDRESS;
                         ct: CancelToken;
                         VAR out: Future): Status;
VAR
  cm: CancMapPtr;
  cp: CancelPtr;
  st: Status;
BEGIN
  IF (s = NIL) OR (f = NIL) OR (ct = NIL) THEN
    out := NIL;
    RETURN Invalid
  END;
  cp := ct;
  NEW(cm);
  IF cm = NIL THEN
    out := NIL;
    RETURN OutOfMemory
  END;
  cm^.fn   := fn;
  cm^.user := user;
  cm^.ct   := cp;
  RetainCancel(cp);  (* internal ref for CancellableThen *)
  st := Map(s, f, CancellableThen, cm, out);
  IF st # OK THEN
    ReleaseCancel(cp);  (* undo internal retain *)
    DISPOSE(cm)
  END;
  RETURN st
END MapCancellable;

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
  ctReady := FALSE;
  execContProc := ExecuteCont
END Promise.
